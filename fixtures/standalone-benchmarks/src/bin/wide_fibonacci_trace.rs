use serde::Serialize;
#[cfg(feature = "cuda-runtime")]
use std::time::Instant;
#[cfg(feature = "cuda-runtime")]
use stwo::core::fields::m31::BaseField;
#[cfg(feature = "cuda-runtime")]
use stwo_metal::{generate_wide_fibonacci_trace, BaseFieldVec, WideFibonacciTraceRequest};
use stwo_metal_standalone_benchmarks::support::{
    env_flag, env_or, env_u32, env_usize, epoch_ms, required_env_path, runner_metadata,
    write_json, RunnerMetadata, SummaryStats,
};
#[cfg(feature = "cuda-runtime")]
use stwo_metal_standalone_benchmarks::support::summarize;

const BENCHMARK_ID: &str = "wide_fibonacci_trace_generation_v1";
const RESULT_SCHEMA_VERSION: u32 = 1;
const DEFAULT_LOG_N_INSTANCES: u32 = 20;
const DEFAULT_N_COLUMNS: u32 = 100;
const DEFAULT_WARMUPS: usize = 1;
const DEFAULT_SAMPLES: usize = 5;

#[derive(Serialize)]
struct BenchmarkResult {
    schema_version: u32,
    benchmark_id: String,
    status: String,
    classification: String,
    dependency_row: String,
    git_commit: String,
    command: String,
    started_at_epoch_ms: u128,
    completed_at_epoch_ms: u128,
    runner: RunnerMetadata,
    workload: WorkloadMetadata,
    timings: TimingMetadata,
    sentinel: Option<WideFibonacciSentinel>,
}

#[derive(Serialize)]
struct WorkloadMetadata {
    family: String,
    channel: String,
    operation: String,
    log_n_instances: u32,
    instances: u64,
    n_columns: u32,
    warmup_iterations: usize,
    sample_iterations: usize,
}

#[derive(Serialize)]
struct TimingMetadata {
    samples_ms: Vec<f64>,
    summary_ms: Option<SummaryStats>,
    throughput_kelem_per_second: Option<f64>,
}

#[derive(Clone, Serialize)]
struct WideFibonacciSentinel {
    first_column_first_value: u32,
    second_column_first_value: u32,
    last_column_first_value: u32,
    last_column_last_value: u32,
}

