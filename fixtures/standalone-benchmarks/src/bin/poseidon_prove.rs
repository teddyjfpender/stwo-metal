#![cfg_attr(test, allow(unused_imports))]
#![cfg_attr(test, allow(dead_code))]

#[cfg(any(feature = "cuda-runtime", test))]
use core::ops::{Add, AddAssign, Mul, Sub};

#[cfg(any(feature = "cuda-runtime", test))]
use num_traits::One;
#[cfg(any(feature = "cuda-runtime", test))]
use num_traits::Zero;
use serde::Serialize;
#[cfg(feature = "cuda-runtime")]
use std::borrow::Cow;
#[cfg(feature = "cuda-runtime")]
use std::time::Instant;
#[cfg(any(feature = "cuda-runtime", test))]
use stwo::core::air::accumulation::PointEvaluationAccumulator;
#[cfg(any(feature = "cuda-runtime", test))]
use stwo::core::air::Component;
#[cfg(any(feature = "cuda-runtime", test))]
use stwo::core::channel::{Blake2sChannel, Channel, MerkleChannel};
#[cfg(any(feature = "cuda-runtime", test))]
use stwo::core::constraints::coset_vanishing;
#[cfg(any(feature = "cuda-runtime", test))]
use stwo::core::fields::m31::BaseField;
#[cfg(any(feature = "cuda-runtime", test))]
use stwo::core::fields::qm31::{SecureField, SECURE_EXTENSION_DEGREE};
#[cfg(any(feature = "cuda-runtime", test))]
use stwo::core::fields::FieldExpOps;
#[cfg(any(feature = "cuda-runtime", test))]
use stwo::core::pcs::utils::get_lifting_log_size;
#[cfg(any(feature = "cuda-runtime", test))]
use stwo::core::pcs::{CommitmentSchemeVerifier, PcsConfig, TreeVec};
#[cfg(any(feature = "cuda-runtime", test))]
use stwo::core::poly::circle::CanonicCoset;
#[cfg(any(feature = "cuda-runtime", test))]
use stwo::core::proof::{StarkProof, StarkProofSizeBreakdown};
#[cfg(any(feature = "cuda-runtime", test))]
use stwo::core::Fraction;
#[cfg(any(feature = "cuda-runtime", test))]
use stwo::core::utils::{bit_reverse, MaybeOwned};
#[cfg(any(feature = "cuda-runtime", test))]
use stwo::core::verifier::{verify, PREPROCESSED_TRACE_IDX};
#[cfg(test)]
use stwo::prover::backend::cpu::CpuBackend;
#[cfg(test)]
use stwo::prover::backend::Column;
#[cfg(any(feature = "cuda-runtime", test))]
use stwo::prover::poly::circle::CircleEvaluation;
#[cfg(any(feature = "cuda-runtime", test))]
use stwo::prover::poly::circle::PolyOps;
#[cfg(any(feature = "cuda-runtime", test))]
use stwo::prover::poly::BitReversedOrder;
#[cfg(any(feature = "cuda-runtime", test))]
use stwo::prover::vcs_lifted::prover::MerkleProverLifted;
#[cfg(any(feature = "cuda-runtime", test))]
use stwo::core::vcs_lifted::blake2_merkle::{Blake2sMerkleChannel, Blake2sMerkleHasher};
#[cfg(any(feature = "cuda-runtime", test))]
use stwo::prover::{
    CommitmentSchemeProver, CommitmentTreeProver, ComponentProver, ComponentProvers,
    DomainEvaluationAccumulator, ProvingError, Trace,
};
#[cfg(any(feature = "cuda-runtime", test))]
use stwo_constraint_framework::{
    relation, EvalAtRow, FrameworkComponent, FrameworkEval, RelationEntry, TraceLocationAllocator,
};
#[cfg(test)]
use stwo_constraint_framework::Relation;
#[cfg(feature = "cuda-runtime")]
use stwo_metal::{
    generate_poseidon_interaction_traces, generate_poseidon_traces,
        launch_constraint_quotients_on_domain, opaque_eval_ptr, BaseFieldVec,
        ConstraintQuotientEvalRequest, CudaBackend, CudaPoseidonLookupElementsAbiV1, SecureFieldVec,
        StwoCudaLookupElements16AbiV1, StwoCudaPoseidonEvalAbiV1, PoseidonInteractionTraceRequest,
        PoseidonTraceRequest,
};
use stwo_metal_standalone_benchmarks::support::{
    env_flag, env_or, env_u32, env_usize, epoch_ms, required_env_path, runner_metadata,
    write_json, RunnerMetadata, SummaryStats,
};
#[cfg(feature = "cuda-runtime")]
use stwo_metal_standalone_benchmarks::support::summarize;

const BENCHMARK_ID: &str = "poseidon_prove_verify_v1";
const RESULT_SCHEMA_VERSION: u32 = 1;
const DEFAULT_LOG_N_INSTANCES: u32 = 22;
const DEFAULT_WARMUPS: usize = 1;
const DEFAULT_SAMPLES: usize = 5;
const POSEIDON_TRACE_COLUMNS: usize = 142;

#[cfg(feature = "cuda-runtime")]
const POSEIDON_EVAL_TAG: &str = "poseidon_example";
#[cfg(feature = "cuda-runtime")]
const POSEIDON_EVAL_ID: u32 = fnv1a_eval_id(POSEIDON_EVAL_TAG);
#[cfg(any(feature = "cuda-runtime", test))]
const MAIN_TRACE_IDX: usize = 1;
#[cfg(any(feature = "cuda-runtime", test))]
const INTERACTION_TRACE_IDX: usize = 2;
#[cfg(any(feature = "cuda-runtime", test))]
const N_LOG_INSTANCES_PER_ROW: usize = 0;
#[cfg(any(feature = "cuda-runtime", test))]
const N_INSTANCES_PER_ROW: usize = 1 << N_LOG_INSTANCES_PER_ROW;
#[cfg(any(feature = "cuda-runtime", test))]
const N_STATE: usize = 16;
#[cfg(any(feature = "cuda-runtime", test))]
const N_PARTIAL_ROUNDS: usize = 14;
#[cfg(any(feature = "cuda-runtime", test))]
const N_HALF_FULL_ROUNDS: usize = 4;
#[cfg(any(feature = "cuda-runtime", test))]
const FULL_ROUNDS: usize = 2 * N_HALF_FULL_ROUNDS;
#[cfg(any(feature = "cuda-runtime", test))]
const N_COLUMNS_PER_REP: usize = N_STATE * (1 + FULL_ROUNDS) + N_PARTIAL_ROUNDS;
#[cfg(any(feature = "cuda-runtime", test))]
const N_COLUMNS: usize = N_INSTANCES_PER_ROW * N_COLUMNS_PER_REP;
#[cfg(any(feature = "cuda-runtime", test))]
const LOG_EXPAND: u32 = 2;
#[cfg(any(feature = "cuda-runtime", test))]
const EXTERNAL_ROUND_CONSTS: [[BaseField; N_STATE]; 2 * N_HALF_FULL_ROUNDS] =
    [[BaseField::from_u32_unchecked(1234); N_STATE]; 2 * N_HALF_FULL_ROUNDS];
#[cfg(any(feature = "cuda-runtime", test))]
const INTERNAL_ROUND_CONSTS: [BaseField; N_PARTIAL_ROUNDS] =
    [BaseField::from_u32_unchecked(1234); N_PARTIAL_ROUNDS];

#[cfg(feature = "cuda-runtime")]
const fn fnv1a_eval_id(value: &str) -> u32 {
    let bytes = value.as_bytes();
    let mut hash = 0x811c_9dc5u32;
    let mut idx = 0usize;
    while idx < bytes.len() {
        hash ^= bytes[idx] as u32;
        hash = hash.wrapping_mul(0x0100_0193);
        idx += 1;
    }
    hash
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
    sentinel: Option<PoseidonSentinel>,
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
    interaction_generation_ms: Vec<f64>,
    interaction_commit_ms: Vec<f64>,
    interaction_commit_interpolation_ms: Vec<f64>,
    interaction_commit_extension_ms: Vec<f64>,
    interaction_commit_merkle_ms: Vec<f64>,
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
    interaction_generation_ms: SummaryStats,
    interaction_commit_ms: SummaryStats,
    interaction_commit_interpolation_ms: SummaryStats,
    interaction_commit_extension_ms: SummaryStats,
    interaction_commit_merkle_ms: SummaryStats,
    prove_core_ms: SummaryStats,
    prove_core_composition_generation_ms: SummaryStats,
    prove_core_composition_commit_ms: SummaryStats,
    prove_core_prove_values_ms: SummaryStats,
    prove_core_sanity_check_ms: SummaryStats,
}

#[derive(Clone, Serialize)]
struct PoseidonSentinel {
    trace_first_column_first_value: u32,
    trace_last_column_first_value: u32,
    interaction_first_column_first_value: u32,
    claimed_sum_m31: [u32; 4],
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
    let warmups = env_usize("STWO_BENCH_WARMUPS", DEFAULT_WARMUPS);
    let samples = env_usize("STWO_BENCH_SAMPLES", DEFAULT_SAMPLES);
    let instances = 1u64 << log_n_instances;
    let started_at = epoch_ms();

