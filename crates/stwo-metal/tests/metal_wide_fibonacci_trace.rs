#![cfg(feature = "prover")]

use stwo::core::fields::m31::BaseField;
use stwo::core::fields::FieldExpOps;
use stwo_metal::{
    declare_wide_fibonacci_benchmark_boundary, generate_metal_wide_fibonacci_trace,
    metal_runtime_support, MetalBenchmarkOperation, MetalBenchmarkReferencePlatform,
    MetalBenchmarkTarget, MetalExecutionIntent, MetalRuntimeSupport,
    MetalWideFibonacciTraceRequest,
};

fn cpu_wide_fibonacci_trace(
    input_a: &[BaseField],
    input_b: &[BaseField],
    n_columns: usize,
) -> Vec<Vec<BaseField>> {
    let input_len = input_a.len();
    let mut columns = vec![vec![BaseField::from_u32_unchecked(0); input_len]; n_columns];
    columns[0].clone_from_slice(input_a);
    columns[1].clone_from_slice(input_b);
    for column in 2..n_columns {
        for row in 0..input_len {
            columns[column][row] =
                columns[column - 2][row].square() + columns[column - 1][row].square();
        }
    }
    columns
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn native_wide_fibonacci_trace_matches_cpu_recurrence() {
    assert_eq!(
        metal_runtime_support(),
        MetalRuntimeSupport::Available,
        "Metal runtime must be available for the native Apple Silicon parity test"
    );

    let input_len = 1usize << 6;
    let n_columns = 8usize;
    let input_a = (0..input_len)
        .map(|i| BaseField::from_u32_unchecked((i as u32 % 17) + 1))
        .collect::<Vec<_>>();
    let input_b = (0..input_len)
        .map(|i| BaseField::from_u32_unchecked(((i as u32 * 5) % 19) + 2))
        .collect::<Vec<_>>();

    let trace = generate_metal_wide_fibonacci_trace(MetalWideFibonacciTraceRequest {
        input_a: &input_a,
        input_b: &input_b,
        n_columns: n_columns as u32,
    })
    .expect("wide-fibonacci Metal trace generation should succeed");
    let cpu_trace = cpu_wide_fibonacci_trace(&input_a, &input_b, n_columns);

    assert_eq!(trace.input_len(), input_len);
    assert_eq!(trace.n_columns(), n_columns as u32);
    for (column_index, expected_column) in cpu_trace.iter().enumerate() {
        assert_eq!(trace.column_values(column_index), *expected_column);
    }

    let metal_evaluations = trace.to_metal_evaluations();
    for (evaluation, expected_column) in metal_evaluations.iter().zip(cpu_trace.iter()) {
        assert_eq!(evaluation.values.to_vec(), *expected_column);
    }
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn benchmark_boundary_witness_inputs_generate_native_trace() {
    assert_eq!(
        metal_runtime_support(),
        MetalRuntimeSupport::Available,
        "Metal runtime must be available for the native Apple Silicon parity test"
    );

    let target = MetalBenchmarkTarget {
        benchmark_id: "wide_fibonacci_trace_generation_test_v1",
        workload_name: "fibonacci_example",
        family: "wide_fibonacci",
        operation: MetalBenchmarkOperation::TraceGeneration,
        log_n_instances: 6,
        n_columns: 8,
        reference_platform: MetalBenchmarkReferencePlatform::Rtx4090Cuda,
        reference_elapsed_ms: 90.0,
    };
    let boundary =
        declare_wide_fibonacci_benchmark_boundary(MetalExecutionIntent::PreferMetal, target)
            .unwrap();
    let input_len = 1usize << target.log_n_instances;
    let input_a = (0..input_len)
        .map(|i| BaseField::from_u32_unchecked((i as u32 % 31) + 3))
        .collect::<Vec<_>>();
    let input_b = (0..input_len)
        .map(|i| BaseField::from_u32_unchecked(((i as u32 * 7) % 29) + 4))
        .collect::<Vec<_>>();

    let witness = boundary
        .ingest_cpu_witness_inputs(&input_a, &input_b)
        .unwrap();
    let trace = witness
        .generate_trace()
        .expect("benchmark witness inputs should generate a native Metal trace");
    let cpu_trace = cpu_wide_fibonacci_trace(&input_a, &input_b, target.n_columns as usize);

    assert_eq!(
        trace.value(target.n_columns as usize - 1, input_len - 1),
        cpu_trace[target.n_columns as usize - 1][input_len - 1]
    );
}
