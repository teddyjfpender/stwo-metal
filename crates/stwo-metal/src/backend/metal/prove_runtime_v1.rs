//! Shared V1 prove runtime.
//!
//! Owns the post-composition prove flow for any workload using V1 evaluation
//! programs and sampled-values ABI. The benchmark module delegates to these
//! shared functions rather than owning the proving semantics itself.
//!
//! This module addresses TD-0035 (generated lane proves through selected
//! MetalEvaluationProgramV1 contract) and TD-0037 (selected V1 runtime
//! consumes live post-composition sampled-values ABI directly through Metal
//! lane) by making the prove flow a first-class shared runtime contract.

use stwo::core::channel::{Blake2sChannel, Channel};
use stwo::core::constraints::coset_vanishing;
use stwo::core::fields::m31::BaseField;
use stwo::core::fields::qm31::{SecureField, SECURE_EXTENSION_DEGREE};
#[allow(unused_imports)]
use stwo::core::fields::FieldExpOps;
use stwo::core::pcs::quotients::CommitmentSchemeProof;
use stwo::core::pcs::utils::{get_lifting_log_size, prepare_query_positions_for_tree};
use stwo::core::pcs::TreeVec;
use stwo::core::poly::circle::CanonicCoset;
use stwo::core::proof::{StarkProof, StarkProofSizeBreakdown};
use stwo::core::proof_of_work::GrindOps;
use stwo::core::utils::MaybeOwned;
use stwo::core::vcs_lifted::blake2_merkle::{Blake2sMerkleChannel, Blake2sMerkleHasher};
use stwo::prover::fri::{FriDecommitResult, FriProver};
use stwo::prover::poly::circle::{CircleEvaluation, PolyOps, SecureCirclePoly, SecureEvaluation};
use stwo::prover::poly::BitReversedOrder;
use stwo::prover::vcs_lifted::ops::MerkleOpsLifted;
use stwo::prover::vcs_lifted::prover::MerkleProverLifted;
use stwo::prover::{
    AccumulationOps, CommitmentSchemeProver, CommitmentTreeProver, ComponentProver,
    ComponentProvers, PreparedCommitmentSchemeProveValues, ProvingError, Trace,
    compute_fri_quotients,
};

use super::accumulation::metal_secure_column_from_values;
use super::eval_program_v1::{
    execute_eval_program_v1_with_gpu_trace,
    execute_selected_metal_evaluation_program_v1_on_metal, interpret_metal_evaluation_program_v1,
    MetalEvaluationProgramCapabilityProfileV1, MetalEvaluationProgramDispatchKindV1,
    MetalEvaluationProgramExecutionError, MetalEvaluationProgramInterpreterError,
    MetalEvaluationProgramRuntimeInputsV1, MetalEvaluationProgramTraceViewV1,
    OwnedMetalEvaluationProgramV1,
};
use stwo_metal_sys::metal::U32Buffer;
use super::planner::MetalExecutionPlan;
use super::poly::evaluate_polys_on_domain_batch;
use super::sampled_values_v1::{
    execute_selected_metal_sampled_values_v1, interpret_metal_sampled_values_v1,
    interpret_metal_sampled_values_v1_reference, lower_metal_sampled_values_v1,
    MetalSampledValuesDispatchKindV1, MetalSampledValuesExecutionError,
    MetalSampledValuesInterpreterError, MetalSampledValuesLoweringError,
    OwnedMetalSampledValuesV1,
};
use super::workload_contract::{MetalWorkloadOwnership, MetalWorkloadStage};
use super::MetalBackend;

const MAIN_TRACE_IDX: usize = 1;

// ---------------------------------------------------------------------------
// Runtime context
// ---------------------------------------------------------------------------

/// Runtime context for V1 prove runtime.
///
/// Encapsulates a lowered, validated evaluation program and the workload shape
/// needed by the prove flow. Any workload that has a V1 evaluation program can
/// construct a context and use the shared prove runtime.
pub struct MetalProveRuntimeContextV1 {
    program: OwnedMetalEvaluationProgramV1,
    log_n_rows: u32,
    skip_reference_sanity_check: bool,
}

impl MetalProveRuntimeContextV1 {
    pub fn new(program: OwnedMetalEvaluationProgramV1, log_n_rows: u32) -> Self {
        Self {
            program,
            log_n_rows,
            skip_reference_sanity_check: false,
        }
    }

    pub fn program(&self) -> &OwnedMetalEvaluationProgramV1 {
        &self.program
    }

    pub fn log_n_rows(&self) -> u32 {
        self.log_n_rows
    }

    pub fn n_constraints(&self) -> u32 {
        self.program.header().n_constraints
    }

    /// Enable skipping the reference-interpreter sanity check during prove.
    ///
    /// When set, `execute_post_composition_runtime` will not run the reference
    /// interpreter to cross-check the selected dispatch result. This reduces
    /// the prove-values cost at the expense of losing the safety net.
    /// Intended for production/benchmark runs where the dispatch has been
    /// validated separately.
    pub fn with_skip_reference_sanity_check(mut self, skip: bool) -> Self {
        self.skip_reference_sanity_check = skip;
        self
    }

    pub fn skip_reference_sanity_check(&self) -> bool {
        self.skip_reference_sanity_check
    }
}

// ---------------------------------------------------------------------------
// Component context for standalone composition
// ---------------------------------------------------------------------------

/// Per-component context for standalone V1 composition.
///
/// Unlike [`MetalProveRuntimeContextV1`], which couples to the full prove
/// pipeline, this struct carries only what is needed to evaluate a single
/// component's constraint quotients from raw column data. It is designed to
/// work without a `CommitmentSchemeProver<MetalBackend>`.
#[allow(dead_code)]
pub struct MetalComponentContextV1 {
    pub program: OwnedMetalEvaluationProgramV1,
    pub log_n_rows: u32,
    pub base_params: Vec<BaseField>,
    pub ext_params: Vec<SecureField>,
}

