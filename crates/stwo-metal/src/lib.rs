//! Companion proving-facing GPU package boundary for Stwo.
//!
//! This crate intentionally exports only the minimum stable surface current
//! non-`stwo` workspace consumers need while the raw/native split is still
//! being shrink-wrapped inside the workspace.

#[cfg(feature = "prover")]
pub mod backend;
#[cfg(feature = "prover")]
#[allow(dead_code)]
mod stwo_cuda;
#[cfg(feature = "prover")]
#[allow(dead_code)]
mod stwo_metal;

#[cfg(feature = "prover")]
pub mod abi {
    pub use crate::backend::cuda::{
        CudaPoseidonLookupElementsAbiV1, OwnedConstraintEvalAbiV1, StwoCudaLookupElements16AbiV1,
        StwoCudaPoseidonEvalAbiV1, StwoCudaWideFibonacciEvalAbiV1,
    };
}

#[cfg(feature = "prover")]
pub mod capability {
    pub use crate::backend::cuda::{
        cuda_backend_surface_detail, cuda_backend_surface_status, CudaBackendSurface,
        CudaBackendSurfaceStatus, STWO_CUDA_BACKEND_SURFACES_V1,
    };
    pub use crate::backend::metal::{
        metal_backend_surface_detail, metal_backend_surface_status, MetalBackendSurface,
        MetalBackendSurfaceStatus, MetalRuntimeSupport, STWO_METAL_BACKEND_SURFACES_V1,
    };
}

#[cfg(feature = "prover")]
pub mod benchmark {
    pub use crate::backend::metal::{
        declare_wide_fibonacci_benchmark_boundary, MetalBenchmarkInputError,
        MetalBenchmarkLaneError, MetalBenchmarkOperation, MetalBenchmarkProgramError,
        MetalBenchmarkProgramExecutionError, MetalBenchmarkProveCoreBreakdown,
        MetalBenchmarkProveCoreError,
        MetalBenchmarkProveValuesBreakdown,
        MetalBenchmarkProveValuesStaging, MetalBenchmarkReferencePlatform, MetalBenchmarkTarget,
        MetalBenchmarkTraceShapeError, MetalWideFibonacciBenchmarkBoundary,
        MetalWideFibonacciWitnessInputs, WIDE_FIBONACCI_PROVE_LOG20_TARGET,
        WIDE_FIBONACCI_TRACE_LOG20_TARGET,
    };
}

#[cfg(feature = "prover")]
pub mod program {
    pub use crate::backend::metal::{
        execute_metal_evaluation_program_v1_on_metal,
        execute_selected_metal_evaluation_program_v1_on_metal,
        interpret_metal_evaluation_program_v1, lookup_metal_evaluation_program_overlay_v1,
        lower_registered_metal_evaluation_program_v1, lower_wide_fibonacci_evaluation_program_v1,
        metal_evaluation_program_semantic_hash_v1, select_metal_evaluation_program_dispatch_v1,
        validate_metal_evaluation_program_v1, MetalEvaluationProgramCapabilityProfileV1,
        MetalEvaluationProgramBaseInstV1, MetalEvaluationProgramBaseOpcodeV1,
        MetalEvaluationProgramBudgetV1, MetalEvaluationProgramExecutionError,
        MetalEvaluationProgramDispatchKindV1, MetalEvaluationProgramOverlayV1,
        MetalEvaluationProgramExtInstV1, MetalEvaluationProgramExtOpcodeV1,
        MetalEvaluationProgramHeaderV1, MetalEvaluationProgramInterpreterError,
        MetalEvaluationProgramLoweringError, MetalEvaluationProgramRuntimeInputsV1,
        metal_evaluation_program_overlays_v1,
        MetalEvaluationProgramSectionDescV1, MetalEvaluationProgramSectionKindV1,
        MetalEvaluationProgramSpecializationV1, MetalEvaluationProgramTraceViewV1,
        MetalEvaluationProgramV1, MetalEvaluationProgramValidationError,
        OwnedMetalEvaluationProgramV1, STWO_METAL_EVAL_PROGRAM_ABI_MAJOR_V1,
        STWO_METAL_EVAL_PROGRAM_ABI_MINOR_V1, STWO_METAL_EVAL_PROGRAM_CAP_BASE_INV_V1,
        STWO_METAL_EVAL_PROGRAM_CAP_EXT_MUL_V1, STWO_METAL_EVAL_PROGRAM_CAP_PREFINALIZED_LOGUP_V1,
        STWO_METAL_EVAL_PROGRAM_FLAG_DEBUG_PRESENT_V1,
        STWO_METAL_EVAL_PROGRAM_FLAG_PREFINALIZED_LOGUP_V1, STWO_METAL_EVAL_PROGRAM_MAGIC_V1,
        STWO_METAL_EVAL_PROGRAM_SECURE_EXT_DEGREE_V1,
    };
}

