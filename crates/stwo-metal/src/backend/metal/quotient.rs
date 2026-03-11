use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, OnceLock};

use ark_std::Zero;
use stwo::core::circle::CirclePoint;
use stwo::core::fields::cm31::CM31;
use stwo::core::fields::m31::BaseField;
use stwo::core::fields::qm31::SecureField;
use stwo::core::pcs::quotients::{quotient_constants, ColumnSampleBatch};
use stwo::core::poly::circle::CanonicCoset;
use stwo::core::utils::bit_reverse_index;
use stwo::prover::poly::circle::{CircleEvaluation, SecureEvaluation};
use stwo::prover::poly::BitReversedOrder;
use stwo::prover::{AccumulatedNumerators, QuotientOps};
use stwo_metal_sys::metal::{MetalError, U32Buffer};

use super::accumulation::metal_secure_column_from_values;
use super::MetalBackend;
use crate::stwo_metal::base_field_vec::BaseFieldVec;
use crate::stwo_metal::secure_field_vec::SecureFieldVec;

type QuotientDomainCache = Mutex<BTreeMap<u32, Arc<(U32Buffer, U32Buffer)>>>;
type PackedPartialNumeratorCache = Mutex<BTreeMap<([usize; 4], usize), Arc<U32Buffer>>>;

fn quotient_domain_cache() -> &'static QuotientDomainCache {
    static CACHE: OnceLock<QuotientDomainCache> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn packed_partial_numerator_cache() -> &'static PackedPartialNumeratorCache {
    static CACHE: OnceLock<PackedPartialNumeratorCache> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn partial_cache_key(columns: &[BaseFieldVec; 4]) -> ([usize; 4], usize) {
    (
        [
            columns[0].buffer_identity(),
            columns[1].buffer_identity(),
            columns[2].buffer_identity(),
            columns[3].buffer_identity(),
        ],
        columns[0].len(),
    )
}

fn cache_packed_partial_numerators(columns: &[BaseFieldVec; 4], packed: Arc<U32Buffer>) {
    packed_partial_numerator_cache()
        .lock()
        .expect("packed partial numerator cache mutex should not be poisoned")
        .insert(partial_cache_key(columns), packed);
}

fn take_cached_packed_partial_numerators(columns: &[BaseFieldVec; 4]) -> Option<Arc<U32Buffer>> {
    packed_partial_numerator_cache()
        .lock()
        .expect("packed partial numerator cache mutex should not be poisoned")
        .remove(&partial_cache_key(columns))
}

fn pack_cm31(value: CM31) -> [u32; 2] {
    [value.0 .0, value.1 .0]
}

fn pack_secure_circle_point(point: CirclePoint<SecureField>) -> [u32; 8] {
    let [x0, x1] = pack_cm31(point.x.0);
    let [x2, x3] = pack_cm31(point.x.1);
    let [y0, y1] = pack_cm31(point.y.0);
    let [y2, y3] = pack_cm31(point.y.1);
    [x0, x1, x2, x3, y0, y1, y2, y3]
}

fn cached_quotient_domain_coords(lifting_log_size: u32) -> Arc<(U32Buffer, U32Buffer)> {
    if let Some(buffers) = quotient_domain_cache()
        .lock()
        .expect("quotient domain cache mutex should not be poisoned")
        .get(&lifting_log_size)
        .cloned()
    {
        return buffers;
    }

    let domain = CanonicCoset::new(lifting_log_size).circle_domain();
    let row_count = 1usize << lifting_log_size;
    let mut domain_x = Vec::with_capacity(row_count);
    let mut domain_y = Vec::with_capacity(row_count);
    for row_index in 0..row_count {
        let point = domain.at(bit_reverse_index(row_index, lifting_log_size));
        domain_x.push(point.x.0);
        domain_y.push(point.y.0);
    }

    let buffers = Arc::new((
        U32Buffer::from_slice(&domain_x).expect("Metal quotient-domain x upload should initialize"),
        U32Buffer::from_slice(&domain_y).expect("Metal quotient-domain y upload should initialize"),
    ));
    quotient_domain_cache()
        .lock()
        .expect("quotient domain cache mutex should not be poisoned")
        .insert(lifting_log_size, buffers.clone());
    buffers
}

