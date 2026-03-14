//! Wide Fibonacci SIMD prove/verify benchmark.
//!
//! Runs the same wide-fibonacci workload (100 columns, 98 constraints) through
//! the standard stwo `prove()` path on SimdBackend for comparison with the
//! Metal V1 runtime.

use std::time::Instant;

use itertools::Itertools;
use num_traits::{One, Zero};
use serde::Serialize;
use stwo::core::air::Component;
use stwo::core::channel::Blake2sChannel;
use stwo::core::fields::m31::BaseField;
use stwo::core::fields::qm31::SecureField;
use stwo::core::pcs::{CommitmentSchemeVerifier, PcsConfig};
use stwo::core::poly::circle::CanonicCoset;
use stwo::core::vcs_lifted::blake2_merkle::Blake2sMerkleChannel;
use stwo::core::verifier::verify;
use stwo::prover::backend::simd::SimdBackend;
use stwo::prover::poly::circle::PolyOps;
use stwo::prover::{prove, CommitmentSchemeProver};
use stwo_constraint_framework::{FrameworkComponent, TraceLocationAllocator};
use stwo_examples::wide_fibonacci::{generate_trace, FibInput, WideFibonacciEval};
use stwo_metal_standalone_benchmarks::support::{
    env_flag, env_or, env_u32, env_usize, epoch_ms, required_env_path, runner_metadata,
    summarize, write_json, RunnerMetadata, SummaryStats,
};

const BENCHMARK_ID: &str = "wide_fibonacci_simd_prove_verify";
const RESULT_SCHEMA_VERSION: u32 = 1;
const DEFAULT_LOG_N_INSTANCES: u32 = 20;
const DEFAULT_N_COLUMNS: usize = 100;
const DEFAULT_WARMUPS: usize = 1;
const DEFAULT_SAMPLES: usize = 3;

// ---------------------------------------------------------------------------
// Output structs
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct BenchmarkResult {
    schema_version: u32,
    benchmark_id: String,
    benchmark_lane: String,
    status: String,
    classification: String,
    git_commit: String,
    started_at_epoch_ms: u128,
    completed_at_epoch_ms: u128,
    runner: RunnerMetadata,
    workload: WorkloadMetadata,
    timings: TimingMetadata,
    proof: Option<ProofMetadata>,
}

#[derive(Serialize)]
struct WorkloadMetadata {
    family: String,
    backend: String,
    channel: String,
    operation: String,
    log_n_instances: u32,
    instances: u64,
    n_columns: usize,
    warmup_iterations: usize,
    sample_iterations: usize,
}

#[derive(Serialize)]
struct TimingMetadata {
    samples_ms: Vec<f64>,
    summary_ms: Option<SummaryStats>,
    prove_samples_ms: Vec<f64>,
    prove_summary_ms: Option<SummaryStats>,
    verify_samples_ms: Vec<f64>,
    verify_summary_ms: Option<SummaryStats>,
    trace_gen_samples_ms: Vec<f64>,
    trace_gen_summary_ms: Option<SummaryStats>,
    trace_commit_samples_ms: Vec<f64>,
    trace_commit_summary_ms: Option<SummaryStats>,
    prove_call_samples_ms: Vec<f64>,
    prove_call_summary_ms: Option<SummaryStats>,
}

#[derive(Clone, Serialize)]
struct ProofMetadata {
    size_estimate_bytes: usize,
    commitments: usize,
    security_bits: u32,
    fri_queries: usize,
    pow_bits: u32,
}

// ---------------------------------------------------------------------------
// Sample
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct SampleResult {
    total_ms: f64,
    prove_ms: f64,
    verify_ms: f64,
    trace_gen_ms: f64,
    trace_commit_ms: f64,
    prove_call_ms: f64,
    proof_metadata: ProofMetadata,
}

fn generate_test_inputs(log_n_instances: u32) -> Vec<FibInput> {
    (0..1u64 << log_n_instances)
        .map(|i| FibInput {
            a: BaseField::one(),
            b: BaseField::from_u32_unchecked(i as u32),
        })
        .collect_vec()
}

