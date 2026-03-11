#[cfg(feature = "metal-runtime")]
use std::time::Instant;

use serde::Serialize;
#[cfg(feature = "metal-runtime")]
use stwo::core::air::accumulation::PointEvaluationAccumulator;
#[cfg(feature = "metal-runtime")]
use stwo::core::air::Component;
#[cfg(feature = "metal-runtime")]
use stwo::core::channel::Blake2sChannel;
#[cfg(feature = "metal-runtime")]
use stwo::core::constraints::coset_vanishing;
#[cfg(feature = "metal-runtime")]
use stwo::core::fields::m31::BaseField;
#[cfg(feature = "metal-runtime")]
use stwo::core::fields::qm31::SecureField;
#[cfg(feature = "metal-runtime")]
use stwo::core::fields::FieldExpOps;
#[cfg(feature = "metal-runtime")]
use stwo::core::pcs::{CommitmentSchemeVerifier, PcsConfig, TreeVec};
#[cfg(feature = "metal-runtime")]
use stwo::core::poly::circle::CanonicCoset;
#[cfg(feature = "metal-runtime")]
use stwo::core::proof::{StarkProof, StarkProofSizeBreakdown};
#[cfg(feature = "metal-runtime")]
use stwo::core::utils::MaybeOwned;
#[cfg(feature = "metal-runtime")]
use stwo::core::vcs_lifted::blake2_merkle::{Blake2sMerkleChannel, Blake2sMerkleHasher};
#[cfg(feature = "metal-runtime")]
use stwo::core::verifier::VerificationError;
#[cfg(feature = "metal-runtime")]
use stwo::core::verifier::{verify, PREPROCESSED_TRACE_IDX};
#[cfg(feature = "metal-runtime")]
use stwo::prover::poly::circle::CircleEvaluation;
#[cfg(feature = "metal-runtime")]
use stwo::prover::poly::circle::PolyOps;
#[cfg(feature = "metal-runtime")]
use stwo::prover::poly::BitReversedOrder;
#[cfg(feature = "metal-runtime")]
use stwo::prover::vcs_lifted::ops::MerkleOpsLifted;
#[cfg(feature = "metal-runtime")]
use stwo::prover::vcs_lifted::prover::MerkleProverLifted;
#[cfg(feature = "metal-runtime")]
use stwo::prover::{
    prove, CommitmentSchemeProver, CommitmentTreeProver, ComponentProver,
    DomainEvaluationAccumulator, ProvingError, Trace,
};
#[cfg(feature = "metal-runtime")]
use stwo_constraint_framework::TraceLocationAllocator;
#[cfg(feature = "metal-runtime")]
use stwo_examples::wide_fibonacci::{
    generate_trace, FibInput, WideFibonacciComponent, WideFibonacciEval,
};
#[cfg(feature = "metal-runtime")]
use stwo_metal::{
    accumulate_wide_fibonacci_quotients, accumulate_wide_fibonacci_quotients_from_batch,
    declare_exemplar_metal_workload_boundary, declare_wide_fibonacci_benchmark_boundary,
    evaluate_polys_on_domain_batch, MetalBackend, MetalBaseFieldVec, MetalBenchmarkOperation,
    MetalBenchmarkReferencePlatform, MetalBenchmarkTarget, MetalExecutionIntent,
    MetalWideFibonacciBatchQuotientRequest, MetalWideFibonacciBenchmarkBoundary,
    MetalWideFibonacciQuotientRequest, MetalWideFibonacciTrace,
};
#[cfg(feature = "metal-runtime")]
use stwo_metal_fixture_shims::acceptance_bridge_catalog;
#[cfg(feature = "metal-runtime")]
use stwo_metal_standalone_benchmarks::support::summarize;
use stwo_metal_standalone_benchmarks::support::{
    enforce_metal_benchmark_contract, env_flag, env_or, env_u32, env_usize, epoch_ms,
    required_env_path, runner_metadata, write_json, RunnerMetadata, SummaryStats,
};

const BENCHMARK_ID: &str = "wide_fibonacci_prove_verify_v1";
const RESULT_SCHEMA_VERSION: u32 = 1;
const DEFAULT_LOG_N_INSTANCES: u32 = 20;
const DEFAULT_N_COLUMNS: u32 = 100;
const DEFAULT_WARMUPS: usize = 1;
const DEFAULT_SAMPLES: usize = 5;
#[cfg(feature = "metal-runtime")]
const MAIN_TRACE_IDX: usize = 1;
#[cfg(feature = "metal-runtime")]
const WIDE_FIBONACCI_COLUMNS: usize = 100;

#[cfg(feature = "metal-runtime")]
fn benchmark_target(
    log_n_instances: u32,
    n_columns: usize,
    operation: MetalBenchmarkOperation,
) -> MetalBenchmarkTarget {
    MetalBenchmarkTarget {
        benchmark_id: BENCHMARK_ID,
        workload_name: "fibonacci_example",
        family: "wide_fibonacci",
        operation,
        log_n_instances,
        n_columns: n_columns as u32,
        reference_platform: MetalBenchmarkReferencePlatform::Rtx4090Cuda,
        reference_elapsed_ms: 90.0,
    }
}

#[derive(Serialize)]
struct BenchmarkResult {
    schema_version: u32,
    benchmark_id: String,
    benchmark_lane: String,
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
    proof: Option<ProofMetadata>,
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
    cold_start_sample_ms: Option<f64>,
    cold_start_phase_ms: Option<SinglePhaseTimings>,
    cold_start_prove_breakdown_ms: Option<SingleProveBreakdownTimings>,
    steady_state_samples_ms: Option<Vec<f64>>,
    steady_state_summary_ms: Option<SummaryStats>,
    steady_state_throughput_kelem_per_second: Option<f64>,
    phase_samples_ms: Option<PhaseSampleTimings>,
    phase_summary_ms: Option<PhaseSummaryTimings>,
    steady_state_phase_samples_ms: Option<PhaseSampleTimings>,
    steady_state_phase_summary_ms: Option<PhaseSummaryTimings>,
    prove_breakdown_samples_ms: Option<ProveBreakdownSampleTimings>,
    prove_breakdown_summary_ms: Option<ProveBreakdownSummaryTimings>,
    steady_state_prove_breakdown_samples_ms: Option<ProveBreakdownSampleTimings>,
    steady_state_prove_breakdown_summary_ms: Option<ProveBreakdownSummaryTimings>,
}

#[derive(Serialize)]
struct PhaseSampleTimings {
    prove_ms: Vec<f64>,
    verify_ms: Vec<f64>,
}