    let runner = runner_metadata();
    let workload = WorkloadMetadata {
        family: "poseidon".to_string(),
        channel: "blake2s".to_string(),
        operation: "prove_verify".to_string(),
        log_n_instances,
        instances,
        n_columns: POSEIDON_TRACE_COLUMNS as u32,
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
                phase_samples_ms: None,
                phase_summary_ms: None,
                prove_breakdown_samples_ms: None,
                prove_breakdown_summary_ms: None,
            },
            proof: None,
            sentinel: None,
        }
    } else {
        #[cfg(not(feature = "cuda-runtime"))]
        panic!("poseidon_prove benchmark requires the cuda-runtime feature for non-plan execution");

        #[cfg(feature = "cuda-runtime")]
        {
            if runner.stwo_cuda_mode == "no-cuda" {
                panic!("poseidon_prove benchmark cannot run with STWO_CUDA_MODE=no-cuda");
            }

            for _ in 0..warmups {
                let _ = run_one_sample(log_n_instances);
            }

            let mut sample_results = Vec::with_capacity(samples);
            for _ in 0..samples {
                sample_results.push(run_one_sample(log_n_instances));
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
            let throughput = if total_summary.mean > 0.0 {
                Some(instances as f64 / total_summary.mean)
            } else {
                None
            };

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
            let interaction_generation_samples_ms = sample_results
                .iter()
                .map(|sample| sample.interaction_generation_ms)
                .collect::<Vec<_>>();
            let interaction_commit_samples_ms = sample_results
                .iter()
                .map(|sample| sample.interaction_commit_ms)
                .collect::<Vec<_>>();
            let interaction_commit_interpolation_samples_ms = sample_results
                .iter()
                .map(|sample| sample.interaction_commit_interpolation_ms)
                .collect::<Vec<_>>();
            let interaction_commit_extension_samples_ms = sample_results
                .iter()
                .map(|sample| sample.interaction_commit_extension_ms)
                .collect::<Vec<_>>();
            let interaction_commit_merkle_samples_ms = sample_results
                .iter()
                .map(|sample| sample.interaction_commit_merkle_ms)
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
                        trace_commit_interpolation_ms: trace_commit_interpolation_samples_ms.clone(),
                        trace_commit_extension_ms: trace_commit_extension_samples_ms.clone(),
                        trace_commit_merkle_ms: trace_commit_merkle_samples_ms.clone(),
                        interaction_generation_ms: interaction_generation_samples_ms.clone(),
                        interaction_commit_ms: interaction_commit_samples_ms.clone(),
                        interaction_commit_interpolation_ms: interaction_commit_interpolation_samples_ms
                            .clone(),
                        interaction_commit_extension_ms: interaction_commit_extension_samples_ms.clone(),
                        interaction_commit_merkle_ms: interaction_commit_merkle_samples_ms.clone(),
                        prove_core_ms: prove_core_samples_ms.clone(),
                        prove_core_composition_generation_ms: prove_core_composition_generation_samples_ms
                            .clone(),
                        prove_core_composition_commit_ms: prove_core_composition_commit_samples_ms
                            .clone(),
                        prove_core_prove_values_ms: prove_core_prove_values_samples_ms.clone(),
                        prove_core_sanity_check_ms: prove_core_sanity_check_samples_ms.clone(),
                    }),
                    prove_breakdown_summary_ms: Some(ProveBreakdownSummaryTimings {
                        setup_and_preprocessed_commit_ms: summarize(&setup_samples_ms),
                        trace_generation_ms: summarize(&trace_generation_samples_ms),
                        trace_commit_ms: summarize(&trace_commit_samples_ms),
                        trace_commit_interpolation_ms: summarize(&trace_commit_interpolation_samples_ms),
                        trace_commit_extension_ms: summarize(&trace_commit_extension_samples_ms),
                        trace_commit_merkle_ms: summarize(&trace_commit_merkle_samples_ms),
                        interaction_generation_ms: summarize(&interaction_generation_samples_ms),
                        interaction_commit_ms: summarize(&interaction_commit_samples_ms),
                        interaction_commit_interpolation_ms: summarize(
                            &interaction_commit_interpolation_samples_ms,
                        ),
                        interaction_commit_extension_ms: summarize(
                            &interaction_commit_extension_samples_ms,
                        ),
                        interaction_commit_merkle_ms: summarize(&interaction_commit_merkle_samples_ms),
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

#[cfg(any(feature = "cuda-runtime", test))]
relation!(PoseidonElements, N_STATE);

#[cfg(feature = "cuda-runtime")]
impl CudaPoseidonLookupElementsAbiV1 for PoseidonElements {
    fn to_cuda_poseidon_lookup_elements_abi_v1(&self) -> StwoCudaLookupElements16AbiV1 {
        StwoCudaLookupElements16AbiV1::new(self.0.z, self.0.alpha, self.0.alpha_powers)
    }
}

#[cfg(any(feature = "cuda-runtime", test))]
#[derive(Clone)]
struct PoseidonEval {
    log_n_rows: u32,
    lookup_elements: PoseidonElements,
    mode: PoseidonEvalMode,
}

#[cfg(any(feature = "cuda-runtime", test))]
impl FrameworkEval for PoseidonEval {
    fn log_size(&self) -> u32 {
        self.log_n_rows
    }

    fn max_constraint_log_degree_bound(&self) -> u32 {
        self.log_n_rows + LOG_EXPAND
    }

    fn evaluate<E: EvalAtRow>(&self, mut eval: E) -> E {
        eval_poseidon_constraints_mode(&mut eval, &self.lookup_elements, self.mode);
        eval
    }
}

#[cfg(feature = "cuda-runtime")]
struct PoseidonBenchmarkComponent {
    inner: FrameworkComponent<PoseidonEval>,
    log_n_rows: u32,
    eval_abi: StwoCudaPoseidonEvalAbiV1,
    mode: PoseidonEvalMode,
}

#[cfg(any(feature = "cuda-runtime", test))]
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PoseidonEvalMode {
    Full,
    AlgebraicOnly,
    LogupOnly,
    AlgebraicFirstHalfFullOnly,
    AlgebraicPartialRoundsOnly,
    AlgebraicSecondHalfFullOnly,
    AlgebraicFirstHalfFullRound(usize),
    AlgebraicPartialRound(usize),
    AlgebraicSecondHalfFullRound(usize),
    AlgebraicFirstHalfFullCell(usize, usize),
    AlgebraicSecondHalfFullCell(usize, usize),
}

#[cfg(any(feature = "cuda-runtime", test))]
impl PoseidonEvalMode {
    fn includes_first_half_full_cell(self, round: usize, index: usize) -> bool {
        matches!(
            self,
            Self::Full | Self::AlgebraicOnly | Self::AlgebraicFirstHalfFullOnly
        ) || matches!(self, Self::AlgebraicFirstHalfFullRound(selected_round) if selected_round == round)
            || matches!(
                self,
                Self::AlgebraicFirstHalfFullCell(selected_round, selected_index)
                    if selected_round == round && selected_index == index
            )
    }

    fn includes_partial_round(self, round: usize) -> bool {
        matches!(
            self,
            Self::Full | Self::AlgebraicOnly | Self::AlgebraicPartialRoundsOnly
        ) || matches!(self, Self::AlgebraicPartialRound(selected_round) if selected_round == round)
    }

    fn includes_second_half_full_cell(self, round: usize, index: usize) -> bool {
        matches!(
            self,
            Self::Full | Self::AlgebraicOnly | Self::AlgebraicSecondHalfFullOnly
        ) || matches!(self, Self::AlgebraicSecondHalfFullRound(selected_round) if selected_round == round)
            || matches!(
                self,
                Self::AlgebraicSecondHalfFullCell(selected_round, selected_index)
                    if selected_round == round && selected_index == index
            )
    }

    fn includes_logup(self) -> bool {
        matches!(self, Self::Full | Self::LogupOnly)
    }
}

#[cfg(feature = "cuda-runtime")]
struct VariantPointEvaluator<'a> {
    mask: TreeVec<Vec<&'a Vec<SecureField>>>,
    evaluation_accumulator: &'a mut PointEvaluationAccumulator,
    col_index: Vec<usize>,
    denom_inverse: SecureField,
    logup: stwo_constraint_framework::logup::LogupAtRow<Self>,
}

#[cfg(feature = "cuda-runtime")]
impl<'a> VariantPointEvaluator<'a> {
    fn new(
        mask: TreeVec<Vec<&'a Vec<SecureField>>>,
        evaluation_accumulator: &'a mut PointEvaluationAccumulator,
        denom_inverse: SecureField,
        logup_log_size: u32,
        claimed_sum: SecureField,
    ) -> Self {
        let col_index = vec![0; mask.len()];
        Self {
            mask,
            evaluation_accumulator,
            col_index,
            denom_inverse,
            logup: stwo_constraint_framework::logup::LogupAtRow::new(
                INTERACTION_TRACE_IDX,
                claimed_sum,
                logup_log_size,
            ),
        }
    }
}

#[cfg(feature = "cuda-runtime")]
impl EvalAtRow for VariantPointEvaluator<'_> {
    type F = SecureField;
    type EF = SecureField;

    fn next_interaction_mask<const N: usize>(
        &mut self,
        interaction: usize,
        _offsets: [isize; N],
    ) -> [Self::F; N] {
        let col_index = self.col_index[interaction];
        self.col_index[interaction] += 1;
        let mask = self.mask[interaction][col_index].clone();
        assert_eq!(mask.len(), N);
        mask.try_into().unwrap()
    }

    fn add_constraint<G>(&mut self, constraint: G)
    where
        Self::EF: Mul<G, Output = Self::EF>,
    {
        self.evaluation_accumulator
            .accumulate(self.denom_inverse * constraint);
    }

    fn combine_ef(values: [Self::F; SECURE_EXTENSION_DEGREE]) -> Self::EF {
        SecureField::from_partial_evals(values)
    }

    fn write_logup_frac(&mut self, fraction: Fraction<Self::EF, Self::EF>) {
        if self.logup.fracs.is_empty() {
            self.logup.is_finalized = false;
        }
        self.logup.fracs.push(fraction);
    }

    fn finalize_logup_batched(&mut self, batching: &Vec<usize>) {
        assert!(!self.logup.is_finalized, "LogupAtRow was already finalized");
        assert_eq!(
            batching.len(),
            self.logup.fracs.len(),
            "Batching must be of the same length as the number of entries"
        );

        let last_batch = *batching.iter().max().expect("at least one logup batch");
        let mut fracs_by_batch = vec![Vec::<Fraction<Self::EF, Self::EF>>::new(); last_batch + 1];

        for (batch, frac) in batching.iter().zip(self.logup.fracs.iter()) {
            fracs_by_batch[*batch].push(frac.clone());
        }

        let mut prev_col_cumsum = SecureField::zero();

        for fracs in fracs_by_batch.iter().take(last_batch) {
            let cur_frac: Fraction<_, _> = fracs.iter().cloned().sum();
            let [cur_cumsum] = self.next_extension_interaction_mask(self.logup.interaction, [0]);
            let diff = cur_cumsum.clone() - prev_col_cumsum.clone();
            prev_col_cumsum = cur_cumsum;
            self.add_constraint(diff * cur_frac.denominator - cur_frac.numerator);
        }

        let frac: Fraction<_, _> = fracs_by_batch[last_batch].clone().into_iter().sum();
        let [prev_row_cumsum, cur_cumsum] =
            self.next_extension_interaction_mask(self.logup.interaction, [-1, 0]);
        let diff = cur_cumsum - prev_row_cumsum - prev_col_cumsum.clone();
        let shifted_diff = diff + self.logup.cumsum_shift;
        self.add_constraint(shifted_diff * frac.denominator - frac.numerator);

        self.logup.is_finalized = true;
    }

    fn finalize_logup(&mut self) {
        let batches = (0..self.logup.fracs.len()).collect();
        self.finalize_logup_batched(&batches);
    }

    fn finalize_logup_in_pairs(&mut self) {
        let batches = (0..self.logup.fracs.len()).map(|n| n / 2).collect();
        self.finalize_logup_batched(&batches);
    }
}

#[cfg(all(feature = "cuda-runtime", test))]
struct CollectingPointEvaluator<'a> {
    mask: TreeVec<Vec<&'a Vec<SecureField>>>,
    col_index: Vec<usize>,
    denom_inverse: SecureField,
    emitted_evaluations: Vec<SecureField>,
    logup: stwo_constraint_framework::logup::LogupAtRow<Self>,
    logged_fractions: Vec<Fraction<SecureField, SecureField>>,
    logged_relation_values: Vec<[SecureField; N_STATE]>,
    last_batching: Vec<usize>,
    last_batched_fractions: Vec<Fraction<SecureField, SecureField>>,
    last_prev_row_cumsum: Option<SecureField>,
    last_cur_cumsum: Option<SecureField>,
    last_shifted_diff: Option<SecureField>,
    last_cumsum_times_denominator: Option<SecureField>,
    last_numerator: Option<SecureField>,
    last_constraint_before_denom_inverse: Option<SecureField>,
}

#[cfg(all(feature = "cuda-runtime", test))]
impl<'a> CollectingPointEvaluator<'a> {
    fn new(
        mask: TreeVec<Vec<&'a Vec<SecureField>>>,
        denom_inverse: SecureField,
        logup_log_size: u32,
        claimed_sum: SecureField,
    ) -> Self {
        let col_index = vec![0; mask.len()];
        Self {
            mask,
            col_index,
            denom_inverse,
            emitted_evaluations: vec![],
            logup: stwo_constraint_framework::logup::LogupAtRow::new(
                INTERACTION_TRACE_IDX,
                claimed_sum,
                logup_log_size,
            ),
            logged_fractions: vec![],
            logged_relation_values: vec![],
            last_batching: vec![],
            last_batched_fractions: vec![],
            last_prev_row_cumsum: None,
            last_cur_cumsum: None,
            last_shifted_diff: None,
            last_cumsum_times_denominator: None,
            last_numerator: None,
            last_constraint_before_denom_inverse: None,
        }
    }
}

#[cfg(all(feature = "cuda-runtime", test))]
impl EvalAtRow for CollectingPointEvaluator<'_> {
    type F = SecureField;
    type EF = SecureField;

    fn next_interaction_mask<const N: usize>(
        &mut self,
        interaction: usize,
        _offsets: [isize; N],
    ) -> [Self::F; N] {
        let col_index = self.col_index[interaction];
        self.col_index[interaction] += 1;
        let mask = self.mask[interaction][col_index].clone();
        assert_eq!(mask.len(), N);
        mask.try_into().unwrap()
    }

    fn add_constraint<G>(&mut self, constraint: G)
    where
        Self::EF: Mul<G, Output = Self::EF>,
    {
        self.emitted_evaluations.push(self.denom_inverse * constraint);
    }

    fn combine_ef(values: [Self::F; SECURE_EXTENSION_DEGREE]) -> Self::EF {
        SecureField::from_partial_evals(values)
    }

    fn write_logup_frac(&mut self, fraction: Fraction<Self::EF, Self::EF>) {
        if self.logup.fracs.is_empty() {
            self.logup.is_finalized = false;
        }
        self.logged_fractions.push(fraction.clone());
        self.logup.fracs.push(fraction);
    }

    fn finalize_logup_batched(&mut self, batching: &Vec<usize>) {
        assert!(!self.logup.is_finalized, "LogupAtRow was already finalized");
        assert_eq!(
            batching.len(),
            self.logup.fracs.len(),
            "Batching must be of the same length as the number of entries"
        );

        let last_batch = *batching.iter().max().expect("at least one logup batch");
        let mut fracs_by_batch = vec![Vec::<Fraction<Self::EF, Self::EF>>::new(); last_batch + 1];
        self.last_batching = batching.clone();
        self.last_batched_fractions.clear();

        for (batch, frac) in batching.iter().zip(self.logup.fracs.iter()) {
            fracs_by_batch[*batch].push(frac.clone());
        }

        let mut prev_col_cumsum = SecureField::zero();

        for fracs in fracs_by_batch.iter().take(last_batch) {
            let cur_frac: Fraction<_, _> = fracs.iter().cloned().sum();
            self.last_batched_fractions.push(cur_frac.clone());
            let [cur_cumsum] = self.next_extension_interaction_mask(self.logup.interaction, [0]);
            let diff = cur_cumsum.clone() - prev_col_cumsum.clone();
            prev_col_cumsum = cur_cumsum;
            self.add_constraint(diff * cur_frac.denominator - cur_frac.numerator);
        }

        let frac: Fraction<_, _> = fracs_by_batch[last_batch].clone().into_iter().sum();
        self.last_batched_fractions.push(frac.clone());
        let [prev_row_cumsum, cur_cumsum] =
            self.next_extension_interaction_mask(self.logup.interaction, [-1, 0]);
        let diff = cur_cumsum - prev_row_cumsum - prev_col_cumsum.clone();
        let shifted_diff = diff + self.logup.cumsum_shift;
        let cumsum_times_denominator = shifted_diff.clone() * frac.denominator;
        let numerator = frac.numerator;
        let constraint_before_denom_inverse = cumsum_times_denominator.clone() - numerator.clone();
        self.last_prev_row_cumsum = Some(prev_row_cumsum);
        self.last_cur_cumsum = Some(cur_cumsum);
        self.last_shifted_diff = Some(shifted_diff);
        self.last_cumsum_times_denominator = Some(cumsum_times_denominator);
        self.last_numerator = Some(numerator);
        self.last_constraint_before_denom_inverse = Some(constraint_before_denom_inverse.clone());
        self.add_constraint(constraint_before_denom_inverse);

        self.logup.is_finalized = true;
    }

    fn finalize_logup(&mut self) {
        let batches = (0..self.logup.fracs.len()).collect();
        self.finalize_logup_batched(&batches);
    }

    fn finalize_logup_in_pairs(&mut self) {
        let batches = (0..self.logup.fracs.len()).map(|n| n / 2).collect();
        self.finalize_logup_batched(&batches);
    }
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LogupFinalizationMode {
    Sequential,
    InPairs,
    Skip,
}

#[cfg(all(feature = "cuda-runtime", test))]
#[derive(Clone, Copy, Debug)]
struct PointContractVariantSnapshot {
    current_framework_eval: SecureField,
    trace_denominator_eval: SecureField,
    trace_points_current_denominator_eval: SecureField,
    trace_points_trace_denominator_eval: SecureField,
    trace_points_trace_lifted_denominator_eval: SecureField,
}

#[cfg(all(feature = "cuda-runtime", test))]
#[derive(Clone, Copy, Debug)]
struct LogupContractVariantSnapshot {
    trace_points_trace_in_pairs_eval: SecureField,
    trace_points_trace_sequential_eval: Option<SecureField>,
    trace_points_trace_in_pairs_logup_delta_eval: SecureField,
    trace_points_trace_sequential_logup_delta_eval: Option<SecureField>,
    trace_points_trace_sequential_supported: bool,
    trace_points_trace_sequential_first_expected_arity: Option<usize>,
    trace_points_trace_sequential_first_actual_arity: Option<usize>,
    cpu_trace_logup_delta_eval: SecureField,
}

#[cfg(all(feature = "cuda-runtime", test))]
#[derive(Clone, Copy, Debug)]
struct LogupOnlyVariantSnapshot {
    current_framework_eval: SecureField,
    trace_denominator_eval: SecureField,
    trace_points_current_denominator_eval: SecureField,
    trace_points_trace_in_pairs_eval: SecureField,
    cpu_trace_eval: SecureField,
}

#[cfg(all(feature = "cuda-runtime", test))]
#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
struct SecureFieldFractionSnapshot {
    numerator: SecureField,
    denominator: SecureField,
}

#[cfg(all(feature = "cuda-runtime", test))]
#[derive(Clone, Debug)]
struct InPairsLogupBoundarySnapshot {
    frac_count: usize,
    batch_count: usize,
    denom_inverse: SecureField,
    cumsum_shift: SecureField,
    logged_relation_values: Vec<[SecureField; N_STATE]>,
    first_fraction: Option<SecureFieldFractionSnapshot>,
    second_fraction: Option<SecureFieldFractionSnapshot>,
    batched_fraction: Option<SecureFieldFractionSnapshot>,
    prev_row_cumsum: Option<SecureField>,
    cur_cumsum: Option<SecureField>,
    shifted_diff: Option<SecureField>,
    cumsum_times_denominator: Option<SecureField>,
    numerator: Option<SecureField>,
    constraint_before_denom_inverse: Option<SecureField>,
    emitted_evaluation_count: usize,
    emitted_horner_eval: SecureField,
}

#[cfg(all(feature = "cuda-runtime", test))]
#[derive(Clone, Copy, Debug)]
struct InPairsCumsumVariantSnapshot {
    current_eval: SecureField,
    swapped_diff_eval: SecureField,
    subtract_shift_eval: SecureField,
    swapped_diff_subtract_shift_eval: SecureField,
    no_shift_eval: SecureField,
    cpu_trace_eval: SecureField,
}

#[cfg(all(feature = "cuda-runtime", test))]
#[derive(Clone, Copy, Debug)]
struct InPairsFractionAssemblyVariantSnapshot {
    current_eval: SecureField,
    first_only_eval: SecureField,
    second_only_eval: SecureField,
    both_positive_eval: SecureField,
    swapped_signs_eval: SecureField,
    naive_pair_current_eval: SecureField,
    naive_pair_positive_eval: SecureField,
    cpu_trace_eval: SecureField,
}

#[cfg(all(feature = "cuda-runtime", test))]
#[derive(Clone, Copy, Debug)]
struct InPairsDenominatorVariantSnapshot {
    current_eval: SecureField,
    first_reversed_eval: SecureField,
    second_reversed_eval: SecureField,
    both_reversed_eval: SecureField,
    cpu_trace_eval: SecureField,
}