impl MetalComponentContextV1 {
    pub fn new(
        program: OwnedMetalEvaluationProgramV1,
        log_n_rows: u32,
        base_params: Vec<BaseField>,
        ext_params: Vec<SecureField>,
    ) -> Self {
        Self {
            program,
            log_n_rows,
            base_params,
            ext_params,
        }
    }

    pub fn program(&self) -> &OwnedMetalEvaluationProgramV1 {
        &self.program
    }

    pub fn log_n_rows(&self) -> u32 {
        self.log_n_rows
    }

    pub fn n_constraints(&self) -> u32 {
        self.program.header().n_constraints
    }
}

// ---------------------------------------------------------------------------
// Output types
// ---------------------------------------------------------------------------

/// Staging state for prove-values phase.
pub struct MetalProveValuesStaging {
    pub oods_point: stwo::core::circle::CirclePoint<SecureField>,
    pub max_log_degree_bound: u32,
    pub sample_points: TreeVec<Vec<Vec<stwo::core::circle::CirclePoint<SecureField>>>>,
}

/// Breakdown of prove-core timing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MetalProveCoreBreakdown {
    pub evaluation_program_v1_ms: f64,
    pub evaluation_program_v1_dispatch: MetalEvaluationProgramDispatchKindV1,
    pub composition_generation_ms: f64,
    pub composition_commit_ms: f64,
    pub prove_values_ms: f64,
    pub sanity_check_ms: f64,
    /// Detailed sub-phase timing within composition generation.
    pub composition_detail: MetalCompositionDetailBreakdown,
    /// Detailed sub-phase timing within prove values.
    pub prove_values_detail: MetalProveValuesDetailBreakdown,
}

/// Sub-phase timing within composition polynomial generation.
///
/// Allows identifying which phase of composition scales poorly at high logs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MetalCompositionDetailBreakdown {
    /// Twiddle precomputation for the eval domain.
    pub twiddle_ms: f64,
    /// Trace extension to eval domain (if coefficients path was used).
    pub trace_extension_ms: f64,
    /// V1 evaluation program execution on GPU.
    pub eval_program_ms: f64,
    /// Denominator inverse computation + quotient application.
    pub quotient_application_ms: f64,
    /// Composition polynomial interpolation.
    pub interpolation_ms: f64,
}

/// Sub-phase timing within prove-values execution.
///
/// Covers the full prove-values cost: preparation, sampled-values dispatch,
/// FRI commit/decommit, and tree decommit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MetalProveValuesDetailBreakdown {
    /// Sampling and lowering to V1 sampled-values ABI.
    pub prepare_ms: f64,
    /// V1 sampled-values dispatch execution.
    pub sampled_values_v1_ms: f64,
    /// FRI quotient computation, FRI commit, proof-of-work, and decommit.
    pub fri_and_decommit_ms: f64,
}

/// Breakdown of prove-values timing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MetalProveValuesBreakdown {
    pub prove_values_ms: f64,
    pub sampled_values_v1_ms: f64,
    pub sampled_values_dispatch: MetalSampledValuesDispatchKindV1,
    pub sanity_check_ms: f64,
    pub fri_and_decommit_ms: f64,
}

/// Post-composition sampled values in V1 ABI.
#[derive(Clone, Debug)]
pub struct MetalPostCompositionSampledValuesV1 {
    sampled_values: OwnedMetalSampledValuesV1,
}

impl MetalPostCompositionSampledValuesV1 {
    pub fn from_owned(sampled_values: OwnedMetalSampledValuesV1) -> Self {
        Self { sampled_values }
    }

    pub fn sampled_values(&self) -> &OwnedMetalSampledValuesV1 {
        &self.sampled_values
    }
}

/// Result of prove-values execution.
#[derive(Clone, Debug)]
pub struct MetalProveValuesResult {
    proof: StarkProof<Blake2sMerkleHasher>,
    sampled_values: MetalPostCompositionSampledValuesV1,
    post_composition_eval: SecureField,
    sampled_values_dispatch: MetalSampledValuesDispatchKindV1,
}

impl MetalProveValuesResult {
    #[cfg(test)]
    pub(crate) fn new_for_test(
        proof: StarkProof<Blake2sMerkleHasher>,
        post_composition_eval: SecureField,
        sampled_values_dispatch: MetalSampledValuesDispatchKindV1,
    ) -> Self {
        Self {
            proof,
            sampled_values: MetalPostCompositionSampledValuesV1 {
                sampled_values: lower_metal_sampled_values_v1(&TreeVec::default()).unwrap(),
            },
            post_composition_eval,
            sampled_values_dispatch,
        }
    }

    pub fn proof(&self) -> &StarkProof<Blake2sMerkleHasher> {
        &self.proof
    }

    pub fn proof_metadata(&self) -> MetalProofMetadata {
        let size_breakdown = self.proof.size_breakdown_estimate();
        MetalProofMetadata {
            size_estimate_bytes: self.proof.size_estimate(),
            commitments: self.proof.commitments.len(),
            security_bits: self.proof.config.security_bits(),
            fri_queries: self.proof.config.fri_config.n_queries,
            pow_bits: self.proof.config.pow_bits,
            size_breakdown_bytes: size_breakdown.into(),
        }
    }

    pub fn sampled_values(&self) -> &MetalPostCompositionSampledValuesV1 {
        &self.sampled_values
    }

    pub fn post_composition_eval(&self) -> SecureField {
        self.post_composition_eval
    }

    pub fn sampled_values_dispatch(&self) -> MetalSampledValuesDispatchKindV1 {
        self.sampled_values_dispatch
    }
}

/// Proof metadata.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MetalProofMetadata {
    pub size_estimate_bytes: usize,
    pub commitments: usize,
    pub security_bits: u32,
    pub fri_queries: usize,
    pub pow_bits: u32,
    pub size_breakdown_bytes: MetalProofSizeBreakdown,
}

