use stwo::core::fields::m31::BaseField;
use stwo::core::vcs_lifted::blake2_merkle::Blake2sMerkleChannel;
use stwo::prover::backend::{Backend, BackendForChannel, Col, ColumnOps};
use stwo_metal::{
    plan_exemplar_prove_by_name, BaseFieldVec, CudaBackend, CudaExecutionIntent,
    CudaExecutionPlan,
};

fn assert_backend<B: Backend>() {}

fn assert_backend_for_channel<B: BackendForChannel<Blake2sMerkleChannel>>() {}

fn assert_base_column_ops<B: ColumnOps<BaseField>>() {}

#[test]
fn pinned_stwo_accepts_cuda_backend_traits() {
    assert_backend::<CudaBackend>();
    assert_backend_for_channel::<CudaBackend>();
    assert_base_column_ops::<CudaBackend>();
    let _ = core::mem::size_of::<Col<CudaBackend, BaseField>>();
}

#[test]
fn pinned_stwo_pairing_exposes_companion_surface() {
    let plan =
        plan_exemplar_prove_by_name(CudaExecutionIntent::RequireCuda, &["poseidon_example"])
            .unwrap();

    assert_eq!(plan, CudaExecutionPlan::CudaFull);
    let _ = core::mem::size_of::<BaseFieldVec>();
}
