#![cfg(all(feature = "prover", target_os = "macos", target_arch = "aarch64"))]

use std::borrow::Cow;

use ark_std::{One, Zero};
use stwo::core::fields::m31::BaseField;
use stwo::core::fields::qm31::SecureField;
use stwo::core::fields::FieldExpOps;
use stwo::core::Fraction;
use stwo::prover::backend::{Column, CpuBackend};
use stwo::prover::lookups::gkr_prover::{EqEvals, GkrMultivariatePolyOracle, GkrOps, Layer};
use stwo::prover::lookups::mle::{Mle, MleOps};
use stwo::prover::lookups::utils::{eq, Reciprocal};
use stwo_metal::{
    metal_runtime_support, MetalBackend, MetalBaseFieldVec, MetalRuntimeSupport,
    MetalSecureFieldVec,
};

fn require_metal_runtime() {
    assert_eq!(
        metal_runtime_support(),
        MetalRuntimeSupport::Available,
        "Metal runtime must be available for the native Apple Silicon parity test"
    );
}

fn sf(seed: u32) -> SecureField {
    SecureField::from_u32_unchecked(seed, seed + 1, seed + 2, seed + 3)
}

fn base_values(count: usize, seed: u32) -> Vec<BaseField> {
    (0..count)
        .map(|i| BaseField::from_u32_unchecked(seed + i as u32))
        .collect()
}

fn secure_values(count: usize, seed: u32) -> Vec<SecureField> {
    (0..count).map(|i| sf(seed + 4 * i as u32)).collect()
}

fn into_metal_layer(cpu_layer: Layer<CpuBackend>) -> Layer<MetalBackend> {
    match cpu_layer {
        Layer::GrandProduct(mle) => {
            Layer::GrandProduct(Mle::new(MetalSecureFieldVec::from_vec(mle.into_evals())))
        }
        Layer::LogUpGeneric {
            numerators,
            denominators,
        } => Layer::LogUpGeneric {
            numerators: Mle::new(MetalSecureFieldVec::from_vec(numerators.into_evals())),
            denominators: Mle::new(MetalSecureFieldVec::from_vec(denominators.into_evals())),
        },
        Layer::LogUpMultiplicities {
            numerators,
            denominators,
        } => Layer::LogUpMultiplicities {
            numerators: Mle::new(MetalBaseFieldVec::from_vec(numerators.into_evals())),
            denominators: Mle::new(MetalSecureFieldVec::from_vec(denominators.into_evals())),
        },
        Layer::LogUpSingles { denominators } => Layer::LogUpSingles {
            denominators: Mle::new(MetalSecureFieldVec::from_vec(denominators.into_evals())),
        },
    }
}

fn assert_layers_eq(actual: Layer<CpuBackend>, expected: Layer<CpuBackend>) {
    match (actual, expected) {
        (Layer::GrandProduct(actual), Layer::GrandProduct(expected)) => {
            assert_eq!(actual.into_evals(), expected.into_evals())
        }
        (
            Layer::LogUpGeneric {
                numerators: actual_numerators,
                denominators: actual_denominators,
            },
            Layer::LogUpGeneric {
                numerators: expected_numerators,
                denominators: expected_denominators,
            },
        ) => {
            assert_eq!(
                actual_numerators.into_evals(),
                expected_numerators.into_evals()
            );
            assert_eq!(
                actual_denominators.into_evals(),
                expected_denominators.into_evals()
            );
        }
        (actual, expected) => panic!(
            "layer variants differ: actual={:?}, expected={:?}",
            actual, expected
        ),
    }
}

fn into_cpu_oracle(
    cpu_layer: Layer<CpuBackend>,
    y: &[SecureField],
    lambda: SecureField,
) -> GkrMultivariatePolyOracle<'static, CpuBackend> {
    GkrMultivariatePolyOracle {
        eq_evals: Cow::Owned(EqEvals::<CpuBackend>::generate(y)),
        input_layer: cpu_layer,
        eq_fixed_var_correction: SecureField::one(),
        lambda,
    }
}

fn into_metal_oracle(
    cpu_layer: Layer<CpuBackend>,
    y: &[SecureField],
    lambda: SecureField,
) -> GkrMultivariatePolyOracle<'static, MetalBackend> {
    GkrMultivariatePolyOracle {
        eq_evals: Cow::Owned(EqEvals::<MetalBackend>::generate(y)),
        input_layer: into_metal_layer(cpu_layer),
        eq_fixed_var_correction: SecureField::one(),
        lambda,
    }
}

