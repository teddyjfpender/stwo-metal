//! Virtual SNOS prove/verify benchmark.
//!
//! Models the SNIP-36 counter-increment pattern: each trace column pair sums
//! to a constant parameter (modeling storage-read + amount = write), plus
//! preprocessed-column parity, inversion self-check, and ext-field
//! multiplication constraints — the full V1 capability surface.

#[cfg(feature = "metal-runtime")]
use std::time::Instant;

use serde::Serialize;
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
use stwo::core::vcs_lifted::blake2_merkle::{Blake2sMerkleChannel, Blake2sMerkleHasher};
#[cfg(feature = "metal-runtime")]
use stwo::core::verifier::verify;
#[cfg(feature = "metal-runtime")]
use stwo::prover::poly::circle::{CircleEvaluation, PolyOps};
#[cfg(feature = "metal-runtime")]
use stwo::prover::poly::BitReversedOrder;
#[cfg(feature = "metal-runtime")]
use stwo::prover::CommitmentSchemeProver;
#[cfg(feature = "metal-runtime")]
use stwo_metal::{
    MetalBackend, MetalEvaluationProgramBudgetV1, MetalEvaluationProgramSpecializationV1,
    lower_virtual_snos_evaluation_program_v1,
};
#[cfg(feature = "metal-runtime")]
use stwo_metal::prove_runtime::{
    MetalProveRuntimeContextV1,
    commit_trace_with_breakdown, execute_prove_core_v1,
};
#[cfg(feature = "metal-runtime")]
use stwo_metal_standalone_benchmarks::support::summarize;
use stwo_metal_standalone_benchmarks::support::{
    enforce_metal_benchmark_contract, env_flag, env_or, env_u32, env_usize, epoch_ms,
    required_env_path, runner_metadata, write_json, RunnerMetadata, SummaryStats,
};

// ---------------------------------------------------------------------------
// Virtual SNOS component
// ---------------------------------------------------------------------------

/// Minimal component for the virtual SNOS constraint set.
///
/// Provides the `Component` trait (needed by the verifier and by
/// `stage_prove_values_v1`) and `ComponentProver<MetalBackend>` (needed by
/// `execute_prove_core_v1`, though composition is handled by the Metal
/// evaluation program, not by this component's domain evaluator).
///
/// Constraints: for each consecutive pair `(col_i, col_{i+1})` in the main
/// trace, `col_i + col_{i+1} - param_value = 0`. There are `n_columns - 1`
/// such constraints.
#[cfg(feature = "metal-runtime")]
#[derive(Clone)]
struct VirtualSnosComponent {
    log_n_rows: u32,
    n_columns: usize,
    param_value: BaseField,
}

#[cfg(feature = "metal-runtime")]
impl VirtualSnosComponent {
    fn new(log_n_rows: u32, n_columns: usize, param_value: BaseField) -> Self {
        assert!(n_columns >= 2, "virtual SNOS requires at least 2 columns");
        Self { log_n_rows, n_columns, param_value }
    }

    fn denom_inverse(
        &self,
        point: stwo::core::circle::CirclePoint<SecureField>,
    ) -> SecureField {
        coset_vanishing(CanonicCoset::new(self.log_n_rows).coset(), point).inverse()
    }
}

#[cfg(feature = "metal-runtime")]
const MAIN_TRACE_IDX: usize = 1;

#[cfg(feature = "metal-runtime")]
impl Component for VirtualSnosComponent {
    fn n_constraints(&self) -> usize {
        self.n_columns - 1
    }

    fn max_constraint_log_degree_bound(&self) -> u32 {
        self.log_n_rows + 1
    }

    fn trace_log_degree_bounds(&self) -> TreeVec<Vec<u32>> {
        TreeVec::new(vec![
            vec![self.log_n_rows],           // preprocessed: 1 column
            vec![self.log_n_rows; self.n_columns], // main trace
        ])
    }

    fn mask_points(
        &self,
        point: stwo::core::circle::CirclePoint<SecureField>,
        _max_log_degree_bound: u32,
    ) -> TreeVec<Vec<Vec<stwo::core::circle::CirclePoint<SecureField>>>> {
        TreeVec::new(vec![
            vec![vec![point]],                                // preprocessed
            (0..self.n_columns).map(|_| vec![point]).collect(), // main
        ])
    }

