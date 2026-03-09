use stwo::core::circle::{CirclePoint, Coset};
use stwo::core::fields::m31::BaseField;
use stwo::core::fields::qm31::SecureField;
use stwo::core::poly::circle::{CanonicCoset, CircleDomain};
use stwo::prover::backend::cpu::{CpuCircleEvaluation, CpuCirclePoly};
use stwo::prover::backend::{Col, CpuBackend};
use stwo::prover::poly::circle::{CircleCoefficients, CircleEvaluation, PolyOps};
use stwo::prover::poly::twiddles::TwiddleTree;
use stwo::prover::poly::BitReversedOrder;

use super::MetalBackend;
use crate::stwo_metal::base_field_vec::BaseFieldVec;
use crate::stwo_metal::secure_field_vec::SecureFieldVec;

pub fn permute_coset_to_circle_domain_bit_reversed(values: &BaseFieldVec) -> BaseFieldVec {
    values.coset_to_circle_domain_bit_reversed()
}

fn to_cpu_twiddle_tree(twiddles: &TwiddleTree<MetalBackend>) -> TwiddleTree<CpuBackend> {
    TwiddleTree {
        root_coset: twiddles.root_coset,
        twiddles: twiddles.twiddles.clone(),
        itwiddles: twiddles.itwiddles.clone(),
    }
}

fn to_cpu_circle_poly(poly: &CircleCoefficients<MetalBackend>) -> CpuCirclePoly {
    CpuCirclePoly::new(poly.coeffs.to_vec())
}

fn to_cpu_circle_eval(
    eval: &CircleEvaluation<MetalBackend, BaseField, BitReversedOrder>,
) -> CpuCircleEvaluation<BaseField, BitReversedOrder> {
    CpuCircleEvaluation::new(eval.domain, eval.values.to_vec())
}

fn into_metal_circle_poly(poly: CpuCirclePoly) -> CircleCoefficients<MetalBackend> {
    CircleCoefficients::new(BaseFieldVec::from_vec(poly.coeffs))
}

fn into_metal_circle_eval(
    eval: CpuCircleEvaluation<BaseField, BitReversedOrder>,
) -> CircleEvaluation<MetalBackend, BaseField, BitReversedOrder> {
    CircleEvaluation::new(eval.domain, BaseFieldVec::from_vec(eval.values))
}

impl PolyOps for MetalBackend {
    type Twiddles = Vec<BaseField>;

    fn interpolate(
        eval: CircleEvaluation<Self, BaseField, BitReversedOrder>,
        twiddles: &TwiddleTree<Self>,
    ) -> CircleCoefficients<Self> {
        let cpu_twiddles = to_cpu_twiddle_tree(twiddles);
        let cpu_eval = CpuCircleEvaluation::new(eval.domain, eval.values.to_vec());
        into_metal_circle_poly(CpuBackend::interpolate(cpu_eval, &cpu_twiddles))
    }

    fn eval_at_point(
        poly: &CircleCoefficients<Self>,
        point: CirclePoint<SecureField>,
    ) -> SecureField {
        let cpu_poly = to_cpu_circle_poly(poly);
        CpuBackend::eval_at_point(&cpu_poly, point)
    }

    fn barycentric_weights(
        coset: CanonicCoset,
        p: CirclePoint<SecureField>,
    ) -> Col<Self, SecureField> {
        SecureFieldVec::from_vec(CpuBackend::barycentric_weights(coset, p))
    }

    fn barycentric_eval_at_point(
        evals: &CircleEvaluation<Self, BaseField, BitReversedOrder>,
        weights: &Col<Self, SecureField>,
    ) -> SecureField {
        let cpu_eval = to_cpu_circle_eval(evals);
        let cpu_weights = weights.to_vec();
        CpuBackend::barycentric_eval_at_point(&cpu_eval, &cpu_weights)
    }

    fn eval_at_point_by_folding(
        evals: &CircleEvaluation<Self, BaseField, BitReversedOrder>,
        point: CirclePoint<SecureField>,
        twiddles: &TwiddleTree<Self>,
    ) -> SecureField {
        let cpu_eval = to_cpu_circle_eval(evals);
        let cpu_twiddles = to_cpu_twiddle_tree(twiddles);
        CpuBackend::eval_at_point_by_folding(&cpu_eval, point, &cpu_twiddles)
    }

    fn extend(poly: &CircleCoefficients<Self>, log_size: u32) -> CircleCoefficients<Self> {
        let cpu_poly = to_cpu_circle_poly(poly);
        into_metal_circle_poly(CpuBackend::extend(&cpu_poly, log_size))
    }

    fn evaluate(
        poly: &CircleCoefficients<Self>,
        domain: CircleDomain,
        twiddles: &TwiddleTree<Self>,
    ) -> CircleEvaluation<Self, BaseField, BitReversedOrder> {
        let buffer = BaseFieldVec::new_zeroes(domain.size());
        Self::evaluate_into(poly, domain, twiddles, buffer)
    }

    fn evaluate_into(
        poly: &CircleCoefficients<Self>,
        domain: CircleDomain,
        twiddles: &TwiddleTree<Self>,
        mut buffer: Col<Self, BaseField>,
    ) -> CircleEvaluation<Self, BaseField, BitReversedOrder> {
        let cpu_poly = to_cpu_circle_poly(poly);
        let cpu_twiddles = to_cpu_twiddle_tree(twiddles);
        let cpu_eval = CpuBackend::evaluate_into(
            &cpu_poly,
            domain,
            &cpu_twiddles,
            vec![BaseField::default(); domain.size()],
        );

        let metal_values = BaseFieldVec::from_vec(cpu_eval.values.clone());
        buffer.copy_from(&metal_values);
        CircleEvaluation::new(cpu_eval.domain, buffer)
    }

    fn precompute_twiddles(coset: Coset) -> TwiddleTree<Self> {
        let cpu_twiddles = CpuBackend::precompute_twiddles(coset);
        TwiddleTree {
            root_coset: cpu_twiddles.root_coset,
            twiddles: cpu_twiddles.twiddles,
            itwiddles: cpu_twiddles.itwiddles,
        }
    }

    fn split_at_mid(
        poly: CircleCoefficients<Self>,
    ) -> (CircleCoefficients<Self>, CircleCoefficients<Self>) {
        let cpu_poly = CpuCirclePoly::new(poly.coeffs.to_vec());
        let (left, right) = CpuBackend::split_at_mid(cpu_poly);
        (into_metal_circle_poly(left), into_metal_circle_poly(right))
    }
}

#[allow(dead_code)]
fn _assert_eval_bridge_type(
    eval: CpuCircleEvaluation<BaseField, BitReversedOrder>,
) -> CircleEvaluation<MetalBackend, BaseField, BitReversedOrder> {
    into_metal_circle_eval(eval)
}
