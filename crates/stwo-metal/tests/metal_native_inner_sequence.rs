#![cfg(all(feature = "prover", target_os = "macos", target_arch = "aarch64"))]

use stwo::core::fields::qm31::SecureField;
use stwo::core::fri::FriConfig;
use stwo::core::poly::circle::CanonicCoset;
use stwo::core::queries::Queries;
use stwo::core::vcs_lifted::blake2_merkle::Blake2sMerkleHasher;
use stwo::prover::backend::CpuBackend;
use stwo::prover::fri::FriOps;
use stwo::prover::line::LineEvaluation;
use stwo::prover::poly::circle::{PolyOps, SecureEvaluation};
use stwo::prover::poly::BitReversedOrder;
use stwo::prover::vcs_lifted::prover::MerkleProverLifted;
use stwo_metal::{
    metal_runtime_support, MetalFriInnerLayerSequence, MetalRuntimeSupport, MetalSecureFieldVec,
};

#[test]
fn metal_native_inner_sequence_matches_cpu_reference() {
    assert_eq!(
        metal_runtime_support(),
        MetalRuntimeSupport::Available,
        "Metal runtime must be available for the native Apple Silicon parity test"
    );

    const LOG_SIZE: u32 = 10;
    let circle_domain = CanonicCoset::new(LOG_SIZE).circle_domain();
    let config = FriConfig::new(3, 2, 3, 2);
    let first_alpha = SecureField::from_u32_unchecked(1, 3, 5, 7);
    let inner_alphas = [
        SecureField::from_u32_unchecked(2, 4, 6, 8),
        SecureField::from_u32_unchecked(9, 7, 5, 3),
    ];
    let values = (0..(1 << LOG_SIZE))
        .map(|i| {
            SecureField::from_u32_unchecked(
                7 * i as u32,
                7 * i as u32 + 1,
                7 * i as u32 + 2,
                7 * i as u32 + 3,
            )
        })
        .collect::<Vec<_>>();
    let queries = Queries::new(&[1, 2, 5, 11], LOG_SIZE - 1);

    let src_cpu = SecureEvaluation::<CpuBackend, BitReversedOrder>::new(
        circle_domain,
        values.clone().into_iter().collect(),
    );
    let twiddles = CpuBackend::precompute_twiddles(circle_domain.half_coset);
    let mut cpu_eval = LineEvaluation::<CpuBackend>::new_zero(
        stwo::core::poly::line::LineDomain::new(circle_domain.half_coset),
    );
    CpuBackend::fold_circle_into_line(&mut cpu_eval, &src_cpu, first_alpha, &twiddles);

    let mut expected_roots = Vec::new();
    let mut expected_decommits = Vec::new();
    let mut current_queries = queries.clone();
    let last_layer_log_domain_size = config.last_layer_domain_size().ilog2();
    let mut alpha_index = 0usize;

    while cpu_eval.domain().log_size() > last_layer_log_domain_size + config.line_fold_step {
        let merkle = MerkleProverLifted::<CpuBackend, Blake2sMerkleHasher>::commit(
            cpu_eval.values.columns.iter().collect(),
            cpu_eval.values.len().ilog2(),
        );
        expected_roots.push(merkle.root());
        let decommit_positions =
            grouped_positions(&current_queries.positions, config.line_fold_step);
        let (_values, proof) = merkle.decommit(
            &decommit_positions,
            cpu_eval.values.columns.iter().collect(),
        );
        expected_decommits.push(proof.decommitment.hash_witness);
        cpu_eval = CpuBackend::fold_line(
            &cpu_eval,
            inner_alphas[alpha_index],
            &twiddles,
            config.line_fold_step,
        );
        alpha_index += 1;
        current_queries = current_queries.fold(config.line_fold_step);
    }

    let last_fold_step = cpu_eval.domain().log_size() - last_layer_log_domain_size;
    let merkle = MerkleProverLifted::<CpuBackend, Blake2sMerkleHasher>::commit(
        cpu_eval.values.columns.iter().collect(),
        cpu_eval.values.len().ilog2(),
    );
    expected_roots.push(merkle.root());
    let decommit_positions = grouped_positions(&current_queries.positions, last_fold_step);
    let (_values, proof) = merkle.decommit(
        &decommit_positions,
        cpu_eval.values.columns.iter().collect(),
    );
    expected_decommits.push(proof.decommitment.hash_witness);
    cpu_eval = CpuBackend::fold_line(
        &cpu_eval,
        inner_alphas[alpha_index],
        &twiddles,
        last_fold_step,
    );

    let src_metal = MetalSecureFieldVec::from_vec(values);
    let sequence = MetalFriInnerLayerSequence::<Blake2sMerkleHasher>::from_first_circle_fold(
        &src_metal,
        circle_domain,
        config,
        first_alpha,
        &inner_alphas,
    );
    let proofs = sequence.decommit_on_queries(&queries);

    assert_eq!(sequence.rows().len(), expected_roots.len());
    for (row, expected_root) in sequence.rows().iter().zip(&expected_roots) {
        assert_eq!(row.root(), *expected_root);
    }
    for (proof, expected_hash_witness) in proofs.iter().zip(&expected_decommits) {
        assert_eq!(
            proof.proof.decommitment.hash_witness,
            *expected_hash_witness
        );
    }
    assert_eq!(sequence.last_evaluation().len(), cpu_eval.len());
    assert_eq!(
        sequence.last_evaluation().values().to_vec(),
        cpu_eval.values.to_vec()
    );
}

fn grouped_positions(query_positions: &[usize], fold_step: u32) -> Vec<usize> {
    let mut decommitment_positions = Vec::new();
    for subset_queries in query_positions.chunk_by(|a, b| a >> fold_step == b >> fold_step) {
        let subset_start = (subset_queries[0] >> fold_step) << fold_step;
        decommitment_positions.extend(subset_start..subset_start + (1 << fold_step));
    }
    decommitment_positions
}
