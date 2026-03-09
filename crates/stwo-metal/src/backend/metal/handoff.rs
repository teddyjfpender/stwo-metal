use stwo::core::poly::line::LineDomain;
use stwo::core::vcs_lifted::merkle_hasher::MerkleHasherLifted;
use stwo::prover::backend::CpuBackend;
use stwo::prover::line::LineEvaluation;
use stwo::prover::vcs_lifted::prover::MerkleProverLifted;

use crate::stwo_metal::secure_field_vec::SecureFieldVec;

#[derive(Debug)]
pub struct CpuLineCommitmentBridge<H: MerkleHasherLifted> {
    pub evaluation: LineEvaluation<CpuBackend>,
    pub merkle_tree: MerkleProverLifted<CpuBackend, H>,
}

pub fn materialize_line_evaluation_via_cpu_bridge(
    domain: LineDomain,
    values: &SecureFieldVec,
) -> LineEvaluation<CpuBackend> {
    assert_eq!(
        values.len(),
        domain.size(),
        "line-evaluation handoff requires one secure-field value per line-domain point"
    );
    LineEvaluation::new(domain, values.to_vec().into_iter().collect())
}

pub fn commit_line_evaluation_via_cpu_bridge<H: MerkleHasherLifted>(
    domain: LineDomain,
    values: &SecureFieldVec,
) -> CpuLineCommitmentBridge<H> {
    let evaluation = materialize_line_evaluation_via_cpu_bridge(domain, values);
    let merkle_tree = MerkleProverLifted::<CpuBackend, H>::commit(
        evaluation.values.columns.iter().collect(),
        evaluation.values.len().ilog2(),
    );
    CpuLineCommitmentBridge {
        evaluation,
        merkle_tree,
    }
}