#[cfg(all(feature = "cuda-runtime", test))]
#[derive(Clone, Copy, Debug)]
struct PointContributionSplitSnapshot {
    point_full_eval: SecureField,
    point_algebraic_only_eval: SecureField,
    point_algebraic_trace_denominator_eval: SecureField,
    trace_points_algebraic_only_eval: SecureField,
    trace_points_algebraic_trace_denominator_eval: SecureField,
    point_logup_delta_eval: SecureField,
    cpu_trace_full_eval: SecureField,
    cpu_trace_algebraic_only_eval: SecureField,
    cpu_trace_logup_delta_eval: SecureField,
}

#[cfg(all(feature = "cuda-runtime", test))]
#[derive(Clone, Copy, Debug)]
struct AlgebraicFamilySplitSnapshot {
    point_first_half_full_eval: SecureField,
    point_partial_rounds_eval: SecureField,
    point_second_half_full_eval: SecureField,
    cpu_first_half_full_eval: SecureField,
    cpu_partial_rounds_eval: SecureField,
    cpu_second_half_full_eval: SecureField,
}

#[cfg(all(feature = "cuda-runtime", test))]
#[derive(Clone, Copy, Debug)]
struct AlgebraicSequenceLawSnapshot {
    evaluation_count: usize,
    current_horner_eval: SecureField,
    reversed_horner_eval: SecureField,
    point_algebraic_only_eval: SecureField,
    cpu_trace_algebraic_only_eval: SecureField,
}

#[cfg(feature = "cuda-runtime")]
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Clone, Debug)]
struct AlgebraicPerRoundSnapshot {
    point_first_half_full_round_evals: Vec<SecureField>,
    point_partial_round_evals: Vec<SecureField>,
    point_second_half_full_round_evals: Vec<SecureField>,
    cpu_first_half_full_round_evals: Vec<SecureField>,
    cpu_partial_round_evals: Vec<SecureField>,
    cpu_second_half_full_round_evals: Vec<SecureField>,
}

#[cfg(feature = "cuda-runtime")]
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Clone, Debug)]
struct AlgebraicPerCellSnapshot {
    point_first_half_full_cell_evals: Vec<Vec<SecureField>>,
    point_second_half_full_cell_evals: Vec<Vec<SecureField>>,
    cpu_first_half_full_cell_evals: Vec<Vec<SecureField>>,
    cpu_second_half_full_cell_evals: Vec<Vec<SecureField>>,
}

#[cfg(feature = "cuda-runtime")]
impl PoseidonBenchmarkComponent {
    fn new(log_n_rows: u32, lookup_elements: PoseidonElements, claimed_sum: SecureField) -> Self {
        Self::new_with_mode(
            log_n_rows,
            lookup_elements,
            claimed_sum,
            PoseidonEvalMode::Full,
        )
    }

    fn new_with_mode(
        log_n_rows: u32,
        lookup_elements: PoseidonElements,
        claimed_sum: SecureField,
        mode: PoseidonEvalMode,
    ) -> Self {
        let eval = PoseidonEval {
            log_n_rows,
            lookup_elements: lookup_elements.clone(),
            mode,
        };
        let inner = FrameworkComponent::new(&mut TraceLocationAllocator::default(), eval, claimed_sum);
        let eval_abi = StwoCudaPoseidonEvalAbiV1::new(
            POSEIDON_EVAL_ID,
            log_n_rows,
            lookup_elements.to_cuda_poseidon_lookup_elements_abi_v1(),
            claimed_sum,
        );
        Self {
            inner,
            log_n_rows,
            eval_abi,
            mode,
        }
    }
}

#[cfg(feature = "cuda-runtime")]
impl PoseidonBenchmarkComponent {
    #[cfg(test)]
    fn point_mask_points(
        &self,
        point: stwo::core::circle::CirclePoint<SecureField>,
        mask_log_size: u32,
    ) -> TreeVec<Vec<Vec<stwo::core::circle::CirclePoint<SecureField>>>> {
        <FrameworkComponent<PoseidonEval> as Component>::mask_points(&self.inner, point, mask_log_size)
    }

    fn evaluate_constraint_quotients_at_point_with_contract(
        &self,
        point: stwo::core::circle::CirclePoint<SecureField>,
        mask: &TreeVec<Vec<Vec<SecureField>>>,
        evaluation_accumulator: &mut PointEvaluationAccumulator,
        denominator_log_size: u32,
        logup_log_size: u32,
    ) {
        let preprocessed_mask = self
            .inner
            .preprocessed_column_indices()
            .iter()
            .map(|idx| &mask[PREPROCESSED_TRACE_IDX][*idx])
            .collect();

        let mut mask_points = mask.sub_tree(self.inner.trace_locations());
        mask_points[PREPROCESSED_TRACE_IDX] = preprocessed_mask;

        self.inner.evaluate(VariantPointEvaluator::new(
            mask_points,
            evaluation_accumulator,
            coset_vanishing(CanonicCoset::new(denominator_log_size).coset(), point).inverse(),
            logup_log_size,
            self.inner.claimed_sum(),
        ));
    }

    #[cfg_attr(not(test), allow(dead_code))]
    fn eval_composition_polynomial_at_point_with_contract(
        &self,
        point: stwo::core::circle::CirclePoint<SecureField>,
        mask: &TreeVec<Vec<Vec<SecureField>>>,
        random_coeff: SecureField,
        denominator_log_size: u32,
        logup_log_size: u32,
    ) -> SecureField {
        let mut evaluation_accumulator = PointEvaluationAccumulator::new(random_coeff);
        self.evaluate_constraint_quotients_at_point_with_contract(
            point,
            mask,
            &mut evaluation_accumulator,
            denominator_log_size,
            logup_log_size,
        );
        evaluation_accumulator.finalize()
    }

    #[cfg(test)]
    fn eval_composition_polynomial_at_point_with_logup_finalization(
        &self,
        point: stwo::core::circle::CirclePoint<SecureField>,
        mask: &TreeVec<Vec<Vec<SecureField>>>,
        random_coeff: SecureField,
        denominator_log_size: u32,
        logup_log_size: u32,
        logup_finalization_mode: LogupFinalizationMode,
    ) -> SecureField {
        let preprocessed_mask = self
            .inner
            .preprocessed_column_indices()
            .iter()
            .map(|idx| &mask[PREPROCESSED_TRACE_IDX][*idx])
            .collect();

        let mut mask_points = mask.sub_tree(self.inner.trace_locations());
        mask_points[PREPROCESSED_TRACE_IDX] = preprocessed_mask;

        let mut evaluation_accumulator = PointEvaluationAccumulator::new(random_coeff);
        {
            let mut evaluator = VariantPointEvaluator::new(
                mask_points,
                &mut evaluation_accumulator,
                coset_vanishing(CanonicCoset::new(denominator_log_size).coset(), point).inverse(),
                logup_log_size,
                self.inner.claimed_sum(),
            );
            diagnostics::eval_poseidon_constraints_mode_with_logup_finalization(
                &mut evaluator,
                &self.inner.lookup_elements,
                self.mode,
                logup_finalization_mode,
            );
        }
        evaluation_accumulator.finalize()
    }
}

#[cfg(feature = "cuda-runtime")]
impl Component for PoseidonBenchmarkComponent {
    fn n_constraints(&self) -> usize {
        <FrameworkComponent<PoseidonEval> as Component>::n_constraints(&self.inner)
    }

    fn max_constraint_log_degree_bound(&self) -> u32 {
        <FrameworkComponent<PoseidonEval> as Component>::max_constraint_log_degree_bound(&self.inner)
    }

    fn trace_log_degree_bounds(&self) -> TreeVec<Vec<u32>> {
        <FrameworkComponent<PoseidonEval> as Component>::trace_log_degree_bounds(&self.inner)
    }

    fn mask_points(
        &self,
        point: stwo::core::circle::CirclePoint<SecureField>,
        max_log_degree_bound: u32,
    ) -> TreeVec<Vec<Vec<stwo::core::circle::CirclePoint<SecureField>>>> {
        // Keep the benchmark-local denominator contract explicit in the point evaluator, but ask
        // prove_values for points on the same max-degree coset it uses to fetch committed values.
        <FrameworkComponent<PoseidonEval> as Component>::mask_points(
            &self.inner,
            point,
            max_log_degree_bound,
        )
    }

    fn preprocessed_column_indices(&self) -> Vec<usize> {
        <FrameworkComponent<PoseidonEval> as Component>::preprocessed_column_indices(&self.inner)
    }

    fn evaluate_constraint_quotients_at_point(
        &self,
        point: stwo::core::circle::CirclePoint<SecureField>,
        mask: &TreeVec<Vec<Vec<SecureField>>>,
        evaluation_accumulator: &mut PointEvaluationAccumulator,
        max_log_degree_bound: u32,
    ) {
        self.evaluate_constraint_quotients_at_point_with_contract(
            point,
            mask,
            evaluation_accumulator,
            max_log_degree_bound,
            self.log_n_rows,
        );
    }
}

#[cfg(all(feature = "cuda-runtime", test))]
fn expected_logup_cumsum_arities(
    frac_count: usize,
    logup_finalization_mode: LogupFinalizationMode,
) -> Vec<usize> {
    if frac_count == 0 {
        return vec![];
    }
    let unary_count = match logup_finalization_mode {
        LogupFinalizationMode::Sequential => frac_count.saturating_sub(1),
        LogupFinalizationMode::InPairs => (frac_count.saturating_sub(1)) / 2,
        LogupFinalizationMode::Skip => 0,
    };
    let mut arities = vec![1; unary_count];
    arities.push(2);
    arities
}

#[cfg(all(feature = "cuda-runtime", test))]
fn first_arity_mismatch(expected: &[usize], actual: &[usize]) -> Option<(usize, usize)> {
    for (expected_arity, actual_arity) in expected.iter().zip(actual.iter()) {
        if expected_arity != actual_arity {
            return Some((*expected_arity, *actual_arity));
        }
    }
    None
}

#[cfg(feature = "cuda-runtime")]
impl ComponentProver<CudaBackend> for PoseidonBenchmarkComponent {
    fn evaluate_constraint_quotients_on_domain(
        &self,
        trace: &Trace<'_, CudaBackend>,
        evaluation_accumulator: &mut DomainEvaluationAccumulator<CudaBackend>,
    ) {
        assert!(
            matches!(self.mode, PoseidonEvalMode::Full),
            "only the full Poseidon evaluator has a CUDA domain-kernel implementation",
        );
        if self.n_constraints() == 0 {
            return;
        }

        if self.inner.is_disabled() {
            evaluation_accumulator.skip_coeffs(self.n_constraints());
            return;
        }

        let eval_domain = CanonicCoset::new(self.max_constraint_log_degree_bound()).circle_domain();
        let trace_domain = CanonicCoset::new(self.log_n_rows);
        let mut component_polys = trace.polys.sub_tree(self.inner.trace_locations());
        component_polys[PREPROCESSED_TRACE_IDX] = self
            .inner
            .preprocessed_column_indices()
            .iter()
            .map(|idx| &trace.polys[PREPROCESSED_TRACE_IDX][*idx])
            .collect();

        let need_to_extend = component_polys
            .iter()
            .flatten()
            .any(|column| column.evals.domain.log_size() != eval_domain.log_size());

        let trace = if need_to_extend {
            let twiddles = CudaBackend::precompute_twiddles(eval_domain.half_coset);
            component_polys.as_cols_ref().map_cols(|column| {
                Cow::Owned(column.get_evaluation_on_domain(eval_domain, &twiddles))
            })
        } else {
            component_polys.map_cols(|column| Cow::Borrowed(&column.evals))
        };

        let log_expand = eval_domain.log_size() - trace_domain.log_size();
        let mut denominator_inverses = (0..(1 << log_expand))
            .map(|index| coset_vanishing(trace_domain.coset(), eval_domain.at(index)).inverse())
            .collect::<Vec<_>>();
        bit_reverse(&mut denominator_inverses);
        let denominator_inverses = BaseFieldVec::from_vec(denominator_inverses);

        let [mut accum] =
            evaluation_accumulator.columns([(eval_domain.log_size(), self.n_constraints())]);
        accum.random_coeff_powers.reverse();
        let random_coeff_powers = SecureFieldVec::from_vec(accum.random_coeff_powers);
        let eval_log_size = eval_domain.log_size();

        let trace0_evaluations = trace[PREPROCESSED_TRACE_IDX]
            .iter()
            .map(|column| column.as_ref().values.device_ptr)
            .collect::<Vec<_>>();
        let trace1_evaluations = trace[MAIN_TRACE_IDX]
            .iter()
            .map(|column| column.as_ref().values.device_ptr)
            .collect::<Vec<_>>();
        let trace2_evaluations = trace[INTERACTION_TRACE_IDX]
            .iter()
            .map(|column| column.as_ref().values.device_ptr)
            .collect::<Vec<_>>();
        let total_logup_entries = self.inner.logup_counts().values().sum::<usize>();
        assert_eq!(
            total_logup_entries,
            2 * (1usize << trace_domain.log_size()),
            "poseidon benchmark logup-entry contract drifted"
        );
        let logup_counts = 2;

        launch_constraint_quotients_on_domain(ConstraintQuotientEvalRequest {
            quotient_columns: &accum.col.columns,
            trace0_evaluations: &trace0_evaluations,
            trace1_evaluations: &trace1_evaluations,
            trace2_evaluations: &trace2_evaluations,
            random_coeff_powers: &random_coeff_powers,
            denominator_inverses: &denominator_inverses,
            domain_log_size: trace_domain.log_size(),
            eval_domain_log_size: eval_log_size,
            number_of_columns: self.n_constraints() as u32,
            logup_counts,
            eval: opaque_eval_ptr(&self.eval_abi),
            // Poseidon interaction traces are generated with a trace-row cumsum shift.
            claimed_sum_shift: self.inner.claimed_sum()
                / BaseField::from_u32_unchecked(1 << trace_domain.log_size()),
            should_accumulate: true,
        });
    }
}

#[cfg(feature = "cuda-runtime")]
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
    interaction_generation_ms: f64,
    interaction_commit_ms: f64,
    interaction_commit_interpolation_ms: f64,
    interaction_commit_extension_ms: f64,
    interaction_commit_merkle_ms: f64,
    prove_core_ms: f64,
    prove_core_composition_generation_ms: f64,
    prove_core_composition_commit_ms: f64,
    prove_core_prove_values_ms: f64,
    prove_core_sanity_check_ms: f64,
    proof_metadata: ProofMetadata,
    sentinel: PoseidonSentinel,
}

#[cfg(feature = "cuda-runtime")]
fn run_one_sample(log_n_instances: u32) -> SampleResult {
    let config = PcsConfig::default();

    let prove_start = Instant::now();
    let (component, proof, sentinel, prove_breakdown) =
        prove_poseidon_blake(log_n_instances, config);
    let prove_elapsed_ms = prove_start.elapsed().as_secs_f64() * 1000.0;

    let proof_metadata = proof_metadata(&proof);

    let verify_start = Instant::now();
    verify_poseidon_blake(&component, &proof).expect("poseidon proof should verify");
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
        interaction_generation_ms: prove_breakdown.interaction_generation_ms,
        interaction_commit_ms: prove_breakdown.interaction_commit_ms,
        interaction_commit_interpolation_ms: prove_breakdown.interaction_commit_interpolation_ms,
        interaction_commit_extension_ms: prove_breakdown.interaction_commit_extension_ms,
        interaction_commit_merkle_ms: prove_breakdown.interaction_commit_merkle_ms,
        prove_core_ms: prove_breakdown.prove_core_ms,
        prove_core_composition_generation_ms: prove_breakdown.prove_core_composition_generation_ms,
        prove_core_composition_commit_ms: prove_breakdown.prove_core_composition_commit_ms,
        prove_core_prove_values_ms: prove_breakdown.prove_core_prove_values_ms,
        prove_core_sanity_check_ms: prove_breakdown.prove_core_sanity_check_ms,
        proof_metadata,
        sentinel,
    }
}

