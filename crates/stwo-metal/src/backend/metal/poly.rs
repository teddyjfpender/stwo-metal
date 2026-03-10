use ark_std::Zero;
use itertools::Itertools;
#[cfg(feature = "parallel")]
use rayon::prelude::*;
use stwo::core::circle::{CirclePoint, CirclePointIndex, Coset};
use stwo::core::constraints::{coset_vanishing, coset_vanishing_derivative, point_vanishing};
use stwo::core::fields::m31::BaseField;
use stwo::core::fields::qm31::SecureField;
use stwo::core::poly::circle::{CanonicCoset, CircleDomain};
use stwo::core::poly::line::LineDomain;
use stwo::core::poly::utils::{fold, get_folding_alphas};
use stwo::core::utils::bit_reverse_index;
use stwo::prover::backend::cpu::{CpuCircleEvaluation, CpuCirclePoly};
use stwo::prover::backend::{Col, Column, CpuBackend};
use stwo::prover::fri::FriOps;
use stwo::prover::line::LineEvaluation;
use stwo::prover::poly::circle::{CircleCoefficients, CircleEvaluation, PolyOps};
use stwo::prover::poly::twiddles::TwiddleTree;
use stwo::prover::poly::BitReversedOrder;
use stwo::prover::secure_column::SecureColumnByCoords;
use stwo_metal_sys::metal::{MetalError, U32Buffer};

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

fn point_xy(point: CirclePoint<BaseField>) -> [u32; 2] {
    [point.x.0, point.y.0]
}

fn precompute_twiddles_native(coset: Coset) -> Result<TwiddleTree<MetalBackend>, MetalError> {
    let mut twiddles = U32Buffer::uninitialized(coset.size())?;
    let mut current_initial = coset.initial;
    let mut current_step = coset.step;
    let mut current_log_size = coset.log_size();
    let mut offset = 0usize;

    while current_log_size > 0 {
        let level_len = 1usize << (current_log_size - 1);
        twiddles.write_twiddle_level(
            offset,
            point_xy(current_initial),
            point_xy(current_step),
            current_log_size,
        )?;
        offset += level_len;
        current_initial = current_initial.double();
        current_step = current_step.double();
        current_log_size -= 1;
    }

    twiddles.set(coset.size() - 1, 1);

    let mut itwiddles = twiddles.clone();
    itwiddles.invert_m31_in_place()?;

    Ok(TwiddleTree {
        root_coset: coset,
        twiddles: twiddles
            .to_vec()?
            .into_iter()
            .map(BaseField::from_u32_unchecked)
            .collect(),
        itwiddles: itwiddles
            .to_vec()?
            .into_iter()
            .map(BaseField::from_u32_unchecked)
            .collect(),
    })
}

fn tail_twiddle_buffer(values_len: usize, twiddles: &[BaseField]) -> Result<U32Buffer, MetalError> {
    let eval_domain_size = values_len / 2;
    assert!(
        eval_domain_size <= twiddles.len(),
        "twiddle tree tail length {} exceeds available twiddle len {}",
        eval_domain_size,
        twiddles.len()
    );
    let slice = &twiddles[twiddles.len() - eval_domain_size..];
    let raw: Vec<u32> = slice.iter().map(|value| value.0).collect();
    U32Buffer::from_slice(&raw)
}