    fn preprocessed_column_indices(&self) -> Vec<usize> {
        vec![0]
    }

    fn evaluate_constraint_quotients_at_point(
        &self,
        point: stwo::core::circle::CirclePoint<SecureField>,
        mask: &TreeVec<Vec<Vec<SecureField>>>,
        evaluation_accumulator: &mut stwo::core::air::accumulation::PointEvaluationAccumulator,
        _max_log_degree_bound: u32,
    ) {
        let trace_mask = &mask[MAIN_TRACE_IDX];
        assert_eq!(trace_mask.len(), self.n_columns);
        let denom_inverse = self.denom_inverse(point);
        let param = SecureField::from(self.param_value);
        for i in 0..(self.n_columns - 1) {
            let sum = trace_mask[i][0] + trace_mask[i + 1][0];
            evaluation_accumulator.accumulate(denom_inverse * (sum - param));
        }
    }
}

#[cfg(feature = "metal-runtime")]
impl stwo::prover::ComponentProver<MetalBackend> for VirtualSnosComponent {
    fn evaluate_constraint_quotients_on_domain(
        &self,
        _trace: &stwo::prover::Trace<'_, MetalBackend>,
        _evaluation_accumulator: &mut stwo::prover::DomainEvaluationAccumulator<MetalBackend>,
    ) {
        // Composition is handled by the Metal evaluation program, not by this
        // component's domain evaluator. This method is never called via
        // execute_prove_core_v1.
        unimplemented!(
            "VirtualSnosComponent domain evaluation is delegated to the Metal evaluation program"
        );
    }
}

const BENCHMARK_ID: &str = "virtual_snos_prove_verify_v1";
const RESULT_SCHEMA_VERSION: u32 = 1;
const DEFAULT_LOG_N_INSTANCES: u32 = 20;
const DEFAULT_N_COLUMNS: u32 = 100;
const DEFAULT_WARMUPS: usize = 1;
const DEFAULT_SAMPLES: usize = 5;

// ---------------------------------------------------------------------------
// Output structs (same JSON schema as wide_fibonacci for comparability)
// ---------------------------------------------------------------------------

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
    sentinel: Option<VirtualSnosSentinel>,
}

#[derive(Serialize)]
struct WorkloadMetadata {
    family: String,
    channel: String,
    operation: String,
    log_n_instances: u32,
    instances: u64,
    n_columns: u32,
    n_constraints: u32,
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
    composition_twiddle_ms: Vec<f64>,
    composition_trace_extension_ms: Vec<f64>,
    composition_eval_program_ms: Vec<f64>,
    composition_quotient_application_ms: Vec<f64>,
    composition_interpolation_ms: Vec<f64>,
    prove_values_prepare_ms: Vec<f64>,
    prove_values_sampled_values_v1_ms: Vec<f64>,
    prove_values_fri_and_decommit_ms: Vec<f64>,
    fri_quotients_ms: Vec<f64>,
    fri_commit_ms: Vec<f64>,
    proof_of_work_ms: Vec<f64>,
    fri_decommit_ms: Vec<f64>,
    tree_decommit_ms: Vec<f64>,
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
    composition_twiddle_ms: SummaryStats,
    composition_trace_extension_ms: SummaryStats,
    composition_eval_program_ms: SummaryStats,
    composition_quotient_application_ms: SummaryStats,
    composition_interpolation_ms: SummaryStats,
    prove_values_prepare_ms: SummaryStats,
    prove_values_sampled_values_v1_ms: SummaryStats,
    prove_values_fri_and_decommit_ms: SummaryStats,
    fri_quotients_ms: SummaryStats,
    fri_commit_ms: SummaryStats,
    proof_of_work_ms: SummaryStats,
    fri_decommit_ms: SummaryStats,
    tree_decommit_ms: SummaryStats,
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
    composition_twiddle_ms: f64,
    composition_trace_extension_ms: f64,
    composition_eval_program_ms: f64,
    composition_quotient_application_ms: f64,
    composition_interpolation_ms: f64,
    prove_values_prepare_ms: f64,
    prove_values_sampled_values_v1_ms: f64,
    prove_values_fri_and_decommit_ms: f64,
    fri_quotients_ms: f64,
    fri_commit_ms: f64,
    proof_of_work_ms: f64,
    fri_decommit_ms: f64,
    tree_decommit_ms: f64,
}

