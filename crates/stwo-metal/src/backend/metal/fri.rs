use std::array;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, OnceLock};

use stwo::core::fields::m31::BaseField;
use stwo::core::fields::qm31::SecureField;
use stwo::core::fri::CIRCLE_TO_LINE_FOLD_STEP;
use stwo::core::poly::circle::CircleDomain;
use stwo::core::poly::line::LineDomain;
use stwo::core::utils::bit_reverse_index;
use stwo::prover::fri::FriOps;
use stwo::prover::line::LineEvaluation;
use stwo::prover::poly::circle::SecureEvaluation;
use stwo::prover::poly::twiddles::TwiddleTree;
use stwo::prover::poly::BitReversedOrder;
use stwo::prover::secure_column::SecureColumnByCoords;
use stwo_metal_sys::metal::U32Buffer;

use super::MetalBackend;
use crate::stwo_metal::base_field_vec::BaseFieldVec;
use crate::stwo_metal::secure_field_vec::SecureFieldVec;

type DomainFactorCache = Mutex<BTreeMap<(usize, usize, u32), Arc<U32Buffer>>>;

fn line_inverse_x_factor_cache() -> &'static DomainFactorCache {
    static CACHE: OnceLock<DomainFactorCache> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn circle_inverse_y_factor_cache() -> &'static DomainFactorCache {
    static CACHE: OnceLock<DomainFactorCache> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn cached_line_inverse_x_factors(domain: LineDomain) -> Arc<U32Buffer> {
    let coset = domain.coset();
    let key = (coset.initial_index.0, coset.step_size.0, coset.log_size());
    if let Some(factors) = line_inverse_x_factor_cache()
        .lock()
        .expect("line inverse-x factor cache mutex should not be poisoned")
        .get(&key)
        .cloned()
    {
        return factors;
    }

    let factors = Arc::new(
        U32Buffer::from_slice(
            &(0..(domain.size() >> 1))
                .map(|i| {
                    domain
                        .at(bit_reverse_index(i << 1, domain.log_size()))
                        .inverse()
                        .0
                })
                .collect::<Vec<_>>(),
        )
        .expect("Metal inverse-x factor upload should initialize"),
    );
    line_inverse_x_factor_cache()
        .lock()
        .expect("line inverse-x factor cache mutex should not be poisoned")
        .insert(key, factors.clone());
    factors
}

fn cached_circle_inverse_y_factors(domain: CircleDomain) -> Arc<U32Buffer> {
    let coset = domain.half_coset;
    let key = (coset.initial_index.0, coset.step_size.0, domain.log_size());
    if let Some(factors) = circle_inverse_y_factor_cache()
        .lock()
        .expect("circle inverse-y factor cache mutex should not be poisoned")
        .get(&key)
        .cloned()
    {
        return factors;
    }

    let factors = Arc::new(
        U32Buffer::from_slice(
            &(0..(domain.size() >> CIRCLE_TO_LINE_FOLD_STEP))
                .map(|i| {
                    domain
                        .at(bit_reverse_index(
                            i << CIRCLE_TO_LINE_FOLD_STEP,
                            domain.log_size(),
                        ))
                        .y
                        .inverse()
                        .0
                })
                .collect::<Vec<_>>(),
        )
        .expect("Metal inverse-y factor upload should initialize"),
    );
    circle_inverse_y_factor_cache()
        .lock()
        .expect("circle inverse-y factor cache mutex should not be poisoned")
        .insert(key, factors.clone());
    factors
}

pub fn fold_circle_into_line_first_layer(
    src: &SecureFieldVec,
    domain: CircleDomain,
    alpha: SecureField,
) -> SecureFieldVec {
    assert_eq!(
        src.len(),
        domain.size(),
        "FRI first-layer fold requires one secure-field value per domain point"
    );

    let inverse_y_factors = (0..(src.len() >> CIRCLE_TO_LINE_FOLD_STEP))
        .map(|i| {
            domain
                .at(bit_reverse_index(
                    i << CIRCLE_TO_LINE_FOLD_STEP,
                    domain.log_size(),
                ))
                .y
                .inverse()
                .0
        })
        .collect::<Vec<_>>();

    src.fold_circle_into_line_first_layer(&inverse_y_factors, alpha)
}