#[derive(Serialize)]
struct PhaseSummaryTimings {
    prove_ms: SummaryStats,
    verify_ms: SummaryStats,
}

#[derive(Serialize)]
struct SinglePhaseTimings {
    prove_ms: f64,
    verify_ms: f64,
}

#[derive(Serialize)]
struct ProveBreakdownSampleTimings {
    setup_and_preprocessed_commit_ms: Vec<f64>,
    trace_generation_ms: Vec<f64>,
    trace_commit_ms: Vec<f64>,
    trace_commit_interpolation_ms: Vec<f64>,
    trace_commit_extension_ms: Vec<f64>,
    trace_commit_merkle_ms: Vec<f64>,
    prove_core_ms: Vec<f64>,
    prove_core_evaluation_program_v1_ms: Vec<f64>,
    prove_core_composition_generation_ms: Vec<f64>,
    prove_core_composition_commit_ms: Vec<f64>,
    prove_core_prove_values_ms: Vec<f64>,
    prove_core_sanity_check_ms: Vec<f64>,
}

#[derive(Serialize)]
struct ProveBreakdownSummaryTimings {
    setup_and_preprocessed_commit_ms: SummaryStats,
    trace_generation_ms: SummaryStats,
    trace_commit_ms: SummaryStats,
    trace_commit_interpolation_ms: SummaryStats,
    trace_commit_extension_ms: SummaryStats,
    trace_commit_merkle_ms: SummaryStats,
    prove_core_ms: SummaryStats,
    prove_core_evaluation_program_v1_ms: SummaryStats,
    prove_core_composition_generation_ms: SummaryStats,
    prove_core_composition_commit_ms: SummaryStats,
    prove_core_prove_values_ms: SummaryStats,
    prove_core_sanity_check_ms: SummaryStats,
}

#[derive(Serialize)]
struct SingleProveBreakdownTimings {
    setup_and_preprocessed_commit_ms: f64,
    trace_generation_ms: f64,
    trace_commit_ms: f64,
    trace_commit_interpolation_ms: f64,
    trace_commit_extension_ms: f64,
    trace_commit_merkle_ms: f64,
    prove_core_ms: f64,
    prove_core_evaluation_program_v1_ms: f64,
    prove_core_composition_generation_ms: f64,
    prove_core_composition_commit_ms: f64,
    prove_core_prove_values_ms: f64,
    prove_core_sanity_check_ms: f64,
}

#[cfg(feature = "metal-runtime")]
fn steady_state_tail<T>(samples: &[T]) -> Option<&[T]> {
    (samples.len() >= 2).then(|| &samples[1..])
}

#[cfg(feature = "metal-runtime")]
fn throughput_from_summary(instances: u64, summary: &SummaryStats) -> Option<f64> {
    (summary.mean > 0.0).then_some(instances as f64 / summary.mean)
}

#[cfg(feature = "metal-runtime")]
fn phase_sample_timings_from_samples(samples: &[SampleResult]) -> PhaseSampleTimings {
    PhaseSampleTimings {
        prove_ms: samples
            .iter()
            .map(|sample| sample.prove_elapsed_ms)
            .collect(),
        verify_ms: samples
            .iter()
            .map(|sample| sample.verify_elapsed_ms)
            .collect(),
    }
}

#[cfg(feature = "metal-runtime")]
fn phase_summary_timings_from_samples(samples: &[SampleResult]) -> PhaseSummaryTimings {
    PhaseSummaryTimings {
        prove_ms: summarize(
            &samples
                .iter()
                .map(|sample| sample.prove_elapsed_ms)
                .collect::<Vec<_>>(),
        ),
        verify_ms: summarize(
            &samples
                .iter()
                .map(|sample| sample.verify_elapsed_ms)
                .collect::<Vec<_>>(),
        ),
    }
}

#[cfg(feature = "metal-runtime")]
fn single_phase_timings_from_sample(sample: &SampleResult) -> SinglePhaseTimings {
    SinglePhaseTimings {
        prove_ms: sample.prove_elapsed_ms,
        verify_ms: sample.verify_elapsed_ms,
    }
}

#[cfg(feature = "metal-runtime")]
fn prove_breakdown_samples_from_samples(samples: &[ProveBreakdown]) -> ProveBreakdownSampleTimings {
    let setup_and_preprocessed_commit_ms = samples
        .iter()
        .map(|sample| sample.setup_and_preprocessed_commit_ms)
        .collect::<Vec<_>>();
    let trace_generation_ms = samples
        .iter()
        .map(|sample| sample.trace_generation_ms)
        .collect::<Vec<_>>();
    let trace_commit_ms = samples
        .iter()
        .map(|sample| sample.trace_commit_ms)
        .collect::<Vec<_>>();
    let trace_commit_interpolation_ms = samples
        .iter()
        .map(|sample| sample.trace_commit_interpolation_ms)
        .collect::<Vec<_>>();
    let trace_commit_extension_ms = samples
        .iter()
        .map(|sample| sample.trace_commit_extension_ms)
        .collect::<Vec<_>>();
    let trace_commit_merkle_ms = samples
        .iter()
        .map(|sample| sample.trace_commit_merkle_ms)
        .collect::<Vec<_>>();
    let prove_core_ms = samples
        .iter()
        .map(|sample| sample.prove_core_ms)
        .collect::<Vec<_>>();
    let prove_core_evaluation_program_v1_ms = samples
        .iter()
        .map(|sample| sample.prove_core_evaluation_program_v1_ms)
        .collect::<Vec<_>>();
    let prove_core_composition_generation_ms = samples
        .iter()
        .map(|sample| sample.prove_core_composition_generation_ms)
        .collect::<Vec<_>>();
    let prove_core_composition_commit_ms = samples
        .iter()
        .map(|sample| sample.prove_core_composition_commit_ms)
        .collect::<Vec<_>>();
    let prove_core_prove_values_ms = samples
        .iter()
        .map(|sample| sample.prove_core_prove_values_ms)
        .collect::<Vec<_>>();
    let prove_core_sanity_check_ms = samples
        .iter()
        .map(|sample| sample.prove_core_sanity_check_ms)
        .collect::<Vec<_>>();

    ProveBreakdownSampleTimings {
        setup_and_preprocessed_commit_ms,
        trace_generation_ms,
        trace_commit_ms,
        trace_commit_interpolation_ms,
        trace_commit_extension_ms,
        trace_commit_merkle_ms,
        prove_core_ms,
        prove_core_evaluation_program_v1_ms,
        prove_core_composition_generation_ms,
        prove_core_composition_commit_ms,
        prove_core_prove_values_ms,
        prove_core_sanity_check_ms,
    }
}

