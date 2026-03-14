use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};
use std::time::Instant;
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

/// Regression test for GPU IFFT kernel bug that produced incorrect coefficients
/// for columns with ≥ 2^17 elements. The bug was in `ifft_line_part_u32` using
/// Metal buffer offset for twiddle addressing instead of an explicit
/// `layer_domain_offset` parameter (which the working RFFT kernel uses).
#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn metal_ifft_large_column_regression() {
    require_metal_runtime();

    // Test at log_size 17 — the smallest size that triggered the bug.
    let log_size: u32 = 17;
    let domain = CircleDomain::new(Coset::half_odds(log_size));
    let mut rng = SmallRng::seed_from_u64(42);
    let coeffs: Vec<BaseField> = (0..domain.size()).map(|_| rng.gen()).collect();

    let cpu_poly = CircleCoefficients::<CpuBackend>::new(coeffs.clone());
    let metal_poly = CircleCoefficients::<MetalBackend>::new(coeffs.into_iter().collect());

    // Use a twiddle tree large enough to cover the domain.
    let cpu_twiddles = CpuBackend::precompute_twiddles(domain.half_coset);
    let metal_twiddles = MetalBackend::precompute_twiddles(domain.half_coset);

    // Evaluate on domain (RFFT — already known to work).
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
    assert_eq!(metal_eval.values.to_cpu(), cpu_eval.values, "RFFT mismatch at log_size {log_size}");

    // Interpolate back (IFFT — this was the buggy path).
    let cpu_roundtrip = CpuBackend::interpolate(cpu_eval, &cpu_twiddles);
    let metal_roundtrip = MetalBackend::interpolate(metal_eval, &metal_twiddles);

    let cpu_coeffs = &cpu_roundtrip.coeffs;
    let metal_coeffs = metal_roundtrip.coeffs.to_cpu();
    assert_eq!(
        metal_coeffs.len(),
        cpu_coeffs.len(),
        "IFFT output length mismatch at log_size {log_size}"
    );
    // Check first mismatch location for a clear error message.
    for (i, (m, c)) in metal_coeffs.iter().zip(cpu_coeffs.iter()).enumerate() {
        assert_eq!(
            m, c,
            "IFFT coefficient mismatch at index {i} for log_size {log_size}"
        );
    }
}

/// Benchmark RFFT at various sizes to measure fused-tail kernel improvement.
/// Run with: cargo test --release -p stwo-metal --features prover --test metal_rfft_ifft_native -- rfft_benchmark --nocapture --ignored
#[test]
#[ignore]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn rfft_benchmark() {
    require_metal_runtime();

    for log_size in [12u32, 14, 16, 18, 20, 22, 24, 25] {
        let domain = CircleDomain::new(Coset::half_odds(log_size));
        let mut rng = SmallRng::seed_from_u64(42);
        let coeffs: Vec<BaseField> = (0..domain.size()).map(|_| rng.gen()).collect();

        let metal_poly = CircleCoefficients::<MetalBackend>::new(coeffs.into_iter().collect());
        let metal_twiddles = MetalBackend::precompute_twiddles(domain.half_coset);

        // Warmup
        let _ = MetalBackend::evaluate_into(
            &metal_poly,
            domain,
            &metal_twiddles,
            (0..domain.size()).map(|_| BaseField::default()).collect(),
        );

        // Timed runs
        let n_runs = 5;
        let start = Instant::now();
        for _ in 0..n_runs {
            let _ = MetalBackend::evaluate_into(
                &metal_poly,
                domain,
                &metal_twiddles,
                (0..domain.size()).map(|_| BaseField::default()).collect(),
            );
        }
        let elapsed = start.elapsed();
        let per_run = elapsed / n_runs;
        eprintln!(
            "RFFT log_size={:2}: {:>8.3}ms (avg of {} runs, {:.1} MB)",
            log_size,
            per_run.as_secs_f64() * 1000.0,
            n_runs,
            (1u64 << (log_size + 1)) as f64 * 4.0 / 1_000_000.0,
        );
    }
}
