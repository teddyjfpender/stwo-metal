use stwo::core::fields::qm31::SecureField;
use stwo::core::fri::ExtendedFriLayerProof;
use stwo::core::poly::circle::CircleDomain;
use stwo::core::queries::Queries;
use stwo::core::vcs_lifted::merkle_hasher::MerkleHasherLifted;

use super::line::{MetalFriLayerDecommitment, MetalLineCommitment, MetalLineEvaluation};
use crate::stwo_metal::secure_field_vec::SecureFieldVec;

#[derive(Clone, Debug)]
pub struct MetalFriInnerLayerRow<H: MerkleHasherLifted> {
    evaluation: MetalLineEvaluation,
    commitment: MetalLineCommitment<H>,
    fold_step: u32,
}

impl<H: MerkleHasherLifted> MetalFriInnerLayerRow<H> {
    pub fn new(evaluation: MetalLineEvaluation, fold_step: u32) -> Self {
        assert!(
            fold_step >= 1,
            "FRI inner-layer row requires a positive fold_step"
        );
        assert!(
            evaluation.domain().log_size() >= fold_step,
            "FRI inner-layer row fold_step cannot exceed the line-domain log size"
        );
        let commitment = evaluation.commitment::<H>();
        Self {
            evaluation,
            commitment,
            fold_step,
        }
    }

    pub fn from_first_circle_fold(
        src: &SecureFieldVec,
        domain: CircleDomain,
        alpha: SecureField,
        fold_step: u32,
    ) -> Self {
        Self::new(
            MetalLineEvaluation::from_first_circle_fold(src, domain, alpha),
            fold_step,
        )
    }

    pub fn root(&self) -> H::Hash {
        self.commitment.root()
    }

    pub fn fold_step(&self) -> u32 {
        self.fold_step
    }

    pub fn evaluation(&self) -> &MetalLineEvaluation {
        &self.evaluation
    }

    pub fn commitment(&self) -> &MetalLineCommitment<H> {
        &self.commitment
    }

    pub fn decommit(&self, queries: &Queries) -> ExtendedFriLayerProof<H> {
        self.commitment
            .decommit_fri_layer(&self.evaluation, queries, self.fold_step)
            .proof
    }

    pub fn fold(&self, alpha: SecureField, next_fold_step: u32) -> Self {
        Self::new(self.evaluation.fold(alpha, self.fold_step), next_fold_step)
    }

    pub fn decommit_with_positions(&self, queries: &Queries) -> MetalFriLayerDecommitment<H> {
        self.commitment
            .decommit_fri_layer(&self.evaluation, queries, self.fold_step)
    }
}
