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
    metal_runtime_support, MetalLineEvaluation, MetalRuntimeSupport, MetalSecureFieldVec,
};

#[test]
fn metal_native_first_inner_layer_commitment_matches_cpu_reference_root() {
    assert_eq!(
        metal_runtime_support(),
        MetalRuntimeSupport::Available,
        "Metal runtime must be available for the native Apple Silicon parity test"
    );

    const LOG_SIZE: u32 = 10;
    let circle_domain = CanonicCoset::new(LOG_SIZE).circle_domain();
    let line_domain = LineDomain::new(circle_domain.half_coset);
    let alpha = SecureField::from_u32_unchecked(3, 5, 7, 11);
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
    let mut expected_eval = LineEvaluation::<CpuBackend>::new_zero(line_domain);
    let twiddles = CpuBackend::precompute_twiddles(circle_domain.half_coset);
    CpuBackend::fold_circle_into_line(&mut expected_eval, &src_cpu, alpha, &twiddles);
    let expected_commitment = MerkleProverLifted::<CpuBackend, Blake2sMerkleHasher>::commit(
        expected_eval.values.columns.iter().collect(),
        expected_eval.values.len().ilog2(),
    );

    let src_metal = MetalSecureFieldVec::from_vec(values);
    let line_eval = MetalLineEvaluation::from_first_circle_fold(&src_metal, circle_domain, alpha);
    let commitment = line_eval.commitment::<Blake2sMerkleHasher>();

    assert_eq!(line_eval.domain().size(), line_domain.size());
    assert_eq!(line_eval.domain().log_size(), line_domain.log_size());
    assert_eq!(line_eval.domain().at(0), line_domain.at(0));
    assert_eq!(commitment.root(), expected_commitment.root());
    assert_eq!(commitment.layer_count(), expected_commitment.layers.len());
}

#[test]
fn metal_native_line_commitment_matches_cpu_reference_after_line_fold() {
    assert_eq!(
        metal_runtime_support(),
        MetalRuntimeSupport::Available,
        "Metal runtime must be available for the native Apple Silicon parity test"
    );

    const LOG_SIZE: u32 = 10;
    let circle_domain = CanonicCoset::new(LOG_SIZE).circle_domain();
    let first_line_domain = LineDomain::new(circle_domain.half_coset);
    let alpha0 = SecureField::from_u32_unchecked(1, 4, 9, 16);
    let alpha1 = SecureField::from_u32_unchecked(2, 3, 5, 7);
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
    let twiddles = CpuBackend::precompute_twiddles(circle_domain.half_coset);
    let mut expected_first = LineEvaluation::<CpuBackend>::new_zero(first_line_domain);
    CpuBackend::fold_circle_into_line(&mut expected_first, &src_cpu, alpha0, &twiddles);
    let expected_second = CpuBackend::fold_line(&expected_first, alpha1, &twiddles, 1);
    let expected_commitment = MerkleProverLifted::<CpuBackend, Blake2sMerkleHasher>::commit(
        expected_second.values.columns.iter().collect(),
        expected_second.values.len().ilog2(),
    );

    let src_metal = MetalSecureFieldVec::from_vec(values);
    let first_eval = MetalLineEvaluation::from_first_circle_fold(&src_metal, circle_domain, alpha0);
    let second_eval = first_eval.fold(alpha1, 1);
    let commitment = second_eval.commitment::<Blake2sMerkleHasher>();

    assert_eq!(second_eval.domain().size(), expected_second.domain().size());
    assert_eq!(
        second_eval.domain().log_size(),
        expected_second.domain().log_size()
    );
    assert_eq!(second_eval.domain().at(0), expected_second.domain().at(0));
    assert_eq!(second_eval.len(), expected_second.len());
    assert_eq!(commitment.root(), expected_commitment.root());
    assert_eq!(commitment.layer_count(), expected_commitment.layers.len());
}
