use stwo::prover::backend::{Column, CpuBackend};
use stwo_examples::wide_fibonacci::{generate_trace, FibInput};
use stwo_metal::{
    declare_exemplar_metal_workload_boundary, metal_runtime_support, MetalExecutionIntent,
    MetalRuntimeSupport,
};

const WIDE_FIBONACCI_COLUMNS: usize = 100;

fn example_inputs(log_n_instances: u32) -> Vec<FibInput> {
    (0..(1 << log_n_instances))
        .map(|index| FibInput {
            a: stwo::core::fields::m31::BaseField::from_u32_unchecked((index % 17) + 1),
            b: stwo::core::fields::m31::BaseField::from_u32_unchecked(((index * 5) % 19) + 2),
        })
        .collect()
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn vendored_upstream_wide_fibonacci_example_feeds_native_metal_trace() {
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

    let upstream_trace = generate_trace::<WIDE_FIBONACCI_COLUMNS, CpuBackend>(&inputs);

    assert_eq!(metal_trace.input_len(), 1usize << log_n_instances);
    assert_eq!(metal_trace.n_columns(), WIDE_FIBONACCI_COLUMNS as u32);
    for (column_index, upstream_column) in upstream_trace.iter().enumerate() {
        assert_eq!(
            metal_trace.column_values(column_index),
            upstream_column.values.to_cpu(),
            "column {column_index} should match the vendored upstream example trace"
        );
    }
}
