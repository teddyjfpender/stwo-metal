use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};
use stwo::core::circle::CirclePoint;
use stwo::core::fields::m31::BaseField;
use stwo::core::fields::qm31::SecureField;
use stwo::core::poly::circle::CanonicCoset;
use stwo::core::poly::utils::{fold, get_folding_alphas};
use stwo::prover::backend::{Column, CpuBackend};
use stwo::prover::poly::circle::{CircleCoefficients, PolyOps};
use stwo_metal::{
    metal_runtime_support, MetalBackend, MetalBaseFieldVec, MetalRuntimeSupport,
    MetalSecureFieldVec,
};
use stwo_metal_sys::metal::U32Buffer;

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

    const LOG_SIZE: u32 = 10;
    const N_POLYS: usize = 8;

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
fn metal_poly_ops_batch_eval_matches_cpu_for_large_multi_stage_native_reduction() {
    require_metal_runtime();

    const LOG_SIZE: u32 = 19;
    const N_POLYS: usize = 4;

    let mut rng = SmallRng::seed_from_u64(29);
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

    let point = CirclePoint::get_point(1 << 16).into_ef::<SecureField>();
    let cpu_refs = cpu_polys.iter().collect::<Vec<_>>();
    let metal_refs = metal_polys.iter().collect::<Vec<_>>();

    let cpu_expected = cpu_refs
        .iter()
        .map(|poly| CpuBackend::eval_at_point(poly, point))
        .collect::<Vec<_>>();
    let metal_batch = MetalBackend::batch_eval_at_point(&metal_refs, point);

    assert_eq!(metal_batch, cpu_expected);
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn metal_poly_ops_single_eval_matches_cpu_for_large_native_path() {
    require_metal_runtime();

    const LOG_SIZE: u32 = 19;

    let mut rng = SmallRng::seed_from_u64(37);
    let coeffs = (0..(1 << LOG_SIZE)).map(|_| rng.gen()).collect::<Vec<_>>();
    let cpu_poly = CircleCoefficients::<CpuBackend>::new(coeffs.clone());
    let metal_poly = CircleCoefficients::<MetalBackend>::new(MetalBaseFieldVec::from_vec(coeffs));
    let point = CirclePoint::get_point(1 << 16).into_ef::<SecureField>();

    assert_eq!(
        MetalBackend::eval_at_point(&metal_poly, point),
        CpuBackend::eval_at_point(&cpu_poly, point)
    );
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn metal_poly_ops_first_pass_partials_match_cpu_chunk_folds() {
    require_metal_runtime();

    const LOG_SIZE: u32 = 19;

    let mut rng = SmallRng::seed_from_u64(31);
    let coeffs: Vec<BaseField> = (0..(1 << LOG_SIZE)).map(|_| rng.gen()).collect();
    let point = CirclePoint::get_point(1 << 16).into_ef::<SecureField>();
    let folding_factors = get_folding_alphas(point, LOG_SIZE as usize);
    let factor_limbs = folding_factors
        .iter()
        .copied()
        .flat_map(|value| value.to_m31_array().map(|limb| limb.0))
        .collect::<Vec<_>>();
    let coeff_limbs = coeffs.iter().map(|value| value.0).collect::<Vec<_>>();

    let coeffs_buffer = U32Buffer::from_slice(&coeff_limbs).unwrap();
    let factors_buffer = U32Buffer::from_slice(&factor_limbs).unwrap();
    let metal_partials = coeffs_buffer
        .batch_eval_first_pass_base_field(&factors_buffer, LOG_SIZE, 1)
        .unwrap()
        .to_vec()
        .unwrap()
        .chunks_exact(4)
        .map(|limbs| SecureField::from_u32_unchecked(limbs[0], limbs[1], limbs[2], limbs[3]))
        .collect::<Vec<_>>();

    let cpu_partials = coeffs
        .chunks_exact(1 << 9)
        .map(|chunk| fold(chunk, &folding_factors[(LOG_SIZE as usize - 9)..]))
        .collect::<Vec<_>>();

    assert_eq!(metal_partials, cpu_partials);
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn metal_poly_ops_batch_eval_multi_size_fused_matches_cpu() {
    require_metal_runtime();

    // Create polynomials of three different sizes to exercise the multi-group
    // fused command buffer path (all GPU-eligible: log_size > 9).
    let mut rng = SmallRng::seed_from_u64(42);
    let log_sizes: Vec<u32> = vec![10, 13, 19];
    let counts: Vec<usize> = vec![3, 2, 1]; // polys per size group

    let mut cpu_polys = Vec::new();
    let mut metal_polys = Vec::new();
    for (&log_size, &count) in log_sizes.iter().zip(counts.iter()) {
        for _ in 0..count {
            let coeffs: Vec<BaseField> = (0..(1 << log_size)).map(|_| rng.gen()).collect();
            cpu_polys.push(CircleCoefficients::<CpuBackend>::new(coeffs.clone()));
            metal_polys.push(CircleCoefficients::<MetalBackend>::new(
                MetalBaseFieldVec::from_vec(coeffs),
            ));
        }
    }

    let point = CirclePoint::get_point(1 << 17).into_ef::<SecureField>();
    let cpu_refs = cpu_polys.iter().collect::<Vec<_>>();
    let metal_refs = metal_polys.iter().collect::<Vec<_>>();

    let cpu_expected: Vec<SecureField> = cpu_refs
        .iter()
        .map(|poly| CpuBackend::eval_at_point(poly, point))
        .collect();
    let metal_batch = MetalBackend::batch_eval_at_point(&metal_refs, point);

    assert_eq!(
        metal_batch, cpu_expected,
        "multi-size fused batch eval must match CPU reference"
    );
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn metal_poly_ops_cpu_bridge_surface_types_stay_stable() {
    require_metal_runtime();

    let _weights: fn(CanonicCoset, CirclePoint<SecureField>) -> MetalSecureFieldVec =
        <MetalBackend as PolyOps>::barycentric_weights;
}
