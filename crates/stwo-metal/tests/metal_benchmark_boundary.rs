#![cfg(feature = "prover")]

use stwo::core::fields::m31::BaseField;
use stwo_metal::{
    declare_wide_fibonacci_benchmark_boundary, MetalBenchmarkInputError, MetalBenchmarkOperation,
    MetalBenchmarkReferencePlatform, MetalExecutionIntent, MetalExecutionPlan,
    MetalWorkloadOwnership, MetalWorkloadStage, WIDE_FIBONACCI_PROVE_LOG20_TARGET,
    WIDE_FIBONACCI_TRACE_LOG20_TARGET,
};

#[test]
fn wide_fibonacci_benchmark_targets_pin_log20_and_reference_goal() {
    assert_eq!(WIDE_FIBONACCI_TRACE_LOG20_TARGET.log_n_instances, 20);
    assert_eq!(WIDE_FIBONACCI_TRACE_LOG20_TARGET.n_columns, 100);
    assert_eq!(
        WIDE_FIBONACCI_TRACE_LOG20_TARGET.reference_platform,
        MetalBenchmarkReferencePlatform::Rtx4090Cuda
    );
    assert_eq!(WIDE_FIBONACCI_PROVE_LOG20_TARGET.log_n_instances, 20);
    assert_eq!(WIDE_FIBONACCI_PROVE_LOG20_TARGET.reference_elapsed_ms, 90.0);
    assert_eq!(
        WIDE_FIBONACCI_PROVE_LOG20_TARGET.operation,
        MetalBenchmarkOperation::ProveVerify
    );
}

#[test]
fn wide_fibonacci_benchmark_boundary_is_hybrid_with_cpu_witness_stage() {
    let boundary = declare_wide_fibonacci_benchmark_boundary(
        MetalExecutionIntent::PreferMetal,
        WIDE_FIBONACCI_PROVE_LOG20_TARGET,
    )
    .unwrap();

    assert_eq!(
        boundary.workload_boundary().execution_authority().plan(),
        MetalExecutionPlan::MetalFriHybrid
    );
    assert_eq!(
        boundary
            .workload_boundary()
            .execution_authority()
            .stage_ownership(MetalWorkloadStage::WitnessMain),
        Some(MetalWorkloadOwnership::CpuOwned)
    );
}

#[test]
fn wide_fibonacci_benchmark_boundary_validates_witness_input_lengths() {
    let boundary = declare_wide_fibonacci_benchmark_boundary(
        MetalExecutionIntent::PreferMetal,
        WIDE_FIBONACCI_TRACE_LOG20_TARGET,
    )
    .unwrap();
    let expected_len = 1usize << 20;
    let input_a = vec![BaseField::from_u32_unchecked(1); expected_len];
    let input_b = vec![BaseField::from_u32_unchecked(2); expected_len - 1];

    let error = boundary
        .ingest_cpu_witness_inputs(&input_a, &input_b)
        .unwrap_err();

    assert_eq!(
        error,
        MetalBenchmarkInputError::InputLengthMismatch {
            expected_len,
            actual_a: expected_len,
            actual_b: expected_len - 1,
        }
    );
}
