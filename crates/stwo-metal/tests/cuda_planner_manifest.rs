use stwo_metal::{
    plan_exemplar_prove_by_name, CudaExecutionIntent, CudaExecutionPlan, CudaOperationKind,
    CudaPlannerError, UnknownCudaComponent,
};

#[test]
fn manifest_backed_poseidon_force_cpu_falls_back_explicitly() {
    let plan =
        plan_exemplar_prove_by_name(CudaExecutionIntent::ForceCpu, &["poseidon_example"]).unwrap();

    assert_eq!(plan, CudaExecutionPlan::CpuOnly);
}

#[test]
fn manifest_backed_poseidon_plans_cuda_full_for_require_cuda() {
    let plan = plan_exemplar_prove_by_name(CudaExecutionIntent::RequireCuda, &["poseidon_example"])
        .unwrap();

    assert_eq!(plan, CudaExecutionPlan::CudaFull);
}

#[test]
fn manifest_backed_fibonacci_plans_cuda_full_for_require_cuda() {
    let plan =
        plan_exemplar_prove_by_name(CudaExecutionIntent::RequireCuda, &["fibonacci_example"])
            .unwrap();

    assert_eq!(plan, CudaExecutionPlan::CudaFull);
}

#[test]
fn unknown_manifest_component_fails_explicitly() {
    let error = plan_exemplar_prove_by_name(CudaExecutionIntent::RequireCuda, &["missing_example"])
        .unwrap_err();

    assert_eq!(
        error,
        CudaPlannerError::UnknownComponent(UnknownCudaComponent {
            component_name: "missing_example",
            operation: CudaOperationKind::Prove,
        })
    );
}
