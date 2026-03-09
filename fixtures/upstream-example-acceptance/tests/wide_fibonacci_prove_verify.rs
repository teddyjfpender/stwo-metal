use stwo::core::fields::m31::BaseField;
use stwo::core::fields::qm31::SecureField;
use stwo::prover::backend::CpuBackend;
use stwo_constraint_framework::TraceLocationAllocator;
use stwo_examples::wide_fibonacci::{
    generate_trace as generate_upstream_trace, FibInput, WideFibonacciComponent, WideFibonacciEval,
};
use stwo_metal::{
    declare_exemplar_metal_workload_boundary, metal_runtime_support, MetalExecutionIntent,
    MetalRuntimeSupport,
};
use stwo_metal_upstream_example_acceptance::{
    bridge_framework_component_to_metal,
    prove_and_verify_single_trace_component_via_backend_blake2s,
    prove_and_verify_single_trace_component_via_cpu_blake2s,
};

const WIDE_FIBONACCI_COLUMNS: usize = 100;

fn example_inputs(log_n_instances: u32) -> Vec<FibInput> {
    (0..(1 << log_n_instances))
        .map(|index| FibInput {
            a: BaseField::from_u32_unchecked((index % 17) + 1),
            b: BaseField::from_u32_unchecked(((index * 5) % 19) + 2),
        })
        .collect()
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn vendored_upstream_wide_fibonacci_example_proves_and_verifies_via_metal_backend() {
    assert_eq!(
        metal_runtime_support(),
        MetalRuntimeSupport::Available,
        "Metal runtime must be available for the native Apple Silicon acceptance test"
    );

    let log_n_instances = 6u32;
    let inputs = example_inputs(log_n_instances);
    let input_a = inputs.iter().map(|input| input.a).collect::<Vec<_>>();
    let input_b = inputs.iter().map(|input| input.b).collect::<Vec<_>>();

    let boundary = declare_exemplar_metal_workload_boundary(
        MetalExecutionIntent::PreferMetal,
        "fibonacci_example",
    )
    .expect("fibonacci example workload boundary should be declared");
    let witness = boundary
        .ingest_cpu_wide_fibonacci_witness(&input_a, &input_b, WIDE_FIBONACCI_COLUMNS as u32)
        .expect("upstream example inputs should feed the native Metal witness handoff");
    let metal_trace = witness
        .generate_trace()
        .expect("upstream example inputs should generate a native Metal trace");
    let hybrid_component = WideFibonacciComponent::new(
        &mut TraceLocationAllocator::default(),
        WideFibonacciEval::<WIDE_FIBONACCI_COLUMNS> {
            log_n_rows: log_n_instances,
        },
        SecureField::from_u32_unchecked(0, 0, 0, 0),
    );
    let metal_component = bridge_framework_component_to_metal(&hybrid_component);
    let hybrid_proof = prove_and_verify_single_trace_component_via_backend_blake2s(
        metal_trace.to_metal_evaluations(),
        &metal_component,
        log_n_instances,
    )
    .expect("wide fibonacci should prove and verify through MetalBackend");

    let cpu_trace = generate_upstream_trace::<WIDE_FIBONACCI_COLUMNS, CpuBackend>(&inputs);
    let cpu_component = WideFibonacciComponent::new(
        &mut TraceLocationAllocator::default(),
        WideFibonacciEval::<WIDE_FIBONACCI_COLUMNS> {
            log_n_rows: log_n_instances,
        },
        SecureField::from_u32_unchecked(0, 0, 0, 0),
    );
    let cpu_proof = prove_and_verify_single_trace_component_via_cpu_blake2s(
        cpu_trace,
        &cpu_component,
        log_n_instances,
    )
    .expect("upstream CPU trace should prove and verify through the stock CPU path");

    assert_eq!(
        hybrid_proof.commitments.0, cpu_proof.commitments.0,
        "the MetalBackend proving path should preserve the upstream proof commitments"
    );
}
