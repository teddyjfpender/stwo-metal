use std::collections::HashSet;

use stwo::core::vcs_lifted::blake2_merkle::Blake2sMerkleHasher;
use stwo_metal::{
    cuda_backend_surface_status, declare_exemplar_hybrid_fri_workload,
    declare_exemplar_metal_workload_boundary, plan_exemplar_metal_prove_by_name,
    plan_exemplar_prove_by_name, BaseFieldVec, CudaBackend, CudaBackendSurface,
    CudaBackendSurfaceStatus, CudaExecutionIntent, CudaExecutionPlan, MetalBackend,
    MetalBaseFieldVec, MetalExecutionIntent, MetalExecutionPlan, MetalFriBlake2sSubpath,
    MetalFriFirstLayer, MetalFriInnerLayerRow, MetalFriInnerProofSlice, MetalFriLayerDecommitment,
    MetalFriProofSlice, MetalFriProver, MetalHybridFriWorkload, MetalLineCommitment,
    MetalLineEvaluation, MetalSecureFieldVec, MetalWorkloadBoundary, MetalWorkloadOwnership,
    MetalWorkloadStage, OwnedConstraintEvalAbiV1, SecureFieldVec, StwoCudaWideFibonacciEvalAbiV1,
    STWO_CUDA_BACKEND_SURFACES_V1,
};

#[test]
fn companion_surface_exports_backend_core_types() {
    let _ = std::mem::size_of::<CudaBackend>();
    let _ = std::mem::size_of::<BaseFieldVec>();
    let _ = std::mem::size_of::<MetalBackend>();
    let _ = std::mem::size_of::<MetalBaseFieldVec>();
    let _ = std::mem::size_of::<MetalFriFirstLayer<Blake2sMerkleHasher>>();
    let _ = std::mem::size_of::<MetalFriInnerLayerRow<Blake2sMerkleHasher>>();
    let _ = std::mem::size_of::<MetalFriInnerProofSlice<Blake2sMerkleHasher>>();
    let _ = std::mem::size_of::<MetalFriProofSlice<Blake2sMerkleHasher>>();
    let _ = std::mem::size_of::<
        MetalFriProver<stwo::core::vcs_lifted::blake2_merkle::Blake2sMerkleChannel>,
    >();
    let _ = std::mem::size_of::<MetalFriBlake2sSubpath>();
    let _ = std::mem::size_of::<MetalFriLayerDecommitment<Blake2sMerkleHasher>>();
    let _ = std::mem::size_of::<MetalWorkloadBoundary>();
    let _ = std::mem::size_of::<MetalHybridFriWorkload>();
    let _ = std::mem::size_of::<MetalLineEvaluation>();
    let _ = std::mem::size_of::<MetalLineCommitment<Blake2sMerkleHasher>>();
    let _ = std::mem::size_of::<MetalSecureFieldVec>();
    let _ = std::mem::size_of::<SecureFieldVec>();
}

#[test]
fn companion_surface_exports_planner_api() {
    let plan = plan_exemplar_prove_by_name(CudaExecutionIntent::RequireCuda, &["poseidon_example"])
        .unwrap();
    let metal_plan = plan_exemplar_metal_prove_by_name(
        MetalExecutionIntent::PreferMetal,
        &["fibonacci_example"],
    )
    .unwrap();

    assert_eq!(plan, CudaExecutionPlan::CudaFull);
    assert_eq!(metal_plan, MetalExecutionPlan::MetalFriHybrid);
}

#[test]
fn companion_surface_exports_capability_diagnostics() {
    let surfaces: HashSet<_> = STWO_CUDA_BACKEND_SURFACES_V1.iter().copied().collect();

    assert_eq!(surfaces.len(), STWO_CUDA_BACKEND_SURFACES_V1.len());
    assert_eq!(
        cuda_backend_surface_status(CudaBackendSurface::GkrSumAsPolyInFirstVariable),
        CudaBackendSurfaceStatus::Supported
    );
}

#[test]
fn companion_surface_exports_workload_boundary_api() {
    let boundary = declare_exemplar_metal_workload_boundary(
        MetalExecutionIntent::PreferMetal,
        "fibonacci_example",
    )
    .unwrap();
    let workload = declare_exemplar_hybrid_fri_workload(
        MetalExecutionIntent::PreferMetal,
        "fibonacci_example",
        stwo::core::fri::FriConfig::new(3, 2, 3, 2),
    )
    .unwrap();

    assert_eq!(boundary.plan(), MetalExecutionPlan::MetalFriHybrid);
    assert_eq!(
        workload.boundary().plan(),
        MetalExecutionPlan::MetalFriHybrid
    );
    assert_eq!(
        boundary.stage_ownership(MetalWorkloadStage::FriBlake2s),
        Some(MetalWorkloadOwnership::MetalNative)
    );
}

#[test]
fn companion_surface_exports_constraint_eval_abi_values() {
    let abi = OwnedConstraintEvalAbiV1::WideFibonacci(StwoCudaWideFibonacciEvalAbiV1::new(7, 8));

    assert_eq!(abi.as_ptr().is_null(), false);
}