#[derive(Clone, Serialize)]
struct VirtualSnosSentinel {
    param_value: u32,
    n_columns: u32,
    first_column_first_value: u32,
    preprocessed_first_value: u32,
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

// ---------------------------------------------------------------------------
// Breakdown extraction (shared structure with wide_fibonacci)
// ---------------------------------------------------------------------------

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
    composition_twiddle_ms: f64,
    composition_trace_extension_ms: f64,
    composition_eval_program_ms: f64,
    composition_quotient_application_ms: f64,
    composition_interpolation_ms: f64,
    prove_values_prepare_ms: f64,
    prove_values_sampled_values_v1_ms: f64,
    prove_values_fri_and_decommit_ms: f64,
    fri_quotients_ms: f64,
    fri_commit_ms: f64,
    proof_of_work_ms: f64,
    fri_decommit_ms: f64,
    tree_decommit_ms: f64,
}

// ---------------------------------------------------------------------------
// Conversion helpers
// ---------------------------------------------------------------------------

#[cfg(feature = "metal-runtime")]
macro_rules! collect_field {
    ($samples:expr, $field:ident) => {
        $samples.iter().map(|s| s.$field).collect::<Vec<_>>()
    };
}

#[cfg(feature = "metal-runtime")]
fn prove_breakdown_samples_from_samples(samples: &[ProveBreakdown]) -> ProveBreakdownSampleTimings {
    ProveBreakdownSampleTimings {
        setup_and_preprocessed_commit_ms: collect_field!(samples, setup_and_preprocessed_commit_ms),
        trace_generation_ms: collect_field!(samples, trace_generation_ms),
        trace_commit_ms: collect_field!(samples, trace_commit_ms),
        trace_commit_interpolation_ms: collect_field!(samples, trace_commit_interpolation_ms),
        trace_commit_extension_ms: collect_field!(samples, trace_commit_extension_ms),
        trace_commit_merkle_ms: collect_field!(samples, trace_commit_merkle_ms),
        prove_core_ms: collect_field!(samples, prove_core_ms),
        prove_core_evaluation_program_v1_ms: collect_field!(samples, prove_core_evaluation_program_v1_ms),
        prove_core_composition_generation_ms: collect_field!(samples, prove_core_composition_generation_ms),
        prove_core_composition_commit_ms: collect_field!(samples, prove_core_composition_commit_ms),
        prove_core_prove_values_ms: collect_field!(samples, prove_core_prove_values_ms),
        prove_core_sanity_check_ms: collect_field!(samples, prove_core_sanity_check_ms),
        composition_twiddle_ms: collect_field!(samples, composition_twiddle_ms),
        composition_trace_extension_ms: collect_field!(samples, composition_trace_extension_ms),
        composition_eval_program_ms: collect_field!(samples, composition_eval_program_ms),
        composition_quotient_application_ms: collect_field!(samples, composition_quotient_application_ms),
        composition_interpolation_ms: collect_field!(samples, composition_interpolation_ms),
        prove_values_prepare_ms: collect_field!(samples, prove_values_prepare_ms),
        prove_values_sampled_values_v1_ms: collect_field!(samples, prove_values_sampled_values_v1_ms),
        prove_values_fri_and_decommit_ms: collect_field!(samples, prove_values_fri_and_decommit_ms),
        fri_quotients_ms: collect_field!(samples, fri_quotients_ms),
        fri_commit_ms: collect_field!(samples, fri_commit_ms),
        proof_of_work_ms: collect_field!(samples, proof_of_work_ms),
        fri_decommit_ms: collect_field!(samples, fri_decommit_ms),
        tree_decommit_ms: collect_field!(samples, tree_decommit_ms),
    }
}

#[cfg(feature = "metal-runtime")]
macro_rules! summarize_field {
    ($samples:expr, $field:ident) => {
        summarize(&$samples.iter().map(|s| s.$field).collect::<Vec<_>>())
    };
}

