#![cfg(all(feature = "prover", target_os = "macos", target_arch = "aarch64"))]

use hashbrown::HashMap;
use stwo::core::fields::m31::BaseField;
use stwo::core::fields::qm31::{SecureField, QM31};
use stwo::core::poly::circle::CanonicCoset;
use stwo::core::queries::Queries;
use stwo::core::vcs_lifted::blake2_merkle::Blake2sMerkleHasher;
use stwo::prover::backend::cpu::CpuCirclePoly;
use stwo::prover::backend::CpuBackend;
use stwo::prover::poly::circle::SecureEvaluation;
use stwo::prover::poly::BitReversedOrder;
use stwo::prover::vcs_lifted::prover::MerkleProverLifted;
use stwo_metal::{
    metal_runtime_support, MetalFriFirstLayer, MetalRuntimeSupport, MetalSecureFieldVec,
};

#[test]
fn metal_bounded_first_layer_proof_matches_cpu_reference() {
    assert_eq!(
        metal_runtime_support(),
        MetalRuntimeSupport::Available,
        "Metal runtime must be available for the native Apple Silicon parity test"
    );

    const LOG_SIZE: u32 = 10;
    let circle_domain = CanonicCoset::new(LOG_SIZE).circle_domain();
    let queries = Queries::new(&[1, 2, 7, 8], LOG_SIZE);
    let values = CpuCirclePoly::new(
        (1..=(1 << 6))
            .map(|i| BaseField::from_u32_unchecked(i))
            .collect(),
    )
    .evaluate(circle_domain)
    .values
    .into_iter()
    .map(SecureField::from)
    .collect::<Vec<_>>();

    let src_cpu = SecureEvaluation::<CpuBackend, BitReversedOrder>::new(
        circle_domain,
        values.clone().into_iter().collect(),
    );
    let expected_merkle = MerkleProverLifted::<CpuBackend, Blake2sMerkleHasher>::commit(
        src_cpu.values.columns.iter().collect(),
        src_cpu.domain.log_size(),
    );
    let decommit_positions = grouped_positions(&queries.positions);
    let (_values, expected_decommitment) =
        expected_merkle.decommit(&decommit_positions, src_cpu.values.columns.iter().collect());
    let (expected_witness, expected_value_map) =
        witness_and_values(&values, &queries.positions, &decommit_positions);

    let src_metal = MetalSecureFieldVec::from_vec(values);
    let first_layer = MetalFriFirstLayer::<Blake2sMerkleHasher>::new(circle_domain, src_metal);
    let proof = first_layer.decommit(&queries);

    assert_eq!(first_layer.root(), expected_merkle.root());
    assert_eq!(first_layer.domain(), circle_domain);
    assert_eq!(proof.proof.commitment, expected_merkle.root());
    assert_eq!(proof.proof.fri_witness, expected_witness);
    assert_eq!(
        proof.proof.decommitment.hash_witness,
        expected_decommitment.decommitment.hash_witness
    );
    assert_eq!(proof.aux.all_values, vec![expected_value_map]);
    assert_eq!(
        proof.aux.decommitment.all_node_values,
        expected_decommitment.aux.all_node_values
    );
}

fn grouped_positions(query_positions: &[usize]) -> Vec<usize> {
    let mut decommitment_positions = Vec::new();
    for subset_queries in query_positions.chunk_by(|a, b| a >> 1 == b >> 1) {
        let subset_start = (subset_queries[0] >> 1) << 1;
        decommitment_positions.extend(subset_start..subset_start + 2);
    }
    decommitment_positions
}

fn witness_and_values(
    values: &[SecureField],
    query_positions: &[usize],
    decommitment_positions: &[usize],
) -> (Vec<SecureField>, HashMap<usize, QM31>) {
    let query_set: std::collections::HashSet<_> = query_positions.iter().copied().collect();
    let mut witness = Vec::new();
    let mut value_map = HashMap::new();

    for position in decommitment_positions.iter().copied() {
        let value = values[position];
        value_map.insert(position, value);
        if !query_set.contains(&position) {
            witness.push(value);
        }
    }

    (witness, value_map)
}
