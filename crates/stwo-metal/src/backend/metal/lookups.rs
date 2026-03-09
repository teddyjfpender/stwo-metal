use stwo::core::fields::m31::BaseField;
use stwo::core::fields::qm31::SecureField;
use stwo::prover::backend::{Column, CpuBackend};
use stwo::prover::lookups::gkr_prover::{GkrMultivariatePolyOracle, GkrOps, Layer};
use stwo::prover::lookups::mle::{Mle, MleOps};
use stwo::prover::lookups::utils::UnivariatePoly;

use super::MetalBackend;
use crate::stwo_metal::{BaseFieldVec, SecureFieldVec};

fn metal_base_mle_from_cpu(mle: Mle<CpuBackend, BaseField>) -> Mle<MetalBackend, BaseField> {
    Mle::new(BaseFieldVec::from_vec(mle.into_evals()))
}

fn metal_secure_mle_from_cpu(mle: Mle<CpuBackend, SecureField>) -> Mle<MetalBackend, SecureField> {
    Mle::new(SecureFieldVec::from_vec(mle.into_evals()))
}

fn cpu_base_mle_from_metal(mle: Mle<MetalBackend, BaseField>) -> Mle<CpuBackend, BaseField> {
    Mle::new(mle.into_evals().to_cpu())
}

fn cpu_secure_mle_from_metal(mle: Mle<MetalBackend, SecureField>) -> Mle<CpuBackend, SecureField> {
    Mle::new(mle.into_evals().to_cpu())
}

fn metal_layer_from_cpu(layer: Layer<CpuBackend>) -> Layer<MetalBackend> {
    match layer {
        Layer::GrandProduct(mle) => Layer::GrandProduct(metal_secure_mle_from_cpu(mle)),
        Layer::LogUpGeneric {
            numerators,
            denominators,
        } => Layer::LogUpGeneric {
            numerators: metal_secure_mle_from_cpu(numerators),
            denominators: metal_secure_mle_from_cpu(denominators),
        },
        Layer::LogUpMultiplicities {
            numerators,
            denominators,
        } => Layer::LogUpMultiplicities {
            numerators: metal_base_mle_from_cpu(numerators),
            denominators: metal_secure_mle_from_cpu(denominators),
        },
        Layer::LogUpSingles { denominators } => Layer::LogUpSingles {
            denominators: metal_secure_mle_from_cpu(denominators),
        },
    }
}

impl MleOps<BaseField> for MetalBackend {
    fn fix_first_variable(
        mle: Mle<Self, BaseField>,
        assignment: SecureField,
    ) -> Mle<Self, SecureField>
    where
        Self: MleOps<SecureField>,
    {
        metal_secure_mle_from_cpu(CpuBackend::fix_first_variable(
            cpu_base_mle_from_metal(mle),
            assignment,
        ))
    }
}

impl MleOps<SecureField> for MetalBackend {
    fn fix_first_variable(
        mle: Mle<Self, SecureField>,
        assignment: SecureField,
    ) -> Mle<Self, SecureField>
    where
        Self: MleOps<SecureField>,
    {
        metal_secure_mle_from_cpu(CpuBackend::fix_first_variable(
            cpu_secure_mle_from_metal(mle),
            assignment,
        ))
    }
}

impl GkrOps for MetalBackend {
    fn gen_eq_evals(y: &[SecureField], v: SecureField) -> Mle<Self, SecureField> {
        metal_secure_mle_from_cpu(CpuBackend::gen_eq_evals(y, v))
    }

    fn next_layer(layer: &Layer<Self>) -> Layer<Self> {
        metal_layer_from_cpu(CpuBackend::next_layer(&layer.to_cpu()))
    }

    fn sum_as_poly_in_first_variable(
        h: &GkrMultivariatePolyOracle<'_, Self>,
        claim: SecureField,
    ) -> UnivariatePoly<SecureField> {
        CpuBackend::sum_as_poly_in_first_variable(&h.to_cpu(), claim)
    }
}
