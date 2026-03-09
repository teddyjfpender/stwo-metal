#![cfg(all(feature = "prover", target_os = "macos", target_arch = "aarch64"))]

use stwo::core::fields::m31::BaseField;
use stwo::core::fields::qm31::SecureField;
use stwo::core::fri::FriConfig;
use stwo::core::poly::circle::CanonicCoset;
use stwo::prover::backend::cpu::CpuCirclePoly;
use stwo::prover::backend::CpuBackend;
use stwo::prover::fri::FriOps;
use stwo::prover::line::LineEvaluation;
use stwo::prover::poly::circle::{PolyOps, SecureEvaluation};
use stwo::prover::poly::BitReversedOrder;
use stwo_metal::{
    metal_runtime_support, MetalFriCommitmentSlice, MetalRuntimeSupport, MetalSecureFieldVec,
};

#[test]
fn metal_bounded_fri_commitment_slice_matches_cpu_reference() {
    assert_eq!(
        metal_runtime_support(),
        MetalRuntimeSupport::Available,
        "Metal runtime must be available for the native Apple Silicon parity test"
    );

    const LOG_SIZE: u32 = 10;
    let circle_domain = CanonicCoset::new(LOG_SIZE).circle_domain();
    let config = FriConfig::new(3, 2, 3, 2);
    let first_alpha = SecureField::from_u32_unchecked(1, 2, 3, 4);
    let inner_alphas = [
        SecureField::from_u32_unchecked(5, 6, 7, 8),
        SecureField::from_u32_unchecked(9, 10, 11, 12),
    ];
    let low_degree_poly = CpuCirclePoly::new(
        (1..=(1 << 6))
            .map(|i| BaseField::from_u32_unchecked(i))
            .collect(),
    );
    let values = low_degree_poly
        .evaluate(circle_domain)
        .values
        .into_iter()
        .map(SecureField::from)
        .collect::<Vec<_>>();

    let src_cpu = SecureEvaluation::<CpuBackend, BitReversedOrder>::new(
        circle_domain,
        values.clone().into_iter().collect(),
    );
    let twiddles = CpuBackend::precompute_twiddles(circle_domain.half_coset);
    let mut cpu_eval = LineEvaluation::<CpuBackend>::new_zero(
        stwo::core::poly::line::LineDomain::new(circle_domain.half_coset),
    );
    CpuBackend::fold_circle_into_line(&mut cpu_eval, &src_cpu, first_alpha, &twiddles);
    cpu_eval = CpuBackend::fold_line(&cpu_eval, inner_alphas[0], &twiddles, config.line_fold_step);
    cpu_eval = CpuBackend::fold_line(
        &cpu_eval,
        inner_alphas[1],
        &twiddles,
        cpu_eval.domain().log_size() - config.last_layer_domain_size().ilog2(),
    );
    let mut expected_coeffs = cpu_eval.clone().interpolate().into_ordered_coefficients();
    let zero_tail = expected_coeffs.split_off(1 << config.log_last_layer_degree_bound);
    assert!(
        zero_tail
            .iter()
            .all(|value| *value == SecureField::from_u32_unchecked(0, 0, 0, 0)),
        "CPU reference must satisfy the configured last-layer degree bound"
    );

    let src_metal = MetalSecureFieldVec::from_vec(values);
    let slice = MetalFriCommitmentSlice::<stwo::core::vcs_lifted::blake2_merkle::Blake2sMerkleHasher>::from_first_circle_fold(
        &src_metal,
        circle_domain,
        config,
        first_alpha,
        &inner_alphas,
    );

    assert_eq!(
        slice.last_layer_poly().clone().into_ordered_coefficients(),
        expected_coeffs
    );
}
