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
pub mod planner {
    pub use crate::backend::cuda::{
        plan_exemplar_prove_by_name, CudaComponentCapability, CudaExecutionIntent,
        CudaExecutionPlan, CudaOperationKind, CudaPlannerError, CudaSupportTier,
        UnknownCudaComponent, UnsupportedCudaPlan,
    };
}

#[cfg(feature = "prover")]
pub mod quotient {
    pub use crate::backend::cuda::{
        launch_constraint_quotients_on_domain, opaque_eval_ptr, ConstraintQuotientEvalRequest,
    };
}

#[cfg(feature = "prover")]
pub mod witness {
    pub use crate::backend::cuda::{
        generate_poseidon_interaction_traces, generate_poseidon_traces,
        generate_wide_fibonacci_trace, PoseidonInteractionTraceRequest, PoseidonTraceRequest,
        WideFibonacciTraceRequest,
    };
}

#[cfg(feature = "prover")]
pub use abi::{
    CudaPoseidonLookupElementsAbiV1, OwnedConstraintEvalAbiV1, StwoCudaLookupElements16AbiV1,
    StwoCudaPoseidonEvalAbiV1, StwoCudaWideFibonacciEvalAbiV1,
};
#[cfg(feature = "prover")]
pub use capability::{
    cuda_backend_surface_detail, cuda_backend_surface_status, CudaBackendSurface,
    CudaBackendSurfaceStatus, STWO_CUDA_BACKEND_SURFACES_V1,
};
#[cfg(feature = "prover")]
pub use planner::{
    plan_exemplar_prove_by_name, CudaComponentCapability, CudaExecutionIntent, CudaExecutionPlan,
    CudaOperationKind, CudaPlannerError, CudaSupportTier, UnknownCudaComponent,
    UnsupportedCudaPlan,
};
#[cfg(feature = "prover")]
pub use quotient::{
    launch_constraint_quotients_on_domain, opaque_eval_ptr, ConstraintQuotientEvalRequest,
};
#[cfg(feature = "prover")]
pub use witness::{
    generate_poseidon_interaction_traces, generate_poseidon_traces, generate_wide_fibonacci_trace,
    PoseidonInteractionTraceRequest, PoseidonTraceRequest, WideFibonacciTraceRequest,
};

#[cfg(feature = "prover")]
pub use crate::backend::cuda::{BaseFieldVec, CudaBackend, SecureFieldVec};
#[cfg(feature = "prover")]
pub use crate::backend::metal::{
    commit_line_evaluation_via_cpu_bridge, fold_circle_into_line_first_layer, fold_line,
    materialize_line_evaluation_via_cpu_bridge, metal_backend_surface_detail,
    metal_backend_surface_status, metal_runtime_error, metal_runtime_support,
    permute_coset_to_circle_domain_bit_reversed, CpuLineCommitmentBridge, MetalBackend,
    MetalBackendSurface, MetalBackendSurfaceStatus, MetalBaseFieldVec, MetalFriInnerLayerRow,
    MetalFriInnerLayerSequence, MetalFriLayerDecommitment, MetalLineCommitment,
    MetalLineEvaluation, MetalRuntimeSupport, MetalSecureFieldVec, STWO_METAL_BACKEND_SURFACES_V1,
};