#[cfg(feature = "prover")]
pub mod planner {
    pub use crate::backend::cuda::{
        plan_exemplar_prove_by_name, CudaComponentCapability, CudaExecutionIntent,
        CudaExecutionPlan, CudaOperationKind, CudaPlannerError, CudaSupportTier,
        UnknownCudaComponent, UnsupportedCudaPlan,
    };
    pub use crate::backend::metal::{
        plan_exemplar_metal_prove_by_name, plan_metal_operation, MetalComponentCapability,
        MetalComponentPlanInput, MetalExecutionIntent, MetalExecutionPlan, MetalOperationKind,
        MetalPlannerError, MetalSupportTier, UnknownMetalComponent, UnsupportedGeneratedMetalRoute,
        UnsupportedMetalPlan, STWO_METAL_PLANNER_COMPONENTS_V1,
    };
}

#[cfg(feature = "prover")]
pub mod workload {
    pub use crate::backend::metal::{
        declare_exemplar_hybrid_fri_workload, declare_exemplar_metal_workload_boundary,
        MetalAcceptanceLaneError, MetalCpuQuotientEvaluationInput,
        MetalCpuWideFibonacciWitnessInput, MetalFriReadyEvaluationInput, MetalHybridFriWorkload,
        MetalQuotientEvaluationInput, MetalWorkloadBoundary, MetalWorkloadHandoffError,
        MetalWorkloadOwnership, MetalWorkloadStage,
    };
}

#[cfg(feature = "prover")]
pub mod quotient {
    pub use crate::backend::cuda::{
        launch_constraint_quotients_on_domain, opaque_eval_ptr, ConstraintQuotientEvalRequest,
    };
    pub use crate::backend::metal::{
        accumulate_wide_fibonacci_quotients, accumulate_wide_fibonacci_quotients_from_batch,
        MetalWideFibonacciBatchQuotientRequest, MetalWideFibonacciQuotientError,
        MetalWideFibonacciQuotientRequest, MetalWideFibonacciQuotients,
    };
}

#[cfg(feature = "prover")]
pub mod witness {
    pub use crate::backend::cuda::{
        generate_poseidon_interaction_traces, generate_poseidon_traces,
        generate_wide_fibonacci_trace, PoseidonInteractionTraceRequest, PoseidonTraceRequest,
        WideFibonacciTraceRequest,
    };
    pub use crate::backend::metal::{
        generate_metal_wide_fibonacci_trace, MetalWideFibonacciTrace, MetalWideFibonacciTraceError,
        MetalWideFibonacciTraceRequest,
    };
}