/// Proof size breakdown.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MetalProofSizeBreakdown {
    pub oods_samples: usize,
    pub queries_values: usize,
    pub fri_samples: usize,
    pub fri_decommitments: usize,
    pub trace_decommitments: usize,
}

impl From<StarkProofSizeBreakdown> for MetalProofSizeBreakdown {
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

// ---------------------------------------------------------------------------
// Lane validation
// ---------------------------------------------------------------------------

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum MetalProveRuntimeLaneError {
    PlanNotMetalCapable {
        workload_name: &'static str,
        plan: MetalExecutionPlan,
    },
    UnsupportedWitnessOwnership {
        workload_name: &'static str,
        stage: MetalWorkloadStage,
    },
    UnsupportedFriOwnership {
        workload_name: &'static str,
        stage: MetalWorkloadStage,
    },
}

/// Validate that the execution environment supports V1 prove runtime.
pub fn validate_prove_runtime_lane_v1(
    workload_name: &'static str,
    plan: MetalExecutionPlan,
    witness_ownership: Option<MetalWorkloadOwnership>,
    fri_ownership: Option<MetalWorkloadOwnership>,
) -> Result<(), MetalProveRuntimeLaneError> {
    if !matches!(
        plan,
        MetalExecutionPlan::MetalFriHybrid | MetalExecutionPlan::MetalFull
    ) {
        return Err(MetalProveRuntimeLaneError::PlanNotMetalCapable {
            workload_name,
            plan,
        });
    }
    if witness_ownership != Some(MetalWorkloadOwnership::CpuOwned) {
        return Err(MetalProveRuntimeLaneError::UnsupportedWitnessOwnership {
            workload_name,
            stage: MetalWorkloadStage::WitnessMain,
        });
    }
    if fri_ownership != Some(MetalWorkloadOwnership::MetalNative) {
        return Err(MetalProveRuntimeLaneError::UnsupportedFriOwnership {
            workload_name,
            stage: MetalWorkloadStage::FriBlake2s,
        });
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Composition polynomial generation
// ---------------------------------------------------------------------------

/// Error from composition polynomial generation via V1 program.
#[derive(Debug)]
pub enum MetalProveRuntimeCompositionError {
    ProgramExecution {
        source: MetalEvaluationProgramExecutionError,
    },
    ReferenceExecution {
        source: MetalEvaluationProgramInterpreterError,
    },
    CompositionShape {
        message: String,
    },
}

fn denominator_inverses_for_eval_domain(log_n_rows: u32, eval_domain_log_size: u32) -> Vec<BaseField> {
    let trace_domain = CanonicCoset::new(log_n_rows);
    let eval_domain = CanonicCoset::new(eval_domain_log_size).circle_domain();
    let log_expand = eval_domain.log_size() - trace_domain.log_size();
    let mut denominator_inverses = (0..(1 << log_expand))
        .map(|index| coset_vanishing(trace_domain.coset(), eval_domain.at(index)).inverse())
        .collect::<Vec<_>>();
    stwo::core::utils::bit_reverse(&mut denominator_inverses);
    denominator_inverses
}

#[allow(dead_code)]
fn execute_evaluation_program_v1_on_trace_interactions(
    program: &OwnedMetalEvaluationProgramV1,
    trace_interactions: &[&[&[BaseField]]],
    preprocessed_columns: &[&[BaseField]],
    base_params: &[BaseField],
    ext_params: &[SecureField],
    random_coeff_powers: &[SecureField],
) -> Result<
    (Vec<SecureField>, MetalEvaluationProgramDispatchKindV1),
    MetalProveRuntimeCompositionError,
> {
    let runtime = MetalEvaluationProgramRuntimeInputsV1 {
        trace: MetalEvaluationProgramTraceViewV1 {
            trace_interactions,
            preprocessed_columns,
        },
        base_params,
        ext_params,
        random_coeff_powers,
    };
    execute_selected_metal_evaluation_program_v1_on_metal(
        program,
        runtime,
        MetalEvaluationProgramCapabilityProfileV1::current(),
    )
    .map_err(|source| MetalProveRuntimeCompositionError::ProgramExecution { source })
}

#[allow(dead_code)]
fn interpret_evaluation_program_v1_on_trace_interactions(
    program: &OwnedMetalEvaluationProgramV1,
    trace_interactions: &[&[&[BaseField]]],
    preprocessed_columns: &[&[BaseField]],
    base_params: &[BaseField],
    ext_params: &[SecureField],
    random_coeff_powers: &[SecureField],
) -> Result<Vec<SecureField>, MetalProveRuntimeCompositionError> {
    let runtime = MetalEvaluationProgramRuntimeInputsV1 {
        trace: MetalEvaluationProgramTraceViewV1 {
            trace_interactions,
            preprocessed_columns,
        },
        base_params,
        ext_params,
        random_coeff_powers,
    };
    interpret_metal_evaluation_program_v1(program, runtime)
        .map_err(|source| MetalProveRuntimeCompositionError::ReferenceExecution { source })
}

/// Compute composition polynomial via selected V1 evaluation program.
///
/// Returns the composition polynomial, the dispatch kind used, and the
/// detailed composition breakdown.
pub fn compute_composition_polynomial_v1(
    ctx: &MetalProveRuntimeContextV1,
    trace: &Trace<'_, MetalBackend>,
    random_coeff: SecureField,
) -> Result<
    (
        SecureCirclePoly<MetalBackend>,
        MetalEvaluationProgramDispatchKindV1,
        MetalCompositionDetailBreakdown,
    ),
    MetalProveRuntimeCompositionError,
> {
    let eval_domain = CanonicCoset::new(ctx.log_n_rows + 1).circle_domain();
    let trace_columns = &trace.polys[MAIN_TRACE_IDX];
    let n_constraints = ctx.n_constraints() as usize;
    let expected_columns = n_constraints + 2;
    if trace_columns.len() != expected_columns {
        return Err(MetalProveRuntimeCompositionError::CompositionShape {
            message: format!(
                "V1 runtime composition expected {} trace columns (n_constraints={} + 2), got {}",
                expected_columns, n_constraints, trace_columns.len()
            ),
        });
    }

    let mut random_coeff_powers =
        <MetalBackend as AccumulationOps>::generate_secure_powers(random_coeff, n_constraints);
    random_coeff_powers.reverse();

    let twiddle_start = std::time::Instant::now();
    let twiddles = MetalBackend::precompute_twiddles(eval_domain.half_coset);
    let twiddle_ms = twiddle_start.elapsed().as_secs_f64() * 1000.0;

    let trace_extension_start = std::time::Instant::now();
    // Build GPU-resident flat trace buffer, avoiding CPU round-trip.
    //
    // Two paths:
    //   1. Polys already on eval_domain → concat their GPU buffers directly.
    //   2. Need domain extension → batch RFFT produces a single GPU buffer.
    let all_on_eval_domain = trace_columns
        .iter()
        .all(|poly| poly.evals.domain == eval_domain);

    let (gpu_trace_buffer, n_trace_columns) = if all_on_eval_domain {
        // Concatenate individual BaseFieldVec GPU buffers.
        let n_cols = trace_columns.len();
        let col_len = eval_domain.size();
        let total_len = n_cols * col_len;
        let mut flat = U32Buffer::uninitialized(total_len).map_err(|e| {
            MetalProveRuntimeCompositionError::CompositionShape {
                message: format!("GPU trace concat alloc failed: {}", e.message()),
            }
        })?;
        for (i, poly) in trace_columns.iter().enumerate() {
            flat.copy_from_offset(&poly.evals.values.buffer, i * col_len)
                .map_err(|e| MetalProveRuntimeCompositionError::CompositionShape {
                    message: format!("GPU trace concat copy failed: {}", e.message()),
                })?;
        }
        (flat, n_cols)
    } else {
        // Batch RFFT: the resulting BaseFieldColumnBatch already has a contiguous
        // GPU buffer in column-major layout — use it directly.
        let coeffs = trace_columns
            .iter()
            .map(|poly| {
                poly.coeffs.as_ref().ok_or_else(|| {
                    MetalProveRuntimeCompositionError::CompositionShape {
                        message: "V1 runtime composition requires stored trace coefficients for eval-domain extension".to_string(),
                    }
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let batch = evaluate_polys_on_domain_batch(&coeffs, eval_domain, &twiddles).map_err(
            |source| MetalProveRuntimeCompositionError::CompositionShape {
                message: format!("V1 runtime eval-domain extension failed: {source}"),
            },
        )?;
        let n_cols = batch.n_columns();
        // Take ownership of the batch's GPU buffer directly.
        let buf = batch.into_buffer();
        (buf, n_cols)
    };
    let trace_extension_ms = trace_extension_start.elapsed().as_secs_f64() * 1000.0;

    // interaction 0 = empty (preprocessed, handled separately), interaction 1 = all trace columns
    let interaction_offsets: Vec<u32> = vec![0, 0, n_trace_columns as u32];
    let n_interactions = 2u32;

    let eval_program_start = std::time::Instant::now();
    let (mut row_res, dispatch) = execute_eval_program_v1_with_gpu_trace(
        &ctx.program,
        gpu_trace_buffer,
        &interaction_offsets,
        eval_domain.size(),
        n_interactions,
        0, // n_preprocessed_columns
        &[], // base_params
        &[], // ext_params
        &random_coeff_powers,
    ).map_err(|source| MetalProveRuntimeCompositionError::ProgramExecution { source })?;
    let eval_program_ms = eval_program_start.elapsed().as_secs_f64() * 1000.0;

    if row_res.len() != eval_domain.size() {
        return Err(MetalProveRuntimeCompositionError::CompositionShape {
            message: format!(
                "V1 runtime composition expected {} eval rows, got {}",
                eval_domain.size(),
                row_res.len()
            ),
        });
    }

    let quotient_start = std::time::Instant::now();
    let denominator_inverses =
        denominator_inverses_for_eval_domain(ctx.log_n_rows, eval_domain.log_size());
    // Apply denominator inverses in-place to avoid a second allocation.
    let log_n_rows = ctx.log_n_rows;
    for (row_index, value) in row_res.iter_mut().enumerate() {
        *value = *value * denominator_inverses[row_index >> log_n_rows];
    }
    let quotient_application_ms = quotient_start.elapsed().as_secs_f64() * 1000.0;

    let interpolation_start = std::time::Instant::now();
    let composition_eval = SecureEvaluation::new(
        eval_domain,
        metal_secure_column_from_values(row_res),
    );
    let composition_poly = composition_eval.interpolate_with_twiddles(&twiddles);
    let interpolation_ms = interpolation_start.elapsed().as_secs_f64() * 1000.0;

    Ok((
        composition_poly,
        dispatch,
        MetalCompositionDetailBreakdown {
            twiddle_ms,
            trace_extension_ms,
            eval_program_ms,
            quotient_application_ms,
            interpolation_ms,
        },
    ))
}

// ---------------------------------------------------------------------------
// Standalone composition from raw column data
// ---------------------------------------------------------------------------

/// Compute composition polynomial quotient values from raw column evaluations
/// on the eval domain.
///
/// For each component:
/// 1. Builds V1 runtime inputs from the provided column data
/// 2. Executes the V1 program on Metal GPU (or CPU fallback)
/// 3. Applies vanishing polynomial inverse (denom_inv)
/// 4. Returns per-component quotient evaluations
///
/// The caller is responsible for accumulating results and IFFTing.
///
/// Unlike [`compute_composition_polynomial_v1`], this function does not require
/// a `CommitmentSchemeProver<MetalBackend>` — it takes raw column slices
/// that are already evaluated on the constraint evaluation domain.
pub fn evaluate_composition_quotients_v1(
    components: &[MetalComponentContextV1],
    component_trace_evals: &[Vec<Vec<&[BaseField]>>],
    component_preprocessed_evals: &[Vec<&[BaseField]>],
    random_coeff: SecureField,
    log_blowup_factor: u32,
) -> Result<Vec<(u32, Vec<SecureField>)>, MetalEvaluationProgramExecutionError> {
    assert_eq!(
        components.len(),
        component_trace_evals.len(),
        "component count mismatch between contexts and trace evals"
    );
    assert_eq!(
        components.len(),
        component_preprocessed_evals.len(),
        "component count mismatch between contexts and preprocessed evals"
    );

    // Pre-compute total constraint count for random_coeff_powers.
    let total_constraints: usize = components
        .iter()
        .map(|c| c.n_constraints() as usize)
        .sum();
    let mut all_random_coeff_powers =
        <MetalBackend as AccumulationOps>::generate_secure_powers(random_coeff, total_constraints);
    all_random_coeff_powers.reverse();

    let mut results = Vec::with_capacity(components.len());
    let mut coeff_offset: usize = 0;

    for (comp_idx, comp) in components.iter().enumerate() {
        let n_constraints = comp.n_constraints() as usize;
        let random_coeff_powers = &all_random_coeff_powers[coeff_offset..coeff_offset + n_constraints];
        coeff_offset += n_constraints;

        // Build trace interaction refs for this component.
        let interaction_refs: Vec<Vec<&[BaseField]>> =
            component_trace_evals[comp_idx].clone();
        let interaction_slice_refs: Vec<&[&[BaseField]]> =
            interaction_refs.iter().map(|cols| cols.as_slice()).collect();
        let preprocessed_refs: &[&[BaseField]] =
            &component_preprocessed_evals[comp_idx];

        // Determine eval domain log size from the first non-empty column.
        let eval_domain_log_size = interaction_refs
            .iter()
            .flatten()
            .chain(preprocessed_refs.iter())
            .map(|col| col.len().trailing_zeros())
            .next()
            .unwrap_or(comp.log_n_rows + 1 + log_blowup_factor);

        let runtime = MetalEvaluationProgramRuntimeInputsV1 {
            trace: MetalEvaluationProgramTraceViewV1 {
                trace_interactions: &interaction_slice_refs,
                preprocessed_columns: preprocessed_refs,
            },
            base_params: &comp.base_params,
            ext_params: &comp.ext_params,
            random_coeff_powers,
        };

        let (mut row_res, _dispatch) =
            execute_selected_metal_evaluation_program_v1_on_metal(
                &comp.program,
                runtime,
                MetalEvaluationProgramCapabilityProfileV1::current(),
            )?;

        // Apply vanishing polynomial inverse (denom_inv).
        let denominator_inverses =
            denominator_inverses_for_eval_domain(comp.log_n_rows, eval_domain_log_size);
        let log_n_rows = comp.log_n_rows;
        for (row_index, value) in row_res.iter_mut().enumerate() {
            *value = *value * denominator_inverses[row_index >> log_n_rows];
        }

        results.push((eval_domain_log_size, row_res));
    }

    Ok(results)
}

// ---------------------------------------------------------------------------
// Post-composition prove flow
// ---------------------------------------------------------------------------

struct PostCompositionRuntime<'a> {
    prepared_prove_values:
        PreparedCommitmentSchemeProveValues<'a, MetalBackend, Blake2sMerkleChannel>,
    sampled_values: MetalPostCompositionSampledValuesV1,
    oods_point: stwo::core::circle::CirclePoint<SecureField>,
    max_log_degree_bound: u32,
}

struct PostCompositionFinishRuntime<'a> {
    commitment_scheme: CommitmentSchemeProver<'a, MetalBackend, Blake2sMerkleChannel>,
    sampled_values_tree:
        TreeVec<Vec<Vec<SecureField>>>,
    commitments: TreeVec<
        <Blake2sMerkleHasher as stwo::core::vcs_lifted::merkle_hasher::MerkleHasherLifted>::Hash,
    >,
    proof_of_work: u64,
    fri_proof: stwo::core::fri::ExtendedFriProof<Blake2sMerkleHasher>,
    tree_query_positions: Vec<Vec<usize>>,
    sampled_values: MetalPostCompositionSampledValuesV1,
    post_composition_eval: SecureField,
    sampled_values_dispatch: MetalSampledValuesDispatchKindV1,
}

struct PostCompositionTreeDecommitRuntime<'a> {
    commitment_scheme: CommitmentSchemeProver<'a, MetalBackend, Blake2sMerkleChannel>,
    sampled_values_tree:
        TreeVec<Vec<Vec<SecureField>>>,
    commitments: TreeVec<
        <Blake2sMerkleHasher as stwo::core::vcs_lifted::merkle_hasher::MerkleHasherLifted>::Hash,
    >,
    proof_of_work: u64,
    fri_proof: stwo::core::fri::ExtendedFriProof<Blake2sMerkleHasher>,
    tree_query_positions: Vec<Vec<usize>>,
    sampled_values: MetalPostCompositionSampledValuesV1,
    post_composition_eval: SecureField,
    sampled_values_dispatch: MetalSampledValuesDispatchKindV1,
}

/// Stage prove-values phase. Extracts oods_point, max_log_degree_bound, and
/// sample_points from channel and commitment scheme.
pub fn stage_prove_values_v1(
    component_provers: &ComponentProvers<'_, MetalBackend>,
    channel: &mut Blake2sChannel,
    commitment_scheme: &CommitmentSchemeProver<'_, MetalBackend, Blake2sMerkleChannel>,
) -> MetalProveValuesStaging {
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

    let mut sample_points =
        component_provers
            .components()
            .mask_points(oods_point, max_log_degree_bound, false);
    sample_points.push(vec![vec![oods_point]; 2 * SECURE_EXTENSION_DEGREE]);

    MetalProveValuesStaging {
        oods_point,
        max_log_degree_bound,
        sample_points,
    }
}

/// Lower sampled values from tree-vec form to V1 ABI.
pub fn lower_post_composition_sampled_values_v1(
    sampled_values: &TreeVec<Vec<Vec<SecureField>>>,
) -> Result<MetalPostCompositionSampledValuesV1, MetalSampledValuesLoweringError> {
    Ok(MetalPostCompositionSampledValuesV1 {
        sampled_values: lower_metal_sampled_values_v1(sampled_values)?,
    })
}

/// Interpret post-composition sampled values using the generic AIR component
/// interpreter.
pub fn interpret_post_composition_sampled_values_v1(
    components: &stwo::core::air::Components<'_>,
    point: stwo::core::circle::CirclePoint<SecureField>,
    sampled_values: &MetalPostCompositionSampledValuesV1,
    random_coeff: SecureField,
    max_log_degree_bound: u32,
) -> Result<SecureField, MetalSampledValuesInterpreterError> {
    interpret_metal_sampled_values_v1(
        components,
        point,
        sampled_values.sampled_values(),
        random_coeff,
        max_log_degree_bound,
    )
}

/// Interpret post-composition sampled values using the reference interpreter.
pub fn interpret_post_composition_sampled_values_v1_reference(
    point: stwo::core::circle::CirclePoint<SecureField>,
    sampled_values: &MetalPostCompositionSampledValuesV1,
    max_log_degree_bound: u32,
) -> Result<SecureField, MetalSampledValuesExecutionError> {
    interpret_metal_sampled_values_v1_reference(
        point,
        sampled_values.sampled_values(),
        max_log_degree_bound,
    )
}

/// Execute post-composition sampled values using selected dispatch.
pub fn execute_post_composition_sampled_values_v1(
    point: stwo::core::circle::CirclePoint<SecureField>,
    sampled_values: &MetalPostCompositionSampledValuesV1,
    max_log_degree_bound: u32,
) -> Result<(SecureField, MetalSampledValuesDispatchKindV1), MetalSampledValuesExecutionError> {
    execute_selected_metal_sampled_values_v1(
        point,
        sampled_values.sampled_values(),
        max_log_degree_bound,
    )
}

fn prepare_post_composition_runtime<'a>(
    channel: &mut Blake2sChannel,
    commitment_scheme: CommitmentSchemeProver<'a, MetalBackend, Blake2sMerkleChannel>,
    staging: MetalProveValuesStaging,
) -> Result<PostCompositionRuntime<'a>, ProvingError> {
    let prepared_prove_values =
        commitment_scheme.prepare_prove_values(staging.sample_points, channel);
    let sampled_values = lower_post_composition_sampled_values_v1(
        prepared_prove_values.sampled_values(),
    )
    .map_err(|_| ProvingError::ConstraintsNotSatisfied)?;

    Ok(PostCompositionRuntime {
        prepared_prove_values,
        sampled_values,
        oods_point: staging.oods_point,
        max_log_degree_bound: staging.max_log_degree_bound,
    })
}

fn execute_post_composition_runtime(
    channel: &mut Blake2sChannel,
    runtime: PostCompositionRuntime<'_>,
    skip_reference_sanity_check: bool,
) -> Result<(MetalProveValuesResult, MetalProveValuesBreakdown), ProvingError> {
    let sampled_values_v1_start = std::time::Instant::now();
    let (post_composition_eval, sampled_values_dispatch) =
        execute_post_composition_sampled_values_v1(
            runtime.oods_point,
            &runtime.sampled_values,
            runtime.max_log_degree_bound,
        )
        .map_err(|_| ProvingError::ConstraintsNotSatisfied)?;
    let sampled_values_v1_ms = sampled_values_v1_start.elapsed().as_secs_f64() * 1000.0;

    let sanity_check_start = std::time::Instant::now();
    if !skip_reference_sanity_check {
        let reference_sampled_values_eval =
            interpret_post_composition_sampled_values_v1_reference(
                runtime.oods_point,
                &runtime.sampled_values,
                runtime.max_log_degree_bound,
            )
            .map_err(|_| ProvingError::ConstraintsNotSatisfied)?;
        if reference_sampled_values_eval != post_composition_eval {
            return Err(ProvingError::ConstraintsNotSatisfied);
        }
    }
    let sanity_check_ms = sanity_check_start.elapsed().as_secs_f64() * 1000.0;

    let fri_and_decommit_start = std::time::Instant::now();
    let finish_runtime = prepare_post_composition_finish_runtime(
        channel,
        runtime,
        post_composition_eval,
        sampled_values_dispatch,
    )?;
    let result = execute_post_composition_finish_runtime(finish_runtime);
    let fri_and_decommit_ms = fri_and_decommit_start.elapsed().as_secs_f64() * 1000.0;

    Ok((
        result,
        MetalProveValuesBreakdown {
            prove_values_ms: 0.0, // overwritten by caller with preparation time
            sampled_values_v1_ms,
            sampled_values_dispatch,
            sanity_check_ms,
            fri_and_decommit_ms,
        },
    ))
}

fn prepare_post_composition_finish_runtime<'a>(
    channel: &mut Blake2sChannel,
    runtime: PostCompositionRuntime<'a>,
    post_composition_eval: SecureField,
    sampled_values_dispatch: MetalSampledValuesDispatchKindV1,
) -> Result<PostCompositionFinishRuntime<'a>, ProvingError> {
    let (commitment_scheme, samples, sampled_values_tree) =
        runtime.prepared_prove_values.into_parts();
    let columns = commitment_scheme.evaluations();
    let lifting_log_size = commitment_scheme.lifting_log_size();
    let quotients = compute_fri_quotients(
        &columns,
        &samples,
        channel.draw_secure_felt(),
        lifting_log_size,
        commitment_scheme.config.fri_config.log_blowup_factor,
    );
    let fri_prover = FriProver::<MetalBackend, Blake2sMerkleChannel>::commit(
        channel,
        commitment_scheme.config.fri_config,
        &quotients,
        commitment_scheme.twiddles(),
    );
    let proof_of_work = MetalBackend::grind(channel, commitment_scheme.config.pow_bits);
    channel.mix_u64(proof_of_work);
    let FriDecommitResult {
        fri_proof,
        query_positions,
        unsorted_query_locations: _unsorted_query_locations,
    } = fri_prover.decommit(channel);
    let tree_log_sizes = commitment_scheme
        .trees
        .iter()
        .map(|tree| tree.commitment.layers.len() as u32 - 1)
        .collect::<Vec<_>>();
    let tree_query_positions = tree_log_sizes
        .iter()
        .map(|&tree_log_size| {
            prepare_query_positions_for_tree(&query_positions, lifting_log_size, tree_log_size)
        })
        .collect();
    let commitments = commitment_scheme.roots();
    Ok(PostCompositionFinishRuntime {
        commitment_scheme,
        sampled_values_tree,
        commitments,
        proof_of_work,
        fri_proof,
        tree_query_positions,
        sampled_values: runtime.sampled_values,
        post_composition_eval,
        sampled_values_dispatch,
    })
}