#[cfg(feature = "metal-runtime")]
fn prove_breakdown_summary_from_samples(
    samples: &[ProveBreakdown],
) -> ProveBreakdownSummaryTimings {
    ProveBreakdownSummaryTimings {
        setup_and_preprocessed_commit_ms: summarize(
            &samples
                .iter()
                .map(|sample| sample.setup_and_preprocessed_commit_ms)
                .collect::<Vec<_>>(),
        ),
        trace_generation_ms: summarize(
            &samples
                .iter()
                .map(|sample| sample.trace_generation_ms)
                .collect::<Vec<_>>(),
        ),
        trace_commit_ms: summarize(
            &samples
                .iter()
                .map(|sample| sample.trace_commit_ms)
                .collect::<Vec<_>>(),
        ),
        trace_commit_interpolation_ms: summarize(
            &samples
                .iter()
                .map(|sample| sample.trace_commit_interpolation_ms)
                .collect::<Vec<_>>(),
        ),
        trace_commit_extension_ms: summarize(
            &samples
                .iter()
                .map(|sample| sample.trace_commit_extension_ms)
                .collect::<Vec<_>>(),
        ),
        trace_commit_merkle_ms: summarize(
            &samples
                .iter()
                .map(|sample| sample.trace_commit_merkle_ms)
                .collect::<Vec<_>>(),
        ),
        prove_core_ms: summarize(
            &samples
                .iter()
                .map(|sample| sample.prove_core_ms)
                .collect::<Vec<_>>(),
        ),
        prove_core_evaluation_program_v1_ms: summarize(
            &samples
                .iter()
                .map(|sample| sample.prove_core_evaluation_program_v1_ms)
                .collect::<Vec<_>>(),
        ),
        prove_core_composition_generation_ms: summarize(
            &samples
                .iter()
                .map(|sample| sample.prove_core_composition_generation_ms)
                .collect::<Vec<_>>(),
        ),
        prove_core_composition_commit_ms: summarize(
            &samples
                .iter()
                .map(|sample| sample.prove_core_composition_commit_ms)
                .collect::<Vec<_>>(),
        ),
        prove_core_prove_values_ms: summarize(
            &samples
                .iter()
                .map(|sample| sample.prove_core_prove_values_ms)
                .collect::<Vec<_>>(),
        ),
        prove_core_sanity_check_ms: summarize(
            &samples
                .iter()
                .map(|sample| sample.prove_core_sanity_check_ms)
                .collect::<Vec<_>>(),
        ),
    }
}

#[cfg(feature = "metal-runtime")]
fn single_prove_breakdown_timings_from_sample(
    sample: &ProveBreakdown,
) -> SingleProveBreakdownTimings {
    SingleProveBreakdownTimings {
        setup_and_preprocessed_commit_ms: sample.setup_and_preprocessed_commit_ms,
        trace_generation_ms: sample.trace_generation_ms,
        trace_commit_ms: sample.trace_commit_ms,
        trace_commit_interpolation_ms: sample.trace_commit_interpolation_ms,
        trace_commit_extension_ms: sample.trace_commit_extension_ms,
        trace_commit_merkle_ms: sample.trace_commit_merkle_ms,
        prove_core_ms: sample.prove_core_ms,
        prove_core_evaluation_program_v1_ms: sample.prove_core_evaluation_program_v1_ms,
        prove_core_composition_generation_ms: sample.prove_core_composition_generation_ms,
        prove_core_composition_commit_ms: sample.prove_core_composition_commit_ms,
        prove_core_prove_values_ms: sample.prove_core_prove_values_ms,
        prove_core_sanity_check_ms: sample.prove_core_sanity_check_ms,
    }
}

#[derive(Clone, Serialize)]
struct WideFibonacciSentinel {
    first_column_first_value: u32,
    second_column_first_value: u32,
    last_column_first_value: u32,
    last_column_last_value: u32,
}

#[derive(Clone, Copy, Serialize)]
struct ProofSizeBreakdownMetadata {
    oods_samples: usize,
    queries_values: usize,
    fri_samples: usize,
    fri_decommitments: usize,
    trace_decommitments: usize,
}

#[derive(Clone, Serialize)]
struct ProofMetadata {
    size_estimate_bytes: usize,
    commitments: usize,
    security_bits: u32,
    fri_queries: usize,
    pow_bits: u32,
    size_breakdown_bytes: ProofSizeBreakdownMetadata,
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
    enforce_metal_benchmark_contract(BENCHMARK_ID, plan_only, &runner);
    let workload = WorkloadMetadata {
        family: "wide_fibonacci".to_string(),
        channel: "blake2s".to_string(),
        operation: "prove_verify".to_string(),
        log_n_instances,
        instances,
        n_columns,
        warmup_iterations: warmups,
        sample_iterations: samples,
    };

    let classification = env_or("STWO_BENCH_CLASSIFICATION", "supported-benchmark-candidate");
    let benchmark_lane = env_or("STWO_BENCH_LANE", "generated-metal");
    let dependency_row = env_or("STWO_BENCH_DEPENDENCY_ROW", "metal-backend-e2e-v1");
    let git_commit = env_or("STWO_BENCH_GIT_COMMIT", "unknown");
    let command = env_or("STWO_BENCH_COMMAND", "unknown");

