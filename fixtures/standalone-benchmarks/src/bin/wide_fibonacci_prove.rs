#[cfg(feature = "metal-runtime")]
use std::time::Instant;

use serde::Serialize;
#[cfg(feature = "metal-runtime")]
use stwo::core::air::accumulation::PointEvaluationAccumulator;
#[cfg(feature = "metal-runtime")]
use stwo::core::air::Component;
#[cfg(feature = "metal-runtime")]
use stwo::core::channel::{Blake2sChannel, Channel};
#[cfg(feature = "metal-runtime")]
use stwo::core::constraints::coset_vanishing;
#[cfg(feature = "metal-runtime")]
use stwo::core::fields::m31::BaseField;
#[cfg(feature = "metal-runtime")]
use stwo::core::fields::qm31::{SecureField, SECURE_EXTENSION_DEGREE};
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
use stwo::core::verifier::{verify, PREPROCESSED_TRACE_IDX};
#[cfg(feature = "metal-runtime")]
use stwo::prover::poly::circle::CircleEvaluation;
#[cfg(feature = "metal-runtime")]
use stwo::prover::poly::circle::PolyOps;
#[cfg(feature = "metal-runtime")]
use stwo::prover::poly::BitReversedOrder;
#[cfg(feature = "metal-runtime")]
use stwo::prover::vcs_lifted::prover::MerkleProverLifted;
#[cfg(feature = "metal-runtime")]
use stwo::prover::{
    CommitmentSchemeProver, CommitmentTreeProver, ComponentProver, ComponentProvers,
    DomainEvaluationAccumulator, ProvingError, Trace,
};
#[cfg(feature = "metal-runtime")]
use stwo_metal::{
    accumulate_wide_fibonacci_quotients, declare_wide_fibonacci_benchmark_boundary, MetalBackend,
    MetalBenchmarkOperation, MetalBenchmarkReferencePlatform, MetalBenchmarkTarget,
    MetalExecutionIntent, MetalWideFibonacciBenchmarkBoundary, MetalWideFibonacciQuotientRequest,
    MetalWideFibonacciTrace,
};
#[cfg(feature = "metal-runtime")]
use stwo_metal_benchmark_bridge::stage_wide_fibonacci_prove_values;
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
    phase_samples_ms: Option<PhaseSampleTimings>,
    phase_summary_ms: Option<PhaseSummaryTimings>,
    prove_breakdown_samples_ms: Option<ProveBreakdownSampleTimings>,
    prove_breakdown_summary_ms: Option<ProveBreakdownSummaryTimings>,
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
struct ProveBreakdownSampleTimings {
    setup_and_preprocessed_commit_ms: Vec<f64>,
    trace_generation_ms: Vec<f64>,
    trace_commit_ms: Vec<f64>,
    trace_commit_interpolation_ms: Vec<f64>,
    trace_commit_extension_ms: Vec<f64>,
    trace_commit_merkle_ms: Vec<f64>,
    prove_core_ms: Vec<f64>,
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
    prove_core_composition_generation_ms: SummaryStats,
    prove_core_composition_commit_ms: SummaryStats,
    prove_core_prove_values_ms: SummaryStats,
    prove_core_sanity_check_ms: SummaryStats,
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
    let dependency_row = env_or("STWO_BENCH_DEPENDENCY_ROW", "metal-backend-e2e-v1");
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
                phase_samples_ms: None,
                phase_summary_ms: None,
                prove_breakdown_samples_ms: None,
                prove_breakdown_summary_ms: None,
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
                );
            }

            let mut sample_results = Vec::with_capacity(samples);
            for _ in 0..samples {
                sample_results.push(run_one_sample(
                    &input_a_host,
                    &input_b_host,
                    log_n_instances,
                    n_columns as usize,
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
            let setup_samples_ms = sample_results
                .iter()
                .map(|sample| sample.setup_and_preprocessed_commit_ms)
                .collect::<Vec<_>>();
            let trace_generation_samples_ms = sample_results
                .iter()
                .map(|sample| sample.trace_generation_ms)
                .collect::<Vec<_>>();
            let trace_commit_samples_ms = sample_results
                .iter()
                .map(|sample| sample.trace_commit_ms)
                .collect::<Vec<_>>();
            let trace_commit_interpolation_samples_ms = sample_results
                .iter()
                .map(|sample| sample.trace_commit_interpolation_ms)
                .collect::<Vec<_>>();
            let trace_commit_extension_samples_ms = sample_results
                .iter()
                .map(|sample| sample.trace_commit_extension_ms)
                .collect::<Vec<_>>();
            let trace_commit_merkle_samples_ms = sample_results
                .iter()
                .map(|sample| sample.trace_commit_merkle_ms)
                .collect::<Vec<_>>();
            let prove_core_samples_ms = sample_results
                .iter()
                .map(|sample| sample.prove_core_ms)
                .collect::<Vec<_>>();
            let prove_core_composition_generation_samples_ms = sample_results
                .iter()
                .map(|sample| sample.prove_core_composition_generation_ms)
                .collect::<Vec<_>>();
            let prove_core_composition_commit_samples_ms = sample_results
                .iter()
                .map(|sample| sample.prove_core_composition_commit_ms)
                .collect::<Vec<_>>();
            let prove_core_prove_values_samples_ms = sample_results
                .iter()
                .map(|sample| sample.prove_core_prove_values_ms)
                .collect::<Vec<_>>();
            let prove_core_sanity_check_samples_ms = sample_results
                .iter()
                .map(|sample| sample.prove_core_sanity_check_ms)
                .collect::<Vec<_>>();
            let throughput = if total_summary.mean > 0.0 {
                // elements per millisecond is numerically equal to Kelem/s
                Some(instances as f64 / total_summary.mean)
            } else {
                None
            };
            let last_sample = sample_results.last().expect("at least one sample");

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
                    summary_ms: Some(total_summary),
                    throughput_kelem_per_second: throughput,
                    phase_samples_ms: Some(PhaseSampleTimings {
                        prove_ms: prove_samples_ms,
                        verify_ms: verify_samples_ms,
                    }),
                    phase_summary_ms: Some(PhaseSummaryTimings {
                        prove_ms: prove_summary,
                        verify_ms: verify_summary,
                    }),
                    prove_breakdown_samples_ms: Some(ProveBreakdownSampleTimings {
                        setup_and_preprocessed_commit_ms: setup_samples_ms.clone(),
                        trace_generation_ms: trace_generation_samples_ms.clone(),
                        trace_commit_ms: trace_commit_samples_ms.clone(),
                        trace_commit_interpolation_ms: trace_commit_interpolation_samples_ms
                            .clone(),
                        trace_commit_extension_ms: trace_commit_extension_samples_ms.clone(),
                        trace_commit_merkle_ms: trace_commit_merkle_samples_ms.clone(),
                        prove_core_ms: prove_core_samples_ms.clone(),
                        prove_core_composition_generation_ms:
                            prove_core_composition_generation_samples_ms.clone(),
                        prove_core_composition_commit_ms: prove_core_composition_commit_samples_ms
                            .clone(),
                        prove_core_prove_values_ms: prove_core_prove_values_samples_ms.clone(),
                        prove_core_sanity_check_ms: prove_core_sanity_check_samples_ms.clone(),
                    }),
                    prove_breakdown_summary_ms: Some(ProveBreakdownSummaryTimings {
                        setup_and_preprocessed_commit_ms: summarize(&setup_samples_ms),
                        trace_generation_ms: summarize(&trace_generation_samples_ms),
                        trace_commit_ms: summarize(&trace_commit_samples_ms),
                        trace_commit_interpolation_ms: summarize(
                            &trace_commit_interpolation_samples_ms,
                        ),
                        trace_commit_extension_ms: summarize(&trace_commit_extension_samples_ms),
                        trace_commit_merkle_ms: summarize(&trace_commit_merkle_samples_ms),
                        prove_core_ms: summarize(&prove_core_samples_ms),
                        prove_core_composition_generation_ms: summarize(
                            &prove_core_composition_generation_samples_ms,
                        ),
                        prove_core_composition_commit_ms: summarize(
                            &prove_core_composition_commit_samples_ms,
                        ),
                        prove_core_prove_values_ms: summarize(&prove_core_prove_values_samples_ms),
                        prove_core_sanity_check_ms: summarize(&prove_core_sanity_check_samples_ms),
                    }),
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

        let owned_trace_evaluations;
        let trace1_evaluation_refs = if trace_columns
            .iter()
            .all(|poly| poly.evals.domain == eval_domain)
        {
            trace_columns
                .iter()
                .map(|poly| &poly.evals.values)
                .collect::<Vec<_>>()
        } else {
            let twiddles = MetalBackend::precompute_twiddles(eval_domain.half_coset);
            owned_trace_evaluations = trace_columns
                .iter()
                .map(|poly| poly.get_evaluation_on_domain(eval_domain, &twiddles))
                .collect::<Vec<_>>();
            owned_trace_evaluations
                .iter()
                .map(|column| &column.values)
                .collect::<Vec<_>>()
        };

        let log_expand = eval_domain.log_size() - trace_domain.log_size();
        let mut denominator_inverses = (0..(1 << log_expand))
            .map(|index| coset_vanishing(trace_domain.coset(), eval_domain.at(index)).inverse())
            .collect::<Vec<_>>();
        stwo::core::utils::bit_reverse(&mut denominator_inverses);

        let [mut accum] =
            evaluation_accumulator.columns([(eval_domain.log_size(), self.n_constraints())]);
        accum.random_coeff_powers.reverse();
        let quotients = accumulate_wide_fibonacci_quotients(MetalWideFibonacciQuotientRequest {
            trace_evaluations: &trace1_evaluation_refs,
            random_coeff_powers: &accum.random_coeff_powers,
            denominator_inverses: &denominator_inverses,
            domain_log_size: trace_domain.log_size(),
            eval_domain_log_size: eval_domain.log_size(),
        })
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
    setup_and_preprocessed_commit_ms: f64,
    trace_generation_ms: f64,
    trace_commit_ms: f64,
    trace_commit_interpolation_ms: f64,
    trace_commit_extension_ms: f64,
    trace_commit_merkle_ms: f64,
    prove_core_ms: f64,
    prove_core_composition_generation_ms: f64,
    prove_core_composition_commit_ms: f64,
    prove_core_prove_values_ms: f64,
    prove_core_sanity_check_ms: f64,
    proof_metadata: ProofMetadata,
    sentinel: WideFibonacciSentinel,
}

#[cfg(feature = "metal-runtime")]
fn run_one_sample(
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
        setup_and_preprocessed_commit_ms: prove_breakdown.setup_and_preprocessed_commit_ms,
        trace_generation_ms: prove_breakdown.trace_generation_ms,
        trace_commit_ms: prove_breakdown.trace_commit_ms,
        trace_commit_interpolation_ms: prove_breakdown.trace_commit_interpolation_ms,
        trace_commit_extension_ms: prove_breakdown.trace_commit_extension_ms,
        trace_commit_merkle_ms: prove_breakdown.trace_commit_merkle_ms,
        prove_core_ms: prove_breakdown.prove_core_ms,
        prove_core_composition_generation_ms: prove_breakdown.prove_core_composition_generation_ms,
        prove_core_composition_commit_ms: prove_breakdown.prove_core_composition_commit_ms,
        prove_core_prove_values_ms: prove_breakdown.prove_core_prove_values_ms,
        prove_core_sanity_check_ms: prove_breakdown.prove_core_sanity_check_ms,
        proof_metadata,
        sentinel,
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
    prove_core_composition_generation_ms: f64,
    prove_core_composition_commit_ms: f64,
    prove_core_prove_values_ms: f64,
    prove_core_sanity_check_ms: f64,
}

#[cfg(feature = "metal-runtime")]
#[derive(Clone, Copy)]
struct ProveCoreBreakdown {
    composition_generation_ms: f64,
    composition_commit_ms: f64,
    prove_values_ms: f64,
    sanity_check_ms: f64,
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
    let (trace, sentinel) = generate_wide_fibonacci_trace_evaluations(
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
    let (proof, prove_core_breakdown) = prove_with_breakdown(
        &benchmark_boundary,
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
            prove_core_composition_generation_ms: prove_core_breakdown.composition_generation_ms,
            prove_core_composition_commit_ms: prove_core_breakdown.composition_commit_ms,
            prove_core_prove_values_ms: prove_core_breakdown.prove_values_ms,
            prove_core_sanity_check_ms: prove_core_breakdown.sanity_check_ms,
        },
    )
}

#[cfg(feature = "metal-runtime")]
fn prove_with_breakdown(
    benchmark_boundary: &MetalWideFibonacciBenchmarkBoundary,
    components: &[&dyn ComponentProver<MetalBackend>],
    channel: &mut Blake2sChannel,
    mut commitment_scheme: CommitmentSchemeProver<'_, MetalBackend, Blake2sMerkleChannel>,
) -> Result<(StarkProof<Blake2sMerkleHasher>, ProveCoreBreakdown), ProvingError> {
    let n_preprocessed_columns = commitment_scheme.trees[PREPROCESSED_TRACE_IDX]
        .polynomials
        .len();
    let component_provers = ComponentProvers {
        components: components.to_vec(),
        n_preprocessed_columns,
    };
    let trace = commitment_scheme.trace();

    let random_coeff = channel.draw_secure_felt();

    let composition_generation_start = Instant::now();
    let composition_poly = component_provers.compute_composition_polynomial(random_coeff, &trace);
    let composition_generation_ms = composition_generation_start.elapsed().as_secs_f64() * 1000.0;

    let composition_commit_start = Instant::now();
    let mut tree_builder = commitment_scheme.tree_builder();
    let (left_comp_poly_half, right_comp_poly_half) = composition_poly.split_at_mid();
    tree_builder.extend_polys(left_comp_poly_half.into_coordinate_polys());
    tree_builder.extend_polys(right_comp_poly_half.into_coordinate_polys());
    tree_builder.commit(channel);
    let composition_commit_ms = composition_commit_start.elapsed().as_secs_f64() * 1000.0;

    let execution_authority = benchmark_boundary.workload_boundary().execution_authority();
    let prove_values_stage = stage_wide_fibonacci_prove_values(
        &execution_authority,
        components,
        channel,
        &commitment_scheme,
    );

    let prove_values_start = Instant::now();
    let commitment_scheme_proof =
        commitment_scheme.prove_values(prove_values_stage.sample_points, channel);
    let prove_values_ms = prove_values_start.elapsed().as_secs_f64() * 1000.0;
    let proof = StarkProof(commitment_scheme_proof.proof);

    let sanity_check_start = Instant::now();
    if extract_composition_oods_eval(
        &proof,
        prove_values_stage.oods_point,
        prove_values_stage.max_log_degree_bound,
    )
    .unwrap()
        != component_provers
            .components()
            .eval_composition_polynomial_at_point(
                prove_values_stage.oods_point,
                &proof.sampled_values,
                random_coeff,
                prove_values_stage.max_log_degree_bound,
            )
    {
        return Err(ProvingError::ConstraintsNotSatisfied);
    }
    let sanity_check_ms = sanity_check_start.elapsed().as_secs_f64() * 1000.0;

    Ok((
        proof,
        ProveCoreBreakdown {
            composition_generation_ms,
            composition_commit_ms,
            prove_values_ms,
            sanity_check_ms,
        },
    ))
}

#[cfg(feature = "metal-runtime")]
fn extract_composition_oods_eval<H: stwo::core::vcs_lifted::merkle_hasher::MerkleHasherLifted>(
    proof: &StarkProof<H>,
    oods_point: stwo::core::circle::CirclePoint<SecureField>,
    max_log_degree_bound: u32,
) -> Option<SecureField> {
    let [.., left_and_right_composition_mask] = &**proof.sampled_values else {
        return None;
    };
    let left_and_right_coordinate_evals: [SecureField; 2 * SECURE_EXTENSION_DEGREE] =
        left_and_right_composition_mask
            .iter()
            .map(|columns| {
                let &[eval] = &columns[..] else {
                    return None;
                };
                Some(eval)
            })
            .collect::<Option<Vec<_>>>()?
            .try_into()
            .ok()?;

    let (left_coordinate_evals, right_coordinate_evals) =
        left_and_right_coordinate_evals.split_at(SECURE_EXTENSION_DEGREE);
    let left_eval = SecureField::from_partial_evals(left_coordinate_evals.try_into().ok()?);
    let right_eval = SecureField::from_partial_evals(right_coordinate_evals.try_into().ok()?);
    Some(left_eval + oods_point.repeated_double(max_log_degree_bound - 1).x * right_eval)
}

#[cfg(feature = "metal-runtime")]
struct TraceCommitBreakdown {
    interpolation_ms: f64,
    extension_ms: f64,
    merkle_ms: f64,
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
    let commitment = MerkleProverLifted::<MetalBackend, Blake2sMerkleHasher>::commit(
        trace_polynomials
            .iter()
            .map(|poly| &poly.evals.values)
            .collect(),
        lifting_log_size,
    );
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
fn generate_wide_fibonacci_trace_evaluations(
    benchmark_boundary: &MetalWideFibonacciBenchmarkBoundary,
    input_a_host: &[BaseField],
    input_b_host: &[BaseField],
    log_n_instances: u32,
    n_columns: usize,
) -> (
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

    let trace = trace.to_metal_evaluations();

    (trace, sentinel)
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