fn run_one_sample<const N: usize>(log_n_instances: u32) -> SampleResult {
    let config = PcsConfig::default();
    let start = Instant::now();

    // Setup.
    let twiddles = SimdBackend::precompute_twiddles(
        CanonicCoset::new(log_n_instances + 1 + config.fri_config.log_blowup_factor)
            .circle_domain()
            .half_coset,
    );
    let prover_channel = &mut Blake2sChannel::default();
    let mut commitment_scheme =
        CommitmentSchemeProver::<SimdBackend, Blake2sMerkleChannel>::new(config, &twiddles);

    // Empty preprocessed tree.
    let mut tree_builder = commitment_scheme.tree_builder();
    tree_builder.extend_evals(vec![]);
    tree_builder.commit(prover_channel);

    // Generate trace.
    let trace_gen_start = Instant::now();
    let trace = generate_trace::<N, SimdBackend>(&generate_test_inputs(log_n_instances));
    let trace_gen_ms = trace_gen_start.elapsed().as_secs_f64() * 1000.0;

    // Commit trace.
    let trace_commit_start = Instant::now();
    let mut tree_builder = commitment_scheme.tree_builder();
    tree_builder.extend_evals(trace);
    tree_builder.commit(prover_channel);
    let trace_commit_ms = trace_commit_start.elapsed().as_secs_f64() * 1000.0;

    // Prove.
    let component = FrameworkComponent::new(
        &mut TraceLocationAllocator::default(),
        WideFibonacciEval::<N> { log_n_rows: log_n_instances },
        SecureField::zero(),
    );

    let prove_call_start = Instant::now();
    let proof = prove::<SimdBackend, Blake2sMerkleChannel>(
        &[&component],
        prover_channel,
        commitment_scheme,
    )
    .expect("SIMD prove should succeed");
    let prove_call_ms = prove_call_start.elapsed().as_secs_f64() * 1000.0;
    let prove_ms = start.elapsed().as_secs_f64() * 1000.0;

    // Verify.
    let verify_start = Instant::now();
    let verifier_channel = &mut Blake2sChannel::default();
    let commitment_scheme_verifier =
        &mut CommitmentSchemeVerifier::<Blake2sMerkleChannel>::new(proof.config);
    let sizes = component.trace_log_degree_bounds();
    commitment_scheme_verifier.commit(proof.commitments[0], &sizes[0], verifier_channel);
    commitment_scheme_verifier.commit(proof.commitments[1], &sizes[1], verifier_channel);
    verify(
        &[&component as &dyn Component],
        verifier_channel,
        commitment_scheme_verifier,
        proof.clone(),
    )
    .expect("SIMD verification should succeed");
    let verify_ms = verify_start.elapsed().as_secs_f64() * 1000.0;
    let total_ms = start.elapsed().as_secs_f64() * 1000.0;

    SampleResult {
        total_ms,
        prove_ms,
        verify_ms,
        trace_gen_ms,
        trace_commit_ms,
        prove_call_ms,
        proof_metadata: ProofMetadata {
            size_estimate_bytes: proof.size_estimate(),
            commitments: proof.commitments.len(),
            security_bits: proof.config.security_bits(),
            fri_queries: proof.config.fri_config.n_queries,
            pow_bits: proof.config.pow_bits,
        },
    }
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    let output_path = required_env_path("STWO_BENCH_OUTPUT_JSON");
    let plan_only = env_flag("STWO_BENCH_PLAN_ONLY");
    let log_n_instances = env_u32("STWO_BENCH_LOG_N_INSTANCES", DEFAULT_LOG_N_INSTANCES);
    let _n_columns = DEFAULT_N_COLUMNS; // const generic, always 100
    let warmups = env_usize("STWO_BENCH_WARMUPS", DEFAULT_WARMUPS);
    let samples = env_usize("STWO_BENCH_SAMPLES", DEFAULT_SAMPLES);
    let instances = 1u64 << log_n_instances;
    let started_at = epoch_ms();
    let runner = runner_metadata();

    let classification = env_or("STWO_BENCH_CLASSIFICATION", "simd-baseline");
    let benchmark_lane = env_or("STWO_BENCH_LANE", "simd-cpu");
    let git_commit = env_or("STWO_BENCH_GIT_COMMIT", "unknown");

    let workload = WorkloadMetadata {
        family: "wide_fibonacci".to_string(),
        backend: "simd".to_string(),
        channel: "blake2s".to_string(),
        operation: "prove_verify".to_string(),
        log_n_instances,
        instances,
        n_columns: DEFAULT_N_COLUMNS,
        warmup_iterations: warmups,
        sample_iterations: samples,
    };

    if plan_only {
        let result = BenchmarkResult {
            schema_version: RESULT_SCHEMA_VERSION,
            benchmark_id: BENCHMARK_ID.to_string(),
            benchmark_lane,
            status: "planned".to_string(),
            classification,
            git_commit,
            started_at_epoch_ms: started_at,
            completed_at_epoch_ms: epoch_ms(),
            runner,
            workload,
            timings: TimingMetadata {
                samples_ms: vec![],
                summary_ms: None,
                prove_samples_ms: vec![],
                prove_summary_ms: None,
                verify_samples_ms: vec![],
                verify_summary_ms: None,
                trace_gen_samples_ms: vec![],
                trace_gen_summary_ms: None,
                trace_commit_samples_ms: vec![],
                trace_commit_summary_ms: None,
                prove_call_samples_ms: vec![],
                prove_call_summary_ms: None,
            },
            proof: None,
        };
        write_json(&output_path, &result);
        println!("{}", output_path.display());
        return;
    }

    // Warmup.
    for _ in 0..warmups {
        let _ = run_one_sample::<100>(log_n_instances);
    }

    // Timed samples.
    let sample_results: Vec<SampleResult> = (0..samples)
        .map(|_| run_one_sample::<100>(log_n_instances))
        .collect();

    let total_ms: Vec<f64> = sample_results.iter().map(|s| s.total_ms).collect();
    let prove_ms: Vec<f64> = sample_results.iter().map(|s| s.prove_ms).collect();
    let verify_ms: Vec<f64> = sample_results.iter().map(|s| s.verify_ms).collect();
    let trace_gen_ms: Vec<f64> = sample_results.iter().map(|s| s.trace_gen_ms).collect();
    let trace_commit_ms: Vec<f64> = sample_results.iter().map(|s| s.trace_commit_ms).collect();
    let prove_call_ms: Vec<f64> = sample_results.iter().map(|s| s.prove_call_ms).collect();

    let last = sample_results.last().expect("at least one sample");

    let result = BenchmarkResult {
        schema_version: RESULT_SCHEMA_VERSION,
        benchmark_id: BENCHMARK_ID.to_string(),
        benchmark_lane,
        status: "completed".to_string(),
        classification,
        git_commit,
        started_at_epoch_ms: started_at,
        completed_at_epoch_ms: epoch_ms(),
        runner,
        workload,
        timings: TimingMetadata {
            samples_ms: total_ms.clone(),
            summary_ms: Some(summarize(&total_ms)),
            prove_samples_ms: prove_ms.clone(),
            prove_summary_ms: Some(summarize(&prove_ms)),
            verify_samples_ms: verify_ms.clone(),
            verify_summary_ms: Some(summarize(&verify_ms)),
            trace_gen_samples_ms: trace_gen_ms.clone(),
            trace_gen_summary_ms: Some(summarize(&trace_gen_ms)),
            trace_commit_samples_ms: trace_commit_ms.clone(),
            trace_commit_summary_ms: Some(summarize(&trace_commit_ms)),
            prove_call_samples_ms: prove_call_ms.clone(),
            prove_call_summary_ms: Some(summarize(&prove_call_ms)),
        },
        proof: Some(last.proof_metadata.clone()),
    };

    write_json(&output_path, &result);
    println!("{}", output_path.display());
}
