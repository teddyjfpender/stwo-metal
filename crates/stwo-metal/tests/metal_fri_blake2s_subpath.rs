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
use stwo_metal::{
    metal_runtime_support, MetalFriBlake2sSubpath, MetalRuntimeSupport, MetalSecureFieldVec,
};

#[test]
fn metal_declared_blake2s_fri_subpath_matches_cpu_reference() {
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
    let metal_result = MetalFriBlake2sSubpath::new(config).prove(&src_metal, circle_domain);

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
        metal_result.fri_proof.proof.inner_layers.len(),
        cpu_result.fri_proof.proof.inner_layers.len()
    );
}