fn oracle_claim(oracle: &GkrMultivariatePolyOracle<'_, CpuBackend>) -> SecureField {
    let n_variables = oracle.input_layer.n_variables() - 1;
    let n_terms = 1usize << (n_variables - 1);
    let y = oracle.eq_evals.y();
    let prefix_scale = eq(
        &vec![SecureField::zero(); y.len() - n_variables + 1],
        &y[..y.len() - n_variables + 1],
    )
    .inverse();
    let scale_at_0 = prefix_scale * eq(&[SecureField::zero()], &[y[y.len() - n_variables]]);
    let scale_at_1 = prefix_scale * eq(&[SecureField::one()], &[y[y.len() - n_variables]]);

    let (f_at_0, f_at_1) = match &oracle.input_layer {
        Layer::GrandProduct(col) => claim_grand_product(col, oracle, n_terms),
        Layer::LogUpGeneric {
            numerators,
            denominators,
        } => claim_logup_generic(numerators, denominators, oracle, n_terms),
        Layer::LogUpMultiplicities {
            numerators,
            denominators,
        } => claim_logup_multiplicities(numerators, denominators, oracle, n_terms),
        Layer::LogUpSingles { denominators } => claim_logup_singles(denominators, oracle, n_terms),
    };

    oracle.eq_fixed_var_correction * (scale_at_0 * f_at_0 + scale_at_1 * f_at_1)
}

fn claim_grand_product(
    col: &Mle<CpuBackend, SecureField>,
    oracle: &GkrMultivariatePolyOracle<'_, CpuBackend>,
    n_terms: usize,
) -> (SecureField, SecureField) {
    let mut f_at_0 = SecureField::zero();
    let mut f_at_1 = SecureField::zero();

    for i in 0..n_terms {
        let eq_eval = oracle.eq_evals.at(i);
        f_at_0 += eq_eval * col[i * 2] * col[i * 2 + 1];
        f_at_1 += eq_eval * col[(n_terms + i) * 2] * col[(n_terms + i) * 2 + 1];
    }

    (f_at_0, f_at_1)
}

fn claim_logup_generic(
    numerators: &Mle<CpuBackend, SecureField>,
    denominators: &Mle<CpuBackend, SecureField>,
    oracle: &GkrMultivariatePolyOracle<'_, CpuBackend>,
    n_terms: usize,
) -> (SecureField, SecureField) {
    let mut f_at_0 = SecureField::zero();
    let mut f_at_1 = SecureField::zero();

    for i in 0..n_terms {
        let eq_eval = oracle.eq_evals.at(i);

        let a0 = Fraction::new(numerators[i * 2], denominators[i * 2]);
        let b0 = Fraction::new(numerators[i * 2 + 1], denominators[i * 2 + 1]);
        let sum0 = a0 + b0;

        let a1 = Fraction::new(
            numerators[(n_terms + i) * 2],
            denominators[(n_terms + i) * 2],
        );
        let b1 = Fraction::new(
            numerators[(n_terms + i) * 2 + 1],
            denominators[(n_terms + i) * 2 + 1],
        );
        let sum1 = a1 + b1;

        f_at_0 += eq_eval * (sum0.numerator + oracle.lambda * sum0.denominator);
        f_at_1 += eq_eval * (sum1.numerator + oracle.lambda * sum1.denominator);
    }

    (f_at_0, f_at_1)
}

fn claim_logup_multiplicities(
    numerators: &Mle<CpuBackend, BaseField>,
    denominators: &Mle<CpuBackend, SecureField>,
    oracle: &GkrMultivariatePolyOracle<'_, CpuBackend>,
    n_terms: usize,
) -> (SecureField, SecureField) {
    let mut f_at_0 = SecureField::zero();
    let mut f_at_1 = SecureField::zero();

    for i in 0..n_terms {
        let eq_eval = oracle.eq_evals.at(i);

        let a0 = Fraction::new(numerators[i * 2], denominators[i * 2]);
        let b0 = Fraction::new(numerators[i * 2 + 1], denominators[i * 2 + 1]);
        let sum0 = a0 + b0;

        let a1 = Fraction::new(
            numerators[(n_terms + i) * 2],
            denominators[(n_terms + i) * 2],
        );
        let b1 = Fraction::new(
            numerators[(n_terms + i) * 2 + 1],
            denominators[(n_terms + i) * 2 + 1],
        );
        let sum1 = a1 + b1;

        f_at_0 += eq_eval * (sum0.numerator + oracle.lambda * sum0.denominator);
        f_at_1 += eq_eval * (sum1.numerator + oracle.lambda * sum1.denominator);
    }

    (f_at_0, f_at_1)
}