#[derive(Clone, Copy, Debug)]
pub struct MetalWideFibonacciQuotientRequest<'a> {
    pub trace_evaluations: &'a [&'a BaseFieldVec],
    pub random_coeff_powers: &'a [SecureField],
    pub denominator_inverses: &'a [BaseField],
    pub domain_log_size: u32,
    pub eval_domain_log_size: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MetalWideFibonacciQuotientError {
    NotEnoughTraceColumns {
        trace_columns: usize,
    },
    InconsistentTraceColumnLength {
        expected_len: usize,
        actual_len: usize,
        column_index: usize,
    },
    InvalidDomainLogSizes {
        domain_log_size: u32,
        eval_domain_log_size: u32,
    },
    RandomCoeffCountMismatch {
        expected_len: usize,
        actual_len: usize,
    },
    DenominatorCountMismatch {
        expected_len: usize,
        actual_len: usize,
    },
    Runtime {
        message: String,
    },
}

impl core::fmt::Display for MetalWideFibonacciQuotientError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NotEnoughTraceColumns { trace_columns } => write!(
                f,
                "wide-fibonacci quotient accumulation requires at least three trace columns, got {trace_columns}"
            ),
            Self::InconsistentTraceColumnLength {
                expected_len,
                actual_len,
                column_index,
            } => write!(
                f,
                "wide-fibonacci quotient accumulation requires equal trace column lengths of {expected_len}, but column {column_index} had length {actual_len}"
            ),
            Self::InvalidDomainLogSizes {
                domain_log_size,
                eval_domain_log_size,
            } => write!(
                f,
                "wide-fibonacci quotient accumulation requires eval_domain_log_size >= domain_log_size, got {eval_domain_log_size} < {domain_log_size}"
            ),
            Self::RandomCoeffCountMismatch {
                expected_len,
                actual_len,
            } => write!(
                f,
                "wide-fibonacci quotient accumulation requires {expected_len} random coefficients, got {actual_len}"
            ),
            Self::DenominatorCountMismatch {
                expected_len,
                actual_len,
            } => write!(
                f,
                "wide-fibonacci quotient accumulation requires {expected_len} denominator inverses, got {actual_len}"
            ),
            Self::Runtime { message } => f.write_str(message),
        }
    }
}

impl std::error::Error for MetalWideFibonacciQuotientError {}

impl From<MetalError> for MetalWideFibonacciQuotientError {
    fn from(value: MetalError) -> Self {
        Self::Runtime {
            message: value.message().to_string(),
        }
    }
}

#[derive(Debug)]
pub struct MetalWideFibonacciQuotients {
    values: SecureFieldVec,
    eval_domain_size: usize,
}

impl MetalWideFibonacciQuotients {
    pub fn len(&self) -> usize {
        self.eval_domain_size
    }

    pub fn is_empty(&self) -> bool {
        self.eval_domain_size == 0
    }

    pub fn value(&self, row_index: usize) -> SecureField {
        self.values.get_data(row_index)
    }

    pub fn values(&self) -> &SecureFieldVec {
        &self.values
    }

    pub fn into_coordinate_base_columns(self) -> [BaseFieldVec; 4] {
        self.values.to_base_coords()
    }

    pub fn to_vec(&self) -> Vec<SecureField> {
        self.values.to_vec()
    }

    pub fn to_coordinate_columns(&self) -> [Vec<BaseField>; 4] {
        let mut columns = std::array::from_fn(|_| Vec::with_capacity(self.eval_domain_size));
        for value in self.values.to_vec() {
            for (column, limb) in columns.iter_mut().zip(value.to_m31_array()) {
                column.push(limb);
            }
        }
        columns
    }
}