#[cfg(feature = "cuda-runtime")]
#[derive(Clone, Copy)]
struct ProveBreakdown {
    setup_and_preprocessed_commit_ms: f64,
    trace_generation_ms: f64,
    trace_commit_ms: f64,
    trace_commit_interpolation_ms: f64,
    trace_commit_extension_ms: f64,
    trace_commit_merkle_ms: f64,
    interaction_generation_ms: f64,
    interaction_commit_ms: f64,
    interaction_commit_interpolation_ms: f64,
    interaction_commit_extension_ms: f64,
    interaction_commit_merkle_ms: f64,
    prove_core_ms: f64,
    prove_core_composition_generation_ms: f64,
    prove_core_composition_commit_ms: f64,
    prove_core_prove_values_ms: f64,
    prove_core_sanity_check_ms: f64,
}

#[cfg(feature = "cuda-runtime")]
#[derive(Clone, Copy)]
struct ProveCoreBreakdown {
    composition_generation_ms: f64,
    composition_commit_ms: f64,
    prove_values_ms: f64,
    sanity_check_ms: f64,
}

#[cfg(feature = "cuda-runtime")]
#[derive(Clone, Copy, Debug)]
struct ProveCoreSanitySnapshot {
    sampled_composition_oods_eval: SecureField,
    #[allow(dead_code)]
    direct_composition_oods_eval: SecureField,
    point_composition_oods_eval: SecureField,
    #[allow(dead_code)]
    cpu_trace_composition_oods_eval: Option<SecureField>,
    #[allow(dead_code)]
    direct_trace_mask_point_composition_oods_eval: Option<SecureField>,
    #[allow(dead_code)]
    sampled_trace_mask_values_match_direct: Option<bool>,
    #[allow(dead_code)]
    sampled_point_matches_direct_trace_mask_point: Option<bool>,
    #[allow(dead_code)]
    cpu_trace_matches_direct: Option<bool>,
    #[allow(dead_code)]
    cpu_trace_matches_point: Option<bool>,
    #[allow(dead_code)]
    trace_point_values_match_cpu_committed: Option<bool>,
    #[allow(dead_code)]
    main_trace_point_values_match_cpu_committed: Option<bool>,
    #[allow(dead_code)]
    interaction_trace_point_values_match_cpu_committed: Option<bool>,
    #[allow(dead_code)]
    main_trace_point_value_mismatches: Option<usize>,
    #[allow(dead_code)]
    interaction_trace_point_value_mismatches: Option<usize>,
    #[allow(dead_code)]
    sampled_values_match_cpu_committed: Option<bool>,
    #[allow(dead_code)]
    main_sampled_values_match_cpu_committed: Option<bool>,
    #[allow(dead_code)]
    interaction_sampled_values_match_cpu_committed: Option<bool>,
    #[allow(dead_code)]
    main_sampled_value_mismatches: Option<usize>,
    #[allow(dead_code)]
    interaction_sampled_value_mismatches: Option<usize>,
    #[allow(dead_code)]
    committed_coefficients_match_cpu: Option<bool>,
    #[allow(dead_code)]
    main_committed_coefficients_match_cpu: Option<bool>,
    #[allow(dead_code)]
    interaction_committed_coefficients_match_cpu: Option<bool>,
    #[allow(dead_code)]
    main_committed_coefficient_mismatches: Option<usize>,
    #[allow(dead_code)]
    interaction_committed_coefficient_mismatches: Option<usize>,
}

#[cfg(feature = "cuda-runtime")]
struct CudaLookupData {
    initial_state: [[BaseFieldVec; N_STATE]; N_INSTANCES_PER_ROW],
    final_state: [[BaseFieldVec; N_STATE]; N_INSTANCES_PER_ROW],
}

#[cfg(feature = "cuda-runtime")]
fn prove_poseidon_blake(
    log_n_instances: u32,
    config: PcsConfig,
) -> (
    PoseidonBenchmarkComponent,
    StarkProof<Blake2sMerkleHasher>,
    PoseidonSentinel,
    ProveBreakdown,
) {
    assert!(log_n_instances >= N_LOG_INSTANCES_PER_ROW as u32);
    let log_n_rows = log_n_instances - N_LOG_INSTANCES_PER_ROW as u32;

    let setup_start = Instant::now();
    let twiddles = CudaBackend::precompute_twiddles(
        CanonicCoset::new(log_n_rows + LOG_EXPAND + config.fri_config.log_blowup_factor)
            .circle_domain()
            .half_coset,
    );
    let prover_channel = &mut Blake2sChannel::default();
    let mut commitment_scheme =
        CommitmentSchemeProver::<CudaBackend, Blake2sMerkleChannel>::new(config, &twiddles);
    let mut tree_builder = commitment_scheme.tree_builder();
    tree_builder.extend_evals(vec![]);
    tree_builder.commit(prover_channel);
    maybe_enable_poseidon_store_polynomials_coefficients(&mut commitment_scheme);
    let setup_and_preprocessed_commit_ms = setup_start.elapsed().as_secs_f64() * 1000.0;

    let trace_generation_start = Instant::now();
    let (trace, lookup_data, mut sentinel) = generate_poseidon_trace_evaluations(log_n_rows);
    let trace_generation_ms = trace_generation_start.elapsed().as_secs_f64() * 1000.0;

    let trace_commit_start = Instant::now();
    let trace_commit_breakdown =
        commit_trace_with_breakdown(&mut commitment_scheme, prover_channel, trace, &twiddles);
    let trace_commit_ms = trace_commit_start.elapsed().as_secs_f64() * 1000.0;

    let lookup_elements = PoseidonElements::draw(prover_channel);

    let interaction_generation_start = Instant::now();
    let (interaction_trace, claimed_sum, interaction_sentinel) =
        generate_poseidon_interaction_trace_evaluations(log_n_rows, lookup_data, &lookup_elements);
    let interaction_generation_ms = interaction_generation_start.elapsed().as_secs_f64() * 1000.0;
    sentinel.interaction_first_column_first_value =
        interaction_sentinel.interaction_first_column_first_value;
    sentinel.claimed_sum_m31 = interaction_sentinel.claimed_sum_m31;

    let interaction_commit_start = Instant::now();
    let interaction_commit_breakdown = commit_trace_with_breakdown(
        &mut commitment_scheme,
        prover_channel,
        interaction_trace,
        &twiddles,
    );
    let interaction_commit_ms = interaction_commit_start.elapsed().as_secs_f64() * 1000.0;

    let component = PoseidonBenchmarkComponent::new(log_n_rows, lookup_elements, claimed_sum);
    let prove_core_start = Instant::now();
    let (proof, prove_core_breakdown) =
        prove_with_breakdown::<CudaBackend, Blake2sMerkleChannel>(
            &[&component],
            prover_channel,
            commitment_scheme,
        )
        .expect("poseidon prove should succeed");
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
            interaction_generation_ms,
            interaction_commit_ms,
            interaction_commit_interpolation_ms: interaction_commit_breakdown.interpolation_ms,
            interaction_commit_extension_ms: interaction_commit_breakdown.extension_ms,
            interaction_commit_merkle_ms: interaction_commit_breakdown.merkle_ms,
            prove_core_ms,
            prove_core_composition_generation_ms: prove_core_breakdown.composition_generation_ms,
            prove_core_composition_commit_ms: prove_core_breakdown.composition_commit_ms,
            prove_core_prove_values_ms: prove_core_breakdown.prove_values_ms,
            prove_core_sanity_check_ms: prove_core_breakdown.sanity_check_ms,
        },
    )
}

#[cfg(feature = "cuda-runtime")]
#[cfg_attr(not(test), allow(dead_code))]
fn prove_poseidon_blake_snapshot(
    log_n_instances: u32,
    config: PcsConfig,
) -> (
    PoseidonBenchmarkComponent,
    StarkProof<Blake2sMerkleHasher>,
    PoseidonSentinel,
    ProveBreakdown,
    ProveCoreSanitySnapshot,
) {
    assert!(log_n_instances >= N_LOG_INSTANCES_PER_ROW as u32);
    let log_n_rows = log_n_instances - N_LOG_INSTANCES_PER_ROW as u32;

    let setup_start = Instant::now();
    let twiddles = CudaBackend::precompute_twiddles(
        CanonicCoset::new(log_n_rows + LOG_EXPAND + config.fri_config.log_blowup_factor)
            .circle_domain()
            .half_coset,
    );
    let prover_channel = &mut Blake2sChannel::default();
    let mut commitment_scheme =
        CommitmentSchemeProver::<CudaBackend, Blake2sMerkleChannel>::new(config, &twiddles);
    let mut tree_builder = commitment_scheme.tree_builder();
    tree_builder.extend_evals(vec![]);
    tree_builder.commit(prover_channel);
    maybe_enable_poseidon_store_polynomials_coefficients(&mut commitment_scheme);
    let setup_and_preprocessed_commit_ms = setup_start.elapsed().as_secs_f64() * 1000.0;

    let trace_generation_start = Instant::now();
    let (trace, lookup_data, mut sentinel) = generate_poseidon_trace_evaluations(log_n_rows);
    let trace_generation_ms = trace_generation_start.elapsed().as_secs_f64() * 1000.0;

    let trace_commit_start = Instant::now();
    let trace_commit_breakdown =
        commit_trace_with_breakdown(&mut commitment_scheme, prover_channel, trace, &twiddles);
    let trace_commit_ms = trace_commit_start.elapsed().as_secs_f64() * 1000.0;

    let lookup_elements = PoseidonElements::draw(prover_channel);

    let interaction_generation_start = Instant::now();
    let (interaction_trace, claimed_sum, interaction_sentinel) =
        generate_poseidon_interaction_trace_evaluations(log_n_rows, lookup_data, &lookup_elements);
    let interaction_generation_ms = interaction_generation_start.elapsed().as_secs_f64() * 1000.0;
    sentinel.interaction_first_column_first_value =
        interaction_sentinel.interaction_first_column_first_value;
    sentinel.claimed_sum_m31 = interaction_sentinel.claimed_sum_m31;

    let interaction_commit_start = Instant::now();
    let interaction_commit_breakdown = commit_trace_with_breakdown(
        &mut commitment_scheme,
        prover_channel,
        interaction_trace,
        &twiddles,
    );
    let interaction_commit_ms = interaction_commit_start.elapsed().as_secs_f64() * 1000.0;

    let component = PoseidonBenchmarkComponent::new(log_n_rows, lookup_elements, claimed_sum);
    let prove_core_start = Instant::now();
    let (proof, prove_core_breakdown, snapshot) = prove_with_breakdown_snapshot_impl::<
        CudaBackend,
        Blake2sMerkleChannel,
    >(&[&component], prover_channel, commitment_scheme, false)
    .expect("poseidon snapshot prove should succeed");
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
            interaction_generation_ms,
            interaction_commit_ms,
            interaction_commit_interpolation_ms: interaction_commit_breakdown.interpolation_ms,
            interaction_commit_extension_ms: interaction_commit_breakdown.extension_ms,
            interaction_commit_merkle_ms: interaction_commit_breakdown.merkle_ms,
            prove_core_ms,
            prove_core_composition_generation_ms: prove_core_breakdown.composition_generation_ms,
            prove_core_composition_commit_ms: prove_core_breakdown.composition_commit_ms,
            prove_core_prove_values_ms: prove_core_breakdown.prove_values_ms,
            prove_core_sanity_check_ms: prove_core_breakdown.sanity_check_ms,
        },
        snapshot,
    )
}

#[cfg(feature = "cuda-runtime")]
fn prove_with_breakdown<B: stwo::prover::backend::BackendForChannel<MC>, MC: MerkleChannel>(
    components: &[&dyn ComponentProver<B>],
    channel: &mut MC::C,
    commitment_scheme: CommitmentSchemeProver<'_, B, MC>,
) -> Result<(StarkProof<MC::H>, ProveCoreBreakdown), ProvingError> {
    let (proof, breakdown, snapshot) =
        prove_with_breakdown_snapshot_impl(components, channel, commitment_scheme, false)?;

    if snapshot.sampled_composition_oods_eval != snapshot.point_composition_oods_eval {
        return Err(ProvingError::ConstraintsNotSatisfied);
    }

    Ok((proof, breakdown))
}

#[cfg(feature = "cuda-runtime")]
fn prove_with_breakdown_snapshot_impl<
    B: stwo::prover::backend::BackendForChannel<MC>,
    MC: MerkleChannel,
>(
    components: &[&dyn ComponentProver<B>],
    channel: &mut MC::C,
    mut commitment_scheme: CommitmentSchemeProver<'_, B, MC>,
    capture_trace_mask_diagnostics: bool,
) -> Result<
    (
        StarkProof<MC::H>,
        ProveCoreBreakdown,
        ProveCoreSanitySnapshot,
    ),
    ProvingError,
> {
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

    let oods_point = stwo::core::circle::CirclePoint::<SecureField>::get_random_point(channel);
    let split_composition_log_size = commitment_scheme
        .trees
        .last()
        .unwrap()
        .commitment
        .layers
        .len() as u32
        - 1;
    let lifting_log_size =
        get_lifting_log_size(&commitment_scheme.config, split_composition_log_size);
    let max_log_degree_bound =
        lifting_log_size - commitment_scheme.config.fri_config.log_blowup_factor;
    let direct_composition_oods_eval =
        direct_composition_oods_eval_from_committed_tree(
            &commitment_scheme,
            oods_point,
            max_log_degree_bound,
        )
        .ok_or(ProvingError::ConstraintsNotSatisfied)?;
    let mut sample_points =
        component_provers
            .components()
            .mask_points(oods_point, max_log_degree_bound, false);
    let direct_trace_mask_values = if capture_trace_mask_diagnostics {
        direct_trace_mask_values_from_committed_trees_with_prove_values_lifting(
            &commitment_scheme,
            &sample_points,
        )
    } else {
        None
    };
    sample_points.push(vec![vec![oods_point]; 2 * SECURE_EXTENSION_DEGREE]);

    let prove_values_start = Instant::now();
    let commitment_scheme_proof = commitment_scheme.prove_values(sample_points, channel);
    let prove_values_ms = prove_values_start.elapsed().as_secs_f64() * 1000.0;
    let proof = StarkProof(commitment_scheme_proof.proof);

    let sampled_composition_oods_eval =
        extract_composition_oods_eval(&proof, oods_point, max_log_degree_bound).unwrap();
    let point_composition_oods_eval =
        component_provers.components().eval_composition_polynomial_at_point(
            oods_point,
            &proof.sampled_values,
            random_coeff,
            max_log_degree_bound,
        );
    let direct_trace_mask_point_composition_oods_eval = direct_trace_mask_values.as_ref().map(
        |direct_trace_mask_values| {
            component_provers.components().eval_composition_polynomial_at_point(
                oods_point,
                direct_trace_mask_values,
                random_coeff,
                max_log_degree_bound,
            )
        },
    );
    let sampled_trace_mask_values_match_direct = direct_trace_mask_values.as_ref().map(
        |direct_trace_mask_values| {
            sampled_trace_mask_values_match_direct(&proof.sampled_values, direct_trace_mask_values)
        },
    );
    let sampled_point_matches_direct_trace_mask_point =
        direct_trace_mask_point_composition_oods_eval.map(|direct_trace_mask_eval| {
            direct_trace_mask_eval == point_composition_oods_eval
        });
    let sanity_check_start = Instant::now();
    let sanity_check_ms = sanity_check_start.elapsed().as_secs_f64() * 1000.0;

    Ok((
        proof,
        ProveCoreBreakdown {
            composition_generation_ms,
            composition_commit_ms,
            prove_values_ms,
            sanity_check_ms,
        },
        ProveCoreSanitySnapshot {
            sampled_composition_oods_eval,
            direct_composition_oods_eval,
            point_composition_oods_eval,
            cpu_trace_composition_oods_eval: None,
            direct_trace_mask_point_composition_oods_eval,
            sampled_trace_mask_values_match_direct,
            sampled_point_matches_direct_trace_mask_point,
            cpu_trace_matches_direct: None,
            cpu_trace_matches_point: None,
            trace_point_values_match_cpu_committed: None,
            main_trace_point_values_match_cpu_committed: None,
            interaction_trace_point_values_match_cpu_committed: None,
            main_trace_point_value_mismatches: None,
            interaction_trace_point_value_mismatches: None,
            sampled_values_match_cpu_committed: None,
            main_sampled_values_match_cpu_committed: None,
            interaction_sampled_values_match_cpu_committed: None,
            main_sampled_value_mismatches: None,
            interaction_sampled_value_mismatches: None,
            committed_coefficients_match_cpu: None,
            main_committed_coefficients_match_cpu: None,
            interaction_committed_coefficients_match_cpu: None,
            main_committed_coefficient_mismatches: None,
            interaction_committed_coefficient_mismatches: None,
        },
    ))
}

