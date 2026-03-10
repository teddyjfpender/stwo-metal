mod artifact;
mod accumulation;
mod backend;
mod benchmark;
mod blake2s;
mod capability;
mod execution_plan;
mod column;
mod commitment_slice;
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
mod prover;
mod quotient;
mod row;
mod sequence;
mod subpath;
mod witness;
mod workload;
mod workload_contract;

pub use backend::MetalBackend;
pub use benchmark::{
    declare_wide_fibonacci_benchmark_boundary, MetalBenchmarkInputError, MetalBenchmarkOperation,
    MetalBenchmarkReferencePlatform, MetalBenchmarkTarget, MetalWideFibonacciBenchmarkBoundary,
    MetalWideFibonacciWitnessInputs, WIDE_FIBONACCI_PROVE_LOG20_TARGET,
    WIDE_FIBONACCI_TRACE_LOG20_TARGET,
};
pub use capability::{
    metal_backend_surface_detail, metal_backend_surface_status, metal_runtime_error,
    metal_runtime_support, MetalBackendSurface, MetalBackendSurfaceStatus, MetalRuntimeSupport,
    STWO_METAL_BACKEND_SURFACES_V1,
};
pub use commitment_slice::MetalFriCommitmentSlice;
pub use first_layer::MetalFriFirstLayer;
pub use fri::{fold_circle_into_line_first_layer, fold_line};
pub use handoff::{
    commit_line_evaluation_via_cpu_bridge, materialize_line_evaluation_via_cpu_bridge,
    CpuLineCommitmentBridge,
};
pub use line::{MetalFriLayerDecommitment, MetalLineCommitment, MetalLineEvaluation};
pub use planner::{
    plan_exemplar_metal_prove_by_name, plan_metal_operation, MetalComponentCapability,
    MetalComponentPlanInput, MetalExecutionIntent, MetalExecutionPlan, MetalOperationKind,
    MetalPlannerError, MetalSupportTier, UnknownMetalComponent,
    UnsupportedGeneratedMetalRoute, UnsupportedMetalPlan,
};
pub use planner_manifest_v1_generated::STWO_METAL_PLANNER_COMPONENTS_V1;
pub use poly::permute_coset_to_circle_domain_bit_reversed;
pub use proof::{
    MetalExtendedInnerFriProof, MetalFriInnerProofSlice, MetalInnerFriProof, MetalInnerFriProofAux,
};
pub use proof_slice::MetalFriProofSlice;
pub use prover::MetalFriProver;
pub use quotient::{
    accumulate_wide_fibonacci_quotients, MetalWideFibonacciQuotientError,
    MetalWideFibonacciQuotientRequest, MetalWideFibonacciQuotients,
};
pub use row::MetalFriInnerLayerRow;
pub use sequence::MetalFriInnerLayerSequence;
pub use subpath::MetalFriBlake2sSubpath;
pub use witness::{
    generate_metal_wide_fibonacci_trace, MetalWideFibonacciTrace, MetalWideFibonacciTraceError,
    MetalWideFibonacciTraceRequest,
};
pub use workload::{
    declare_exemplar_hybrid_fri_workload, declare_exemplar_metal_workload_boundary,
    MetalCpuQuotientEvaluationInput, MetalCpuWideFibonacciWitnessInput,
    MetalFriReadyEvaluationInput, MetalHybridFriWorkload, MetalWorkloadBoundary,
    MetalWorkloadHandoffError,
};
pub use workload_contract::{
    MetalWorkloadOwnership, MetalWorkloadStage, MetalWorkloadStageAssignment,
};

pub use crate::stwo_metal::{
    BaseFieldVec as MetalBaseFieldVec, SecureFieldVec as MetalSecureFieldVec,
};