pub fn accumulate_wide_fibonacci_quotients(
    request: MetalWideFibonacciQuotientRequest<'_>,
) -> Result<MetalWideFibonacciQuotients, MetalWideFibonacciQuotientError> {
    if request.trace_evaluations.len() < 3 {
        return Err(MetalWideFibonacciQuotientError::NotEnoughTraceColumns {
            trace_columns: request.trace_evaluations.len(),
        });
    }
    if request.eval_domain_log_size < request.domain_log_size {
        return Err(MetalWideFibonacciQuotientError::InvalidDomainLogSizes {
            domain_log_size: request.domain_log_size,
            eval_domain_log_size: request.eval_domain_log_size,
        });
    }

    let eval_domain_size = 1usize << request.eval_domain_log_size;
    for (column_index, column) in request.trace_evaluations.iter().enumerate() {
        if column.len() != eval_domain_size {
            return Err(
                MetalWideFibonacciQuotientError::InconsistentTraceColumnLength {
                    expected_len: eval_domain_size,
                    actual_len: column.len(),
                    column_index,
                },
            );
        }
    }

    let expected_coeff_len = request.trace_evaluations.len() - 2;
    if request.random_coeff_powers.len() != expected_coeff_len {
        return Err(MetalWideFibonacciQuotientError::RandomCoeffCountMismatch {
            expected_len: expected_coeff_len,
            actual_len: request.random_coeff_powers.len(),
        });
    }

    let expected_denominator_len =
        1usize << (request.eval_domain_log_size - request.domain_log_size);
    if request.denominator_inverses.len() != expected_denominator_len {
        return Err(MetalWideFibonacciQuotientError::DenominatorCountMismatch {
            expected_len: expected_denominator_len,
            actual_len: request.denominator_inverses.len(),
        });
    }

    let mut trace_values = U32Buffer::uninitialized(
        request
            .trace_evaluations
            .len()
            .checked_mul(eval_domain_size)
            .unwrap(),
    )?;
    for (column_index, column) in request.trace_evaluations.iter().enumerate() {
        trace_values.copy_from_offset(&column.buffer, column_index * eval_domain_size)?;
    }
    let random_coeff_powers = request
        .random_coeff_powers
        .iter()
        .flat_map(|value| value.to_m31_array().map(|limb| limb.0))
        .collect::<Vec<_>>();
    let denominator_inverses = request
        .denominator_inverses
        .iter()
        .map(|value| value.0)
        .collect::<Vec<_>>();
    let random_coeff_powers = U32Buffer::from_slice(&random_coeff_powers)?;
    let denominator_inverses = U32Buffer::from_slice(&denominator_inverses)?;
    let values = U32Buffer::accumulate_wide_fibonacci_quotients(
        &trace_values,
        &random_coeff_powers,
        &denominator_inverses,
        request.domain_log_size,
        request.eval_domain_log_size,
        expected_coeff_len as u32,
    )?;

    Ok(MetalWideFibonacciQuotients {
        values: SecureFieldVec::from_buffer(values),
        eval_domain_size,
    })
}