    let result = if plan_only {
        BenchmarkResult {
            schema_version: RESULT_SCHEMA_VERSION,
            benchmark_id: BENCHMARK_ID.to_string(),
            benchmark_lane: benchmark_lane.clone(),
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
                cold_start_sample_ms: None,
                cold_start_phase_ms: None,
                cold_start_prove_breakdown_ms: None,
                steady_state_samples_ms: None,
                steady_state_summary_ms: None,
                steady_state_throughput_kelem_per_second: None,
                phase_samples_ms: None,
                phase_summary_ms: None,
                steady_state_phase_samples_ms: None,
                steady_state_phase_summary_ms: None,
                prove_breakdown_samples_ms: None,
                prove_breakdown_summary_ms: None,
                steady_state_prove_breakdown_samples_ms: None,
                steady_state_prove_breakdown_summary_ms: None,
            },
            proof: None,
            sentinel: None,
        }
    } else {
        #[cfg(not(feature = "metal-runtime"))]
        panic!(
            "wide_fibonacci_prove benchmark requires the metal-runtime feature for non-plan execution"
        );

        #[cfg(feature = "metal-runtime")]
        {
            let input_len = instances as usize;
            let input_a_host = vec![BaseField::from(1u32); input_len];
            let input_b_host = (0..input_len)
                .map(|i| BaseField::from_u32_unchecked(i as u32))
                .collect::<Vec<_>>();

            for _ in 0..warmups {
                let _ = run_one_sample(
                    &input_a_host,
                    &input_b_host,
                    log_n_instances,
                    n_columns as usize,
                    &benchmark_lane,
                );
            }

            let mut sample_results = Vec::with_capacity(samples);
            for _ in 0..samples {
                sample_results.push(run_one_sample(
                    &input_a_host,
                    &input_b_host,
                    log_n_instances,
                    n_columns as usize,
                    &benchmark_lane,
                ));
            }

            let samples_ms = sample_results
                .iter()
                .map(|sample| sample.total_elapsed_ms)
                .collect::<Vec<_>>();
            let prove_samples_ms = sample_results
                .iter()
                .map(|sample| sample.prove_elapsed_ms)
                .collect::<Vec<_>>();
            let verify_samples_ms = sample_results
                .iter()
                .map(|sample| sample.verify_elapsed_ms)
                .collect::<Vec<_>>();

            let total_summary = summarize(&samples_ms);
            let prove_summary = summarize(&prove_samples_ms);
            let verify_summary = summarize(&verify_samples_ms);
            let cold_start_result = sample_results.first();
            let steady_state_results = steady_state_tail(&sample_results);
            let steady_state_samples_ms = steady_state_results.map(|samples| {
                samples
                    .iter()
                    .map(|sample| sample.total_elapsed_ms)
                    .collect::<Vec<_>>()
            });
            let steady_state_summary = steady_state_samples_ms
                .as_ref()
                .map(|samples| summarize(samples));
            let prove_breakdown_samples = sample_results
                .iter()
                .map(|sample| sample.prove_breakdown)
                .collect::<Option<Vec<_>>>();
            let steady_state_phase_samples =
                steady_state_results.map(phase_sample_timings_from_samples);
            let steady_state_phase_summary =
                steady_state_results.map(phase_summary_timings_from_samples);
            let steady_state_prove_breakdown_samples = prove_breakdown_samples
                .as_ref()
                .and_then(|samples| steady_state_tail(samples))
                .map(prove_breakdown_samples_from_samples);
            let steady_state_prove_breakdown_summary = prove_breakdown_samples
                .as_ref()
                .and_then(|samples| steady_state_tail(samples))
                .map(prove_breakdown_summary_from_samples);
            let throughput = throughput_from_summary(instances, &total_summary);
            let steady_state_throughput = steady_state_summary
                .as_ref()
                .and_then(|summary| throughput_from_summary(instances, summary));
            let last_sample = sample_results.last().expect("at least one sample");

            BenchmarkResult {
                schema_version: RESULT_SCHEMA_VERSION,
                benchmark_id: BENCHMARK_ID.to_string(),
                benchmark_lane: benchmark_lane.clone(),
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
                    summary_ms: Some(total_summary),
                    throughput_kelem_per_second: throughput,
                    cold_start_sample_ms: cold_start_result.map(|sample| sample.total_elapsed_ms),
                    cold_start_phase_ms: cold_start_result.map(single_phase_timings_from_sample),
                    cold_start_prove_breakdown_ms: cold_start_result
                        .and_then(|sample| sample.prove_breakdown.as_ref())
                        .map(single_prove_breakdown_timings_from_sample),
                    steady_state_samples_ms,
                    steady_state_summary_ms: steady_state_summary,
                    steady_state_throughput_kelem_per_second: steady_state_throughput,
                    phase_samples_ms: Some(PhaseSampleTimings {
                        prove_ms: prove_samples_ms,
                        verify_ms: verify_samples_ms,
                    }),
                    phase_summary_ms: Some(PhaseSummaryTimings {
                        prove_ms: prove_summary,
                        verify_ms: verify_summary,
                    }),
                    steady_state_phase_samples_ms: steady_state_phase_samples,
                    steady_state_phase_summary_ms: steady_state_phase_summary,
                    prove_breakdown_samples_ms: prove_breakdown_samples
                        .clone()
                        .map(|samples| prove_breakdown_samples_from_samples(&samples)),
                    prove_breakdown_summary_ms: prove_breakdown_samples
                        .map(|samples| prove_breakdown_summary_from_samples(&samples)),
                    steady_state_prove_breakdown_samples_ms: steady_state_prove_breakdown_samples,
                    steady_state_prove_breakdown_summary_ms: steady_state_prove_breakdown_summary,
                },
                proof: Some(last_sample.proof_metadata.clone()),
                sentinel: Some(last_sample.sentinel.clone()),
            }
        }
    };

    write_json(&output_path, &result);
    println!("{}", output_path.display());
}

#[cfg(feature = "metal-runtime")]
struct WideFibonacciBenchmarkComponent {
    log_n_rows: u32,
    n_columns: usize,
}

#[cfg(feature = "metal-runtime")]
impl WideFibonacciBenchmarkComponent {
    fn new(log_n_rows: u32, n_columns: usize) -> Self {
        assert!(
            n_columns >= 3,
            "wide fibonacci benchmark requires at least 3 columns"
        );
        Self {
            log_n_rows,
            n_columns,
        }
    }

    fn denom_inverse(&self, point: stwo::core::circle::CirclePoint<SecureField>) -> SecureField {
        coset_vanishing(CanonicCoset::new(self.log_n_rows).coset(), point).inverse()
    }
}

#[cfg(feature = "metal-runtime")]
impl Component for WideFibonacciBenchmarkComponent {
    fn n_constraints(&self) -> usize {
        self.n_columns - 2
    }

    fn max_constraint_log_degree_bound(&self) -> u32 {
        self.log_n_rows + 1
    }

    fn trace_log_degree_bounds(&self) -> TreeVec<Vec<u32>> {
        TreeVec::new(vec![vec![], vec![self.log_n_rows; self.n_columns]])
    }

    fn mask_points(
        &self,
        point: stwo::core::circle::CirclePoint<SecureField>,
        _max_log_degree_bound: u32,
    ) -> TreeVec<Vec<Vec<stwo::core::circle::CirclePoint<SecureField>>>> {
        TreeVec::new(vec![
            vec![],
            (0..self.n_columns).map(|_| vec![point]).collect(),
        ])
    }

    fn preprocessed_column_indices(&self) -> Vec<usize> {
        vec![]
    }