#[cfg(feature = "metal-runtime")]
fn prove_breakdown_summary_from_samples(samples: &[ProveBreakdown]) -> ProveBreakdownSummaryTimings {
    ProveBreakdownSummaryTimings {
        setup_and_preprocessed_commit_ms: summarize_field!(samples, setup_and_preprocessed_commit_ms),
        trace_generation_ms: summarize_field!(samples, trace_generation_ms),
        trace_commit_ms: summarize_field!(samples, trace_commit_ms),
        trace_commit_interpolation_ms: summarize_field!(samples, trace_commit_interpolation_ms),
        trace_commit_extension_ms: summarize_field!(samples, trace_commit_extension_ms),
        trace_commit_merkle_ms: summarize_field!(samples, trace_commit_merkle_ms),
        prove_core_ms: summarize_field!(samples, prove_core_ms),
        prove_core_evaluation_program_v1_ms: summarize_field!(samples, prove_core_evaluation_program_v1_ms),
        prove_core_composition_generation_ms: summarize_field!(samples, prove_core_composition_generation_ms),
        prove_core_composition_commit_ms: summarize_field!(samples, prove_core_composition_commit_ms),
        prove_core_prove_values_ms: summarize_field!(samples, prove_core_prove_values_ms),
        prove_core_sanity_check_ms: summarize_field!(samples, prove_core_sanity_check_ms),
        composition_twiddle_ms: summarize_field!(samples, composition_twiddle_ms),
        composition_trace_extension_ms: summarize_field!(samples, composition_trace_extension_ms),
        composition_eval_program_ms: summarize_field!(samples, composition_eval_program_ms),
        composition_quotient_application_ms: summarize_field!(samples, composition_quotient_application_ms),
        composition_interpolation_ms: summarize_field!(samples, composition_interpolation_ms),
        prove_values_prepare_ms: summarize_field!(samples, prove_values_prepare_ms),
        prove_values_sampled_values_v1_ms: summarize_field!(samples, prove_values_sampled_values_v1_ms),
        prove_values_fri_and_decommit_ms: summarize_field!(samples, prove_values_fri_and_decommit_ms),
        fri_quotients_ms: summarize_field!(samples, fri_quotients_ms),
        fri_commit_ms: summarize_field!(samples, fri_commit_ms),
        proof_of_work_ms: summarize_field!(samples, proof_of_work_ms),
        fri_decommit_ms: summarize_field!(samples, fri_decommit_ms),
        tree_decommit_ms: summarize_field!(samples, tree_decommit_ms),
    }
}

#[cfg(feature = "metal-runtime")]
fn single_prove_breakdown_from(b: &ProveBreakdown) -> SingleProveBreakdownTimings {
    SingleProveBreakdownTimings {
        setup_and_preprocessed_commit_ms: b.setup_and_preprocessed_commit_ms,
        trace_generation_ms: b.trace_generation_ms,
        trace_commit_ms: b.trace_commit_ms,
        trace_commit_interpolation_ms: b.trace_commit_interpolation_ms,
        trace_commit_extension_ms: b.trace_commit_extension_ms,
        trace_commit_merkle_ms: b.trace_commit_merkle_ms,
        prove_core_ms: b.prove_core_ms,
        prove_core_evaluation_program_v1_ms: b.prove_core_evaluation_program_v1_ms,
        prove_core_composition_generation_ms: b.prove_core_composition_generation_ms,
        prove_core_composition_commit_ms: b.prove_core_composition_commit_ms,
        prove_core_prove_values_ms: b.prove_core_prove_values_ms,
        prove_core_sanity_check_ms: b.prove_core_sanity_check_ms,
        composition_twiddle_ms: b.composition_twiddle_ms,
        composition_trace_extension_ms: b.composition_trace_extension_ms,
        composition_eval_program_ms: b.composition_eval_program_ms,
        composition_quotient_application_ms: b.composition_quotient_application_ms,
        composition_interpolation_ms: b.composition_interpolation_ms,
        prove_values_prepare_ms: b.prove_values_prepare_ms,
        prove_values_sampled_values_v1_ms: b.prove_values_sampled_values_v1_ms,
        prove_values_fri_and_decommit_ms: b.prove_values_fri_and_decommit_ms,
        fri_quotients_ms: b.fri_quotients_ms,
        fri_commit_ms: b.fri_commit_ms,
        proof_of_work_ms: b.proof_of_work_ms,
        fri_decommit_ms: b.fri_decommit_ms,
        tree_decommit_ms: b.tree_decommit_ms,
    }
}

// ---------------------------------------------------------------------------
// Prove/verify core
// ---------------------------------------------------------------------------

