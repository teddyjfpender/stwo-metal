#![cfg(all(feature = "prover", target_os = "macos", target_arch = "aarch64"))]

use stwo::core::fields::m31::BaseField;
use stwo::core::fields::qm31::SecureField;
use stwo::core::fri::FriConfig;
use stwo::core::poly::circle::CanonicCoset;
use stwo::core::queries::Queries;
use stwo::core::vcs_lifted::blake2_merkle::Blake2sMerkleHasher;
use stwo::prover::backend::cpu::CpuCirclePoly;
use stwo_metal::{
    metal_runtime_support, MetalFriCommitmentSlice, MetalFriInnerProofSlice, MetalRuntimeSupport,
    MetalSecureFieldVec,
};

#[test]
fn metal_bounded_inner_fri_proof_slice_packages_commitment_slice_honestly() {
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
    let queries = Queries::new(&[1, 2, 5, 11], LOG_SIZE - 1);
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
    let commitment_slice = MetalFriCommitmentSlice::<Blake2sMerkleHasher>::from_first_circle_fold(
        &src_metal,
        circle_domain,
        config,
        first_alpha,
        &inner_alphas,
    );
    let proof_slice = MetalFriInnerProofSlice::<Blake2sMerkleHasher>::from_first_circle_fold(
        &src_metal,
        circle_domain,
        config,
        first_alpha,
        &inner_alphas,
    );

    let expected_inner_layers = commitment_slice.decommit_on_queries(&queries);
    let proof = proof_slice.decommit_on_queries(&queries);
    let (expected_proofs, expected_aux): (Vec<_>, Vec<_>) = expected_inner_layers
        .into_iter()
        .map(|layer| (layer.proof, layer.aux))
        .unzip();

    assert_eq!(
        proof_slice
            .commitment_slice()
            .last_layer_poly()
            .clone()
            .into_ordered_coefficients(),
        commitment_slice
            .last_layer_poly()
            .clone()
            .into_ordered_coefficients()
    );
    assert_eq!(proof.proof.inner_layers.len(), expected_proofs.len());
    for (actual, expected) in proof.proof.inner_layers.iter().zip(&expected_proofs) {
        assert_eq!(actual.commitment, expected.commitment);
        assert_eq!(actual.fri_witness, expected.fri_witness);
        assert_eq!(
            actual.decommitment.hash_witness,
            expected.decommitment.hash_witness
        );
    }
    assert_eq!(proof.aux.inner_layers.len(), expected_aux.len());
    for (actual, expected) in proof.aux.inner_layers.iter().zip(&expected_aux) {
        assert_eq!(actual.all_values, expected.all_values);
        assert_eq!(
            actual.decommitment.all_node_values,
            expected.decommitment.all_node_values
        );
    }
    assert_eq!(
        proof.proof.last_layer_poly.into_ordered_coefficients(),
        commitment_slice
            .last_layer_poly()
            .clone()
            .into_ordered_coefficients()
    );
}
