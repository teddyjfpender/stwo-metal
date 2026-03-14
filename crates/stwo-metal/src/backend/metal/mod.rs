mod accumulation;
mod artifact;
mod backend;
mod benchmark;
mod blake2s;
mod capability;
mod column;
mod commitment_slice;
mod eval_program_v1;
mod execution_plan;
mod shader_compiler_v1;
pub mod recording_eval;
pub mod recording_eval_v1;
mod first_layer;
mod fri;
mod generated_policy;
mod handoff;
mod line;
mod lookups;
mod planner;
mod planner_manifest_v1_generated;
mod poly;
mod proof;
mod proof_slice;
mod prove_runtime_v1;
mod prover;
mod quotient;
mod row;
mod sampled_values_v1;
mod sequence;
mod subpath;
mod witness;
mod workload;
mod workload_contract;

pub use backend::MetalBackend;
pub use benchmark::{
    declare_wide_fibonacci_benchmark_boundary, MetalBenchmarkInputError,
    MetalBenchmarkOperation, MetalBenchmarkProgramError, MetalBenchmarkProgramExecutionError,
    MetalBenchmarkProveCoreError,
    MetalBenchmarkReferencePlatform, MetalBenchmarkTarget, MetalBenchmarkTraceShapeError,
    MetalGeneratedWideFibonacciBenchmarkBreakdown, MetalGeneratedWideFibonacciBenchmarkError,
    MetalGeneratedWideFibonacciBenchmarkIteration, MetalGeneratedWideFibonacciBenchmarkRun,
    MetalGeneratedWideFibonacciBenchmarkSample,
    MetalWideFibonacciBenchmarkBoundary, MetalWideFibonacciWitnessInputs,
    WIDE_FIBONACCI_PROVE_LOG20_TARGET,
    WIDE_FIBONACCI_TRACE_LOG20_TARGET,
};
pub use capability::{
    metal_backend_surface_detail, metal_backend_surface_status, metal_runtime_error,
    metal_runtime_support, MetalBackendSurface, MetalBackendSurfaceStatus, MetalRuntimeSupport,
    STWO_METAL_BACKEND_SURFACES_V1,
};
pub use commitment_slice::MetalFriCommitmentSlice;
pub use eval_program_v1::{
    execute_compiled_metal_evaluation_program_v1,
    execute_compiled_metal_evaluation_program_v1_async,
    execute_compiled_metal_evaluation_program_v1_tg,
    complete_compiled_metal_evaluation_program_v1_async,
    execute_metal_evaluation_program_v1_on_metal,
    execute_selected_metal_evaluation_program_v1_on_metal, interpret_metal_evaluation_program_v1,
    lookup_metal_evaluation_program_overlay_v1, lower_registered_metal_evaluation_program_v1,
    lower_virtual_snos_evaluation_program_v1, lower_wide_fibonacci_evaluation_program_v1,
    metal_evaluation_program_overlays_v1,
    metal_evaluation_program_semantic_hash_v1, select_metal_evaluation_program_dispatch_v1,
    validate_eval_program_abi_layout_v1, validate_metal_evaluation_program_v1,
    MetalEvaluationProgramBaseInstV1,
    MetalEvaluationProgramBaseOpcodeV1, MetalEvaluationProgramBudgetV1,
    MetalEvaluationProgramCapabilityProfileV1, MetalEvaluationProgramDispatchKindV1,
    MetalEvaluationProgramExecutionError, MetalEvaluationProgramExtInstV1,
    MetalEvaluationProgramExtOpcodeV1, MetalEvaluationProgramHeaderV1,
    MetalEvaluationProgramInterpreterError, MetalEvaluationProgramLoweringError,
    MetalEvaluationProgramOverlayV1, MetalEvaluationProgramRuntimeInputsV1,
    MetalEvaluationProgramSectionDescV1, MetalEvaluationProgramSectionKindV1,
    MetalEvaluationProgramSpecializationV1, MetalEvaluationProgramTraceViewV1,
    MetalEvaluationProgramV1, MetalEvaluationProgramValidationError, OwnedMetalEvaluationProgramV1,
    lower_framework_eval_to_v1, lower_framework_eval_to_v1_with_logup,
    STWO_METAL_EVAL_PROGRAM_ABI_MAJOR_V1, STWO_METAL_EVAL_PROGRAM_ABI_MINOR_V1,
    STWO_METAL_EVAL_PROGRAM_CAP_BASE_INV_V1, STWO_METAL_EVAL_PROGRAM_CAP_EXT_MUL_V1,
    STWO_METAL_EVAL_PROGRAM_CAP_PREFINALIZED_LOGUP_V1,
    STWO_METAL_EVAL_PROGRAM_FLAG_DEBUG_PRESENT_V1,
    STWO_METAL_EVAL_PROGRAM_FLAG_PREFINALIZED_LOGUP_V1, STWO_METAL_EVAL_PROGRAM_MAGIC_V1,
    STWO_METAL_EVAL_PROGRAM_SECURE_EXT_DEGREE_V1,
};
pub use shader_compiler_v1::{compile_v1_to_metal_source, compiled_kernel_name};
pub use first_layer::MetalFriFirstLayer;
pub use fri::{
    fold_circle_into_line_first_layer, fold_line, precompute_fri_factors_cpu, upload_fri_factors,
    PrecomputedFriFactors,
};
pub use handoff::{
    commit_line_evaluation_via_cpu_bridge, materialize_line_evaluation_via_cpu_bridge,
    CpuLineCommitmentBridge,
};
pub use line::{MetalFriLayerDecommitment, MetalLineCommitment, MetalLineEvaluation};
pub use planner::{
    plan_exemplar_metal_prove_by_name, plan_metal_operation, MetalComponentCapability,
    MetalComponentPlanInput, MetalExecutionIntent, MetalExecutionPlan, MetalOperationKind,
    MetalPlannerError, MetalSupportTier, UnknownMetalComponent, UnsupportedGeneratedMetalRoute,
    UnsupportedMetalPlan,
};
pub use planner_manifest_v1_generated::STWO_METAL_PLANNER_COMPONENTS_V1;
pub use poly::{evaluate_polys_on_domain_batch, permute_coset_to_circle_domain_bit_reversed};
pub use proof::{
    MetalExtendedInnerFriProof, MetalFriInnerProofSlice, MetalInnerFriProof, MetalInnerFriProofAux,
};
pub use proof_slice::MetalFriProofSlice;
pub use prover::MetalFriProver;
pub use quotient::{
    accumulate_wide_fibonacci_quotients, accumulate_wide_fibonacci_quotients_from_batch,
    compute_quotient_domain_coords_cpu, upload_quotient_domain_coords,
    MetalWideFibonacciBatchQuotientRequest, MetalWideFibonacciQuotientError,
    MetalWideFibonacciQuotientRequest, MetalWideFibonacciQuotients,
};
pub use row::MetalFriInnerLayerRow;
pub use sampled_values_v1::{
    execute_selected_metal_sampled_values_v1, interpret_metal_sampled_values_v1,
    interpret_metal_sampled_values_v1_reference, lower_metal_sampled_values_v1,
    select_metal_sampled_values_dispatch_v1, validate_sampled_values_abi_layout_v1,
    MetalSampledValuesColumnDescV1,
    MetalSampledValuesDispatchKindV1, MetalSampledValuesExecutionError,
    MetalSampledValuesHeaderV1, MetalSampledValuesInterpreterError,
    MetalSampledValuesLoweringError, MetalSampledValuesTreeDescV1,
    MetalSampledValuesValidationError, MetalSecureFieldValueV1,
    OwnedMetalSampledValuesV1,
    STWO_METAL_SAMPLED_VALUES_ABI_MAJOR_V1, STWO_METAL_SAMPLED_VALUES_ABI_MINOR_V1,
    STWO_METAL_SAMPLED_VALUES_MAGIC_V1,
};
pub use sequence::MetalFriInnerLayerSequence;
pub use subpath::MetalFriBlake2sSubpath;
pub use witness::{
    generate_metal_wide_fibonacci_trace, MetalWideFibonacciTrace, MetalWideFibonacciTraceError,
    MetalWideFibonacciTraceRequest,
};
pub use workload::{
    declare_exemplar_hybrid_fri_workload, declare_exemplar_metal_workload_boundary,
    MetalAcceptanceLaneError, MetalCpuQuotientEvaluationInput, MetalCpuWideFibonacciWitnessInput,
    MetalFriReadyEvaluationInput, MetalHybridFriWorkload, MetalQuotientEvaluationInput,
    MetalWorkloadBoundary, MetalWorkloadHandoffError,
};
pub use prove_runtime_v1::{
    commit_trace_with_breakdown, compute_composition_polynomial_v1,
    evaluate_composition_quotients_v1, execute_prove_core_v1,
    execute_prove_values_v1, lower_post_composition_sampled_values_v1,
    interpret_post_composition_sampled_values_v1,
    interpret_post_composition_sampled_values_v1_reference,
    execute_post_composition_sampled_values_v1,
    stage_prove_values_v1, validate_prove_runtime_lane_v1,
    MetalComponentContextV1, MetalCompositionDetailBreakdown, MetalProveValuesDetailBreakdown,
    MetalPostCompositionSampledValuesV1, MetalProofMetadata, MetalProofSizeBreakdown,
    MetalProveCoreBreakdown, MetalProveCoreError, MetalProveRuntimeCompositionError,
    MetalProveRuntimeContextV1, MetalProveRuntimeLaneError, MetalProveValuesBreakdown,
    MetalProveValuesResult, MetalProveValuesStaging, TraceCommitBreakdown,
};
pub use workload_contract::{MetalWorkloadOwnership, MetalWorkloadStage};

pub use crate::stwo_metal::{
    BaseFieldColumnBatch as MetalBaseFieldColumnBatch, BaseFieldVec as MetalBaseFieldVec,
    SecureFieldVec as MetalSecureFieldVec,
};