    fn evaluate_constraint_quotients_at_point(
        &self,
        point: stwo::core::circle::CirclePoint<SecureField>,
        mask: &TreeVec<Vec<Vec<SecureField>>>,
        evaluation_accumulator: &mut PointEvaluationAccumulator,
        _max_log_degree_bound: u32,
    ) {
        let trace_mask = &mask[MAIN_TRACE_IDX];
        assert_eq!(
            trace_mask.len(),
            self.n_columns,
            "wide fibonacci benchmark mask width drifted"
        );

        let denom_inverse = self.denom_inverse(point);
        let mut a = trace_mask[0][0];
        let mut b = trace_mask[1][0];
        for column in trace_mask.iter().skip(2) {
            let c = column[0];
            evaluation_accumulator.accumulate(denom_inverse * (c - (a.square() + b.square())));
            a = b;
            b = c;
        }
    }
}

#[cfg(feature = "metal-runtime")]
impl ComponentProver<MetalBackend> for WideFibonacciBenchmarkComponent {
    fn evaluate_constraint_quotients_on_domain(
        &self,
        trace: &Trace<'_, MetalBackend>,
        evaluation_accumulator: &mut DomainEvaluationAccumulator<MetalBackend>,
    ) {
        if self.n_constraints() == 0 {
            return;
        }

        let eval_domain = CanonicCoset::new(self.max_constraint_log_degree_bound()).circle_domain();
        let trace_domain = CanonicCoset::new(self.log_n_rows);
        let trace_columns = &trace.polys[MAIN_TRACE_IDX];
        assert_eq!(
            trace_columns.len(),
            self.n_columns,
            "wide fibonacci benchmark trace width drifted"
        );

        if trace_columns.iter().any(|poly| poly.coeffs.is_none())
            && trace_columns
                .iter()
                .any(|poly| poly.evals.domain != eval_domain)
        {
            panic!(
                "wide fibonacci benchmark requires stored trace coefficients for eval-domain extension"
            );
        }

        let log_expand = eval_domain.log_size() - trace_domain.log_size();
        let mut denominator_inverses = (0..(1 << log_expand))
            .map(|index| coset_vanishing(trace_domain.coset(), eval_domain.at(index)).inverse())
            .collect::<Vec<_>>();
        stwo::core::utils::bit_reverse(&mut denominator_inverses);

        let [mut accum] =
            evaluation_accumulator.columns([(eval_domain.log_size(), self.n_constraints())]);
        accum.random_coeff_powers.reverse();
        let quotients = if trace_columns
            .iter()
            .all(|poly| poly.evals.domain == eval_domain)
        {
            let trace1_evaluation_refs = trace_columns
                .iter()
                .map(|poly| &poly.evals.values)
                .collect::<Vec<_>>();
            accumulate_wide_fibonacci_quotients(MetalWideFibonacciQuotientRequest {
                trace_evaluations: &trace1_evaluation_refs,
                random_coeff_powers: &accum.random_coeff_powers,
                denominator_inverses: &denominator_inverses,
                domain_log_size: trace_domain.log_size(),
                eval_domain_log_size: eval_domain.log_size(),
            })
        } else {
            let twiddles = MetalBackend::precompute_twiddles(eval_domain.half_coset);
            let coeffs = trace_columns
                .iter()
                .map(|poly| {
                    poly.coeffs.as_ref().expect(
                        "wide fibonacci benchmark requires stored trace coefficients for eval-domain extension",
                    )
                })
                .collect::<Vec<_>>();
            let trace_batch = evaluate_polys_on_domain_batch(&coeffs, eval_domain, &twiddles)
                .expect("wide-fibonacci trace batch evaluation should stay on Metal");
            accumulate_wide_fibonacci_quotients_from_batch(MetalWideFibonacciBatchQuotientRequest {
                trace_evaluations: &trace_batch,
                random_coeff_powers: &accum.random_coeff_powers,
                denominator_inverses: &denominator_inverses,
                domain_log_size: trace_domain.log_size(),
                eval_domain_log_size: eval_domain.log_size(),
            })
        }
        .expect(
            "wide-fibonacci quotient accumulation should succeed through the native Metal path",
        );
        let quotient_columns = quotients.into_coordinate_base_columns();
        for (dst, src) in accum.col.columns.iter_mut().zip(quotient_columns) {
            *dst = src;
        }
    }
}

#[cfg(feature = "metal-runtime")]
#[derive(Clone)]
struct SampleResult {
    total_elapsed_ms: f64,
    prove_elapsed_ms: f64,
    verify_elapsed_ms: f64,
    prove_breakdown: Option<ProveBreakdown>,
    proof_metadata: ProofMetadata,
    sentinel: WideFibonacciSentinel,
}

#[cfg(feature = "metal-runtime")]
fn run_one_sample(
    input_a_host: &[BaseField],
    input_b_host: &[BaseField],
    log_n_instances: u32,
    n_columns: usize,
    benchmark_lane: &str,
) -> SampleResult {
    match benchmark_lane {
        "generated-metal" => {
            run_one_generated_sample(input_a_host, input_b_host, log_n_instances, n_columns)
        }
        "generic-metal" => {
            run_one_generic_sample(input_a_host, input_b_host, log_n_instances, n_columns)
        }
        other => panic!("unsupported wide_fibonacci benchmark lane: {other}"),
    }
}

#[cfg(feature = "metal-runtime")]
fn run_one_generated_sample(
    input_a_host: &[BaseField],
    input_b_host: &[BaseField],
    log_n_instances: u32,
    n_columns: usize,
) -> SampleResult {
    let config = PcsConfig::default();

    let prove_start = Instant::now();
    let (component, proof, sentinel, prove_breakdown) = prove_wide_fibonacci_blake(
        input_a_host,
        input_b_host,
        log_n_instances,
        n_columns,
        config,
    );
    let prove_elapsed_ms = prove_start.elapsed().as_secs_f64() * 1000.0;

    let proof_metadata = proof_metadata(&proof);

    let verify_start = Instant::now();
    verify_wide_fibonacci_blake(&component, &proof).expect("wide fibonacci proof should verify");
    let verify_elapsed_ms = verify_start.elapsed().as_secs_f64() * 1000.0;

    SampleResult {
        total_elapsed_ms: prove_elapsed_ms + verify_elapsed_ms,
        prove_elapsed_ms,
        verify_elapsed_ms,
        prove_breakdown: Some(prove_breakdown),
        proof_metadata,
        sentinel,
    }
}