fn main() {
    let output_path = required_env_path("STWO_BENCH_OUTPUT_JSON");
    let plan_only = env_flag("STWO_BENCH_PLAN_ONLY");
    let log_n_instances = env_u32("STWO_BENCH_LOG_N_INSTANCES", DEFAULT_LOG_N_INSTANCES);
    let n_columns = env_u32("STWO_BENCH_N_COLUMNS", DEFAULT_N_COLUMNS);
    let warmups = env_usize("STWO_BENCH_WARMUPS", DEFAULT_WARMUPS);
    let samples = env_usize("STWO_BENCH_SAMPLES", DEFAULT_SAMPLES);
    let instances = 1u64 << log_n_instances;
    let started_at = epoch_ms();

    let runner = runner_metadata();

    let workload = WorkloadMetadata {
        family: "wide_fibonacci".to_string(),
        channel: "blake2s".to_string(),
        operation: "trace_generation".to_string(),
        log_n_instances,
        instances,
        n_columns,
        warmup_iterations: warmups,
        sample_iterations: samples,
    };

    let classification = env_or("STWO_BENCH_CLASSIFICATION", "supported-benchmark-candidate");
    let dependency_row = env_or("STWO_BENCH_DEPENDENCY_ROW", "vendored-upstream-bridge-v1");
    let git_commit = env_or("STWO_BENCH_GIT_COMMIT", "unknown");
    let command = env_or("STWO_BENCH_COMMAND", "unknown");

    let result = if plan_only {
        BenchmarkResult {
            schema_version: RESULT_SCHEMA_VERSION,
            benchmark_id: BENCHMARK_ID.to_string(),
            status: "planned".to_string(),
            classification,
            dependency_row,
            git_commit,
            command,
            started_at_epoch_ms: started_at,
            completed_at_epoch_ms: epoch_ms(),
            runner,
            workload,
            timings: TimingMetadata {
                samples_ms: Vec::new(),
                summary_ms: None,
                throughput_kelem_per_second: None,
            },
            sentinel: None,
        }
    } else {
        #[cfg(not(feature = "cuda-runtime"))]
        panic!(
            "wide_fibonacci_trace benchmark requires the cuda-runtime feature for non-plan execution"
        );

        #[cfg(feature = "cuda-runtime")]
        {
        if runner.stwo_cuda_mode == "no-cuda" {
            panic!("wide_fibonacci_trace benchmark cannot run with STWO_CUDA_MODE=no-cuda");
        }

        let input_len = instances as usize;
        let input_a_host = (0..input_len)
            .map(|i| BaseField::from((i as u32 % 1024) + 1))
            .collect::<Vec<_>>();
        let input_b_host = (0..input_len)
            .map(|i| BaseField::from(((i as u32 * 3) % 1024) + 2))
            .collect::<Vec<_>>();

        for _ in 0..warmups {
            let _ = run_one_sample(&input_a_host, &input_b_host, input_len, n_columns as usize);
        }

        let mut sample_results = Vec::with_capacity(samples);
        for _ in 0..samples {
            sample_results.push(run_one_sample(
                &input_a_host,
                &input_b_host,
                input_len,
                n_columns as usize,
            ));
        }

        let samples_ms = sample_results
            .iter()
            .map(|sample| sample.elapsed_ms)
            .collect::<Vec<_>>();
        let summary = summarize(&samples_ms);
        let throughput = if summary.mean > 0.0 {
            // elements per millisecond is numerically equal to Kelem/s
            Some(instances as f64 / summary.mean)
        } else {
            None
        };
        let sentinel = sample_results
            .last()
            .map(|sample| sample.sentinel.clone())
            .expect("at least one sample");

        BenchmarkResult {
            schema_version: RESULT_SCHEMA_VERSION,
            benchmark_id: BENCHMARK_ID.to_string(),
            status: "completed".to_string(),
            classification,
            dependency_row,
            git_commit,
            command,
            started_at_epoch_ms: started_at,
            completed_at_epoch_ms: epoch_ms(),
            runner,
            workload,
            timings: TimingMetadata {
                samples_ms,
                summary_ms: Some(summary),
                throughput_kelem_per_second: throughput,
            },
            sentinel: Some(sentinel),
        }
        }
    };

    write_json(&output_path, &result);
    println!("{}", output_path.display());
}

#[cfg(feature = "cuda-runtime")]
#[derive(Clone)]
struct SampleResult {
    elapsed_ms: f64,
    sentinel: WideFibonacciSentinel,
}

#[cfg(feature = "cuda-runtime")]
fn run_one_sample(
    input_a_host: &[BaseField],
    input_b_host: &[BaseField],
    input_len: usize,
    n_columns: usize,
) -> SampleResult {
    let input_a = BaseFieldVec::from_vec(input_a_host.to_vec());
    let input_b = BaseFieldVec::from_vec(input_b_host.to_vec());
    let trace_columns = (0..n_columns)
        .map(|_| BaseFieldVec::new_zeroes(input_len))
        .collect::<Vec<_>>();
    let trace_column_ptrs = trace_columns
        .iter()
        .map(|col| col.device_ptr)
        .collect::<Vec<_>>();

    let start = Instant::now();
    generate_wide_fibonacci_trace(WideFibonacciTraceRequest {
        input_a: &input_a,
        input_b: &input_b,
        input_len: input_len as u32,
        trace_columns: &trace_column_ptrs,
        n_columns: n_columns as u32,
    });
    let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;

    let sentinel = WideFibonacciSentinel {
        first_column_first_value: trace_columns[0].get_data(0).0,
        second_column_first_value: trace_columns[1].get_data(0).0,
        last_column_first_value: trace_columns[n_columns - 1].get_data(0).0,
        last_column_last_value: trace_columns[n_columns - 1].get_data(input_len - 1).0,
    };

    SampleResult {
        elapsed_ms,
        sentinel,
    }
}