#[cfg(feature = "cuda-runtime")]
fn direct_composition_oods_eval_from_committed_tree<
    B: stwo::prover::backend::BackendForChannel<MC>,
    MC: MerkleChannel,
>(
    commitment_scheme: &CommitmentSchemeProver<'_, B, MC>,
    oods_point: stwo::core::circle::CirclePoint<SecureField>,
    max_log_degree_bound: u32,
) -> Option<SecureField> {
    let composition_tree = commitment_scheme.trees.last()?;
    let composition_polys = &composition_tree.polynomials;
    let [left0, left1, left2, left3, right0, right1, right2, right3] =
        composition_polys.as_slice()
    else {
        return None;
    };

    let left_eval = SecureField::from_partial_evals([
        left0.coeffs.as_ref()?.eval_at_point(oods_point),
        left1.coeffs.as_ref()?.eval_at_point(oods_point),
        left2.coeffs.as_ref()?.eval_at_point(oods_point),
        left3.coeffs.as_ref()?.eval_at_point(oods_point),
    ]);
    let right_eval = SecureField::from_partial_evals([
        right0.coeffs.as_ref()?.eval_at_point(oods_point),
        right1.coeffs.as_ref()?.eval_at_point(oods_point),
        right2.coeffs.as_ref()?.eval_at_point(oods_point),
        right3.coeffs.as_ref()?.eval_at_point(oods_point),
    ]);

    Some(left_eval + oods_point.repeated_double(max_log_degree_bound - 1).x * right_eval)
}

#[cfg(all(feature = "cuda-runtime", test))]
fn copy_trace_to_cpu(
    trace: &[CircleEvaluation<CudaBackend, BaseField, BitReversedOrder>],
) -> Vec<CircleEvaluation<CpuBackend, BaseField, BitReversedOrder>> {
    trace.iter()
        .map(|evaluation| {
            CircleEvaluation::<CpuBackend, BaseField, BitReversedOrder>::new(
                evaluation.domain,
                evaluation.values.to_cpu(),
            )
        })
        .collect()
}

#[cfg(test)]
fn commit_trace_cpu(
    commitment_scheme: &mut CommitmentSchemeProver<'_, CpuBackend, Blake2sMerkleChannel>,
    prover_channel: &mut Blake2sChannel,
    trace: Vec<CircleEvaluation<CpuBackend, BaseField, BitReversedOrder>>,
    twiddles: &stwo::prover::poly::twiddles::TwiddleTree<CpuBackend>,
) {
    let trace_polynomials = CpuBackend::interpolate_columns(trace, twiddles);
    let trace_polynomials = CpuBackend::evaluate_polynomials(
        trace_polynomials,
        commitment_scheme.config.fri_config.log_blowup_factor,
        twiddles,
        commitment_scheme.store_polynomials_coefficients,
        &commitment_scheme.base_column_pool,
    );
    let max_log_domain_size = trace_polynomials
        .iter()
        .map(|poly| poly.evals.domain.log_size())
        .max()
        .unwrap_or_default();
    let lifting_log_size = commitment_scheme
        .config
        .lifting_log_size
        .unwrap_or(max_log_domain_size);
    let commitment = MerkleProverLifted::<CpuBackend, Blake2sMerkleHasher>::commit(
        trace_polynomials
            .iter()
            .map(|poly| &poly.evals.values)
            .collect(),
        lifting_log_size,
    );

    let tree = CommitmentTreeProver {
        polynomials: trace_polynomials,
        commitment,
    };
    commitment_scheme.commit_tree(MaybeOwned::Owned(tree), prover_channel);
}