#[cfg(feature = "metal-runtime")]
fn run_one_generic_sample(
    input_a_host: &[BaseField],
    input_b_host: &[BaseField],
    log_n_instances: u32,
    n_columns: usize,
) -> SampleResult {
    assert_eq!(
        n_columns, WIDE_FIBONACCI_COLUMNS,
        "generic-metal wide fibonacci lane is currently defined only for the upstream 100-column shape"
    );

    let inputs = input_a_host
        .iter()
        .zip(input_b_host.iter())
        .map(|(&a, &b)| FibInput { a, b })
        .collect::<Vec<_>>();

    let boundary = declare_exemplar_metal_workload_boundary(
        MetalExecutionIntent::PreferMetal,
        "fibonacci_example",
    )
    .expect("generic-metal benchmark lane requires the registered fibonacci workload boundary");
    let catalog = acceptance_bridge_catalog(&boundary)
        .expect("generic-metal benchmark lane requires a Metal-capable acceptance bridge catalog");

    let trace_generation_start = Instant::now();
    let trace = generate_trace::<WIDE_FIBONACCI_COLUMNS, MetalBackend>(&inputs);
    let trace_generation_ms = trace_generation_start.elapsed().as_secs_f64() * 1000.0;

    let component = WideFibonacciComponent::new(
        &mut TraceLocationAllocator::default(),
        WideFibonacciEval::<WIDE_FIBONACCI_COLUMNS> {
            log_n_rows: log_n_instances,
        },
        SecureField::from_u32_unchecked(0, 0, 0, 0),
    );
    let metal_component = catalog.framework(&component);

    let prove_start = Instant::now();
    let proof =
        prove_single_trace_component_via_backend_blake2s(trace, &metal_component, log_n_instances)
            .expect("generic-metal wide fibonacci benchmark should prove successfully");
    let prove_elapsed_ms = prove_start.elapsed().as_secs_f64() * 1000.0;

    let proof_metadata = proof_metadata(&proof);

    let verify_start = Instant::now();
    verify_single_trace_component_blake2s(&metal_component, &proof)
        .expect("generic-metal wide fibonacci proof should verify");
    let verify_elapsed_ms = verify_start.elapsed().as_secs_f64() * 1000.0;

    SampleResult {
        total_elapsed_ms: prove_elapsed_ms + verify_elapsed_ms,
        prove_elapsed_ms,
        verify_elapsed_ms,
        prove_breakdown: Some(ProveBreakdown {
            setup_and_preprocessed_commit_ms: 0.0,
            trace_generation_ms,
            trace_commit_ms: 0.0,
            trace_commit_interpolation_ms: 0.0,
            trace_commit_extension_ms: 0.0,
            trace_commit_merkle_ms: 0.0,
            prove_core_ms: prove_elapsed_ms,
            prove_core_evaluation_program_v1_ms: 0.0,
            prove_core_composition_generation_ms: 0.0,
            prove_core_composition_commit_ms: 0.0,
            prove_core_prove_values_ms: 0.0,
            prove_core_sanity_check_ms: 0.0,
        }),
        proof_metadata,
        sentinel: sentinel_from_inputs(input_a_host, input_b_host, n_columns),
    }
}

#[cfg(feature = "metal-runtime")]
#[derive(Clone, Copy)]
struct ProveBreakdown {
    setup_and_preprocessed_commit_ms: f64,
    trace_generation_ms: f64,
    trace_commit_ms: f64,
    trace_commit_interpolation_ms: f64,
    trace_commit_extension_ms: f64,
    trace_commit_merkle_ms: f64,
    prove_core_ms: f64,
    prove_core_evaluation_program_v1_ms: f64,
    prove_core_composition_generation_ms: f64,
    prove_core_composition_commit_ms: f64,
    prove_core_prove_values_ms: f64,
    prove_core_sanity_check_ms: f64,
}

#[cfg(feature = "metal-runtime")]
fn prove_wide_fibonacci_blake(
    input_a_host: &[BaseField],
    input_b_host: &[BaseField],
    log_n_instances: u32,
    n_columns: usize,
    config: PcsConfig,
) -> (
    WideFibonacciBenchmarkComponent,
    StarkProof<Blake2sMerkleHasher>,
    WideFibonacciSentinel,
    ProveBreakdown,
) {
    let input_len = 1usize << log_n_instances;
    assert_eq!(input_a_host.len(), input_len);
    assert_eq!(input_b_host.len(), input_len);

    let benchmark_boundary = declare_wide_fibonacci_benchmark_boundary(
        MetalExecutionIntent::PreferMetal,
        benchmark_target(
            log_n_instances,
            n_columns,
            MetalBenchmarkOperation::ProveVerify,
        ),
    )
    .expect("wide-fibonacci prove benchmark boundary should be declared");

    let setup_start = Instant::now();
    let twiddles = MetalBackend::precompute_twiddles(
        CanonicCoset::new(log_n_instances + 1 + config.fri_config.log_blowup_factor)
            .circle_domain()
            .half_coset,
    );

    let prover_channel = &mut Blake2sChannel::default();
    let mut commitment_scheme =
        CommitmentSchemeProver::<MetalBackend, Blake2sMerkleChannel>::new(config, &twiddles);

    let mut tree_builder = commitment_scheme.tree_builder();
    tree_builder.extend_evals(vec![]);
    tree_builder.commit(prover_channel);

    commitment_scheme.set_store_polynomials_coefficients();
    let setup_and_preprocessed_commit_ms = setup_start.elapsed().as_secs_f64() * 1000.0;

    let trace_generation_start = Instant::now();
    let (native_trace, trace, sentinel) = generate_wide_fibonacci_trace_evaluations(
        &benchmark_boundary,
        input_a_host,
        input_b_host,
        log_n_instances,
        n_columns,
    );
    let trace_generation_ms = trace_generation_start.elapsed().as_secs_f64() * 1000.0;

    let trace_commit_start = Instant::now();
    let trace_commit_breakdown =
        commit_trace_with_breakdown(&mut commitment_scheme, prover_channel, trace, &twiddles);
    let trace_commit_ms = trace_commit_start.elapsed().as_secs_f64() * 1000.0;

    let component = WideFibonacciBenchmarkComponent::new(log_n_instances, n_columns);
    let prove_core_start = Instant::now();
    let (proof, prove_core_breakdown) = benchmark_boundary.execute_prove_core(
        &native_trace,
        &[&component],
        prover_channel,
        commitment_scheme,
    )
    .expect("wide fibonacci prove should succeed");
    let prove_core_ms = prove_core_start.elapsed().as_secs_f64() * 1000.0;

    (
        component,
        proof,
        sentinel,
        ProveBreakdown {
            setup_and_preprocessed_commit_ms,
            trace_generation_ms,
            trace_commit_ms,
            trace_commit_interpolation_ms: trace_commit_breakdown.interpolation_ms,
            trace_commit_extension_ms: trace_commit_breakdown.extension_ms,
            trace_commit_merkle_ms: trace_commit_breakdown.merkle_ms,
            prove_core_ms,
            prove_core_evaluation_program_v1_ms: prove_core_breakdown.evaluation_program_v1_ms,
            prove_core_composition_generation_ms: prove_core_breakdown.composition_generation_ms,
            prove_core_composition_commit_ms: prove_core_breakdown.composition_commit_ms,
            prove_core_prove_values_ms: prove_core_breakdown.prove_values_ms,
            prove_core_sanity_check_ms: prove_core_breakdown.sanity_check_ms,
        },
    )
}