impl QuotientOps for MetalBackend {
    fn accumulate_numerators(
        columns: &[&CircleEvaluation<Self, BaseField, BitReversedOrder>],
        sample_batches: &[ColumnSampleBatch],
        accumulated_numerators_vec: &mut Vec<AccumulatedNumerators<Self>>,
    ) {
        let size = columns[0].len();
        let mut flat_columns = U32Buffer::uninitialized(columns.len() * size)
            .expect("Metal quotient column staging should allocate");
        for (column_index, column) in columns.iter().enumerate() {
            flat_columns
                .copy_from_offset(&column.values.buffer, column_index * size)
                .expect("Metal quotient column staging should copy");
        }
        let quotient_constants = quotient_constants(sample_batches);
        let mut concatenated_column_indices = Vec::new();
        let mut concatenated_b_coeffs = Vec::new();
        let mut concatenated_c_coeffs = Vec::new();
        let mut term_offsets = Vec::with_capacity(sample_batches.len());
        let mut term_counts = Vec::with_capacity(sample_batches.len());
        let mut first_linear_term_accs = Vec::with_capacity(sample_batches.len());

        for (batch, coeffs) in sample_batches
            .iter()
            .zip(quotient_constants.line_coeffs.into_iter())
        {
            term_offsets.push(
                concatenated_column_indices
                    .len()
                    .try_into()
                    .expect("Metal quotient term offset should fit in u32"),
            );
            term_counts.push(
                batch
                    .cols_vals_randpows
                    .len()
                    .try_into()
                    .expect("Metal quotient term count should fit in u32"),
            );
            concatenated_column_indices.extend(batch.cols_vals_randpows.iter().map(|data| {
                u32::try_from(data.column_index)
                    .expect("Metal quotient column index should fit in u32")
            }));
            concatenated_b_coeffs.extend(
                coeffs
                    .iter()
                    .flat_map(|(_, b, _)| b.to_m31_array().map(|limb| limb.0)),
            );
            concatenated_c_coeffs.extend(
                coeffs
                    .iter()
                    .flat_map(|(_, _, c)| c.to_m31_array().map(|limb| limb.0)),
            );
            first_linear_term_accs.push(coeffs.iter().map(|(a, ..)| a).sum::<SecureField>());
        }

        let column_indices = U32Buffer::from_slice(&concatenated_column_indices)
            .expect("Metal quotient index upload should initialize");
        let b_coeffs = U32Buffer::from_slice(&concatenated_b_coeffs)
            .expect("Metal quotient b upload should initialize");
        let c_coeffs = U32Buffer::from_slice(&concatenated_c_coeffs)
            .expect("Metal quotient c upload should initialize");
        let term_offsets = U32Buffer::from_slice(&term_offsets)
            .expect("Metal quotient term-offset upload should initialize");
        let term_counts = U32Buffer::from_slice(&term_counts)
            .expect("Metal quotient term-count upload should initialize");
        let batched_values = U32Buffer::accumulate_partial_numerators_batched(
            &flat_columns,
            &column_indices,
            &b_coeffs,
            &c_coeffs,
            &term_offsets,
            &term_counts,
            size,
        )
        .expect("Metal batched partial numerator accumulation should succeed");

        for (batch_index, batch) in sample_batches.iter().enumerate() {
            let packed_values = Arc::new(
                batched_values
                    .clone_range(batch_index * size * 4, size * 4)
                    .expect("Metal batched partial numerator slice should clone"),
            );
            let values = SecureFieldVec::from_buffer(
                (*packed_values)
                    .clone_range(0, size * 4)
                    .expect("Metal partial numerator buffer should clone"),
            );
            let partial_columns = values.to_base_coords();
            cache_packed_partial_numerators(&partial_columns, packed_values);
            accumulated_numerators_vec.push(AccumulatedNumerators {
                sample_point: batch.point,
                partial_numerators_acc: stwo::prover::secure_column::SecureColumnByCoords {
                    columns: partial_columns,
                },
                first_linear_term_acc: first_linear_term_accs[batch_index],
            });
        }
    }