#[cfg(feature = "prover")]
pub use abi::{
    CudaPoseidonLookupElementsAbiV1, OwnedConstraintEvalAbiV1, StwoCudaLookupElements16AbiV1,
    StwoCudaPoseidonEvalAbiV1, StwoCudaWideFibonacciEvalAbiV1,
};
#[cfg(feature = "prover")]
pub use benchmark::{
    declare_wide_fibonacci_benchmark_boundary, MetalBenchmarkInputError, MetalBenchmarkLaneError,
    MetalBenchmarkOperation, MetalBenchmarkProgramError, MetalBenchmarkProgramExecutionError,
    MetalBenchmarkProveCoreBreakdown, MetalBenchmarkProveCoreError,
    MetalBenchmarkProveValuesBreakdown,
    MetalBenchmarkProveValuesStaging,
    MetalBenchmarkReferencePlatform, MetalBenchmarkTarget, MetalBenchmarkTraceShapeError,
    MetalWideFibonacciBenchmarkBoundary, MetalWideFibonacciWitnessInputs,
    WIDE_FIBONACCI_PROVE_LOG20_TARGET, WIDE_FIBONACCI_TRACE_LOG20_TARGET,
};
#[cfg(feature = "prover")]
pub use capability::{
    cuda_backend_surface_detail, cuda_backend_surface_status, CudaBackendSurface,
    CudaBackendSurfaceStatus, STWO_CUDA_BACKEND_SURFACES_V1,
};
#[cfg(feature = "prover")]
pub use planner::{
    plan_exemplar_metal_prove_by_name, plan_exemplar_prove_by_name, plan_metal_operation,
    CudaComponentCapability, CudaExecutionIntent, CudaExecutionPlan, CudaOperationKind,
    CudaPlannerError, CudaSupportTier, MetalComponentCapability, MetalComponentPlanInput,
    MetalExecutionIntent, MetalExecutionPlan, MetalOperationKind, MetalPlannerError,
    MetalSupportTier, UnknownCudaComponent, UnknownMetalComponent, UnsupportedCudaPlan,
    UnsupportedGeneratedMetalRoute, UnsupportedMetalPlan, STWO_METAL_PLANNER_COMPONENTS_V1,
};
#[cfg(feature = "prover")]
pub use program::{
    execute_metal_evaluation_program_v1_on_metal,
    execute_selected_metal_evaluation_program_v1_on_metal,
    interpret_metal_evaluation_program_v1, lookup_metal_evaluation_program_overlay_v1,
    lower_registered_metal_evaluation_program_v1, lower_wide_fibonacci_evaluation_program_v1,
    metal_evaluation_program_semantic_hash_v1, select_metal_evaluation_program_dispatch_v1,
    validate_metal_evaluation_program_v1, MetalEvaluationProgramCapabilityProfileV1,
    MetalEvaluationProgramBaseInstV1, MetalEvaluationProgramBaseOpcodeV1,
    MetalEvaluationProgramBudgetV1, MetalEvaluationProgramExecutionError,
    MetalEvaluationProgramDispatchKindV1, MetalEvaluationProgramOverlayV1,
    MetalEvaluationProgramExtInstV1, MetalEvaluationProgramExtOpcodeV1,
    MetalEvaluationProgramHeaderV1, MetalEvaluationProgramInterpreterError,
    MetalEvaluationProgramLoweringError, metal_evaluation_program_overlays_v1,
    MetalEvaluationProgramRuntimeInputsV1,
    MetalEvaluationProgramSectionDescV1, MetalEvaluationProgramSectionKindV1,
    MetalEvaluationProgramSpecializationV1, MetalEvaluationProgramTraceViewV1,
    MetalEvaluationProgramV1, MetalEvaluationProgramValidationError, OwnedMetalEvaluationProgramV1,
    STWO_METAL_EVAL_PROGRAM_ABI_MAJOR_V1, STWO_METAL_EVAL_PROGRAM_ABI_MINOR_V1,
    STWO_METAL_EVAL_PROGRAM_CAP_BASE_INV_V1, STWO_METAL_EVAL_PROGRAM_CAP_EXT_MUL_V1,
    STWO_METAL_EVAL_PROGRAM_CAP_PREFINALIZED_LOGUP_V1,
    STWO_METAL_EVAL_PROGRAM_FLAG_DEBUG_PRESENT_V1,
    STWO_METAL_EVAL_PROGRAM_FLAG_PREFINALIZED_LOGUP_V1, STWO_METAL_EVAL_PROGRAM_MAGIC_V1,
    STWO_METAL_EVAL_PROGRAM_SECURE_EXT_DEGREE_V1,
};
#[cfg(feature = "prover")]
pub use quotient::{
    accumulate_wide_fibonacci_quotients, accumulate_wide_fibonacci_quotients_from_batch,
    launch_constraint_quotients_on_domain, opaque_eval_ptr, ConstraintQuotientEvalRequest,
    MetalWideFibonacciBatchQuotientRequest, MetalWideFibonacciQuotientError,
    MetalWideFibonacciQuotientRequest, MetalWideFibonacciQuotients,
};
#[cfg(feature = "prover")]
pub use witness::{
    generate_metal_wide_fibonacci_trace, generate_poseidon_interaction_traces,
    generate_poseidon_traces, generate_wide_fibonacci_trace, MetalWideFibonacciTrace,
    MetalWideFibonacciTraceError, MetalWideFibonacciTraceRequest, PoseidonInteractionTraceRequest,
    PoseidonTraceRequest, WideFibonacciTraceRequest,
};
#[cfg(feature = "prover")]
pub use workload::{
    declare_exemplar_hybrid_fri_workload, declare_exemplar_metal_workload_boundary,
    MetalAcceptanceLaneError, MetalCpuQuotientEvaluationInput, MetalCpuWideFibonacciWitnessInput,
    MetalFriReadyEvaluationInput, MetalHybridFriWorkload, MetalQuotientEvaluationInput,
    MetalWorkloadBoundary, MetalWorkloadHandoffError, MetalWorkloadOwnership, MetalWorkloadStage,
};

#[cfg(feature = "prover")]
pub use crate::backend::cuda::{BaseFieldVec, CudaBackend, SecureFieldVec};
#[cfg(feature = "prover")]
pub use crate::backend::metal::{
    commit_line_evaluation_via_cpu_bridge, evaluate_polys_on_domain_batch,
    fold_circle_into_line_first_layer, fold_line, materialize_line_evaluation_via_cpu_bridge,
    metal_backend_surface_detail, metal_backend_surface_status, metal_runtime_error,
    metal_runtime_support, permute_coset_to_circle_domain_bit_reversed, CpuLineCommitmentBridge,
    MetalBackend, MetalBackendSurface, MetalBackendSurfaceStatus, MetalBaseFieldColumnBatch,
    MetalBaseFieldVec, MetalExtendedInnerFriProof, MetalFriBlake2sSubpath, MetalFriCommitmentSlice,
    MetalFriFirstLayer, MetalFriInnerLayerRow, MetalFriInnerLayerSequence, MetalFriInnerProofSlice,
    MetalFriLayerDecommitment, MetalFriProofSlice, MetalFriProver, MetalInnerFriProof,
    MetalInnerFriProofAux, MetalLineCommitment, MetalLineEvaluation, MetalRuntimeSupport,
    MetalSecureFieldVec, STWO_METAL_BACKEND_SURFACES_V1,
};