fn evaluate_into_native(
    poly: &CircleCoefficients<MetalBackend>,
    domain: CircleDomain,
    twiddles: &TwiddleTree<MetalBackend>,
    mut buffer: BaseFieldVec,
) -> Result<CircleEvaluation<MetalBackend, BaseField, BitReversedOrder>, MetalError> {
    let domain_log_size = domain.log_size();
    assert!(domain.half_coset.is_doubling_of(twiddles.root_coset));
    assert_eq!(buffer.len(), domain.size());

    if domain_log_size <= 3 {
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
        return Ok(CircleEvaluation::new(cpu_eval.domain, buffer));
    }

    let extended = MetalBackend::extend(poly, domain_log_size);
    buffer.copy_from(&extended.coeffs);

    let mut values = U32Buffer::from_slice(
        &buffer
            .to_vec()
            .into_iter()
            .map(|value| value.0)
            .collect::<Vec<_>>(),
    )?;
    let twiddle_tail = tail_twiddle_buffer(values.len(), &twiddles.twiddles)?;
    values.rfft_evaluate_in_place(&twiddle_tail)?;
    let native_values = BaseFieldVec::from_vec(
        values
            .to_vec()?
            .into_iter()
            .map(BaseField::from_u32_unchecked)
            .collect(),
    );
    buffer.copy_from(&native_values);
    Ok(CircleEvaluation::new(domain, buffer))
}

fn interpolate_native(
    eval: CircleEvaluation<MetalBackend, BaseField, BitReversedOrder>,
    twiddles: &TwiddleTree<MetalBackend>,
) -> Result<CircleCoefficients<MetalBackend>, MetalError> {
    assert!(eval.domain.half_coset.is_doubling_of(twiddles.root_coset));

    if eval.domain.log_size() <= 3 {
        let cpu_eval = to_cpu_circle_eval(&eval);
        let cpu_twiddles = to_cpu_twiddle_tree(twiddles);
        return Ok(into_metal_circle_poly(CpuBackend::interpolate(
            cpu_eval,
            &cpu_twiddles,
        )));
    }

    let mut values = U32Buffer::from_slice(
        &eval
            .values
            .to_vec()
            .into_iter()
            .map(|value| value.0)
            .collect::<Vec<_>>(),
    )?;
    let inverse_twiddle_tail = tail_twiddle_buffer(values.len(), &twiddles.itwiddles)?;
    let scale_factor = BaseField::from_u32_unchecked(
        values
            .len()
            .try_into()
            .expect("IFFT scale length should fit in u32"),
    )
    .inverse()
    .0;
    values.ifft_interpolate_in_place(&inverse_twiddle_tail, scale_factor)?;
    Ok(CircleCoefficients::new(BaseFieldVec::from_vec(
        values
            .to_vec()?
            .into_iter()
            .map(BaseField::from_u32_unchecked)
            .collect(),
    )))
}

impl PolyOps for MetalBackend {
    type Twiddles = Vec<BaseField>;

    fn interpolate(
        eval: CircleEvaluation<Self, BaseField, BitReversedOrder>,
        twiddles: &TwiddleTree<Self>,
    ) -> CircleCoefficients<Self> {
        interpolate_native(eval, twiddles)
            .expect("Metal interpolate should complete through the native RFFT/IFFT lane")
    }

    fn eval_at_point(
        poly: &CircleCoefficients<Self>,
        point: CirclePoint<SecureField>,
    ) -> SecureField {
        if poly.log_size() == 0 {
            return poly.coeffs.at(0).into();
        }

        let mut mappings = vec![point.y];
        let mut x = point.x;
        for _ in 1..poly.log_size() {
            mappings.push(x);
            x = CirclePoint::double_x(x);
        }
        mappings.reverse();

        fold(&poly.coeffs.to_vec(), &mappings)
    }

    fn batch_eval_at_point(
        polys: &[&CircleCoefficients<Self>],
        point: CirclePoint<SecureField>,
    ) -> Vec<SecureField> {
        #[cfg(not(feature = "parallel"))]
        let iter = polys.iter();
        #[cfg(feature = "parallel")]
        let iter = polys.par_iter();

        iter.map(|poly| Self::eval_at_point(poly, point)).collect()
    }

