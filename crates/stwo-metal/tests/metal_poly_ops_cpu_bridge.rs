use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};
use stwo::core::circle::CirclePoint;
use stwo::core::fields::m31::BaseField;
use stwo::core::fields::qm31::SecureField;
use stwo::core::poly::circle::CanonicCoset;
use stwo::prover::backend::{Column, CpuBackend};
use stwo::prover::poly::circle::{CircleCoefficients, PolyOps};
use stwo_metal::{
    metal_runtime_support, MetalBackend, MetalBaseFieldVec, MetalRuntimeSupport,
    MetalSecureFieldVec,
};

fn require_metal_runtime() {
    assert_eq!(
        metal_runtime_support(),
        MetalRuntimeSupport::Available,
        "Metal runtime must be available for the native Apple Silicon poly-ops bridge tests"
    );
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn metal_poly_ops_cpu_bridge_matches_cpu_for_interpolate_evaluate_fold_and_split() {
    require_metal_runtime();

    const LOG_SIZE: u32 = 7;
    const EXTENDED_LOG_SIZE: u32 = LOG_SIZE + 1;

    let mut rng = SmallRng::seed_from_u64(11);
    let coeffs: Vec<BaseField> = (0..(1 << LOG_SIZE)).map(|_| rng.gen()).collect();

    let cpu_poly = CircleCoefficients::<CpuBackend>::new(coeffs.clone());
    let metal_poly = CircleCoefficients::<MetalBackend>::new(MetalBaseFieldVec::from_vec(coeffs));

    let domain = CanonicCoset::new(EXTENDED_LOG_SIZE).circle_domain();
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
        MetalBaseFieldVec::new_zeroes(domain.size()),
    );
    assert_eq!(metal_eval.values.to_cpu(), cpu_eval.values);

    let point = CirclePoint::get_point(21903);
    let cpu_fold = CpuBackend::eval_at_point_by_folding(&cpu_eval, point, &cpu_twiddles);
    let metal_fold = MetalBackend::eval_at_point_by_folding(&metal_eval, point, &metal_twiddles);
    assert_eq!(metal_fold, cpu_fold);

    let cpu_interpolated = CpuBackend::interpolate(cpu_eval.clone(), &cpu_twiddles);
    let metal_interpolated = MetalBackend::interpolate(metal_eval.clone(), &metal_twiddles);
    assert_eq!(metal_interpolated.coeffs.to_cpu(), cpu_interpolated.coeffs);

    let (cpu_left, cpu_right) = cpu_poly.clone().split_at_mid();
    let (metal_left, metal_right) = metal_poly.clone().split_at_mid();
    assert_eq!(metal_left.coeffs.to_cpu(), cpu_left.coeffs);
    assert_eq!(metal_right.coeffs.to_cpu(), cpu_right.coeffs);
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn metal_poly_ops_cpu_bridge_matches_cpu_for_barycentric_weights_and_eval() {
    require_metal_runtime();

    const LOG_SIZE: u32 = 6;

    let mut rng = SmallRng::seed_from_u64(7);
    let coeffs: Vec<BaseField> = (0..(1 << LOG_SIZE)).map(|_| rng.gen()).collect();
    let cpu_poly = CircleCoefficients::<CpuBackend>::new(coeffs.clone());
    let metal_poly = CircleCoefficients::<MetalBackend>::new(MetalBaseFieldVec::from_vec(coeffs));

    let coset = CanonicCoset::new(LOG_SIZE);
    let domain = coset.circle_domain();
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
        MetalBaseFieldVec::new_zeroes(domain.size()),
    );

    let point = CirclePoint::get_point(1 << 18).into_ef::<SecureField>();
    let cpu_weights = CpuBackend::barycentric_weights(coset, point);
    let metal_weights = MetalBackend::barycentric_weights(coset, point);

    assert_eq!(metal_weights.to_vec(), cpu_weights);

    let cpu_value = CpuBackend::barycentric_eval_at_point(&cpu_eval, &cpu_weights);
    let metal_value = MetalBackend::barycentric_eval_at_point(&metal_eval, &metal_weights);
    assert_eq!(metal_value, cpu_value);
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn metal_poly_ops_batch_eval_matches_cpu_and_single_eval() {
    require_metal_runtime();

    const LOG_SIZE: u32 = 7;
    const N_POLYS: usize = 16;

    let mut rng = SmallRng::seed_from_u64(19);
    let cpu_polys = (0..N_POLYS)
        .map(|_| {
            CircleCoefficients::<CpuBackend>::new((0..(1 << LOG_SIZE)).map(|_| rng.gen()).collect())
        })
        .collect::<Vec<_>>();
    let metal_polys = cpu_polys
        .iter()
        .map(|poly| {
            CircleCoefficients::<MetalBackend>::new(MetalBaseFieldVec::from_vec(
                poly.coeffs.clone(),
            ))
        })
        .collect::<Vec<_>>();

    let point = CirclePoint::get_point(1 << 17).into_ef::<SecureField>();
    let cpu_refs = cpu_polys.iter().collect::<Vec<_>>();
    let metal_refs = metal_polys.iter().collect::<Vec<_>>();

    let cpu_expected = cpu_refs
        .iter()
        .map(|poly| CpuBackend::eval_at_point(poly, point))
        .collect::<Vec<_>>();
    let metal_single = metal_refs
        .iter()
        .map(|poly| MetalBackend::eval_at_point(poly, point))
        .collect::<Vec<_>>();
    let metal_batch = MetalBackend::batch_eval_at_point(&metal_refs, point);

    assert_eq!(metal_batch, cpu_expected);
    assert_eq!(metal_batch, metal_single);
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn metal_poly_ops_cpu_bridge_surface_types_stay_stable() {
    require_metal_runtime();

    let _weights: fn(CanonicCoset, CirclePoint<SecureField>) -> MetalSecureFieldVec =
        <MetalBackend as PolyOps>::barycentric_weights;
}
