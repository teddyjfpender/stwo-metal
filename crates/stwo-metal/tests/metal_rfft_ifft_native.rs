use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};
use stwo::core::circle::Coset;
use stwo::core::fields::m31::BaseField;
use stwo::core::poly::circle::CircleDomain;
use stwo::prover::backend::{Column, CpuBackend};
use stwo::prover::poly::circle::{CircleCoefficients, PolyOps};
use stwo_metal::{metal_runtime_support, MetalBackend, MetalRuntimeSupport};

fn require_metal_runtime() {
    assert_eq!(
        metal_runtime_support(),
        MetalRuntimeSupport::Available,
        "Metal runtime must be available for native RFFT/IFFT parity tests"
    );
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn metal_rfft_and_ifft_match_cpu_on_shifted_domain() {
    require_metal_runtime();

    let domain = CircleDomain::new(Coset::half_odds(7));
    let mut rng = SmallRng::seed_from_u64(23);
    let coeffs: Vec<BaseField> = (0..domain.size()).map(|_| rng.gen()).collect();

    let cpu_poly = CircleCoefficients::<CpuBackend>::new(coeffs.clone());
    let metal_poly = CircleCoefficients::<MetalBackend>::new(coeffs.into_iter().collect());
    let cpu_twiddles = CpuBackend::precompute_twiddles(domain.half_coset);
    let metal_twiddles = MetalBackend::precompute_twiddles(domain.half_coset);

    let cpu_eval = CpuBackend::evaluate_into(
        &cpu_poly,
        domain,
        &cpu_twiddles,
        vec![BaseField::default(); domain.size()],
    );
    let metal_eval = MetalBackend::evaluate_into(
        &metal_poly,
        domain,
        &metal_twiddles,
        (0..domain.size()).map(|_| BaseField::default()).collect(),
    );

    assert_eq!(metal_eval.values.to_cpu(), cpu_eval.values);

    let cpu_roundtrip = CpuBackend::interpolate(cpu_eval, &cpu_twiddles);
    let metal_roundtrip = MetalBackend::interpolate(metal_eval, &metal_twiddles);

    assert_eq!(metal_roundtrip.coeffs.to_cpu(), cpu_roundtrip.coeffs);
}
