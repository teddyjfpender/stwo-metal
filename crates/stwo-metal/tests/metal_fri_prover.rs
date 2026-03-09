#![cfg(all(feature = "prover", target_os = "macos", target_arch = "aarch64"))]

use stwo::core::channel::Blake2sChannel;
use stwo::core::fields::m31::BaseField;
use stwo::core::fields::qm31::SecureField;
use stwo::core::fri::FriConfig;
use stwo::core::poly::circle::CanonicCoset;
use stwo::core::vcs_lifted::blake2_merkle::Blake2sMerkleChannel;
use stwo::prover::backend::cpu::CpuCirclePoly;
use stwo::prover::backend::CpuBackend;
use stwo::prover::fri::FriProver;
use stwo::prover::poly::circle::{PolyOps, SecureEvaluation};
use stwo::prover::poly::BitReversedOrder;
use stwo_metal::{metal_runtime_support, MetalFriProver, MetalRuntimeSupport, MetalSecureFieldVec};

#[test]
fn metal_bounded_fri_prover_matches_cpu_transcript_and_proof() {
    assert_eq!(
        metal_runtime_support(),
        MetalRuntimeSupport::Available,
        "Metal runtime must be available for the native Apple Silicon parity test"
    );

    const LOG_SIZE: u32 = 10;
    let circle_domain = CanonicCoset::new(LOG_SIZE).circle_domain();
    let config = FriConfig::new(3, 2, 3, 2);
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
    let twiddles = CpuBackend::precompute_twiddles(circle_domain.half_coset);
    let src_metal = MetalSecureFieldVec::from_vec(values);

    let mut cpu_channel = Blake2sChannel::default();
    let cpu_result = FriProver::<CpuBackend, Blake2sMerkleChannel>::commit(
        &mut cpu_channel,
        config,
        &src_cpu,
        &twiddles,
    )
    .decommit(&mut cpu_channel);
    let mut metal_channel = Blake2sChannel::default();
    let metal_result = MetalFriProver::<Blake2sMerkleChannel>::commit(
        &mut metal_channel,
        config,
        &src_metal,
        circle_domain,
    )
    .decommit(&mut metal_channel);

    assert_eq!(
        metal_result.unsorted_query_locations,
        cpu_result.unsorted_query_locations
    );
    assert_eq!(metal_result.query_positions, cpu_result.query_positions);
    assert_eq!(
        metal_result
            .fri_proof
            .proof
            .last_layer_poly
            .clone()
            .into_ordered_coefficients(),
        cpu_result
            .fri_proof
            .proof
            .last_layer_poly
            .clone()
            .into_ordered_coefficients()
    );
    assert_eq!(
        metal_result.fri_proof.proof.first_layer.commitment,
        cpu_result.fri_proof.proof.first_layer.commitment
    );
    assert_eq!(
        metal_result.fri_proof.proof.first_layer.fri_witness,
        cpu_result.fri_proof.proof.first_layer.fri_witness
    );
    assert_eq!(
        metal_result
            .fri_proof
            .proof
            .first_layer
            .decommitment
            .hash_witness,
        cpu_result
            .fri_proof
            .proof
            .first_layer
            .decommitment
            .hash_witness
    );
    assert_eq!(
        metal_result.fri_proof.aux.first_layer.all_values,
        cpu_result.fri_proof.aux.first_layer.all_values
    );
    assert_eq!(
        metal_result
            .fri_proof
            .aux
            .first_layer
            .decommitment
            .all_node_values,
        cpu_result
            .fri_proof
            .aux
            .first_layer
            .decommitment
            .all_node_values
    );
    assert_eq!(
        metal_result.fri_proof.proof.inner_layers.len(),
        cpu_result.fri_proof.proof.inner_layers.len()
    );
    for (actual, expected) in metal_result
        .fri_proof
        .proof
        .inner_layers
        .iter()
        .zip(&cpu_result.fri_proof.proof.inner_layers)
    {
        assert_eq!(actual.commitment, expected.commitment);
        assert_eq!(actual.fri_witness, expected.fri_witness);
        assert_eq!(
            actual.decommitment.hash_witness,
            expected.decommitment.hash_witness
        );
    }
    for (actual, expected) in metal_result
        .fri_proof
        .aux
        .inner_layers
        .iter()
        .zip(&cpu_result.fri_proof.aux.inner_layers)
    {
        assert_eq!(actual.all_values, expected.all_values);
        assert_eq!(
            actual.decommitment.all_node_values,
            expected.decommitment.all_node_values
        );
    }
}
