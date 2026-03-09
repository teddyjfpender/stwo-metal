#![cfg(all(feature = "prover", target_os = "macos", target_arch = "aarch64"))]

use stwo::core::fields::qm31::SecureField;
use stwo::core::poly::line::LineDomain;
use stwo::prover::backend::{Column, CpuBackend};
use stwo::prover::fri::FriOps;
use stwo::prover::line::LineEvaluation;
use stwo::prover::poly::circle::PolyOps;
use stwo_metal::{fold_line, metal_runtime_support, MetalRuntimeSupport, MetalSecureFieldVec};

#[test]
fn metal_line_fold_matches_cpu_backend() {
    assert_eq!(
        metal_runtime_support(),
        MetalRuntimeSupport::Available,
        "Metal runtime must be available for the native Apple Silicon parity test"
    );

    const LOG_SIZE: u32 = 9;
    let domain = LineDomain::new(stwo::core::circle::Coset::half_odds(LOG_SIZE));
    let alpha = SecureField::from_u32_unchecked(2, 4, 6, 8);
    let values = (0..(1 << LOG_SIZE))
        .map(|i| {
            SecureField::from_u32_unchecked(
                8 * i as u32,
                8 * i as u32 + 1,
                8 * i as u32 + 2,
                8 * i as u32 + 3,
            )
        })
        .collect::<Vec<_>>();

    let src_cpu = LineEvaluation::<CpuBackend>::new(domain, values.clone().into_iter().collect());
    let twiddles = CpuBackend::precompute_twiddles(domain.coset());
    let expected = CpuBackend::fold_line(&src_cpu, alpha, &twiddles, 2);

    let src_metal = MetalSecureFieldVec::from_vec(values);
    let actual = fold_line(&src_metal, domain, alpha, 2);

    assert_eq!(actual.to_cpu(), expected.values.to_vec());
}
