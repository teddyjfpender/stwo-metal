use stwo::core::circle::Coset;
use stwo::core::poly::circle::CanonicCoset;
use stwo::prover::backend::CpuBackend;
use stwo::prover::poly::circle::PolyOps;
use stwo_metal::{metal_runtime_support, MetalBackend, MetalRuntimeSupport};

fn require_metal_runtime() {
    assert_eq!(
        metal_runtime_support(),
        MetalRuntimeSupport::Available,
        "Metal runtime must be available for native twiddle parity tests"
    );
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn metal_twiddle_precompute_matches_cpu_on_canonic_cosets() {
    require_metal_runtime();

    for log_size in [1u32, 2, 3, 6, 10] {
        let coset = CanonicCoset::new(log_size).circle_domain().half_coset;
        let cpu_twiddles = CpuBackend::precompute_twiddles(coset);
        let metal_twiddles = MetalBackend::precompute_twiddles(coset);

        assert_eq!(metal_twiddles.root_coset, cpu_twiddles.root_coset);
        assert_eq!(metal_twiddles.twiddles, cpu_twiddles.twiddles);
        assert_eq!(metal_twiddles.itwiddles, cpu_twiddles.itwiddles);
    }
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn metal_twiddle_precompute_matches_cpu_on_shifted_cosets() {
    require_metal_runtime();

    // Twiddle inversion assumes non-zero x-coordinates at every layer. Keep this
    // parity row on cosets the vendored CPU path already treats as valid proving
    // inputs rather than arbitrary shifts that can hit x = 0.
    for coset in [
        Coset::half_odds(4),
        Coset::half_odds(7),
        Coset::half_odds(9),
    ] {
        let cpu_twiddles = CpuBackend::precompute_twiddles(coset);
        let metal_twiddles = MetalBackend::precompute_twiddles(coset);

        assert_eq!(metal_twiddles.root_coset, cpu_twiddles.root_coset);
        assert_eq!(metal_twiddles.twiddles, cpu_twiddles.twiddles);
        assert_eq!(metal_twiddles.itwiddles, cpu_twiddles.itwiddles);
    }
}
