use ark_std::Zero;
use itertools::Itertools;
#[cfg(feature = "parallel")]
use rayon::prelude::*;
use stwo::core::fields::m31::BaseField;
use stwo::core::fields::qm31::SecureField;
use stwo::core::pcs::quotients::{denominator_inverses, quotient_constants, ColumnSampleBatch};
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
        let host_columns = columns
            .iter()
            .map(|column| column.values.host_slice())
            .collect_vec();
        let quotient_constants = quotient_constants(sample_batches);

        for (batch, coeffs) in sample_batches
            .iter()
            .zip(quotient_constants.line_coeffs.into_iter())
        {
            #[cfg(not(feature = "parallel"))]
            let values = (0..size)
                .map(|row| {
                    let mut numerator = SecureField::zero();
                    for (
                        stwo::core::pcs::quotients::NumeratorData { column_index, .. },
                        (_, b, c),
                    ) in batch.cols_vals_randpows.iter().zip_eq(coeffs.iter())
                    {
                        numerator += host_columns[*column_index][row] * *c - *b;
                    }
                    numerator
                })
                .collect();
            #[cfg(feature = "parallel")]
            let values = (0..size)
                .into_par_iter()
                .map(|row| {
                    let mut numerator = SecureField::zero();
                    for (
                        stwo::core::pcs::quotients::NumeratorData { column_index, .. },
                        (_, b, c),
                    ) in batch.cols_vals_randpows.iter().zip_eq(coeffs.iter())
                    {
                        numerator += host_columns[*column_index][row] * *c - *b;
                    }
                    numerator
                })
                .collect();

            let first_linear_term_acc: SecureField = coeffs.iter().map(|(a, ..)| a).sum();
            accumulated_numerators_vec.push(AccumulatedNumerators {
                sample_point: batch.point,
                partial_numerators_acc: metal_secure_column_from_values(values),
                first_linear_term_acc,
            });
        }
    }

    fn compute_quotients_and_combine(
        accumulations: Vec<AccumulatedNumerators<Self>>,
        lifting_log_size: u32,
    ) -> SecureEvaluation<Self, BitReversedOrder> {
        let domain = CanonicCoset::new(lifting_log_size).circle_domain();
        let host_accumulations = accumulations
            .iter()
            .map(|acc| {
                (
                    acc.sample_point,
                    acc.partial_numerators_acc
                        .columns
                        .each_ref()
                        .map(|column| column.host_slice()),
                    acc.partial_numerators_acc.len(),
                    acc.first_linear_term_acc,
                )
            })
            .collect_vec();
        let sample_points = host_accumulations
            .iter()
            .map(|(sample_point, ..)| *sample_point)
            .collect_vec();

        #[cfg(not(feature = "parallel"))]
        let quotient_values = (0..(1usize << lifting_log_size))
            .map(|row| {
                let domain_point = domain.at(bit_reverse_index(row, lifting_log_size));
                let inverses = denominator_inverses(&sample_points, domain_point);
                let mut quotient = SecureField::zero();
                for (
                    (_, partial_numerator_coords, partial_numerator_len, first_linear_term_acc),
                    den_inv,
                ) in host_accumulations.iter().zip_eq(inverses)
                {
                    let log_ratio = lifting_log_size - partial_numerator_len.ilog2();
                    let lifted_idx = (row >> (log_ratio + 1) << 1) + (row & 1);
                    let full_numerator =
                        SecureField::from_m31_array(std::array::from_fn(|coord| {
                            partial_numerator_coords[coord][lifted_idx]
                        })) - *first_linear_term_acc * domain_point.y;
                    quotient += full_numerator.mul_cm31(den_inv);
                }
                quotient
            })
            .collect();
        #[cfg(feature = "parallel")]
        let quotient_values = (0..(1usize << lifting_log_size))
            .into_par_iter()
            .map(|row| {
                let domain_point = domain.at(bit_reverse_index(row, lifting_log_size));
                let inverses = denominator_inverses(&sample_points, domain_point);
                let mut quotient = SecureField::zero();
                for (
                    (_, partial_numerator_coords, partial_numerator_len, first_linear_term_acc),
                    den_inv,
                ) in host_accumulations.iter().zip_eq(inverses)
                {
                    let log_ratio = lifting_log_size - partial_numerator_len.ilog2();
                    let lifted_idx = (row >> (log_ratio + 1) << 1) + (row & 1);
                    let full_numerator =
                        SecureField::from_m31_array(std::array::from_fn(|coord| {
                            partial_numerator_coords[coord][lifted_idx]
                        })) - *first_linear_term_acc * domain_point.y;
                    quotient += full_numerator.mul_cm31(den_inv);
                }
                quotient
            })
            .collect();

        SecureEvaluation::new(domain, metal_secure_column_from_values(quotient_values))
    }
}
