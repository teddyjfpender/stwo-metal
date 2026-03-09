use stwo::core::fields::qm31::SecureField;
use stwo::core::fri::{ExtendedFriLayerProof, FriConfig};
use stwo::core::poly::circle::CircleDomain;
use stwo::core::queries::Queries;
use stwo::core::vcs_lifted::merkle_hasher::MerkleHasherLifted;

use super::line::MetalLineEvaluation;
use super::row::MetalFriInnerLayerRow;
use crate::stwo_metal::secure_field_vec::SecureFieldVec;

#[derive(Clone, Debug)]
pub struct MetalFriInnerLayerSequence<H: MerkleHasherLifted> {
    rows: Vec<MetalFriInnerLayerRow<H>>,
    last_evaluation: MetalLineEvaluation,
}

impl<H: MerkleHasherLifted> MetalFriInnerLayerSequence<H> {
    pub fn from_first_circle_fold(
        src: &SecureFieldVec,
        domain: CircleDomain,
        config: FriConfig,
        first_layer_alpha: SecureField,
        inner_layer_alphas: &[SecureField],
    ) -> Self {
        Self::from_first_evaluation(
            MetalLineEvaluation::from_first_circle_fold(src, domain, first_layer_alpha),
            config,
            inner_layer_alphas,
        )
    }

    pub fn from_first_evaluation(
        first_evaluation: MetalLineEvaluation,
        config: FriConfig,
        inner_layer_alphas: &[SecureField],
    ) -> Self {
        let last_layer_log_domain_size = config.last_layer_domain_size().ilog2();
        let mut current_evaluation = first_evaluation;
        let mut rows = Vec::new();
        let mut alpha_index = 0usize;

        if current_evaluation.domain().log_size() == last_layer_log_domain_size {
            assert!(
                inner_layer_alphas.is_empty(),
                "no inner-layer fold alphas are needed when the first line layer is already the last layer"
            );
            return Self {
                rows,
                last_evaluation: current_evaluation,
            };
        }

        while current_evaluation.domain().log_size()
            > last_layer_log_domain_size + config.line_fold_step
        {
            let row = MetalFriInnerLayerRow::new(current_evaluation, config.line_fold_step);
            let alpha = *inner_layer_alphas
                .get(alpha_index)
                .expect("missing inner-layer fold alpha for configured FRI sequence");
            alpha_index += 1;
            current_evaluation = row.evaluation().fold(alpha, config.line_fold_step);
            rows.push(row);
        }

        let last_fold_step = current_evaluation.domain().log_size() - last_layer_log_domain_size;
        let row = MetalFriInnerLayerRow::new(current_evaluation, last_fold_step);
        let alpha = *inner_layer_alphas
            .get(alpha_index)
            .expect("missing final inner-layer fold alpha for configured FRI sequence");
        alpha_index += 1;
        current_evaluation = row.evaluation().fold(alpha, last_fold_step);
        rows.push(row);

        assert_eq!(
            alpha_index,
            inner_layer_alphas.len(),
            "unused inner-layer fold alphas were provided for the configured FRI sequence"
        );

        Self {
            rows,
            last_evaluation: current_evaluation,
        }
    }

    pub fn rows(&self) -> &[MetalFriInnerLayerRow<H>] {
        &self.rows
    }

    pub fn last_evaluation(&self) -> &MetalLineEvaluation {
        &self.last_evaluation
    }

    pub fn decommit_on_queries(&self, queries: &Queries) -> Vec<ExtendedFriLayerProof<H>> {
        if let Some(first_row) = self.rows.first() {
            assert_eq!(
                queries.log_domain_size,
                first_row.evaluation().domain().log_size(),
                "FRI inner-layer sequence decommit queries must target the first row domain"
            );
        } else {
            assert_eq!(
                queries.log_domain_size,
                self.last_evaluation.domain().log_size(),
                "empty FRI inner-layer sequence queries must target the last evaluation domain"
            );
            return Vec::new();
        }

        let mut current_queries = queries.clone();
        self.rows
            .iter()
            .map(|row| {
                let proof = row.decommit(&current_queries);
                current_queries = current_queries.fold(row.fold_step());
                proof
            })
            .collect()
    }
}