fn claim_logup_singles(
    denominators: &Mle<CpuBackend, SecureField>,
    oracle: &GkrMultivariatePolyOracle<'_, CpuBackend>,
    n_terms: usize,
) -> (SecureField, SecureField) {
    let mut f_at_0 = SecureField::zero();
    let mut f_at_1 = SecureField::zero();

    for i in 0..n_terms {
        let eq_eval = oracle.eq_evals.at(i);

        let sum0 = Reciprocal::new(denominators[i * 2]) + Reciprocal::new(denominators[i * 2 + 1]);
        let sum1 = Reciprocal::new(denominators[(n_terms + i) * 2])
            + Reciprocal::new(denominators[(n_terms + i) * 2 + 1]);

        f_at_0 += eq_eval * (sum0.numerator + oracle.lambda * sum0.denominator);
        f_at_1 += eq_eval * (sum1.numerator + oracle.lambda * sum1.denominator);
    }

    (f_at_0, f_at_1)
}

#[test]
fn metal_mle_ops_native_matches_cpu_for_fix_first_variable() {
    require_metal_runtime();

    let assignment = sf(91);

    for log_len in [1u32, 6u32] {
        let base_cpu = Mle::<CpuBackend, BaseField>::new(base_values(1 << log_len, 5));
        let base_expected = CpuBackend::fix_first_variable(base_cpu.clone(), assignment);
        let base_actual = MetalBackend::fix_first_variable(
            Mle::<MetalBackend, BaseField>::new(MetalBaseFieldVec::from_vec(base_cpu.into_evals())),
            assignment,
        );
        assert_eq!(
            base_actual.into_evals().to_cpu(),
            base_expected.into_evals()
        );

        let secure_cpu = Mle::<CpuBackend, SecureField>::new(secure_values(1 << log_len, 17));
        let secure_expected = CpuBackend::fix_first_variable(secure_cpu.clone(), assignment);
        let secure_actual = MetalBackend::fix_first_variable(
            Mle::<MetalBackend, SecureField>::new(MetalSecureFieldVec::from_vec(
                secure_cpu.into_evals(),
            )),
            assignment,
        );
        assert_eq!(
            secure_actual.into_evals().to_cpu(),
            secure_expected.into_evals()
        );
    }
}

#[test]
fn metal_gkr_ops_cpu_bridge_matches_cpu_for_eq_evals_and_next_layer() {
    require_metal_runtime();

    let y = secure_values(5, 31);
    let v = sf(71);
    let expected_eq = CpuBackend::gen_eq_evals(&y, v);
    let actual_eq = MetalBackend::gen_eq_evals(&y, v);
    assert_eq!(actual_eq.into_evals().to_cpu(), expected_eq.into_evals());

    for cpu_layer in [
        Layer::GrandProduct(Mle::<CpuBackend, SecureField>::new(secure_values(
            1 << 5,
            101,
        ))),
        Layer::LogUpGeneric {
            numerators: Mle::<CpuBackend, SecureField>::new(secure_values(1 << 5, 151)),
            denominators: Mle::<CpuBackend, SecureField>::new(secure_values(1 << 5, 221)),
        },
        Layer::LogUpMultiplicities {
            numerators: Mle::<CpuBackend, BaseField>::new(base_values(1 << 5, 301)),
            denominators: Mle::<CpuBackend, SecureField>::new(secure_values(1 << 5, 351)),
        },
        Layer::LogUpSingles {
            denominators: Mle::<CpuBackend, SecureField>::new(secure_values(1 << 5, 431)),
        },
    ] {
        let expected = CpuBackend::next_layer(&cpu_layer);
        let actual = MetalBackend::next_layer(&into_metal_layer(cpu_layer.clone())).to_cpu();
        assert_layers_eq(actual, expected);
    }
}

#[test]
fn metal_gkr_ops_cpu_bridge_matches_cpu_for_sum_as_poly_in_first_variable() {
    require_metal_runtime();

    let y = secure_values(4, 503);
    let lambda = sf(601);

    for cpu_layer in [
        Layer::GrandProduct(Mle::<CpuBackend, SecureField>::new(secure_values(
            1 << 5,
            701,
        ))),
        Layer::LogUpGeneric {
            numerators: Mle::<CpuBackend, SecureField>::new(secure_values(1 << 5, 801)),
            denominators: Mle::<CpuBackend, SecureField>::new(secure_values(1 << 5, 901)),
        },
    ] {
        let cpu_oracle = into_cpu_oracle(cpu_layer.clone(), &y, lambda);
        let metal_oracle = into_metal_oracle(cpu_layer, &y, lambda);
        let claim = oracle_claim(&cpu_oracle);

        let expected = CpuBackend::sum_as_poly_in_first_variable(&cpu_oracle, claim);
        let actual = MetalBackend::sum_as_poly_in_first_variable(&metal_oracle, claim);

        assert_eq!(&*actual, &*expected);
    }
}
