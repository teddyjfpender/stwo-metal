#![cfg(feature = "prover")]

use stwo::core::fields::m31::BaseField;
use stwo::core::fields::qm31::SecureField;
use stwo::core::fri::FriConfig;
use stwo::core::poly::circle::CanonicCoset;
use stwo::prover::backend::cpu::CpuCirclePoly;
use stwo_metal::{
    declare_exemplar_hybrid_fri_workload, declare_exemplar_metal_workload_boundary,
    metal_runtime_support, MetalExecutionIntent, MetalExecutionPlan, MetalRuntimeSupport,
    MetalSecureFieldVec, MetalWorkloadOwnership, MetalWorkloadStage,
};

#[test]
fn fibonacci_boundary_is_explicitly_hybrid() {
    let boundary = declare_exemplar_metal_workload_boundary(
        MetalExecutionIntent::PreferMetal,
        "fibonacci_example",
    )
    .unwrap();
    let execution_authority = boundary.execution_authority();

    assert_eq!(
        execution_authority.plan(),
        MetalExecutionPlan::MetalFriHybrid
    );
    assert_eq!(
        execution_authority.stage_ownership(MetalWorkloadStage::WitnessMain),
        Some(MetalWorkloadOwnership::CpuOwned)
    );
    assert_eq!(
        execution_authority.stage_ownership(MetalWorkloadStage::QuotientEval),
        Some(MetalWorkloadOwnership::CpuOwned)
    );
    assert_eq!(
        execution_authority.stage_ownership(MetalWorkloadStage::PcsCommitment),
        Some(MetalWorkloadOwnership::CpuOwned)
    );
    assert_eq!(
        execution_authority.stage_ownership(MetalWorkloadStage::FriBlake2s),
        Some(MetalWorkloadOwnership::MetalNative)
    );
}

#[test]
fn poseidon_boundary_keeps_interaction_trace_explicitly_cpu_owned() {
    let boundary = declare_exemplar_metal_workload_boundary(
        MetalExecutionIntent::PreferMetal,
        "poseidon_example",
    )
    .unwrap();

    assert_eq!(
        boundary
            .execution_authority()
            .stage_ownership(MetalWorkloadStage::WitnessInteraction),
        Some(MetalWorkloadOwnership::CpuOwned)
    );
}

#[test]
fn fibonacci_boundary_accepts_wide_fibonacci_witness_handoff() {
    let boundary = declare_exemplar_metal_workload_boundary(
        MetalExecutionIntent::PreferMetal,
        "fibonacci_example",
    )
    .unwrap();
    let input_len = 1usize << 6;
    let input_a = vec![BaseField::from_u32_unchecked(1); input_len];
    let input_b = (0..input_len)
        .map(|i| BaseField::from_u32_unchecked((i as u32 % 17) + 2))
        .collect::<Vec<_>>();

    let handoff = boundary
        .ingest_cpu_wide_fibonacci_witness(&input_a, &input_b, 8)
        .unwrap();

    assert_eq!(handoff.workload_name(), "fibonacci_example");
    assert_eq!(handoff.log_n_instances(), 6);
    assert_eq!(handoff.n_columns(), 8);
    assert_eq!(handoff.input_a(), input_a);
    assert_eq!(handoff.input_b(), input_b);
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn declared_hybrid_workload_executes_the_declared_fri_subpath() {
    assert_eq!(
        metal_runtime_support(),
        MetalRuntimeSupport::Available,
        "Metal runtime must be available for the native Apple Silicon parity test"
    );

    const LOG_SIZE: u32 = 8;

    let domain = CanonicCoset::new(LOG_SIZE).circle_domain();
    let values = CpuCirclePoly::new(
        (1..=(1 << 5))
            .map(|i| BaseField::from_u32_unchecked(i))
            .collect(),
    )
    .evaluate(domain)
    .values
    .into_iter()
    .map(SecureField::from)
    .collect::<Vec<_>>();
    let column = MetalSecureFieldVec::from_vec(values);
    let workload = declare_exemplar_hybrid_fri_workload(
        MetalExecutionIntent::PreferMetal,
        "fibonacci_example",
        FriConfig::new(3, 2, 3, 2),
    )
    .unwrap();

    let result = workload.prove(&column, domain);

    assert!(!result.unsorted_query_locations.is_empty());
    assert!(!result.query_positions.is_empty());
}
