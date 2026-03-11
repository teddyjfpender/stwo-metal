use stwo::core::poly::line::LineDomain;
use stwo::core::vcs_lifted::merkle_hasher::MerkleHasherLifted;
use stwo::prover::backend::CpuBackend;
use stwo::prover::line::LineEvaluation;
use stwo::prover::vcs_lifted::prover::MerkleProverLifted;

use super::line::{MetalLineCommitment, MetalLineEvaluation};
use crate::stwo_metal::secure_field_vec::SecureFieldVec;

#[derive(Debug)]
pub struct CpuLineCommitmentBridge<H: MerkleHasherLifted> {
    pub evaluation: LineEvaluation<CpuBackend>,
    pub merkle_tree: MerkleProverLifted<CpuBackend, H>,
}

fn materialize_line_evaluation(domain: LineDomain, values: &SecureFieldVec) -> MetalLineEvaluation {
    assert_eq!(
        values.len(),
        domain.size(),
        "line-evaluation handoff requires one secure-field value per line-domain point"
    );
    MetalLineEvaluation::new(domain, values.clone())
}

fn commit_line_evaluation<H: MerkleHasherLifted>(
    domain: LineDomain,
    values: &SecureFieldVec,
) -> MetalLineCommitment<H> {
    materialize_line_evaluation(domain, values).commitment()
}

pub fn materialize_line_evaluation_via_cpu_bridge(
    domain: LineDomain,
    values: &SecureFieldVec,
) -> LineEvaluation<CpuBackend> {
    let evaluation = materialize_line_evaluation(domain, values);
    LineEvaluation::new(domain, evaluation.values().to_vec().into_iter().collect())
}

pub fn commit_line_evaluation_via_cpu_bridge<H: MerkleHasherLifted>(
    domain: LineDomain,
    values: &SecureFieldVec,
) -> CpuLineCommitmentBridge<H> {
    let native_commitment = commit_line_evaluation::<H>(domain, values);
    let evaluation = materialize_line_evaluation_via_cpu_bridge(domain, values);
    let merkle_tree = MerkleProverLifted::<CpuBackend, H>::commit(
        evaluation.values.columns.iter().collect(),
        evaluation.values.len().ilog2(),
    );
    debug_assert_eq!(
        native_commitment.root(),
        merkle_tree.root(),
        "CPU compatibility bridge must preserve the native Metal line-commitment root"
    );
    CpuLineCommitmentBridge {
        evaluation,
        merkle_tree,
    }
}
