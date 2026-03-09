#![cfg(all(feature = "prover", target_os = "macos", target_arch = "aarch64"))]

use stwo::core::fields::qm31::SecureField;
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
    metal_runtime_support, MetalFriInnerLayerRow, MetalLineEvaluation, MetalRuntimeSupport,
    MetalSecureFieldVec,
};

#[test]
fn metal_native_first_inner_row_matches_cpu_reference_root_and_decommit() {
    assert_eq!(
        metal_runtime_support(),
        MetalRuntimeSupport::Available,
        "Metal runtime must be available for the native Apple Silicon parity test"
    );

    const LOG_SIZE: u32 = 10;
    let circle_domain = CanonicCoset::new(LOG_SIZE).circle_domain();
    let alpha = SecureField::from_u32_unchecked(9, 7, 5, 3);
    let values = (0..(1 << LOG_SIZE))
        .map(|i| {
            SecureField::from_u32_unchecked(
                5 * i as u32,
                5 * i as u32 + 1,
                5 * i as u32 + 2,
                5 * i as u32 + 3,
            )
        })
        .collect::<Vec<_>>();
    let queries = Queries::new(&[1, 2, 7, 8], LOG_SIZE - 1);
    let fold_step = 2;

    let src_cpu = SecureEvaluation::<CpuBackend, BitReversedOrder>::new(
        circle_domain,
        values.clone().into_iter().collect(),
    );
    let twiddles = CpuBackend::precompute_twiddles(circle_domain.half_coset);
    let mut expected_eval = LineEvaluation::<CpuBackend>::new_zero(
        stwo::core::poly::line::LineDomain::new(circle_domain.half_coset),
    );
    CpuBackend::fold_circle_into_line(&mut expected_eval, &src_cpu, alpha, &twiddles);
    let expected_merkle = MerkleProverLifted::<CpuBackend, Blake2sMerkleHasher>::commit(
        expected_eval.values.columns.iter().collect(),
        expected_eval.values.len().ilog2(),
    );

    let src_metal = MetalSecureFieldVec::from_vec(values);
    let eval = MetalLineEvaluation::from_first_circle_fold(&src_metal, circle_domain, alpha);
    let row = MetalFriInnerLayerRow::<Blake2sMerkleHasher>::new(eval.clone(), fold_step);
    let direct = eval
        .commitment::<Blake2sMerkleHasher>()
        .decommit_fri_layer(&eval, &queries, fold_step);
    let proof = row.decommit(&queries);

    assert_eq!(row.root(), expected_merkle.root());
    assert_eq!(row.fold_step(), fold_step);
    assert_eq!(row.evaluation().len(), expected_eval.len());
    assert_eq!(proof.proof.commitment, direct.proof.proof.commitment);
    assert_eq!(proof.proof.fri_witness, direct.proof.proof.fri_witness);
    assert_eq!(
        proof.proof.decommitment.hash_witness,
        direct.proof.proof.decommitment.hash_witness
    );
    assert_eq!(proof.aux.all_values, direct.proof.aux.all_values);
    assert_eq!(
        proof.aux.decommitment.all_node_values,
        direct.proof.aux.decommitment.all_node_values
    );
}
