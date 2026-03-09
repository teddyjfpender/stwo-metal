#![cfg(all(feature = "prover", target_os = "macos", target_arch = "aarch64"))]

use stwo::core::fields::m31::BaseField;
use stwo::core::fields::qm31::SecureField;
use stwo::core::fri::FriConfig;
use stwo::core::poly::circle::CanonicCoset;
use stwo::core::queries::Queries;
use stwo::core::vcs_lifted::blake2_merkle::Blake2sMerkleHasher;
use stwo::prover::backend::cpu::CpuCirclePoly;
use stwo_metal::{
    metal_runtime_support, MetalFriFirstLayer, MetalFriInnerProofSlice, MetalFriProofSlice,
    MetalRuntimeSupport, MetalSecureFieldVec,
};

#[test]
fn metal_bounded_fri_proof_slice_packages_first_and_inner_layers_honestly() {
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

    let src_metal = MetalSecureFieldVec::from_vec(values);
    let first_layer = MetalFriFirstLayer::<Blake2sMerkleHasher>::new(
        circle_domain,
        MetalSecureFieldVec::from_vec(src_metal.to_vec()),
    );
    let inner_proof = MetalFriInnerProofSlice::<Blake2sMerkleHasher>::from_first_circle_fold(
        &src_metal,
        circle_domain,
        config,
        first_alpha,
        &inner_alphas,
    );
    let proof_slice = MetalFriProofSlice::<Blake2sMerkleHasher>::from_first_circle_fold(
        &src_metal,
        circle_domain,
        config,
        first_alpha,
        &inner_alphas,
    );

    let expected_first_layer = first_layer.decommit(&queries);
    let expected_inner = inner_proof.decommit_on_queries(&queries.fold(1));
    let proof = proof_slice.decommit_on_queries(&queries);

    assert_eq!(
        proof_slice.first_layer().root(),
        expected_first_layer.proof.commitment
    );
    assert_eq!(
        proof.proof.first_layer.commitment,
        expected_first_layer.proof.commitment
    );
    assert_eq!(
        proof.proof.first_layer.fri_witness,
        expected_first_layer.proof.fri_witness
    );
    assert_eq!(
        proof.proof.first_layer.decommitment.hash_witness,
        expected_first_layer.proof.decommitment.hash_witness
    );
    assert_eq!(
        proof.aux.first_layer.all_values,
        expected_first_layer.aux.all_values
    );
    assert_eq!(
        proof.aux.first_layer.decommitment.all_node_values,
        expected_first_layer.aux.decommitment.all_node_values
    );
    assert_eq!(
        proof.proof.last_layer_poly.into_ordered_coefficients(),
        expected_inner
            .proof
            .last_layer_poly
            .into_ordered_coefficients()
    );
    assert_eq!(
        proof.proof.inner_layers.len(),
        expected_inner.proof.inner_layers.len()
    );
    for (actual, expected) in proof
        .proof
        .inner_layers
        .iter()
        .zip(&expected_inner.proof.inner_layers)
    {
        assert_eq!(actual.commitment, expected.commitment);
        assert_eq!(actual.fri_witness, expected.fri_witness);
        assert_eq!(
            actual.decommitment.hash_witness,
            expected.decommitment.hash_witness
        );
    }
    for (actual, expected) in proof
        .aux
        .inner_layers
        .iter()
        .zip(&expected_inner.aux.inner_layers)
    {
        assert_eq!(actual.all_values, expected.all_values);
        assert_eq!(
            actual.decommitment.all_node_values,
            expected.decommitment.all_node_values
        );
    }
}