fn execute_post_composition_finish_runtime(
    runtime: PostCompositionFinishRuntime<'_>,
) -> MetalProveValuesResult {
    let tree_decommit_runtime = prepare_post_composition_tree_decommit_runtime(runtime);
    execute_post_composition_tree_decommit_runtime(tree_decommit_runtime)
}

fn prepare_post_composition_tree_decommit_runtime(
    runtime: PostCompositionFinishRuntime<'_>,
) -> PostCompositionTreeDecommitRuntime<'_> {
    PostCompositionTreeDecommitRuntime {
        commitment_scheme: runtime.commitment_scheme,
        sampled_values_tree: runtime.sampled_values_tree,
        commitments: runtime.commitments,
        proof_of_work: runtime.proof_of_work,
        fri_proof: runtime.fri_proof,
        tree_query_positions: runtime.tree_query_positions,
        sampled_values: runtime.sampled_values,
        post_composition_eval: runtime.post_composition_eval,
        sampled_values_dispatch: runtime.sampled_values_dispatch,
    }
}

fn execute_post_composition_tree_decommit_runtime(
    mut runtime: PostCompositionTreeDecommitRuntime<'_>,
) -> MetalProveValuesResult {
    let mut queried_values = Vec::with_capacity(runtime.commitment_scheme.trees.len());
    let mut decommitments = Vec::with_capacity(runtime.commitment_scheme.trees.len());
    for (tree, query_positions) in runtime
        .commitment_scheme
        .trees
        .as_ref()
        .0
        .into_iter()
        .zip(runtime.tree_query_positions.iter())
    {
        let (values, decommit_result) = tree.decommit_queries(query_positions);
        queried_values.push(values);
        decommitments.push(decommit_result.decommitment);
    }
    runtime.commitment_scheme.recycle_owned_tree_columns();
    let proof = StarkProof(CommitmentSchemeProof {
        config: runtime.commitment_scheme.config,
        commitments: runtime.commitments,
        sampled_values: runtime.sampled_values_tree,
        decommitments: TreeVec(decommitments),
        queried_values: TreeVec(queried_values),
        proof_of_work: runtime.proof_of_work,
        fri_proof: runtime.fri_proof.proof,
    });
    MetalProveValuesResult {
        proof,
        sampled_values: runtime.sampled_values,
        post_composition_eval: runtime.post_composition_eval,
        sampled_values_dispatch: runtime.sampled_values_dispatch,
    }
}