#[cfg(test)]
fn with_cpu_poseidon_commitment_scheme<R>(
    config: PcsConfig,
    log_n_rows: u32,
    main_trace: Vec<CircleEvaluation<CpuBackend, BaseField, BitReversedOrder>>,
    interaction_trace: Vec<CircleEvaluation<CpuBackend, BaseField, BitReversedOrder>>,
    f: impl FnOnce(&CommitmentSchemeProver<'_, CpuBackend, Blake2sMerkleChannel>) -> R,
) -> R {
    let twiddles = CpuBackend::precompute_twiddles(
        CanonicCoset::new(log_n_rows + LOG_EXPAND + config.fri_config.log_blowup_factor)
            .circle_domain()
            .half_coset,
    );
    let prover_channel = &mut Blake2sChannel::default();
    let mut commitment_scheme =
        CommitmentSchemeProver::<CpuBackend, Blake2sMerkleChannel>::new(config, &twiddles);
    let mut tree_builder = commitment_scheme.tree_builder();
    tree_builder.extend_evals(vec![]);
    tree_builder.commit(prover_channel);
    maybe_enable_poseidon_store_polynomials_coefficients(&mut commitment_scheme);

    commit_trace_cpu(
        &mut commitment_scheme,
        prover_channel,
        main_trace,
        &twiddles,
    );
    commit_trace_cpu(
        &mut commitment_scheme,
        prover_channel,
        interaction_trace,
        &twiddles,
    );

    f(&commitment_scheme)
}

#[cfg(test)]
fn cpu_composition_oods_eval_from_generated_traces(
    config: PcsConfig,
    log_n_rows: u32,
    lookup_elements: PoseidonElements,
    claimed_sum: SecureField,
    random_coeff: SecureField,
    oods_point: stwo::core::circle::CirclePoint<SecureField>,
    main_trace: Vec<CircleEvaluation<CpuBackend, BaseField, BitReversedOrder>>,
    interaction_trace: Vec<CircleEvaluation<CpuBackend, BaseField, BitReversedOrder>>,
    mode: PoseidonEvalMode,
) -> SecureField {
    let twiddles = CpuBackend::precompute_twiddles(
        CanonicCoset::new(log_n_rows + LOG_EXPAND + config.fri_config.log_blowup_factor)
            .circle_domain()
            .half_coset,
    );
    let prover_channel = &mut Blake2sChannel::default();
    let mut commitment_scheme =
        CommitmentSchemeProver::<CpuBackend, Blake2sMerkleChannel>::new(config, &twiddles);
    let mut tree_builder = commitment_scheme.tree_builder();
    tree_builder.extend_evals(vec![]);
    tree_builder.commit(prover_channel);
    maybe_enable_poseidon_store_polynomials_coefficients(&mut commitment_scheme);

    commit_trace_cpu(&mut commitment_scheme, prover_channel, main_trace, &twiddles);
    commit_trace_cpu(
        &mut commitment_scheme,
        prover_channel,
        interaction_trace,
        &twiddles,
    );

    let component = FrameworkComponent::new(
        &mut TraceLocationAllocator::default(),
        PoseidonEval {
            log_n_rows,
            lookup_elements,
            mode,
        },
        claimed_sum,
    );
    let n_preprocessed_columns = commitment_scheme.trees[PREPROCESSED_TRACE_IDX]
        .polynomials
        .len();
    let component_provers = ComponentProvers {
        components: vec![&component],
        n_preprocessed_columns,
    };
    let trace = commitment_scheme.trace();
    component_provers
        .compute_composition_polynomial(random_coeff, &trace)
        .eval_at_point(oods_point)
}

#[cfg(test)]
fn direct_trace_mask_values_from_committed_trees<
    B: stwo::prover::backend::BackendForChannel<MC>,
    MC: MerkleChannel,
>(
    commitment_scheme: &CommitmentSchemeProver<'_, B, MC>,
    sample_points: &TreeVec<Vec<Vec<stwo::core::circle::CirclePoint<SecureField>>>>,
) -> Option<TreeVec<Vec<Vec<SecureField>>>> {
    Some(TreeVec(
        commitment_scheme
            .trees
            .iter()
            .take(sample_points.len())
            .zip(sample_points.iter())
            .map(|(tree, tree_points)| {
                let tree_lifting_log_size = tree.commitment.layers.len() as u32 - 1;
                tree.polynomials
                    .iter()
                    .zip(tree_points.iter())
                    .map(|(poly, column_points)| {
                        let coeffs = poly.coeffs.as_ref()?;
                        Some(
                            column_points
                                .iter()
                                .map(|&point| {
                                    coeffs.eval_at_point(
                                        point.repeated_double(
                                            tree_lifting_log_size - poly.evals.domain.log_size(),
                                        ),
                                    )
                                })
                                .collect::<Vec<_>>(),
                        )
                    })
                    .collect::<Option<Vec<_>>>()
            })
            .collect::<Option<Vec<_>>>()?,
    ))
}

#[cfg(any(feature = "cuda-runtime", test))]
fn direct_trace_mask_values_from_committed_trees_with_prove_values_lifting<
    B: stwo::prover::backend::BackendForChannel<MC>,
    MC: MerkleChannel,
>(
    commitment_scheme: &CommitmentSchemeProver<'_, B, MC>,
    sample_points: &TreeVec<Vec<Vec<stwo::core::circle::CirclePoint<SecureField>>>>,
) -> Option<TreeVec<Vec<Vec<SecureField>>>> {
    let prove_values_lifting_log_size = commitment_scheme.trees.last()?.commitment.layers.len() as u32 - 1;
    Some(TreeVec(
        commitment_scheme
            .trees
            .iter()
            .take(sample_points.len())
            .zip(sample_points.iter())
            .map(|(tree, tree_points)| {
                tree.polynomials
                    .iter()
                    .zip(tree_points.iter())
                    .map(|(poly, column_points)| {
                        let coeffs = poly.coeffs.as_ref()?;
                        assert!(
                            prove_values_lifting_log_size >= poly.evals.domain.log_size(),
                            "prove_values lifting log size must cover every sampled polynomial"
                        );
                        Some(
                            column_points
                                .iter()
                                .map(|&point| {
                                    coeffs.eval_at_point(point.repeated_double(
                                        prove_values_lifting_log_size - poly.evals.domain.log_size(),
                                    ))
                                })
                                .collect::<Vec<_>>(),
                        )
                    })
                    .collect::<Option<Vec<_>>>()
            })
            .collect::<Option<Vec<_>>>()?,
    ))
}

#[cfg(test)]
fn count_trace_mask_value_mismatches(
    left: &TreeVec<Vec<Vec<SecureField>>>,
    right: &TreeVec<Vec<Vec<SecureField>>>,
) -> usize {
    left.iter()
        .zip(right.iter())
        .map(|(left_tree, right_tree)| {
            left_tree
                .iter()
                .zip(right_tree.iter())
                .map(|(left_column, right_column)| {
                    left_column
                        .iter()
                        .zip(right_column.iter())
                        .filter(|(left_value, right_value)| left_value != right_value)
                        .count()
                })
                .sum::<usize>()
        })
        .sum()
}

#[cfg(test)]
fn trace_mask_values_equal(
    left: &TreeVec<Vec<Vec<SecureField>>>,
    right: &TreeVec<Vec<Vec<SecureField>>>,
) -> bool {
    count_trace_mask_value_mismatches(left, right) == 0
}

#[cfg(test)]
fn cpu_trace_point_values_from_generated_traces(
    config: PcsConfig,
    log_n_rows: u32,
    sample_points: &TreeVec<Vec<Vec<stwo::core::circle::CirclePoint<SecureField>>>>,
    main_trace: Vec<CircleEvaluation<CpuBackend, BaseField, BitReversedOrder>>,
    interaction_trace: Vec<CircleEvaluation<CpuBackend, BaseField, BitReversedOrder>>,
) -> TreeVec<Vec<Vec<SecureField>>> {
    with_cpu_poseidon_commitment_scheme(
        config,
        log_n_rows,
        main_trace,
        interaction_trace,
        |commitment_scheme| {
            direct_trace_mask_values_from_committed_trees(commitment_scheme, sample_points)
                .expect("CPU committed trace-point values should be available")
        },
    )
}

#[cfg(test)]
#[allow(dead_code)]
fn cpu_trace_point_values_from_generated_traces_with_prove_values_lifting(
    config: PcsConfig,
    log_n_rows: u32,
    sample_points: &TreeVec<Vec<Vec<stwo::core::circle::CirclePoint<SecureField>>>>,
    main_trace: Vec<CircleEvaluation<CpuBackend, BaseField, BitReversedOrder>>,
    interaction_trace: Vec<CircleEvaluation<CpuBackend, BaseField, BitReversedOrder>>,
) -> TreeVec<Vec<Vec<SecureField>>> {
    with_cpu_poseidon_commitment_scheme(
        config,
        log_n_rows,
        main_trace,
        interaction_trace,
        |commitment_scheme| {
            direct_trace_mask_values_from_committed_trees_with_prove_values_lifting(
                commitment_scheme,
                sample_points,
            )
            .expect("CPU prove_values-lifted trace-point values should be available")
        },
    )
}

#[cfg(any(feature = "cuda-runtime", test))]
fn sampled_trace_mask_values_match_direct(
    sampled_values: &TreeVec<Vec<Vec<SecureField>>>,
    direct_trace_mask_values: &TreeVec<Vec<Vec<SecureField>>>,
) -> bool {
    sampled_values
        .iter()
        .take(direct_trace_mask_values.len())
        .zip(direct_trace_mask_values.iter())
        .all(|(sampled_tree, direct_tree)| sampled_tree == direct_tree)
}

#[cfg(all(feature = "cuda-runtime", test))]
fn characterize_point_contract_variants(
    component: &PoseidonBenchmarkComponent,
    sampled_values: &TreeVec<Vec<Vec<SecureField>>>,
    trace_point_values: &TreeVec<Vec<Vec<SecureField>>>,
    lifted_trace_point_values: &TreeVec<Vec<Vec<SecureField>>>,
    oods_point: stwo::core::circle::CirclePoint<SecureField>,
    random_coeff: SecureField,
    max_log_degree_bound: u32,
) -> Option<PointContractVariantSnapshot> {
    Some(PointContractVariantSnapshot {
        current_framework_eval: component.eval_composition_polynomial_at_point_with_contract(
            oods_point,
            sampled_values,
            random_coeff,
            max_log_degree_bound,
            component.log_n_rows,
        ),
        trace_denominator_eval: component.eval_composition_polynomial_at_point_with_contract(
            oods_point,
            sampled_values,
            random_coeff,
            component.log_n_rows,
            component.log_n_rows,
        ),
        trace_points_current_denominator_eval:
            component.eval_composition_polynomial_at_point_with_contract(
                oods_point,
                trace_point_values,
                random_coeff,
                max_log_degree_bound,
                component.log_n_rows,
            ),
        trace_points_trace_denominator_eval:
            component.eval_composition_polynomial_at_point_with_contract(
                oods_point,
                trace_point_values,
                random_coeff,
                component.log_n_rows,
                component.log_n_rows,
            ),
        trace_points_trace_lifted_denominator_eval:
            component.eval_composition_polynomial_at_point_with_contract(
                oods_point,
                lifted_trace_point_values,
                random_coeff,
                component.log_n_rows,
                component.log_n_rows,
            ),
    })
}

#[cfg(all(feature = "cuda-runtime", test))]
fn characterize_point_contribution_split(
    component: &PoseidonBenchmarkComponent,
    direct_trace_mask_values: &TreeVec<Vec<Vec<SecureField>>>,
    trace_point_values: &TreeVec<Vec<Vec<SecureField>>>,
    oods_point: stwo::core::circle::CirclePoint<SecureField>,
    random_coeff: SecureField,
    max_log_degree_bound: u32,
    config: PcsConfig,
    lookup_elements: PoseidonElements,
    claimed_sum: SecureField,
    main_trace: Vec<CircleEvaluation<CpuBackend, BaseField, BitReversedOrder>>,
    interaction_trace: Vec<CircleEvaluation<CpuBackend, BaseField, BitReversedOrder>>,
) -> PointContributionSplitSnapshot {
    let algebraic_only_component = PoseidonBenchmarkComponent::new_with_mode(
        component.log_n_rows,
        lookup_elements.clone(),
        claimed_sum,
        PoseidonEvalMode::AlgebraicOnly,
    );
    let point_full_eval = component.eval_composition_polynomial_at_point_with_contract(
        oods_point,
        direct_trace_mask_values,
        random_coeff,
        max_log_degree_bound,
        component.log_n_rows,
    );
    let point_algebraic_only_eval =
        algebraic_only_component.eval_composition_polynomial_at_point_with_contract(
            oods_point,
            direct_trace_mask_values,
            random_coeff,
            max_log_degree_bound,
            component.log_n_rows,
        );
    let point_algebraic_trace_denominator_eval =
        algebraic_only_component.eval_composition_polynomial_at_point_with_contract(
            oods_point,
            direct_trace_mask_values,
            random_coeff,
            component.log_n_rows,
            component.log_n_rows,
        );
    let trace_points_algebraic_only_eval =
        algebraic_only_component.eval_composition_polynomial_at_point_with_contract(
            oods_point,
            trace_point_values,
            random_coeff,
            max_log_degree_bound,
            component.log_n_rows,
        );
    let trace_points_algebraic_trace_denominator_eval =
        algebraic_only_component.eval_composition_polynomial_at_point_with_contract(
            oods_point,
            trace_point_values,
            random_coeff,
            component.log_n_rows,
            component.log_n_rows,
        );
    let cpu_trace_full_eval = cpu_composition_oods_eval_from_generated_traces(
        config,
        component.log_n_rows,
        lookup_elements.clone(),
        claimed_sum,
        random_coeff,
        oods_point,
        main_trace.clone(),
        interaction_trace.clone(),
        PoseidonEvalMode::Full,
    );
    let cpu_trace_algebraic_only_eval = cpu_composition_oods_eval_from_generated_traces(
        config,
        component.log_n_rows,
        lookup_elements,
        claimed_sum,
        random_coeff,
        oods_point,
        main_trace,
        interaction_trace,
        PoseidonEvalMode::AlgebraicOnly,
    );
    PointContributionSplitSnapshot {
        point_full_eval,
        point_algebraic_only_eval,
        point_algebraic_trace_denominator_eval,
        trace_points_algebraic_only_eval,
        trace_points_algebraic_trace_denominator_eval,
        point_logup_delta_eval: point_full_eval - point_algebraic_only_eval,
        cpu_trace_full_eval,
        cpu_trace_algebraic_only_eval,
        cpu_trace_logup_delta_eval: cpu_trace_full_eval - cpu_trace_algebraic_only_eval,
    }
}

#[cfg(all(feature = "cuda-runtime", test))]
fn characterize_logup_contract_variants(
    component: &PoseidonBenchmarkComponent,
    trace_point_values: &TreeVec<Vec<Vec<SecureField>>>,
    oods_point: stwo::core::circle::CirclePoint<SecureField>,
    random_coeff: SecureField,
    point_contribution_split: PointContributionSplitSnapshot,
) -> LogupContractVariantSnapshot {
    let preprocessed_mask = component
        .inner
        .preprocessed_column_indices()
        .iter()
        .map(|idx| &trace_point_values[PREPROCESSED_TRACE_IDX][*idx])
        .collect();

    let mut point_mask = trace_point_values.sub_tree(component.inner.trace_locations());
    point_mask[PREPROCESSED_TRACE_IDX] = preprocessed_mask;

    let mut preflight_evaluation_accumulator = PointEvaluationAccumulator::new(random_coeff);
    let mut preflight_evaluator = VariantPointEvaluator::new(
        point_mask,
        &mut preflight_evaluation_accumulator,
        coset_vanishing(CanonicCoset::new(component.log_n_rows).coset(), oods_point).inverse(),
        component.log_n_rows,
        component.inner.claimed_sum(),
    );
    diagnostics::eval_poseidon_constraints_mode_with_logup_finalization(
        &mut preflight_evaluator,
        &component.inner.lookup_elements,
        component.mode,
        LogupFinalizationMode::Skip,
    );
    let pending_interaction_mask_arities: Vec<_> = preflight_evaluator.mask
        [INTERACTION_TRACE_IDX][preflight_evaluator.col_index[INTERACTION_TRACE_IDX]..]
        .iter()
        .map(|mask| mask.len())
        .collect();
    let expected_sequential_arities = expected_logup_cumsum_arities(
        preflight_evaluator.logup.fracs.len(),
        LogupFinalizationMode::Sequential,
    );
    preflight_evaluator.logup.is_finalized = true;
    let sequential_first_mismatch =
        first_arity_mismatch(&expected_sequential_arities, &pending_interaction_mask_arities);
    let sequential_supported = sequential_first_mismatch.is_none()
        && pending_interaction_mask_arities.len() >= expected_sequential_arities.len();

    let trace_points_trace_in_pairs_eval =
        component.eval_composition_polynomial_at_point_with_logup_finalization(
            oods_point,
            trace_point_values,
            random_coeff,
            component.log_n_rows,
            component.log_n_rows,
            LogupFinalizationMode::InPairs,
        );
    let trace_points_trace_sequential_eval = sequential_supported.then(|| {
        component.eval_composition_polynomial_at_point_with_logup_finalization(
            oods_point,
            trace_point_values,
            random_coeff,
            component.log_n_rows,
            component.log_n_rows,
            LogupFinalizationMode::Sequential,
        )
    });

    LogupContractVariantSnapshot {
        trace_points_trace_in_pairs_eval,
        trace_points_trace_sequential_eval,
        trace_points_trace_in_pairs_logup_delta_eval:
            trace_points_trace_in_pairs_eval
                - point_contribution_split.trace_points_algebraic_trace_denominator_eval,
        trace_points_trace_sequential_logup_delta_eval:
            trace_points_trace_sequential_eval.map(|eval| {
                eval - point_contribution_split.trace_points_algebraic_trace_denominator_eval
            }),
        trace_points_trace_sequential_supported: sequential_supported,
        trace_points_trace_sequential_first_expected_arity:
            sequential_first_mismatch.map(|(expected, _)| expected),
        trace_points_trace_sequential_first_actual_arity:
            sequential_first_mismatch.map(|(_, actual)| actual),
        cpu_trace_logup_delta_eval: point_contribution_split.cpu_trace_logup_delta_eval,
    }
}

#[cfg(all(feature = "cuda-runtime", test))]
fn characterize_logup_only_variants(
    component: &PoseidonBenchmarkComponent,
    sampled_values: &TreeVec<Vec<Vec<SecureField>>>,
    trace_point_values: &TreeVec<Vec<Vec<SecureField>>>,
    oods_point: stwo::core::circle::CirclePoint<SecureField>,
    random_coeff: SecureField,
    max_log_degree_bound: u32,
    config: PcsConfig,
    lookup_elements: PoseidonElements,
    claimed_sum: SecureField,
    main_trace: Vec<CircleEvaluation<CpuBackend, BaseField, BitReversedOrder>>,
    interaction_trace: Vec<CircleEvaluation<CpuBackend, BaseField, BitReversedOrder>>,
) -> LogupOnlyVariantSnapshot {
    let logup_only_component = PoseidonBenchmarkComponent::new_with_mode(
        component.log_n_rows,
        lookup_elements,
        claimed_sum,
        PoseidonEvalMode::LogupOnly,
    );
    LogupOnlyVariantSnapshot {
        current_framework_eval: logup_only_component
            .eval_composition_polynomial_at_point_with_logup_finalization(
                oods_point,
                sampled_values,
                random_coeff,
                max_log_degree_bound,
                component.log_n_rows,
                LogupFinalizationMode::InPairs,
            ),
        trace_denominator_eval: logup_only_component
            .eval_composition_polynomial_at_point_with_logup_finalization(
                oods_point,
                sampled_values,
                random_coeff,
                component.log_n_rows,
                component.log_n_rows,
                LogupFinalizationMode::InPairs,
            ),
        trace_points_current_denominator_eval: logup_only_component
            .eval_composition_polynomial_at_point_with_logup_finalization(
                oods_point,
                trace_point_values,
                random_coeff,
                max_log_degree_bound,
                component.log_n_rows,
                LogupFinalizationMode::InPairs,
            ),
        trace_points_trace_in_pairs_eval: logup_only_component
            .eval_composition_polynomial_at_point_with_logup_finalization(
                oods_point,
                trace_point_values,
                random_coeff,
                component.log_n_rows,
                component.log_n_rows,
                LogupFinalizationMode::InPairs,
            ),
        cpu_trace_eval: cpu_composition_oods_eval_from_generated_traces(
            config,
            component.log_n_rows,
            logup_only_component.inner.lookup_elements.clone(),
            claimed_sum,
            random_coeff,
            oods_point,
            main_trace,
            interaction_trace,
            PoseidonEvalMode::LogupOnly,
        ),
    }
}

#[cfg(all(feature = "cuda-runtime", test))]
fn fraction_snapshot(
    fraction: &Fraction<SecureField, SecureField>,
) -> SecureFieldFractionSnapshot {
    SecureFieldFractionSnapshot {
        numerator: fraction.numerator,
        denominator: fraction.denominator,
    }
}

#[cfg(all(feature = "cuda-runtime", test))]
fn eval_poseidon_logup_only_with_collection(
    eval: &mut CollectingPointEvaluator<'_>,
    lookup_elements: &PoseidonElements,
) {
    for _ in 0..N_INSTANCES_PER_ROW {
        let mut state: [_; N_STATE] = std::array::from_fn(|_| eval.next_trace_mask());
        let initial_state = state.clone();

        (0..N_HALF_FULL_ROUNDS).for_each(|round| {
            (0..N_STATE).for_each(|index| {
                state[index] += EXTERNAL_ROUND_CONSTS[round][index];
            });
            apply_external_round_matrix(&mut state);
            state = std::array::from_fn(|index| pow5(state[index].clone()));
            state.iter_mut().for_each(|value| {
                *value = eval.next_trace_mask();
            });
        });

        (0..N_PARTIAL_ROUNDS).for_each(|round| {
            state[0] += INTERNAL_ROUND_CONSTS[round];
            apply_internal_round_matrix(&mut state);
            state[0] = pow5(state[0].clone());
            state[0] = eval.next_trace_mask();
        });

        (0..N_HALF_FULL_ROUNDS).for_each(|round| {
            (0..N_STATE).for_each(|index| {
                state[index] += EXTERNAL_ROUND_CONSTS[round + N_HALF_FULL_ROUNDS][index];
            });
            apply_external_round_matrix(&mut state);
            state = std::array::from_fn(|index| pow5(state[index].clone()));
            state.iter_mut().for_each(|value| {
                *value = eval.next_trace_mask();
            });
        });

        eval.logged_relation_values.push(initial_state.clone());
        eval.write_logup_frac(Fraction::new(
            SecureField::one(),
            lookup_elements.combine(&initial_state),
        ));
        eval.logged_relation_values.push(state.clone());
        eval.write_logup_frac(Fraction::new(
            -SecureField::one(),
            lookup_elements.combine(&state),
        ));
    }

    eval.finalize_logup_in_pairs();
}

#[cfg(all(feature = "cuda-runtime", test))]
fn characterize_in_pairs_logup_boundary(
    component: &PoseidonBenchmarkComponent,
    trace_point_values: &TreeVec<Vec<Vec<SecureField>>>,
    oods_point: stwo::core::circle::CirclePoint<SecureField>,
    random_coeff: SecureField,
) -> InPairsLogupBoundarySnapshot {
    let logup_only_component = PoseidonBenchmarkComponent::new_with_mode(
        component.log_n_rows,
        component.inner.lookup_elements.clone(),
        component.inner.claimed_sum(),
        PoseidonEvalMode::LogupOnly,
    );
    let preprocessed_mask = logup_only_component
        .inner
        .preprocessed_column_indices()
        .iter()
        .map(|idx| &trace_point_values[PREPROCESSED_TRACE_IDX][*idx])
        .collect();
    let mut mask_points = trace_point_values.sub_tree(logup_only_component.inner.trace_locations());
    mask_points[PREPROCESSED_TRACE_IDX] = preprocessed_mask;
    let mut evaluator = CollectingPointEvaluator::new(
        mask_points,
        coset_vanishing(CanonicCoset::new(component.log_n_rows).coset(), oods_point).inverse(),
        component.log_n_rows,
        component.inner.claimed_sum(),
    );
    eval_poseidon_logup_only_with_collection(
        &mut evaluator,
        &logup_only_component.inner.lookup_elements,
    );

    InPairsLogupBoundarySnapshot {
        frac_count: evaluator.logged_fractions.len(),
        batch_count: evaluator.last_batched_fractions.len(),
        denom_inverse: evaluator.denom_inverse,
        cumsum_shift: evaluator.logup.cumsum_shift,
        logged_relation_values: evaluator.logged_relation_values,
        first_fraction: evaluator.logged_fractions.first().map(fraction_snapshot),
        second_fraction: evaluator.logged_fractions.get(1).map(fraction_snapshot),
        batched_fraction: evaluator.last_batched_fractions.last().map(fraction_snapshot),
        prev_row_cumsum: evaluator.last_prev_row_cumsum,
        cur_cumsum: evaluator.last_cur_cumsum,
        shifted_diff: evaluator.last_shifted_diff,
        cumsum_times_denominator: evaluator.last_cumsum_times_denominator,
        numerator: evaluator.last_numerator,
        constraint_before_denom_inverse: evaluator.last_constraint_before_denom_inverse,
        emitted_evaluation_count: evaluator.emitted_evaluations.len(),
        emitted_horner_eval: accumulate_horner(random_coeff, &evaluator.emitted_evaluations),
    }
}

#[cfg(all(feature = "cuda-runtime", test))]
fn characterize_in_pairs_cumsum_variants(
    boundary: &InPairsLogupBoundarySnapshot,
    cpu_trace_eval: SecureField,
) -> InPairsCumsumVariantSnapshot {
    let batched_fraction = boundary
        .batched_fraction
        .expect("in-pairs boundary should capture the final batched fraction");
    let prev_row_cumsum = boundary
        .prev_row_cumsum
        .expect("in-pairs boundary should capture prev-row cumsum");
    let cur_cumsum = boundary
        .cur_cumsum
        .expect("in-pairs boundary should capture current-row cumsum");

    let eval_for_shifted_diff = |shifted_diff: SecureField| {
        boundary.denom_inverse
            * (shifted_diff * batched_fraction.denominator - batched_fraction.numerator)
    };

    InPairsCumsumVariantSnapshot {
        current_eval: eval_for_shifted_diff(cur_cumsum - prev_row_cumsum + boundary.cumsum_shift),
        swapped_diff_eval: eval_for_shifted_diff(
            prev_row_cumsum - cur_cumsum + boundary.cumsum_shift,
        ),
        subtract_shift_eval: eval_for_shifted_diff(
            cur_cumsum - prev_row_cumsum - boundary.cumsum_shift,
        ),
        swapped_diff_subtract_shift_eval: eval_for_shifted_diff(
            prev_row_cumsum - cur_cumsum - boundary.cumsum_shift,
        ),
        no_shift_eval: eval_for_shifted_diff(cur_cumsum - prev_row_cumsum),
        cpu_trace_eval,
    }
}

#[cfg(all(feature = "cuda-runtime", test))]
fn characterize_in_pairs_fraction_assembly_variants(
    boundary: &InPairsLogupBoundarySnapshot,
    cpu_trace_eval: SecureField,
) -> InPairsFractionAssemblyVariantSnapshot {
    let first_fraction = boundary
        .first_fraction
        .expect("in-pairs boundary should capture the first fraction");
    let second_fraction = boundary
        .second_fraction
        .expect("in-pairs boundary should capture the second fraction");
    let batched_fraction = boundary
        .batched_fraction
        .expect("in-pairs boundary should capture the final batched fraction");
    let prev_row_cumsum = boundary
        .prev_row_cumsum
        .expect("in-pairs boundary should capture prev-row cumsum");
    let cur_cumsum = boundary
        .cur_cumsum
        .expect("in-pairs boundary should capture current-row cumsum");
    let shifted_diff = cur_cumsum - prev_row_cumsum + boundary.cumsum_shift;
    let eval_for_fraction = |fraction: Fraction<SecureField, SecureField>| {
        boundary.denom_inverse * (shifted_diff * fraction.denominator - fraction.numerator)
    };

    let first_fraction = Fraction::new(first_fraction.numerator, first_fraction.denominator);
    let second_fraction = Fraction::new(second_fraction.numerator, second_fraction.denominator);
    let both_positive_second = Fraction::new(
        SecureField::one(),
        second_fraction.denominator,
    );
    let swapped_sign_first = Fraction::new(
        -SecureField::one(),
        first_fraction.denominator,
    );
    let naive_pair_current = Fraction::new(
        first_fraction.numerator + second_fraction.numerator,
        first_fraction.denominator + second_fraction.denominator,
    );
    let naive_pair_positive = Fraction::new(
        SecureField::one() + SecureField::one(),
        first_fraction.denominator + second_fraction.denominator,
    );

    InPairsFractionAssemblyVariantSnapshot {
        current_eval: eval_for_fraction(Fraction::new(
            batched_fraction.numerator,
            batched_fraction.denominator,
        )),
        first_only_eval: eval_for_fraction(first_fraction.clone()),
        second_only_eval: eval_for_fraction(second_fraction.clone()),
        both_positive_eval: eval_for_fraction(first_fraction.clone() + both_positive_second),
        swapped_signs_eval: eval_for_fraction(swapped_sign_first + Fraction::new(
            SecureField::one(),
            second_fraction.denominator,
        )),
        naive_pair_current_eval: eval_for_fraction(naive_pair_current),
        naive_pair_positive_eval: eval_for_fraction(naive_pair_positive),
        cpu_trace_eval,
    }
}

#[cfg(all(feature = "cuda-runtime", test))]
fn characterize_in_pairs_denominator_variants(
    boundary: &InPairsLogupBoundarySnapshot,
    lookup_elements: &PoseidonElements,
    cpu_trace_eval: SecureField,
) -> InPairsDenominatorVariantSnapshot {
    let first_values = boundary
        .logged_relation_values
        .first()
        .expect("in-pairs boundary should capture the first relation values");
    let second_values = boundary
        .logged_relation_values
        .get(1)
        .expect("in-pairs boundary should capture the second relation values");
    let prev_row_cumsum = boundary
        .prev_row_cumsum
        .expect("in-pairs boundary should capture prev-row cumsum");
    let cur_cumsum = boundary
        .cur_cumsum
        .expect("in-pairs boundary should capture current-row cumsum");
    let shifted_diff = cur_cumsum - prev_row_cumsum + boundary.cumsum_shift;
    let eval_for_dens = |first_den: SecureField, second_den: SecureField| {
        let fraction = Fraction::new(SecureField::one(), first_den)
            + Fraction::new(-SecureField::one(), second_den);
        boundary.denom_inverse * (shifted_diff * fraction.denominator - fraction.numerator)
    };

    let mut first_reversed = *first_values;
    first_reversed.reverse();
    let mut second_reversed = *second_values;
    second_reversed.reverse();

    InPairsDenominatorVariantSnapshot {
        current_eval: eval_for_dens(
            lookup_elements.combine(first_values),
            lookup_elements.combine(second_values),
        ),
        first_reversed_eval: eval_for_dens(
            lookup_elements.combine(&first_reversed),
            lookup_elements.combine(second_values),
        ),
        second_reversed_eval: eval_for_dens(
            lookup_elements.combine(first_values),
            lookup_elements.combine(&second_reversed),
        ),
        both_reversed_eval: eval_for_dens(
            lookup_elements.combine(&first_reversed),
            lookup_elements.combine(&second_reversed),
        ),
        cpu_trace_eval,
    }
}

#[cfg(all(feature = "cuda-runtime", test))]
fn characterize_algebraic_family_split(
    component: &PoseidonBenchmarkComponent,
    direct_trace_mask_values: &TreeVec<Vec<Vec<SecureField>>>,
    oods_point: stwo::core::circle::CirclePoint<SecureField>,
    random_coeff: SecureField,
    max_log_degree_bound: u32,
    config: PcsConfig,
    lookup_elements: PoseidonElements,
    claimed_sum: SecureField,
    main_trace: Vec<CircleEvaluation<CpuBackend, BaseField, BitReversedOrder>>,
    interaction_trace: Vec<CircleEvaluation<CpuBackend, BaseField, BitReversedOrder>>,
) -> AlgebraicFamilySplitSnapshot {
    let first_half_full_component = PoseidonBenchmarkComponent::new_with_mode(
        component.log_n_rows,
        lookup_elements.clone(),
        claimed_sum,
        PoseidonEvalMode::AlgebraicFirstHalfFullOnly,
    );
    let partial_rounds_component = PoseidonBenchmarkComponent::new_with_mode(
        component.log_n_rows,
        lookup_elements.clone(),
        claimed_sum,
        PoseidonEvalMode::AlgebraicPartialRoundsOnly,
    );
    let second_half_full_component = PoseidonBenchmarkComponent::new_with_mode(
        component.log_n_rows,
        lookup_elements.clone(),
        claimed_sum,
        PoseidonEvalMode::AlgebraicSecondHalfFullOnly,
    );

    let point_first_half_full_eval =
        first_half_full_component.eval_composition_polynomial_at_point_with_contract(
            oods_point,
            direct_trace_mask_values,
            random_coeff,
            max_log_degree_bound,
            component.log_n_rows,
        );
    let point_partial_rounds_eval =
        partial_rounds_component.eval_composition_polynomial_at_point_with_contract(
            oods_point,
            direct_trace_mask_values,
            random_coeff,
            max_log_degree_bound,
            component.log_n_rows,
        );
    let point_second_half_full_eval =
        second_half_full_component.eval_composition_polynomial_at_point_with_contract(
            oods_point,
            direct_trace_mask_values,
            random_coeff,
            max_log_degree_bound,
            component.log_n_rows,
        );

    let cpu_first_half_full_eval = cpu_composition_oods_eval_from_generated_traces(
        config,
        component.log_n_rows,
        lookup_elements.clone(),
        claimed_sum,
        random_coeff,
        oods_point,
        main_trace.clone(),
        interaction_trace.clone(),
        PoseidonEvalMode::AlgebraicFirstHalfFullOnly,
    );
    let cpu_partial_rounds_eval = cpu_composition_oods_eval_from_generated_traces(
        config,
        component.log_n_rows,
        lookup_elements.clone(),
        claimed_sum,
        random_coeff,
        oods_point,
        main_trace.clone(),
        interaction_trace.clone(),
        PoseidonEvalMode::AlgebraicPartialRoundsOnly,
    );
    let cpu_second_half_full_eval = cpu_composition_oods_eval_from_generated_traces(
        config,
        component.log_n_rows,
        lookup_elements,
        claimed_sum,
        random_coeff,
        oods_point,
        main_trace,
        interaction_trace,
        PoseidonEvalMode::AlgebraicSecondHalfFullOnly,
    );

    AlgebraicFamilySplitSnapshot {
        point_first_half_full_eval,
        point_partial_rounds_eval,
        point_second_half_full_eval,
        cpu_first_half_full_eval,
        cpu_partial_rounds_eval,
        cpu_second_half_full_eval,
    }
}

#[cfg(all(feature = "cuda-runtime", test))]
fn accumulate_horner(random_coeff: SecureField, evaluations: &[SecureField]) -> SecureField {
    evaluations.iter().fold(SecureField::zero(), |acc, evaluation| {
        acc * random_coeff + *evaluation
    })
}

#[cfg(all(feature = "cuda-runtime", test))]
fn characterize_algebraic_sequence_law(
    component: &PoseidonBenchmarkComponent,
    direct_trace_mask_values: &TreeVec<Vec<Vec<SecureField>>>,
    oods_point: stwo::core::circle::CirclePoint<SecureField>,
    random_coeff: SecureField,
    max_log_degree_bound: u32,
    lookup_elements: PoseidonElements,
    claimed_sum: SecureField,
    cpu_trace_algebraic_only_eval: SecureField,
) -> AlgebraicSequenceLawSnapshot {
    let algebraic_only_component = PoseidonBenchmarkComponent::new_with_mode(
        component.log_n_rows,
        lookup_elements,
        claimed_sum,
        PoseidonEvalMode::AlgebraicOnly,
    );
    let preprocessed_mask = algebraic_only_component
        .inner
        .preprocessed_column_indices()
        .iter()
        .map(|idx| &direct_trace_mask_values[PREPROCESSED_TRACE_IDX][*idx])
        .collect();
    let mut mask_points = direct_trace_mask_values.sub_tree(algebraic_only_component.inner.trace_locations());
    mask_points[PREPROCESSED_TRACE_IDX] = preprocessed_mask;
    let evaluator = CollectingPointEvaluator::new(
        mask_points,
        coset_vanishing(CanonicCoset::new(max_log_degree_bound).coset(), oods_point).inverse(),
        component.log_n_rows,
        component.inner.claimed_sum(),
    );
    let evaluator = algebraic_only_component.inner.evaluate(evaluator);
    let current_horner_eval = accumulate_horner(random_coeff, &evaluator.emitted_evaluations);
    let reversed_evaluations = evaluator
        .emitted_evaluations
        .iter()
        .rev()
        .copied()
        .collect::<Vec<_>>();
    let reversed_horner_eval = accumulate_horner(random_coeff, &reversed_evaluations);
    let point_algebraic_only_eval =
        algebraic_only_component.eval_composition_polynomial_at_point_with_contract(
            oods_point,
            direct_trace_mask_values,
            random_coeff,
            max_log_degree_bound,
            component.log_n_rows,
        );

    AlgebraicSequenceLawSnapshot {
        evaluation_count: evaluator.emitted_evaluations.len(),
        current_horner_eval,
        reversed_horner_eval,
        point_algebraic_only_eval,
        cpu_trace_algebraic_only_eval,
    }
}

#[cfg(all(feature = "cuda-runtime", test))]
fn characterize_algebraic_per_round_split(
    component: &PoseidonBenchmarkComponent,
    direct_trace_mask_values: &TreeVec<Vec<Vec<SecureField>>>,
    oods_point: stwo::core::circle::CirclePoint<SecureField>,
    random_coeff: SecureField,
    max_log_degree_bound: u32,
    config: PcsConfig,
    lookup_elements: PoseidonElements,
    claimed_sum: SecureField,
    main_trace: Vec<CircleEvaluation<CpuBackend, BaseField, BitReversedOrder>>,
    interaction_trace: Vec<CircleEvaluation<CpuBackend, BaseField, BitReversedOrder>>,
) -> AlgebraicPerRoundSnapshot {
    let point_first_half_full_round_evals = (0..N_HALF_FULL_ROUNDS)
        .map(|round| {
            PoseidonBenchmarkComponent::new_with_mode(
                component.log_n_rows,
                lookup_elements.clone(),
                claimed_sum,
                PoseidonEvalMode::AlgebraicFirstHalfFullRound(round),
            )
            .eval_composition_polynomial_at_point_with_contract(
                oods_point,
                direct_trace_mask_values,
                random_coeff,
                max_log_degree_bound,
                component.log_n_rows,
            )
        })
        .collect::<Vec<_>>();
    let point_partial_round_evals = (0..N_PARTIAL_ROUNDS)
        .map(|round| {
            PoseidonBenchmarkComponent::new_with_mode(
                component.log_n_rows,
                lookup_elements.clone(),
                claimed_sum,
                PoseidonEvalMode::AlgebraicPartialRound(round),
            )
            .eval_composition_polynomial_at_point_with_contract(
                oods_point,
                direct_trace_mask_values,
                random_coeff,
                max_log_degree_bound,
                component.log_n_rows,
            )
        })
        .collect::<Vec<_>>();
    let point_second_half_full_round_evals = (0..N_HALF_FULL_ROUNDS)
        .map(|round| {
            PoseidonBenchmarkComponent::new_with_mode(
                component.log_n_rows,
                lookup_elements.clone(),
                claimed_sum,
                PoseidonEvalMode::AlgebraicSecondHalfFullRound(round),
            )
            .eval_composition_polynomial_at_point_with_contract(
                oods_point,
                direct_trace_mask_values,
                random_coeff,
                max_log_degree_bound,
                component.log_n_rows,
            )
        })
        .collect::<Vec<_>>();

    let cpu_first_half_full_round_evals = (0..N_HALF_FULL_ROUNDS)
        .map(|round| {
            cpu_composition_oods_eval_from_generated_traces(
                config,
                component.log_n_rows,
                lookup_elements.clone(),
                claimed_sum,
                random_coeff,
                oods_point,
                main_trace.clone(),
                interaction_trace.clone(),
                PoseidonEvalMode::AlgebraicFirstHalfFullRound(round),
            )
        })
        .collect::<Vec<_>>();
    let cpu_partial_round_evals = (0..N_PARTIAL_ROUNDS)
        .map(|round| {
            cpu_composition_oods_eval_from_generated_traces(
                config,
                component.log_n_rows,
                lookup_elements.clone(),
                claimed_sum,
                random_coeff,
                oods_point,
                main_trace.clone(),
                interaction_trace.clone(),
                PoseidonEvalMode::AlgebraicPartialRound(round),
            )
        })
        .collect::<Vec<_>>();
    let cpu_second_half_full_round_evals = (0..N_HALF_FULL_ROUNDS)
        .map(|round| {
            cpu_composition_oods_eval_from_generated_traces(
                config,
                component.log_n_rows,
                lookup_elements.clone(),
                claimed_sum,
                random_coeff,
                oods_point,
                main_trace.clone(),
                interaction_trace.clone(),
                PoseidonEvalMode::AlgebraicSecondHalfFullRound(round),
            )
        })
        .collect::<Vec<_>>();

    AlgebraicPerRoundSnapshot {
        point_first_half_full_round_evals,
        point_partial_round_evals,
        point_second_half_full_round_evals,
        cpu_first_half_full_round_evals,
        cpu_partial_round_evals,
        cpu_second_half_full_round_evals,
    }
}

#[cfg(all(feature = "cuda-runtime", test))]
fn characterize_algebraic_per_cell_split(
    component: &PoseidonBenchmarkComponent,
    direct_trace_mask_values: &TreeVec<Vec<Vec<SecureField>>>,
    oods_point: stwo::core::circle::CirclePoint<SecureField>,
    random_coeff: SecureField,
    max_log_degree_bound: u32,
    config: PcsConfig,
    lookup_elements: PoseidonElements,
    claimed_sum: SecureField,
    main_trace: Vec<CircleEvaluation<CpuBackend, BaseField, BitReversedOrder>>,
    interaction_trace: Vec<CircleEvaluation<CpuBackend, BaseField, BitReversedOrder>>,
) -> AlgebraicPerCellSnapshot {
    let point_first_half_full_cell_evals = (0..N_HALF_FULL_ROUNDS)
        .map(|round| {
            (0..N_STATE)
                .map(|index| {
                    PoseidonBenchmarkComponent::new_with_mode(
                        component.log_n_rows,
                        lookup_elements.clone(),
                        claimed_sum,
                        PoseidonEvalMode::AlgebraicFirstHalfFullCell(round, index),
                    )
                    .eval_composition_polynomial_at_point_with_contract(
                        oods_point,
                        direct_trace_mask_values,
                        random_coeff,
                        max_log_degree_bound,
                        component.log_n_rows,
                    )
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let point_second_half_full_cell_evals = (0..N_HALF_FULL_ROUNDS)
        .map(|round| {
            (0..N_STATE)
                .map(|index| {
                    PoseidonBenchmarkComponent::new_with_mode(
                        component.log_n_rows,
                        lookup_elements.clone(),
                        claimed_sum,
                        PoseidonEvalMode::AlgebraicSecondHalfFullCell(round, index),
                    )
                    .eval_composition_polynomial_at_point_with_contract(
                        oods_point,
                        direct_trace_mask_values,
                        random_coeff,
                        max_log_degree_bound,
                        component.log_n_rows,
                    )
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    let cpu_first_half_full_cell_evals = (0..N_HALF_FULL_ROUNDS)
        .map(|round| {
            (0..N_STATE)
                .map(|index| {
                    cpu_composition_oods_eval_from_generated_traces(
                        config,
                        component.log_n_rows,
                        lookup_elements.clone(),
                        claimed_sum,
                        random_coeff,
                        oods_point,
                        main_trace.clone(),
                        interaction_trace.clone(),
                        PoseidonEvalMode::AlgebraicFirstHalfFullCell(round, index),
                    )
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let cpu_second_half_full_cell_evals = (0..N_HALF_FULL_ROUNDS)
        .map(|round| {
            (0..N_STATE)
                .map(|index| {
                    cpu_composition_oods_eval_from_generated_traces(
                        config,
                        component.log_n_rows,
                        lookup_elements.clone(),
                        claimed_sum,
                        random_coeff,
                        oods_point,
                        main_trace.clone(),
                        interaction_trace.clone(),
                        PoseidonEvalMode::AlgebraicSecondHalfFullCell(round, index),
                    )
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    AlgebraicPerCellSnapshot {
        point_first_half_full_cell_evals,
        point_second_half_full_cell_evals,
        cpu_first_half_full_cell_evals,
        cpu_second_half_full_cell_evals,
    }
}

#[cfg(feature = "cuda-runtime")]
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

#[cfg(feature = "cuda-runtime")]
struct TraceCommitBreakdown {
    interpolation_ms: f64,
    extension_ms: f64,
    merkle_ms: f64,
}

#[cfg(feature = "cuda-runtime")]
fn commit_trace_with_breakdown(
    commitment_scheme: &mut CommitmentSchemeProver<'_, CudaBackend, Blake2sMerkleChannel>,
    prover_channel: &mut Blake2sChannel,
    trace: Vec<CircleEvaluation<CudaBackend, BaseField, BitReversedOrder>>,
    twiddles: &stwo::prover::poly::twiddles::TwiddleTree<CudaBackend>,
) -> TraceCommitBreakdown {
    let interpolation_start = Instant::now();
    let trace_polynomials = CudaBackend::interpolate_columns(trace, twiddles);
    let interpolation_ms = interpolation_start.elapsed().as_secs_f64() * 1000.0;

    let extension_start = Instant::now();
    let trace_polynomials = CudaBackend::evaluate_polynomials(
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
    let commitment = MerkleProverLifted::<CudaBackend, Blake2sMerkleHasher>::commit(
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

#[cfg(feature = "cuda-runtime")]
fn verify_poseidon_blake(
    component: &PoseidonBenchmarkComponent,
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
    commitment_scheme.commit(
        proof.commitments[INTERACTION_TRACE_IDX],
        &sizes[INTERACTION_TRACE_IDX],
        verifier_channel,
    );
    verify(
        &[component as &dyn Component],
        verifier_channel,
        commitment_scheme,
        proof.clone(),
    )
}

#[cfg(feature = "cuda-runtime")]
fn generate_poseidon_trace_evaluations(
    log_n_rows: u32,
) -> (
    Vec<CircleEvaluation<CudaBackend, BaseField, BitReversedOrder>>,
    CudaLookupData,
    PoseidonSentinel,
) {
    let domain_size = 1usize << log_n_rows;
    let trace_columns = (0..N_COLUMNS)
        .map(|_| BaseFieldVec::new_zeroes(domain_size))
        .collect::<Vec<_>>();
    let lookup_data = CudaLookupData {
        initial_state: std::array::from_fn(|_| {
            std::array::from_fn(|_| BaseFieldVec::new_zeroes(domain_size))
        }),
        final_state: std::array::from_fn(|_| {
            std::array::from_fn(|_| BaseFieldVec::new_zeroes(domain_size))
        }),
    };

    let trace_column_ptrs = trace_columns
        .iter()
        .map(|column| column.device_ptr)
        .collect::<Vec<_>>();
    let lookup_init_ptrs = lookup_data
        .initial_state
        .iter()
        .flat_map(|row| row.iter().map(|column| column.device_ptr))
        .collect::<Vec<_>>();
    let lookup_final_ptrs = lookup_data
        .final_state
        .iter()
        .flat_map(|row| row.iter().map(|column| column.device_ptr))
        .collect::<Vec<_>>();

    generate_poseidon_traces(PoseidonTraceRequest {
        trace_columns: &trace_column_ptrs,
        lookup_init: &lookup_init_ptrs,
        lookup_final: &lookup_final_ptrs,
        trace_log_len: log_n_rows,
    });

    let sentinel = PoseidonSentinel {
        trace_first_column_first_value: trace_columns[0].get_data(0).0,
        trace_last_column_first_value: trace_columns[N_COLUMNS - 1].get_data(0).0,
        interaction_first_column_first_value: 0,
        claimed_sum_m31: [0; 4],
    };

    let domain = CanonicCoset::new(log_n_rows).circle_domain();
    let trace = trace_columns
        .into_iter()
        .map(|values| CircleEvaluation::new(domain, values))
        .collect::<Vec<_>>();

    (trace, lookup_data, sentinel)
}

#[cfg(feature = "cuda-runtime")]
fn generate_poseidon_interaction_trace_evaluations(
    log_n_rows: u32,
    lookup_data: CudaLookupData,
    lookup_elements: &PoseidonElements,
) -> (
    Vec<CircleEvaluation<CudaBackend, BaseField, BitReversedOrder>>,
    SecureField,
    PoseidonSentinel,
) {
    let domain_size = 1usize << log_n_rows;
    let claimed_sum = BaseFieldVec::new_uninitialized(4);
    let interaction_columns = (0..(4 * N_INSTANCES_PER_ROW))
        .map(|_| BaseFieldVec::new_zeroes(domain_size))
        .collect::<Vec<_>>();

    let lookup_init_ptrs = lookup_data
        .initial_state
        .iter()
        .flat_map(|row| row.iter().map(|column| column.device_ptr))
        .collect::<Vec<_>>();
    let lookup_final_ptrs = lookup_data
        .final_state
        .iter()
        .flat_map(|row| row.iter().map(|column| column.device_ptr))
        .collect::<Vec<_>>();
    let interaction_ptrs = interaction_columns
        .iter()
        .map(|column| column.device_ptr)
        .collect::<Vec<_>>();

    generate_poseidon_interaction_traces(PoseidonInteractionTraceRequest {
        lookup_elements,
        lookup_init: &lookup_init_ptrs,
        lookup_final: &lookup_final_ptrs,
        log_size: log_n_rows,
        interaction_traces: &interaction_ptrs,
        claimed_sum: &claimed_sum,
    });

    let claimed_sum_m31_values = claimed_sum.to_vec();
    let claimed_sum_field = SecureField::from_m31_array([
        claimed_sum_m31_values[0],
        claimed_sum_m31_values[1],
        claimed_sum_m31_values[2],
        claimed_sum_m31_values[3],
    ]);

    let sentinel = PoseidonSentinel {
        trace_first_column_first_value: 0,
        trace_last_column_first_value: 0,
        interaction_first_column_first_value: interaction_columns[0].get_data(0).0,
        claimed_sum_m31: [
            claimed_sum_m31_values[0].0,
            claimed_sum_m31_values[1].0,
            claimed_sum_m31_values[2].0,
            claimed_sum_m31_values[3].0,
        ],
    };

    let domain = CanonicCoset::new(log_n_rows).circle_domain();
    let interaction_trace = interaction_columns
        .into_iter()
        .map(|values| CircleEvaluation::new(domain, values))
        .collect::<Vec<_>>();

    (interaction_trace, claimed_sum_field, sentinel)
}

#[cfg(feature = "cuda-runtime")]
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

#[cfg(feature = "cuda-runtime")]
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

#[cfg(any(feature = "cuda-runtime", test))]
fn apply_m4<F>(x: [F; 4]) -> [F; 4]
where
    F: Clone + AddAssign<F> + Add<F, Output = F> + Sub<F, Output = F> + Mul<BaseField, Output = F>,
{
    let t0 = x[0].clone() + x[1].clone();
    let t02 = t0.clone() + t0.clone();
    let t1 = x[2].clone() + x[3].clone();
    let t12 = t1.clone() + t1.clone();
    let t2 = x[1].clone() + x[1].clone() + t1.clone();
    let t3 = x[3].clone() + x[3].clone() + t0.clone();
    let t4 = t12.clone() + t12.clone() + t3.clone();
    let t5 = t02.clone() + t02.clone() + t2.clone();
    let t6 = t3.clone() + t5.clone();
    let t7 = t2.clone() + t4.clone();
    [t6, t5, t7, t4]
}

#[cfg(any(feature = "cuda-runtime", test))]
fn apply_external_round_matrix<F>(state: &mut [F; 16])
where
    F: Clone + AddAssign<F> + Add<F, Output = F> + Sub<F, Output = F> + Mul<BaseField, Output = F>,
{
    for i in 0..4 {
        let [a, b, c, d] = apply_m4([
            state[4 * i].clone(),
            state[4 * i + 1].clone(),
            state[4 * i + 2].clone(),
            state[4 * i + 3].clone(),
        ]);
        state[4 * i] = a;
        state[4 * i + 1] = b;
        state[4 * i + 2] = c;
        state[4 * i + 3] = d;
    }
    for j in 0..4 {
        let s =
            state[j].clone() + state[j + 4].clone() + state[j + 8].clone() + state[j + 12].clone();
        for i in 0..4 {
            state[4 * i + j] += s.clone();
        }
    }
}

#[cfg(any(feature = "cuda-runtime", test))]
fn apply_internal_round_matrix<F>(state: &mut [F; 16])
where
    F: Clone + AddAssign<F> + Add<F, Output = F> + Sub<F, Output = F> + Mul<BaseField, Output = F>,
{
    let sum = state[1..]
        .iter()
        .cloned()
        .fold(state[0].clone(), |acc, s| acc + s);
    state.iter_mut().enumerate().for_each(|(index, value)| {
        *value = value.clone() * BaseField::from_u32_unchecked(1 << (index + 1)) + sum.clone();
    });
}

#[cfg(any(feature = "cuda-runtime", test))]
fn poseidon_store_polynomials_coefficients_enabled() -> bool {
    std::env::var("STWO_POSEIDON_STORE_POLYNOMIALS_COEFFS")
        .map(|value| value != "0")
        .unwrap_or(true)
}

#[cfg(any(feature = "cuda-runtime", test))]
fn maybe_enable_poseidon_store_polynomials_coefficients<'a, B, MC>(
    commitment_scheme: &mut CommitmentSchemeProver<'a, B, MC>,
) where
    B: stwo::prover::backend::BackendForChannel<MC>,
    MC: MerkleChannel,
{
    if poseidon_store_polynomials_coefficients_enabled() {
        commitment_scheme.set_store_polynomials_coefficients();
    }
}

#[cfg(any(feature = "cuda-runtime", test))]
fn pow5<F: FieldExpOps>(x: F) -> F {
    let x2 = x.clone() * x.clone();
    let x4 = x2.clone() * x2.clone();
    x4 * x.clone()
}

#[cfg(any(feature = "cuda-runtime", test))]
fn eval_poseidon_constraints_mode<E: EvalAtRow>(
    eval: &mut E,
    lookup_elements: &PoseidonElements,
    mode: PoseidonEvalMode,
) {
    for _ in 0..N_INSTANCES_PER_ROW {
        let mut state: [_; N_STATE] = std::array::from_fn(|_| eval.next_trace_mask());
        let initial_state = state.clone();

        (0..N_HALF_FULL_ROUNDS).for_each(|round| {
            (0..N_STATE).for_each(|index| {
                state[index] += EXTERNAL_ROUND_CONSTS[round][index];
            });
            apply_external_round_matrix(&mut state);
            state = std::array::from_fn(|index| pow5(state[index].clone()));
            state.iter_mut().enumerate().for_each(|(index, value)| {
                let mask = eval.next_trace_mask();
                if mode.includes_first_half_full_cell(round, index) {
                    eval.add_constraint(value.clone() - mask.clone());
                }
                *value = mask;
            });
        });

        (0..N_PARTIAL_ROUNDS).for_each(|round| {
            state[0] += INTERNAL_ROUND_CONSTS[round];
            apply_internal_round_matrix(&mut state);
            state[0] = pow5(state[0].clone());
            let mask = eval.next_trace_mask();
            if mode.includes_partial_round(round) {
                eval.add_constraint(state[0].clone() - mask.clone());
            }
            state[0] = mask;
        });

        (0..N_HALF_FULL_ROUNDS).for_each(|round| {
            (0..N_STATE).for_each(|index| {
                state[index] += EXTERNAL_ROUND_CONSTS[round + N_HALF_FULL_ROUNDS][index];
            });
            apply_external_round_matrix(&mut state);
            state = std::array::from_fn(|index| pow5(state[index].clone()));
            state.iter_mut().enumerate().for_each(|(index, value)| {
                let mask = eval.next_trace_mask();
                if mode.includes_second_half_full_cell(round, index) {
                    eval.add_constraint(value.clone() - mask.clone());
                }
                *value = mask;
            });
        });

        if mode.includes_logup() {
            eval.add_to_relation(RelationEntry::new(lookup_elements, E::EF::one(), &initial_state));
            eval.add_to_relation(RelationEntry::new(lookup_elements, -E::EF::one(), &state));
        }
    }

    if mode.includes_logup() {
        eval.finalize_logup_in_pairs();
    }
}

#[cfg(test)]
#[path = "poseidon_prove/diagnostics.rs"]
mod diagnostics;