    fn compute_quotients_and_combine(
        accumulations: Vec<AccumulatedNumerators<Self>>,
        lifting_log_size: u32,
    ) -> SecureEvaluation<Self, BitReversedOrder> {
        let domain = CanonicCoset::new(lifting_log_size).circle_domain();
        if accumulations.is_empty() {
            return SecureEvaluation::new(
                domain,
                metal_secure_column_from_values(vec![
                    SecureField::zero();
                    1usize << lifting_log_size
                ]),
            );
        }
        let total_partial_len = accumulations
            .iter()
            .map(|acc| acc.partial_numerators_acc.len())
            .sum::<usize>();
        let mut partial_offsets = Vec::with_capacity(accumulations.len());
        let mut partial_log_sizes = Vec::with_capacity(accumulations.len());
        let mut sample_points = Vec::with_capacity(accumulations.len() * 8);
        let mut first_linear_terms = Vec::with_capacity(accumulations.len() * 4);
        let packed_accumulations = accumulations
            .iter()
            .map(|accumulation| {
                take_cached_packed_partial_numerators(&accumulation.partial_numerators_acc.columns)
            })
            .collect::<Option<Vec<_>>>();

        let mut partials_packed = packed_accumulations.as_ref().map(|_| {
            U32Buffer::uninitialized(total_partial_len * 4)
                .expect("Metal packed quotient-combine partial staging should allocate")
        });
        let mut partial_coords: Option<[U32Buffer; 4]> = if packed_accumulations.is_none() {
            Some(std::array::from_fn(|_| {
                U32Buffer::uninitialized(total_partial_len)
                    .expect("Metal quotient-combine partial staging should allocate")
            }))
        } else {
            None
        };
        let mut offset = 0usize;

        for (accumulation, packed_partial) in accumulations.iter().zip(
            packed_accumulations
                .as_ref()
                .map(|partials| partials.iter().map(Some))
                .into_iter()
                .flatten()
                .chain(std::iter::repeat(None).take(accumulations.len())),
        ) {
            let partial_len = accumulation.partial_numerators_acc.len();
            partial_offsets.push(
                offset
                    .try_into()
                    .expect("partial numerator offset should fit in u32"),
            );
            partial_log_sizes.push(accumulation.partial_numerators_acc.len().ilog2());
            sample_points.extend_from_slice(&pack_secure_circle_point(accumulation.sample_point));
            first_linear_terms.extend(
                accumulation
                    .first_linear_term_acc
                    .to_m31_array()
                    .map(|limb| limb.0),
            );

            if let (Some(partials_packed), Some(packed_partial)) =
                (partials_packed.as_mut(), packed_partial)
            {
                partials_packed
                    .copy_range_from(packed_partial.as_ref(), 0, partial_len * 4, offset * 4)
                    .expect("Metal packed quotient-combine partial staging should copy");
            } else if let Some(partial_coords) = partial_coords.as_mut() {
                for (coord_buffer, column) in partial_coords
                    .iter_mut()
                    .zip(accumulation.partial_numerators_acc.columns.each_ref())
                {
                    coord_buffer
                        .copy_range_from(&column.buffer, 0, partial_len, offset)
                        .expect("Metal quotient-combine partial staging should copy");
                }
            }
            offset += partial_len;
        }

        let sample_points = U32Buffer::from_slice(&sample_points)
            .expect("Metal quotient-combine sample-point upload should initialize");
        let first_linear_terms = U32Buffer::from_slice(&first_linear_terms)
            .expect("Metal quotient-combine first-linear-term upload should initialize");
        let partial_log_sizes = U32Buffer::from_slice(&partial_log_sizes)
            .expect("Metal quotient-combine partial log-size upload should initialize");
        let partial_offsets = U32Buffer::from_slice(&partial_offsets)
            .expect("Metal quotient-combine partial offset upload should initialize");
        let domain_coords = cached_quotient_domain_coords(lifting_log_size);

        let result = if let Some(partials_packed) = partials_packed.as_ref() {
            U32Buffer::compute_quotients_and_combine_packed(
                partials_packed,
                &sample_points,
                &first_linear_terms,
                &partial_log_sizes,
                &partial_offsets,
                &domain_coords.0,
                &domain_coords.1,
                lifting_log_size,
            )
            .expect("Metal packed quotient-combine kernel should succeed")
        } else {
            let partial_coords = partial_coords
                .as_ref()
                .expect("coordinate quotient-combine staging should be present");
            U32Buffer::compute_quotients_and_combine(
                [
                    &partial_coords[0],
                    &partial_coords[1],
                    &partial_coords[2],
                    &partial_coords[3],
                ],
                &sample_points,
                &first_linear_terms,
                &partial_log_sizes,
                &partial_offsets,
                &domain_coords.0,
                &domain_coords.1,
                lifting_log_size,
            )
            .expect("Metal quotient-combine kernel should succeed")
        };

        let columns = SecureFieldVec::from_buffer(result).to_base_coords();
        SecureEvaluation::new(
            domain,
            stwo::prover::secure_column::SecureColumnByCoords { columns },
        )
    }
}
