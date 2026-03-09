#![cfg(feature = "prover")]

use stwo::core::channel::Blake2sChannel;
use stwo::core::fields::m31::BaseField;
use stwo::core::fields::qm31::SecureField;
use stwo::core::fri::FriConfig;
use stwo::core::poly::circle::CanonicCoset;
use stwo::core::vcs_lifted::blake2_merkle::Blake2sMerkleChannel;
use stwo::prover::backend::cpu::CpuCirclePoly;
use stwo::prover::backend::CpuBackend;
use stwo::prover::fri::FriProver;
use stwo::prover::poly::circle::PolyOps;
use stwo::prover::poly::circle::SecureEvaluation;
use stwo::prover::poly::BitReversedOrder;
use stwo_metal::{
    declare_exemplar_hybrid_fri_workload, declare_exemplar_metal_workload_boundary,
    metal_runtime_support, MetalExecutionIntent, MetalExecutionPlan, MetalRuntimeSupport,
};

fn exemplar_cpu_evaluation(log_size: u32) -> SecureEvaluation<CpuBackend, BitReversedOrder> {
    let domain = CanonicCoset::new(log_size).circle_domain();
    let values = CpuCirclePoly::new(
        (1..=(1 << 6))
            .map(|i| BaseField::from_u32_unchecked(i))
            .collect(),
    )
    .evaluate(domain)
    .values
    .into_iter()
    .map(SecureField::from)
    .collect::<Vec<_>>();

    SecureEvaluation::<CpuBackend, BitReversedOrder>::new(domain, values.into_iter().collect())
}

#[test]
fn cpu_owned_fri_ready_handoff_preserves_domain_and_plan() {
    let evaluation = exemplar_cpu_evaluation(8);
    let boundary = declare_exemplar_metal_workload_boundary(
        MetalExecutionIntent::PreferMetal,
        "fibonacci_example",
    )
    .unwrap();

    let input = boundary
        .ingest_cpu_fri_ready_evaluation(&evaluation)
        .unwrap();

    assert_eq!(boundary.plan(), MetalExecutionPlan::MetalFriHybrid);
    assert_eq!(input.workload_name(), "fibonacci_example");
    assert_eq!(input.domain(), evaluation.domain);
    assert_eq!(input.column().to_vec(), evaluation.values.to_vec());
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn cpu_owned_fri_ready_handoff_matches_cpu_reference() {
    assert_eq!(
        metal_runtime_support(),
        MetalRuntimeSupport::Available,
        "Metal runtime must be available for the native Apple Silicon parity test"
    );

    let config = FriConfig::new(3, 2, 3, 2);
    let evaluation = exemplar_cpu_evaluation(10);
    let twiddles = CpuBackend::precompute_twiddles(evaluation.domain.half_coset);
    let workload = declare_exemplar_hybrid_fri_workload(
        MetalExecutionIntent::PreferMetal,
        "fibonacci_example",
        config,
    )
    .unwrap();

    let mut cpu_channel = Blake2sChannel::default();
    let cpu_result = FriProver::<CpuBackend, Blake2sMerkleChannel>::commit(
        &mut cpu_channel,
        config,
        &evaluation,
        &twiddles,
    )
    .decommit(&mut cpu_channel);
    let metal_result = workload.prove_from_cpu_evaluation(&evaluation).unwrap();

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