/// Execute the full prove-values pipeline through the V1 runtime.
pub fn execute_prove_values_v1(
    channel: &mut Blake2sChannel,
    commitment_scheme: CommitmentSchemeProver<'_, MetalBackend, Blake2sMerkleChannel>,
    staging: MetalProveValuesStaging,
) -> Result<(MetalProveValuesResult, MetalProveValuesBreakdown), ProvingError> {
    execute_prove_values_v1_with_opts(channel, commitment_scheme, staging, false)
}

fn execute_prove_values_v1_with_opts(
    channel: &mut Blake2sChannel,
    commitment_scheme: CommitmentSchemeProver<'_, MetalBackend, Blake2sMerkleChannel>,
    staging: MetalProveValuesStaging,
    skip_reference_sanity_check: bool,
) -> Result<(MetalProveValuesResult, MetalProveValuesBreakdown), ProvingError> {
    let prove_values_start = std::time::Instant::now();
    let runtime = prepare_post_composition_runtime(channel, commitment_scheme, staging)?;
    let prove_values_ms = prove_values_start.elapsed().as_secs_f64() * 1000.0;
    let (result, mut breakdown) =
        execute_post_composition_runtime(channel, runtime, skip_reference_sanity_check)?;
    breakdown.prove_values_ms = prove_values_ms;
    Ok((result, breakdown))
}

