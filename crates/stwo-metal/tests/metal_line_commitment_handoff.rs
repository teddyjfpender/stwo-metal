#![cfg(all(feature = "prover", target_os = "macos", target_arch = "aarch64"))]

use stwo::core::fields::qm31::SecureField;
use stwo::core::poly::circle::CanonicCoset;
use stwo::core::poly::line::LineDomain;
use stwo::core::vcs_lifted::blake2_merkle::Blake2sMerkleHasher;
use stwo::prover::backend::CpuBackend;
use stwo::prover::fri::FriOps;
use stwo::prover::line::LineEvaluation;
use stwo::prover::poly::circle::{PolyOps, SecureEvaluation};
use stwo::prover::poly::BitReversedOrder;
use stwo::prover::vcs_lifted::prover::MerkleProverLifted;
use stwo_metal::{
    commit_line_evaluation_via_cpu_bridge, fold_circle_into_line_first_layer,
    materialize_line_evaluation_via_cpu_bridge, metal_runtime_support, MetalRuntimeSupport,
    MetalSecureFieldVec,
};

#[test]
fn metal_line_materialization_cpu_bridge_matches_cpu_reference() {
    assert_eq!(
        metal_runtime_support(),
        MetalRuntimeSupport::Available,
        "Metal runtime must be available for the native Apple Silicon parity test"
    );

    const LOG_SIZE: u32 = 10;
    let circle_domain = CanonicCoset::new(LOG_SIZE).circle_domain();
    let line_domain = LineDomain::new(circle_domain.half_coset);
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
        circle_domain,
        values.clone().into_iter().collect(),
    );
    let mut expected = LineEvaluation::<CpuBackend>::new_zero(line_domain);
    let twiddles = CpuBackend::precompute_twiddles(circle_domain.half_coset);
    CpuBackend::fold_circle_into_line(&mut expected, &src_cpu, alpha, &twiddles);

    let src_metal = MetalSecureFieldVec::from_vec(values);
    let folded = fold_circle_into_line_first_layer(&src_metal, circle_domain, alpha);
    let actual = materialize_line_evaluation_via_cpu_bridge(line_domain, &folded);

    assert_eq!(actual.values.to_vec(), expected.values.to_vec());
}

#[test]
fn metal_first_inner_layer_commitment_cpu_bridge_matches_cpu_reference_root() {
    assert_eq!(
        metal_runtime_support(),
        MetalRuntimeSupport::Available,
        "Metal runtime must be available for the native Apple Silicon parity test"
    );

    const LOG_SIZE: u32 = 10;
    let circle_domain = CanonicCoset::new(LOG_SIZE).circle_domain();
    let line_domain = LineDomain::new(circle_domain.half_coset);
    let alpha = SecureField::from_u32_unchecked(2, 5, 8, 13);
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

    let src_cpu = SecureEvaluation::<CpuBackend, BitReversedOrder>::new(
        circle_domain,
        values.clone().into_iter().collect(),
    );
    let mut expected_eval = LineEvaluation::<CpuBackend>::new_zero(line_domain);
    let twiddles = CpuBackend::precompute_twiddles(circle_domain.half_coset);
    CpuBackend::fold_circle_into_line(&mut expected_eval, &src_cpu, alpha, &twiddles);
    let expected_merkle = MerkleProverLifted::<CpuBackend, Blake2sMerkleHasher>::commit(
        expected_eval.values.columns.iter().collect(),
        expected_eval.values.len().ilog2(),
    );

    let src_metal = MetalSecureFieldVec::from_vec(values);
    let folded = fold_circle_into_line_first_layer(&src_metal, circle_domain, alpha);
    let bridge = commit_line_evaluation_via_cpu_bridge::<Blake2sMerkleHasher>(line_domain, &folded);

    assert_eq!(
        bridge.evaluation.values.to_vec(),
        expected_eval.values.to_vec()
    );
    assert_eq!(bridge.merkle_tree.root(), expected_merkle.root());
}
