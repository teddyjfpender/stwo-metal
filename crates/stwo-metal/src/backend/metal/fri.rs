use std::array;

use stwo::core::fields::m31::BaseField;
use stwo::core::fields::qm31::SecureField;
use stwo::core::fri::CIRCLE_TO_LINE_FOLD_STEP;
use stwo::core::poly::circle::CircleDomain;
use stwo::core::poly::line::LineDomain;
use stwo::core::utils::bit_reverse_index;
use stwo::prover::backend::CpuBackend;
use stwo::prover::fri::FriOps;
use stwo::prover::line::LineEvaluation;
use stwo::prover::poly::circle::SecureEvaluation;
use stwo::prover::poly::twiddles::TwiddleTree;
use stwo::prover::poly::BitReversedOrder;
use stwo::prover::secure_column::SecureColumnByCoords;

use super::accumulation::metal_secure_column_from_cpu;
use super::MetalBackend;
use crate::stwo_metal::base_field_vec::BaseFieldVec;
use crate::stwo_metal::secure_field_vec::SecureFieldVec;

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

fn pack_secure_column(values: &SecureColumnByCoords<MetalBackend>) -> SecureFieldVec {
    // `MetalBackend` still stores secure columns by coordinates, so FriOps currently repacks
    // through CPU-visible values before entering the bounded packed Metal fold kernels.
    SecureFieldVec::from_vec(values.to_cpu().to_vec())
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

fn metal_line_evaluation_from_packed(
    domain: LineDomain,
    values: SecureFieldVec,
) -> LineEvaluation<MetalBackend> {
    LineEvaluation::new(domain, metal_secure_column_from_values(values.to_vec()))
}

impl FriOps for MetalBackend {
    fn fold_line(
        eval: &LineEvaluation<Self>,
        alpha: SecureField,
        _twiddles: &TwiddleTree<Self>,
        fold_step: u32,
    ) -> LineEvaluation<Self> {
        let packed = pack_secure_column(&eval.values);
        let folded = fold_line(&packed, eval.domain(), alpha, fold_step);
        metal_line_evaluation_from_packed(eval.domain().repeated_double(fold_step), folded)
    }

    fn fold_circle_into_line(
        dst: &mut LineEvaluation<Self>,
        src: &SecureEvaluation<Self, BitReversedOrder>,
        alpha: SecureField,
        _twiddles: &TwiddleTree<Self>,
    ) {
        let alpha_sq = alpha * alpha;
        let folded =
            fold_circle_into_line_first_layer(&pack_secure_column(&src.values), src.domain, alpha);
        let combined = dst
            .values
            .to_cpu()
            .to_vec()
            .into_iter()
            .zip(folded.to_vec())
            .map(|(previous, folded_value)| previous * alpha_sq + folded_value)
            .collect();

        *dst = LineEvaluation::new(dst.domain(), metal_secure_column_from_values(combined));
    }

    fn decompose(
        eval: &SecureEvaluation<Self, BitReversedOrder>,
    ) -> (SecureEvaluation<Self, BitReversedOrder>, SecureField) {
        let (decomposed, lambda) = CpuBackend::decompose(&eval.to_cpu());
        (
            SecureEvaluation::new(
                decomposed.domain,
                metal_secure_column_from_cpu(decomposed.values),
            ),
            lambda,
        )
    }
}