// ---------------------------------------------------------------------------
// Prove core
// ---------------------------------------------------------------------------

/// Error from prove-core execution.
#[derive(Debug)]
pub enum MetalProveCoreError {
    CompositionGeneration {
        source: MetalProveRuntimeCompositionError,
    },
    Proving {
        source: ProvingError,
    },
}

impl From<ProvingError> for MetalProveCoreError {
    fn from(source: ProvingError) -> Self {
        Self::Proving { source }
    }
}

/// Execute the full prove-core pipeline through the V1 runtime.
///
/// This is the primary entry point for proving through the shared V1 runtime.
/// It orchestrates: composition polynomial generation via V1, commit, staging,
/// and prove-values execution.
pub fn execute_prove_core_v1(
    ctx: &MetalProveRuntimeContextV1,
    components: &[&dyn ComponentProver<MetalBackend>],
    channel: &mut Blake2sChannel,
    mut commitment_scheme: CommitmentSchemeProver<'_, MetalBackend, Blake2sMerkleChannel>,
) -> Result<(MetalProveValuesResult, MetalProveCoreBreakdown), MetalProveCoreError> {
    let n_preprocessed_columns = commitment_scheme.trees
        [stwo::core::verifier::PREPROCESSED_TRACE_IDX]
        .polynomials
        .len();
    let component_provers = ComponentProvers {
        components: components.to_vec(),
        n_preprocessed_columns,
    };
    let trace = commitment_scheme.trace();

    let random_coeff = channel.draw_secure_felt();
    let composition_generation_start = std::time::Instant::now();
    let (composition_poly, evaluation_program_v1_dispatch, composition_detail) =
        compute_composition_polynomial_v1(ctx, &trace, random_coeff)
            .map_err(|source| MetalProveCoreError::CompositionGeneration { source })?;
    let composition_generation_ms =
        composition_generation_start.elapsed().as_secs_f64() * 1000.0;

    let composition_commit_start = std::time::Instant::now();
    let mut tree_builder = commitment_scheme.tree_builder();
    let (left_comp_poly_half, right_comp_poly_half) = composition_poly.split_at_mid();
    tree_builder.extend_polys(left_comp_poly_half.into_coordinate_polys());
    tree_builder.extend_polys(right_comp_poly_half.into_coordinate_polys());
    tree_builder.commit(channel);
    let composition_commit_ms = composition_commit_start.elapsed().as_secs_f64() * 1000.0;

    let prove_values_stage =
        stage_prove_values_v1(&component_provers, channel, &commitment_scheme);
    let prove_values_total_start = std::time::Instant::now();
    let (prove_values_result, prove_values_breakdown) =
        execute_prove_values_v1_with_opts(
            channel,
            commitment_scheme,
            prove_values_stage,
            ctx.skip_reference_sanity_check,
        )
        .map_err(MetalProveCoreError::from)?;
    let prove_values_total_ms = prove_values_total_start.elapsed().as_secs_f64() * 1000.0;

    Ok((
        prove_values_result,
        MetalProveCoreBreakdown {
            evaluation_program_v1_ms: composition_detail.eval_program_ms,
            evaluation_program_v1_dispatch,
            composition_generation_ms,
            composition_commit_ms,
            prove_values_ms: prove_values_total_ms,
            sanity_check_ms: prove_values_breakdown.sanity_check_ms,
            composition_detail,
            prove_values_detail: MetalProveValuesDetailBreakdown {
                prepare_ms: prove_values_breakdown.prove_values_ms,
                sampled_values_v1_ms: prove_values_breakdown.sampled_values_v1_ms,
                fri_and_decommit_ms: prove_values_breakdown.fri_and_decommit_ms,
            },
        },
    ))
}