pub fn fold_line(
    src: &SecureFieldVec,
    mut domain: LineDomain,
    alpha: SecureField,
    fold_step: u32,
) -> SecureFieldVec {
    assert!(
        fold_step >= 1,
        "FRI line fold requires a positive fold_step"
    );
    assert_eq!(
        src.len(),
        domain.size(),
        "FRI line fold requires one secure-field value per domain point"
    );
    assert!(
        domain.log_size() >= fold_step,
        "FRI line fold cannot remove more layers than the domain contains"
    );

    let mut current = src.clone();
    let mut current_alpha = alpha;
    for _ in 0..fold_step {
        let inverse_x_factors = (0..(current.len() >> 1))
            .map(|i| {
                domain
                    .at(bit_reverse_index(i << 1, domain.log_size()))
                    .inverse()
                    .0
            })
            .collect::<Vec<_>>();
        current = current.fold_line_step(&inverse_x_factors, current_alpha);
        domain = domain.double();
        current_alpha = current_alpha * current_alpha;
    }

    current
}

fn metal_secure_column_from_values(values: Vec<SecureField>) -> SecureColumnByCoords<MetalBackend> {
    let mut columns = array::from_fn(|_| Vec::<BaseField>::with_capacity(values.len()));
    for value in values {
        for (column, coord) in columns.iter_mut().zip(value.to_m31_array()) {
            column.push(coord);
        }
    }
    SecureColumnByCoords {
        columns: columns.map(BaseFieldVec::from_vec),
    }
}

fn metal_line_evaluation_from_base_coords(
    domain: LineDomain,
    columns: [BaseFieldVec; 4],
) -> LineEvaluation<MetalBackend> {
    LineEvaluation::new(domain, SecureColumnByCoords { columns })
}

impl FriOps for MetalBackend {
    fn fold_line(
        eval: &LineEvaluation<Self>,
        alpha: SecureField,
        _twiddles: &TwiddleTree<Self>,
        fold_step: u32,
    ) -> LineEvaluation<Self> {
        let mut domain = eval.domain();
        let mut current = [
            eval.values.columns[0].clone(),
            eval.values.columns[1].clone(),
            eval.values.columns[2].clone(),
            eval.values.columns[3].clone(),
        ];
        let mut current_alpha = alpha;
        for _ in 0..fold_step {
            let inverse_x_factors = cached_line_inverse_x_factors(domain);
            current = SecureFieldVec::fold_line_step_base_coords_with_factor_buffer(
                [&current[0], &current[1], &current[2], &current[3]],
                inverse_x_factors.as_ref(),
                current_alpha,
            );
            domain = domain.double();
            current_alpha = current_alpha * current_alpha;
        }

        metal_line_evaluation_from_base_coords(domain, current)
    }

    fn fold_circle_into_line(
        dst: &mut LineEvaluation<Self>,
        src: &SecureEvaluation<Self, BitReversedOrder>,
        alpha: SecureField,
        _twiddles: &TwiddleTree<Self>,
    ) {
        let inverse_y_factors = cached_circle_inverse_y_factors(src.domain);
        SecureFieldVec::fold_circle_into_line_accumulate_base_coords_with_factor_buffer(
            [
                &src.values.columns[0],
                &src.values.columns[1],
                &src.values.columns[2],
                &src.values.columns[3],
            ],
            &mut dst.values.columns,
            inverse_y_factors.as_ref(),
            alpha,
        );
    }

    fn decompose(
        eval: &SecureEvaluation<Self, BitReversedOrder>,
    ) -> (SecureEvaluation<Self, BitReversedOrder>, SecureField) {
        let domain_size = eval.len();
        let half_domain_size = domain_size / 2;

        let a_sum = (0..half_domain_size)
            .map(|i| eval.values.at(i))
            .sum::<SecureField>();
        let b_sum = (half_domain_size..domain_size)
            .map(|i| eval.values.at(i))
            .sum::<SecureField>();
        let lambda = (a_sum - b_sum) / BaseField::from_u32_unchecked(domain_size as u32);

        let values = (0..domain_size)
            .map(|i| {
                let value = eval.values.at(i);
                if i < half_domain_size {
                    value - lambda
                } else {
                    value + lambda
                }
            })
            .collect();
        (
            SecureEvaluation::new(eval.domain, metal_secure_column_from_values(values)),
            lambda,
        )
    }
}