#[cfg(feature = "metal-runtime")]
#[derive(Clone)]
struct SampleResult {
    total_elapsed_ms: f64,
    prove_elapsed_ms: f64,
    verify_elapsed_ms: f64,
    prove_breakdown: ProveBreakdown,
    proof_metadata: ProofMetadata,
    sentinel: VirtualSnosSentinel,
}

/// Run a single virtual_snos prove/verify sample through the V1 runtime.
///
/// The trace models SNIP-36 counter increments: `n_columns` trace columns
/// where each consecutive pair sums to `param_value`, plus a preprocessed
/// column equal to `param_value` at every row.
#[cfg(feature = "metal-runtime")]
fn run_one_sample(
    log_n_rows: u32,
    n_columns: u32,
    param_value: BaseField,
) -> SampleResult {
    let config = PcsConfig::default();
    let n_rows = 1usize << log_n_rows;

    // Lower the V1 evaluation program.
    let program = lower_virtual_snos_evaluation_program_v1(
        MetalEvaluationProgramSpecializationV1 { log_n_rows, n_columns },
    )
    .expect("virtual_snos lowering should succeed");
    program
        .validate(MetalEvaluationProgramBudgetV1::new(2048, 512))
        .expect("virtual_snos program should fit within budget");

    let ctx = MetalProveRuntimeContextV1::new(program, log_n_rows);

    let prove_start = Instant::now();

    // Setup twiddles and commitment scheme.
    let setup_start = Instant::now();
    let twiddles = MetalBackend::precompute_twiddles(
        CanonicCoset::new(log_n_rows + 1 + config.fri_config.log_blowup_factor)
            .circle_domain()
            .half_coset,
    );
    let prover_channel = &mut Blake2sChannel::default();
    let mut commitment_scheme =
        CommitmentSchemeProver::<MetalBackend, Blake2sMerkleChannel>::new(config, &twiddles);
    commitment_scheme.set_store_polynomials_coefficients();

    // Commit preprocessed trace (1 column, all values = param_value).
    let trace_domain = CanonicCoset::new(log_n_rows).circle_domain();
    let preproc_values =
        stwo_metal::MetalBaseFieldVec::from_vec(vec![param_value; n_rows]);
    let preprocessed_eval =
        CircleEvaluation::<MetalBackend, BaseField, BitReversedOrder>::new(
            trace_domain,
            preproc_values,
        );
    let _preproc_commit = commit_trace_with_breakdown(
        &mut commitment_scheme,
        prover_channel,
        vec![preprocessed_eval],
        &twiddles,
    );
    let setup_and_preprocessed_commit_ms = setup_start.elapsed().as_secs_f64() * 1000.0;

    // Generate main trace: n_columns columns, each row = 1.
    // Satisfies col_i + col_{i+1} = 2 = param_value for all pairs.
    let trace_gen_start = Instant::now();
    let col_value = BaseField::from_u32_unchecked(1);
    let main_evals: Vec<_> = (0..n_columns)
        .map(|_| {
            let values =
                stwo_metal::MetalBaseFieldVec::from_vec(vec![col_value; n_rows]);
            CircleEvaluation::<MetalBackend, BaseField, BitReversedOrder>::new(
                trace_domain,
                values,
            )
        })
        .collect();
    let trace_generation_ms = trace_gen_start.elapsed().as_secs_f64() * 1000.0;

    // Commit main trace.
    let trace_commit_start = Instant::now();
    let trace_commit = commit_trace_with_breakdown(
        &mut commitment_scheme,
        prover_channel,
        main_evals,
        &twiddles,
    );
    let trace_commit_ms = trace_commit_start.elapsed().as_secs_f64() * 1000.0;

    // Prove via V1 runtime.
    let component = VirtualSnosComponent::new(log_n_rows, n_columns as usize, param_value);
    let prove_core_start = Instant::now();
    let (prove_values_result, prove_core_breakdown) = execute_prove_core_v1(
        &ctx,
        &[&component],
        prover_channel,
        commitment_scheme,
    )
    .expect("virtual_snos prove should succeed");
    let prove_core_ms = prove_core_start.elapsed().as_secs_f64() * 1000.0;
    let prove_elapsed_ms = prove_start.elapsed().as_secs_f64() * 1000.0;

    // Verify.
    let verify_start = Instant::now();
    let proof = prove_values_result.proof().clone();
    let proof_metadata = proof_metadata_from(&proof);
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
        proof,
    )
    .expect("virtual_snos verification should succeed");
    let verify_elapsed_ms = verify_start.elapsed().as_secs_f64() * 1000.0;

    let breakdown = ProveBreakdown {
        setup_and_preprocessed_commit_ms,
        trace_generation_ms,
        trace_commit_ms,
        trace_commit_interpolation_ms: trace_commit.interpolation_ms,
        trace_commit_extension_ms: trace_commit.extension_ms,
        trace_commit_merkle_ms: trace_commit.merkle_ms,
        prove_core_ms,
        prove_core_evaluation_program_v1_ms: prove_core_breakdown.evaluation_program_v1_ms,
        prove_core_composition_generation_ms: prove_core_breakdown.composition_generation_ms,
        prove_core_composition_commit_ms: prove_core_breakdown.composition_commit_ms,
        prove_core_prove_values_ms: prove_core_breakdown.prove_values_ms,
        prove_core_sanity_check_ms: prove_core_breakdown.sanity_check_ms,
        composition_twiddle_ms: prove_core_breakdown.composition_detail.twiddle_ms,
        composition_trace_extension_ms: prove_core_breakdown.composition_detail.trace_extension_ms,
        composition_eval_program_ms: prove_core_breakdown.composition_detail.eval_program_ms,
        composition_quotient_application_ms: prove_core_breakdown.composition_detail.quotient_application_ms,
        composition_interpolation_ms: prove_core_breakdown.composition_detail.interpolation_ms,
        prove_values_prepare_ms: prove_core_breakdown.prove_values_detail.prepare_ms,
        prove_values_sampled_values_v1_ms: prove_core_breakdown.prove_values_detail.sampled_values_v1_ms,
        prove_values_fri_and_decommit_ms: prove_core_breakdown.prove_values_detail.fri_and_decommit_ms,
        fri_quotients_ms: 0.0,
        fri_commit_ms: 0.0,
        proof_of_work_ms: 0.0,
        fri_decommit_ms: 0.0,
        tree_decommit_ms: 0.0,
    };

    SampleResult {
        total_elapsed_ms: prove_elapsed_ms + verify_elapsed_ms,
        prove_elapsed_ms,
        verify_elapsed_ms,
        prove_breakdown: breakdown,
        proof_metadata,
        sentinel: VirtualSnosSentinel {
            param_value: param_value.0,
            n_columns,
            first_column_first_value: col_value.0,
            preprocessed_first_value: param_value.0,
        },
    }
}