#[cfg(feature = "metal-runtime")]
fn prove_single_trace_component_via_backend_blake2s<C>(
    trace: Vec<CircleEvaluation<MetalBackend, BaseField, BitReversedOrder>>,
    component: &C,
    max_trace_log_size: u32,
) -> Result<StarkProof<Blake2sMerkleHasher>, ProvingError>
where
    C: ComponentProver<MetalBackend> + Component,
{
    let config = PcsConfig::default();
    let twiddles = MetalBackend::precompute_twiddles(
        CanonicCoset::new(max_trace_log_size + 1 + config.fri_config.log_blowup_factor)
            .circle_domain()
            .half_coset,
    );

    let prover_channel = &mut Blake2sChannel::default();
    let mut commitment_scheme =
        CommitmentSchemeProver::<MetalBackend, Blake2sMerkleChannel>::new(config, &twiddles);

    let mut tree_builder = commitment_scheme.tree_builder();
    tree_builder.extend_evals(vec![]);
    tree_builder.commit(prover_channel);

    let mut tree_builder = commitment_scheme.tree_builder();
    tree_builder.extend_evals(trace);
    tree_builder.commit(prover_channel);

    prove::<MetalBackend, Blake2sMerkleChannel>(&[component], prover_channel, commitment_scheme)
}

#[cfg(feature = "metal-runtime")]
fn verify_single_trace_component_blake2s<C>(
    component: &C,
    proof: &StarkProof<Blake2sMerkleHasher>,
) -> Result<(), VerificationError>
where
    C: Component,
{
    let verifier_channel = &mut Blake2sChannel::default();
    let commitment_scheme =
        &mut CommitmentSchemeVerifier::<Blake2sMerkleChannel>::new(proof.config);
    let sizes = component.trace_log_degree_bounds();
    commitment_scheme.commit(proof.commitments[0], &sizes[0], verifier_channel);
    commitment_scheme.commit(proof.commitments[1], &sizes[1], verifier_channel);
    verify(
        &[component as &dyn Component],
        verifier_channel,
        commitment_scheme,
        proof.clone(),
    )
}

#[cfg(feature = "metal-runtime")]
struct TraceCommitBreakdown {
    interpolation_ms: f64,
    extension_ms: f64,
    merkle_ms: f64,
}

#[cfg(feature = "metal-runtime")]
fn build_native_standard_blake2s_merkle_prover(
    columns: Vec<&MetalBaseFieldVec>,
    lifting_log_size: u32,
) -> Option<MerkleProverLifted<MetalBackend, Blake2sMerkleHasher>> {
    if columns.is_empty() {
        return Some(
            MerkleProverLifted::<MetalBackend, Blake2sMerkleHasher>::commit(
                Vec::new(),
                lifting_log_size,
            ),
        );
    }

    let leaves = <MetalBackend as MerkleOpsLifted<Blake2sMerkleHasher>>::build_leaves(
        &columns.iter().copied().collect::<Vec<_>>(),
        lifting_log_size,
    );
    let mut layers = vec![leaves.clone()];
    while layers.last()?.len() > 1 {
        let next = <MetalBackend as MerkleOpsLifted<Blake2sMerkleHasher>>::build_next_layer(
            layers.last().expect("Merkle layers should be non-empty"),
        );
        layers.push(next);
    }

    layers.reverse();
    Some(MerkleProverLifted { layers })
}

#[cfg(feature = "metal-runtime")]
fn commit_trace_with_breakdown(
    commitment_scheme: &mut CommitmentSchemeProver<'_, MetalBackend, Blake2sMerkleChannel>,
    prover_channel: &mut Blake2sChannel,
    trace: Vec<CircleEvaluation<MetalBackend, BaseField, BitReversedOrder>>,
    twiddles: &stwo::prover::poly::twiddles::TwiddleTree<MetalBackend>,
) -> TraceCommitBreakdown {
    let interpolation_start = Instant::now();
    let trace_polynomials = MetalBackend::interpolate_columns(trace, twiddles);
    let interpolation_ms = interpolation_start.elapsed().as_secs_f64() * 1000.0;

    let extension_start = Instant::now();
    let trace_polynomials = MetalBackend::evaluate_polynomials(
        trace_polynomials,
        commitment_scheme.config.fri_config.log_blowup_factor,
        twiddles,
        commitment_scheme.store_polynomials_coefficients,
        &commitment_scheme.base_column_pool,
    );
    let extension_ms = extension_start.elapsed().as_secs_f64() * 1000.0;

    let merkle_start = Instant::now();
    let max_log_domain_size = trace_polynomials
        .iter()
        .map(|poly| poly.evals.domain.log_size())
        .max()
        .unwrap_or_default();
    let lifting_log_size = commitment_scheme
        .config
        .lifting_log_size
        .unwrap_or(max_log_domain_size);
    let commitment = build_native_standard_blake2s_merkle_prover(
        trace_polynomials
            .iter()
            .map(|poly| &poly.evals.values)
            .collect(),
        lifting_log_size,
    )
    .unwrap_or_else(|| {
        MerkleProverLifted::<MetalBackend, Blake2sMerkleHasher>::commit(
            trace_polynomials
                .iter()
                .map(|poly| &poly.evals.values)
                .collect(),
            lifting_log_size,
        )
    });
    let merkle_ms = merkle_start.elapsed().as_secs_f64() * 1000.0;

    let tree = CommitmentTreeProver {
        polynomials: trace_polynomials,
        commitment,
    };
    commitment_scheme.commit_tree(MaybeOwned::Owned(tree), prover_channel);

    TraceCommitBreakdown {
        interpolation_ms,
        extension_ms,
        merkle_ms,
    }
}