// ---------------------------------------------------------------------------
// Trace commit helpers
// ---------------------------------------------------------------------------

pub struct TraceCommitBreakdown {
    pub interpolation_ms: f64,
    pub extension_ms: f64,
    pub merkle_ms: f64,
}

fn build_native_standard_blake2s_merkle_prover(
    columns: Vec<&crate::stwo_metal::base_field_vec::BaseFieldVec>,
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

pub fn commit_trace_with_breakdown(
    commitment_scheme: &mut CommitmentSchemeProver<'_, MetalBackend, Blake2sMerkleChannel>,
    prover_channel: &mut Blake2sChannel,
    trace: Vec<CircleEvaluation<MetalBackend, BaseField, BitReversedOrder>>,
    twiddles: &stwo::prover::poly::twiddles::TwiddleTree<MetalBackend>,
) -> TraceCommitBreakdown {
    let interpolation_start = std::time::Instant::now();
    let trace_polynomials = MetalBackend::interpolate_columns(trace, twiddles);
    let interpolation_ms = interpolation_start.elapsed().as_secs_f64() * 1000.0;

    let extension_start = std::time::Instant::now();
    let trace_polynomials = MetalBackend::evaluate_polynomials(
        trace_polynomials,
        commitment_scheme.config.fri_config.log_blowup_factor,
        twiddles,
        commitment_scheme.store_polynomials_coefficients,
        &commitment_scheme.base_column_pool,
    );
    let extension_ms = extension_start.elapsed().as_secs_f64() * 1000.0;

    let merkle_start = std::time::Instant::now();
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::metal::eval_program_v1::{
        lower_registered_metal_evaluation_program_v1,
        MetalEvaluationProgramBudgetV1,
        MetalEvaluationProgramSpecializationV1,
    };

    fn make_test_context(log_n_rows: u32, n_columns: u32) -> MetalProveRuntimeContextV1 {
        let program = lower_registered_metal_evaluation_program_v1(
            "fibonacci_example",
            MetalEvaluationProgramSpecializationV1 {
                log_n_rows,
                n_columns,
            },
        )
        .unwrap();
        program
            .validate(MetalEvaluationProgramBudgetV1::new(2048, 512))
            .unwrap();
        MetalProveRuntimeContextV1::new(program, log_n_rows)
    }

    #[test]
    fn prove_runtime_context_exposes_program_shape() {
        let ctx = make_test_context(3, 6);
        assert_eq!(ctx.log_n_rows(), 3);
        assert_eq!(ctx.n_constraints(), 4);
    }

    #[test]
    fn lane_validation_rejects_cpu_only_plan() {
        let err = validate_prove_runtime_lane_v1(
            "test",
            MetalExecutionPlan::CpuOnly,
            Some(MetalWorkloadOwnership::CpuOwned),
            Some(MetalWorkloadOwnership::MetalNative),
        )
        .unwrap_err();
        assert!(matches!(
            err,
            MetalProveRuntimeLaneError::PlanNotMetalCapable { .. }
        ));
    }

    #[test]
    fn lane_validation_accepts_hybrid_plan() {
        validate_prove_runtime_lane_v1(
            "test",
            MetalExecutionPlan::MetalFriHybrid,
            Some(MetalWorkloadOwnership::CpuOwned),
            Some(MetalWorkloadOwnership::MetalNative),
        )
        .unwrap();
    }

    // TODO: fix this test after Trace API change (polys is now TreeVec<ColumnVec<&Poly<B>>>)
    // #[test]
    // fn composition_polynomial_v1_matches_reference_on_small_trace() { ... }
}