#[cfg(feature = "metal-runtime")]
fn proof_metadata_from(
    proof: &stwo::core::proof::StarkProof<Blake2sMerkleHasher>,
) -> ProofMetadata {
    let size_breakdown = proof.size_breakdown_estimate();
    ProofMetadata {
        size_estimate_bytes: proof.size_estimate(),
        commitments: proof.commitments.len(),
        security_bits: proof.config.security_bits(),
        fri_queries: proof.config.fri_config.n_queries,
        pow_bits: proof.config.pow_bits,
        size_breakdown_bytes: ProofSizeBreakdownMetadata {
            oods_samples: size_breakdown.oods_samples,
            queries_values: size_breakdown.queries_values,
            fri_samples: size_breakdown.fri_samples,
            fri_decommitments: size_breakdown.fri_decommitments,
            trace_decommitments: size_breakdown.trace_decommitments,
        },
    }
}

#[cfg(feature = "metal-runtime")]
fn steady_state_tail<T>(samples: &[T]) -> Option<&[T]> {
    (samples.len() >= 2).then(|| &samples[1..])
}

#[cfg(feature = "metal-runtime")]
fn throughput_from_summary(instances: u64, summary: &SummaryStats) -> Option<f64> {
    (summary.mean > 0.0).then_some(instances as f64 / summary.mean)
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

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

    // Virtual SNOS constraints: (n_columns - 1) pair sums + 3 extra.
    let n_constraints = n_columns + 2;

    let workload = WorkloadMetadata {
        family: "virtual_snos".to_string(),
        channel: "blake2s".to_string(),
        operation: "prove_verify".to_string(),
        log_n_instances,
        instances,
        n_columns,
        n_constraints,
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
            "virtual_snos_prove benchmark requires the metal-runtime feature"
        );

        #[cfg(feature = "metal-runtime")]
        {
            // Counter increment: col_i + col_{i+1} = param_value.
            // Use param_value = 2 with all column values = 1.
            let param_value = BaseField::from_u32_unchecked(2);

            // Warmup.
            for _ in 0..warmups {
                let _ = run_one_sample(log_n_instances, n_columns, param_value);
            }

            // Timed samples.
            let sample_results: Vec<SampleResult> = (0..samples)
                .map(|_| run_one_sample(log_n_instances, n_columns, param_value))
                .collect();

            let samples_ms: Vec<f64> = sample_results.iter().map(|s| s.total_elapsed_ms).collect();
            let prove_ms: Vec<f64> = sample_results.iter().map(|s| s.prove_elapsed_ms).collect();
            let verify_ms: Vec<f64> = sample_results.iter().map(|s| s.verify_elapsed_ms).collect();

            let total_summary = summarize(&samples_ms);
            let prove_summary = summarize(&prove_ms);
            let verify_summary = summarize(&verify_ms);

            let breakdowns: Vec<ProveBreakdown> =
                sample_results.iter().map(|s| s.prove_breakdown).collect();
            let steady_state_results = steady_state_tail(&sample_results);
            let steady_state_breakdowns = steady_state_tail(&breakdowns);
            let steady_state_ms =
                steady_state_results.map(|s| s.iter().map(|r| r.total_elapsed_ms).collect::<Vec<_>>());
            let steady_state_summary = steady_state_ms.as_ref().map(|s| summarize(s));

            let throughput = throughput_from_summary(instances, &total_summary);
            let steady_state_throughput = steady_state_summary
                .as_ref()
                .and_then(|s| throughput_from_summary(instances, s));

            let last = sample_results.last().expect("at least one sample");

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
                    cold_start_sample_ms: sample_results.first().map(|s| s.total_elapsed_ms),
                    cold_start_phase_ms: sample_results.first().map(|s| SinglePhaseTimings {
                        prove_ms: s.prove_elapsed_ms,
                        verify_ms: s.verify_elapsed_ms,
                    }),
                    cold_start_prove_breakdown_ms: sample_results
                        .first()
                        .map(|s| single_prove_breakdown_from(&s.prove_breakdown)),
                    steady_state_samples_ms: steady_state_ms,
                    steady_state_summary_ms: steady_state_summary,
                    steady_state_throughput_kelem_per_second: steady_state_throughput,
                    phase_samples_ms: Some(PhaseSampleTimings {
                        prove_ms: prove_ms,
                        verify_ms: verify_ms,
                    }),
                    phase_summary_ms: Some(PhaseSummaryTimings {
                        prove_ms: prove_summary,
                        verify_ms: verify_summary,
                    }),
                    steady_state_phase_samples_ms: steady_state_results.map(|s| {
                        PhaseSampleTimings {
                            prove_ms: s.iter().map(|r| r.prove_elapsed_ms).collect(),
                            verify_ms: s.iter().map(|r| r.verify_elapsed_ms).collect(),
                        }
                    }),
                    steady_state_phase_summary_ms: steady_state_results.map(|s| {
                        PhaseSummaryTimings {
                            prove_ms: summarize(
                                &s.iter().map(|r| r.prove_elapsed_ms).collect::<Vec<_>>(),
                            ),
                            verify_ms: summarize(
                                &s.iter().map(|r| r.verify_elapsed_ms).collect::<Vec<_>>(),
                            ),
                        }
                    }),
                    prove_breakdown_samples_ms: Some(prove_breakdown_samples_from_samples(
                        &breakdowns,
                    )),
                    prove_breakdown_summary_ms: Some(prove_breakdown_summary_from_samples(
                        &breakdowns,
                    )),
                    steady_state_prove_breakdown_samples_ms: steady_state_breakdowns
                        .map(prove_breakdown_samples_from_samples),
                    steady_state_prove_breakdown_summary_ms: steady_state_breakdowns
                        .map(prove_breakdown_summary_from_samples),
                },
                proof: Some(last.proof_metadata.clone()),
                sentinel: Some(last.sentinel.clone()),
            }
        }
    };

    write_json(&output_path, &result);
    println!("{}", output_path.display());
}
