#![cfg(all(feature = "prover", target_os = "macos", target_arch = "aarch64"))]

use stwo::core::fields::qm31::SecureField;
use stwo::core::poly::circle::CanonicCoset;
use stwo::core::poly::line::LineDomain;
use stwo::prover::backend::{Column, CpuBackend};
use stwo::prover::fri::FriOps;
use stwo::prover::line::LineEvaluation;
use stwo::prover::poly::circle::{PolyOps, SecureEvaluation};
use stwo::prover::poly::BitReversedOrder;
use stwo_metal::{
    fold_circle_into_line_first_layer, metal_runtime_support, MetalRuntimeSupport,
    MetalSecureFieldVec,
};

#[test]
fn metal_fri_first_layer_fold_matches_cpu_backend() {
    assert_eq!(
        metal_runtime_support(),
        MetalRuntimeSupport::Available,
        "Metal runtime must be available for the native Apple Silicon parity test"
    );

    const LOG_SIZE: u32 = 10;
    let domain = CanonicCoset::new(LOG_SIZE).circle_domain();
    let alpha = SecureField::from_u32_unchecked(1, 3, 5, 7);
    let values = (0..(1 << LOG_SIZE))
        .map(|i| {
            SecureField::from_u32_unchecked(
                4 * i as u32,
                4 * i as u32 + 1,
                4 * i as u32 + 2,
                4 * i as u32 + 3,
            )
        })
        .collect::<Vec<_>>();

    let src_cpu = SecureEvaluation::<CpuBackend, BitReversedOrder>::new(
        domain,
        values.clone().into_iter().collect(),
    );
    let mut expected = LineEvaluation::<CpuBackend>::new_zero(LineDomain::new(domain.half_coset));
    let twiddles = CpuBackend::precompute_twiddles(domain.half_coset);
    CpuBackend::fold_circle_into_line(&mut expected, &src_cpu, alpha, &twiddles);

    let src_metal = MetalSecureFieldVec::from_vec(values);
    let actual = fold_circle_into_line_first_layer(&src_metal, domain, alpha);

    assert_eq!(actual.to_cpu(), expected.values.to_vec());
}