#[cfg(all(
    test,
    feature = "metal-runtime",
    target_os = "macos",
    target_arch = "aarch64"
))]
mod native_merkle_tests {
    use super::build_native_standard_blake2s_merkle_prover;
    use stwo::core::fields::m31::BaseField;
    use stwo::core::vcs_lifted::blake2_merkle::Blake2sMerkleHasher;
    use stwo::prover::vcs_lifted::prover::MerkleProverLifted;
    use stwo_metal::{metal_runtime_support, MetalBackend, MetalBaseFieldVec, MetalRuntimeSupport};

    fn require_metal_runtime() {
        assert_eq!(metal_runtime_support(), MetalRuntimeSupport::Available);
    }

    fn base_column(len: usize, seed: u32) -> Vec<BaseField> {
        (0..len)
            .map(|i| BaseField::from_u32_unchecked(seed + i as u32))
            .collect()
    }

    #[test]
    fn native_standard_blake2s_tree_matches_existing_metal_merkle_shape() {
        require_metal_runtime();

        let columns = vec![
            MetalBaseFieldVec::from_vec(base_column(1 << 8, 3)),
            MetalBaseFieldVec::from_vec(base_column(1 << 8, 101)),
            MetalBaseFieldVec::from_vec(base_column(1 << 8, 211)),
            MetalBaseFieldVec::from_vec(base_column(1 << 8, 307)),
        ];
        let refs = columns.iter().collect::<Vec<_>>();

        let expected = MerkleProverLifted::<MetalBackend, Blake2sMerkleHasher>::commit(
            refs.iter().copied().collect(),
            8,
        );
        let actual = build_native_standard_blake2s_merkle_prover(refs.iter().copied().collect(), 8)
            .expect("native standard Blake2s tree should build");

        assert_eq!(actual.layers, expected.layers);
    }
}

#[cfg(feature = "metal-runtime")]
fn verify_wide_fibonacci_blake(
    component: &WideFibonacciBenchmarkComponent,
    proof: &StarkProof<Blake2sMerkleHasher>,
) -> Result<(), stwo::core::verifier::VerificationError> {
    let verifier_channel = &mut Blake2sChannel::default();
    let commitment_scheme =
        &mut CommitmentSchemeVerifier::<Blake2sMerkleChannel>::new(proof.config);
    let sizes = component.trace_log_degree_bounds();
    commitment_scheme.commit(
        proof.commitments[PREPROCESSED_TRACE_IDX],
        &sizes[PREPROCESSED_TRACE_IDX],
        verifier_channel,
    );
    commitment_scheme.commit(
        proof.commitments[MAIN_TRACE_IDX],
        &sizes[MAIN_TRACE_IDX],
        verifier_channel,
    );
    verify(
        &[component as &dyn Component],
        verifier_channel,
        commitment_scheme,
        proof.clone(),
    )
}

#[cfg(feature = "metal-runtime")]
fn sentinel_from_inputs(
    input_a_host: &[BaseField],
    input_b_host: &[BaseField],
    n_columns: usize,
) -> WideFibonacciSentinel {
    let mut last_first = input_b_host[0];
    let mut prev_first = input_a_host[0];
    for _ in 2..n_columns {
        let next = prev_first.square() + last_first.square();
        prev_first = last_first;
        last_first = next;
    }

    let mut last_last = input_b_host[input_b_host.len() - 1];
    let mut prev_last = input_a_host[input_a_host.len() - 1];
    for _ in 2..n_columns {
        let next = prev_last.square() + last_last.square();
        prev_last = last_last;
        last_last = next;
    }

    WideFibonacciSentinel {
        first_column_first_value: input_a_host[0].0,
        second_column_first_value: input_b_host[0].0,
        last_column_first_value: last_first.0,
        last_column_last_value: last_last.0,
    }
}

#[cfg(feature = "metal-runtime")]
fn generate_wide_fibonacci_trace_evaluations(
    benchmark_boundary: &MetalWideFibonacciBenchmarkBoundary,
    input_a_host: &[BaseField],
    input_b_host: &[BaseField],
    log_n_instances: u32,
    n_columns: usize,
) -> (
    MetalWideFibonacciTrace,
    Vec<CircleEvaluation<MetalBackend, BaseField, BitReversedOrder>>,
    WideFibonacciSentinel,
) {
    let input_len = 1usize << log_n_instances;
    let witness_inputs = benchmark_boundary
        .workload_boundary()
        .ingest_cpu_wide_fibonacci_witness(input_a_host, input_b_host, n_columns as u32)
        .expect("wide-fibonacci prove witness handoff should be accepted");
    let trace = witness_inputs
        .generate_trace()
        .expect("wide-fibonacci prove benchmark should generate the trace through Metal");
    let sentinel = sentinel_from_metal_trace(&trace, input_len, n_columns);
    let trace_evaluations = trace.to_metal_evaluations();

    (trace, trace_evaluations, sentinel)
}

#[cfg(feature = "metal-runtime")]
fn sentinel_from_metal_trace(
    trace: &MetalWideFibonacciTrace,
    input_len: usize,
    n_columns: usize,
) -> WideFibonacciSentinel {
    WideFibonacciSentinel {
        first_column_first_value: trace.value(0, 0).0,
        second_column_first_value: trace.value(1, 0).0,
        last_column_first_value: trace.value(n_columns - 1, 0).0,
        last_column_last_value: trace.value(n_columns - 1, input_len - 1).0,
    }
}

#[cfg(feature = "metal-runtime")]
fn proof_metadata(proof: &StarkProof<Blake2sMerkleHasher>) -> ProofMetadata {
    let size_breakdown = proof.size_breakdown_estimate();
    ProofMetadata {
        size_estimate_bytes: proof.size_estimate(),
        commitments: proof.commitments.len(),
        security_bits: proof.config.security_bits(),
        fri_queries: proof.config.fri_config.n_queries,
        pow_bits: proof.config.pow_bits,
        size_breakdown_bytes: ProofSizeBreakdownMetadata::from(size_breakdown),
    }
}

#[cfg(feature = "metal-runtime")]
impl From<StarkProofSizeBreakdown> for ProofSizeBreakdownMetadata {
    fn from(value: StarkProofSizeBreakdown) -> Self {
        Self {
            oods_samples: value.oods_samples,
            queries_values: value.queries_values,
            fri_samples: value.fri_samples,
            fri_decommitments: value.fri_decommitments,
            trace_decommitments: value.trace_decommitments,
        }
    }
}