    fn barycentric_weights(
        coset: CanonicCoset,
        p: CirclePoint<SecureField>,
    ) -> Col<Self, SecureField> {
        let domain = coset.circle_domain();

        let (si_i, vi_p): (Vec<_>, Vec<_>) = (0..domain.size())
            .map(|i| {
                let coset_point = domain
                    .at(bit_reverse_index(i, domain.log_size()))
                    .into_ef::<SecureField>();
                let minus_two_coset_point_y = coset_point.y * SecureField::from(-2);
                (
                    minus_two_coset_point_y
                        * coset_vanishing_derivative(
                            Coset::new(CirclePointIndex::generator(), domain.log_size()),
                            coset_point,
                        ),
                    point_vanishing(coset_point, p.into_ef::<SecureField>()),
                )
            })
            .unzip();

        let vn_p: SecureField = coset_vanishing(
            CanonicCoset::new(domain.log_size()).coset,
            p.into_ef::<SecureField>(),
        );

        SecureFieldVec::from_vec(
            (0..domain.size())
                .map(|i| vn_p / (si_i[i] * vi_p[i]))
                .collect_vec(),
        )
    }

    fn barycentric_eval_at_point(
        evals: &CircleEvaluation<Self, BaseField, BitReversedOrder>,
        weights: &Col<Self, SecureField>,
    ) -> SecureField {
        (0..evals.domain.size()).fold(SecureField::zero(), |acc, i| {
            acc + (evals.values.at(i) * weights.at(i))
        })
    }

    fn eval_at_point_by_folding(
        evals: &CircleEvaluation<Self, BaseField, BitReversedOrder>,
        point: CirclePoint<SecureField>,
        twiddles: &TwiddleTree<Self>,
    ) -> SecureField {
        let log_size = evals.domain.log_size();
        let mut folding_alphas = get_folding_alphas(point, log_size as usize);
        let first_inner_layer_domain = LineDomain::new(Coset::half_odds(log_size - 1));
        let mut layer_evaluation = LineEvaluation::new_zero(first_inner_layer_domain);

        let base_values = evals.values.to_vec();
        let secure_field_values = SecureColumnByCoords {
            columns: std::array::from_fn(|coord| {
                if coord == 0 {
                    BaseFieldVec::from_vec(base_values.clone())
                } else {
                    BaseFieldVec::new_zeroes(base_values.len())
                }
            }),
        };

        MetalBackend::fold_circle_into_line(
            &mut layer_evaluation,
            &stwo::prover::poly::circle::SecureEvaluation::new(evals.domain, secure_field_values),
            folding_alphas.pop().unwrap(),
            twiddles,
        );

        while layer_evaluation.len() > 1 {
            layer_evaluation = MetalBackend::fold_line(
                &layer_evaluation,
                folding_alphas.pop().unwrap(),
                twiddles,
                1,
            );
        }

        layer_evaluation.values.at(0) / SecureField::from(2_u32.pow(log_size))
    }

    fn extend(poly: &CircleCoefficients<Self>, log_size: u32) -> CircleCoefficients<Self> {
        assert!(
            log_size >= poly.log_size(),
            "Metal extend requires a target log size at least as large as the source"
        );
        let mut coeffs = poly.coeffs.clone();
        coeffs.pad_to_size(1usize << log_size);
        CircleCoefficients::new(coeffs)
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
        buffer: Col<Self, BaseField>,
    ) -> CircleEvaluation<Self, BaseField, BitReversedOrder> {
        evaluate_into_native(poly, domain, twiddles, buffer)
            .expect("Metal evaluate should complete through the native RFFT/IFFT lane")
    }

    fn precompute_twiddles(coset: Coset) -> TwiddleTree<Self> {
        precompute_twiddles_native(coset)
            .expect("Metal twiddle precompute should produce native parity-tested twiddles")
    }

    fn split_at_mid(
        poly: CircleCoefficients<Self>,
    ) -> (CircleCoefficients<Self>, CircleCoefficients<Self>) {
        let (left, right) = poly.coeffs.split_at_mid();
        (
            CircleCoefficients::new(left),
            CircleCoefficients::new(right),
        )
    }
}

#[allow(dead_code)]
fn _assert_eval_bridge_type(
    eval: CpuCircleEvaluation<BaseField, BitReversedOrder>,
) -> CircleEvaluation<MetalBackend, BaseField, BitReversedOrder> {
    into_metal_circle_eval(eval)
}
