use stwo::core::channel::{Channel, MerkleChannel};
use stwo::core::fri::{
    ExtendedFriProof, FriConfig, FriProof, FriProofAux, CIRCLE_TO_LINE_FOLD_STEP,
};
use stwo::core::poly::circle::CircleDomain;
use stwo::core::poly::line::LinePoly;
use stwo::core::queries::{draw_queries, Queries};
use stwo::prover::fri::FriDecommitResult;

use super::commitment_slice::interpolate_last_layer;
use super::first_layer::MetalFriFirstLayer;
use super::line::MetalLineEvaluation;
use super::row::MetalFriInnerLayerRow;
use crate::stwo_metal::secure_field_vec::SecureFieldVec;

#[derive(Clone, Debug)]
pub struct MetalFriProver<MC: MerkleChannel> {
    config: FriConfig,
    first_layer: MetalFriFirstLayer<MC::H>,
    inner_layers: Vec<MetalFriInnerLayerRow<MC::H>>,
    last_layer_poly: LinePoly,
}

impl<MC: MerkleChannel> MetalFriProver<MC> {
    pub fn commit(
        channel: &mut MC::C,
        config: FriConfig,
        column: &SecureFieldVec,
        domain: CircleDomain,
    ) -> Self {
        assert!(domain.is_canonic(), "not canonic");

        let first_layer = MetalFriFirstLayer::<MC::H>::new(domain, column.clone());
        MC::mix_root(channel, first_layer.root());

        let first_layer_alpha = channel.draw_secure_felt();
        let (inner_layers, last_layer_evaluation) =
            Self::commit_inner_layers(channel, config, &first_layer, first_layer_alpha);
        let last_layer_poly = Self::commit_last_layer(channel, config, &last_layer_evaluation);

        Self {
            config,
            first_layer,
            inner_layers,
            last_layer_poly,
        }
    }

    pub fn config(&self) -> FriConfig {
        self.config
    }

    pub fn first_layer(&self) -> &MetalFriFirstLayer<MC::H> {
        &self.first_layer
    }

    pub fn inner_layers(&self) -> &[MetalFriInnerLayerRow<MC::H>] {
        &self.inner_layers
    }

    pub fn last_layer_poly(&self) -> &LinePoly {
        &self.last_layer_poly
    }

    pub fn decommit(self, channel: &mut MC::C) -> FriDecommitResult<MC::H> {
        let first_layer_log_size = self.first_layer.domain().log_size();
        let unsorted_query_locations =
            draw_queries(channel, first_layer_log_size, self.config.n_queries);
        let queries = Queries::new(&unsorted_query_locations, first_layer_log_size);

        FriDecommitResult {
            fri_proof: self.decommit_on_queries(&queries),
            query_positions: queries.positions.clone(),
            unsorted_query_locations,
        }
    }

    pub fn decommit_on_queries(self, queries: &Queries) -> ExtendedFriProof<MC::H> {
        assert_eq!(
            queries.log_domain_size,
            self.first_layer.domain().log_size(),
            "bounded Metal FRI proof queries must target the first-layer circle domain"
        );

        let first_layer_proof = self.first_layer.decommit(queries);
        let inner_layer_proofs: Vec<_> = self
            .inner_layers
            .into_iter()
            .scan(
                queries.fold(CIRCLE_TO_LINE_FOLD_STEP),
                |layer_queries, layer| {
                    let proof = layer.decommit(layer_queries);
                    *layer_queries = layer_queries.fold(layer.fold_step());
                    Some(proof)
                },
            )
            .collect();
        let (inner_layers, inner_layers_aux): (Vec<_>, Vec<_>) = inner_layer_proofs
            .into_iter()
            .map(|proof| (proof.proof, proof.aux))
            .unzip();

        ExtendedFriProof {
            proof: FriProof {
                first_layer: first_layer_proof.proof,
                inner_layers,
                last_layer_poly: self.last_layer_poly,
            },
            aux: FriProofAux {
                first_layer: first_layer_proof.aux,
                inner_layers: inner_layers_aux,
            },
        }
    }

    fn commit_inner_layers(
        channel: &mut MC::C,
        config: FriConfig,
        first_layer: &MetalFriFirstLayer<MC::H>,
        first_layer_alpha: stwo::core::fields::qm31::SecureField,
    ) -> (Vec<MetalFriInnerLayerRow<MC::H>>, MetalLineEvaluation) {
        let mut layer_evaluation = first_layer.fold(first_layer_alpha);
        let mut layers = Vec::new();
        let last_layer_log_domain_size = config.last_layer_domain_size().ilog2();

        if layer_evaluation.domain().log_size() == last_layer_log_domain_size {
            return (layers, layer_evaluation);
        }

        while layer_evaluation.domain().log_size()
            > last_layer_log_domain_size + config.line_fold_step
        {
            let layer = MetalFriInnerLayerRow::new(layer_evaluation, config.line_fold_step);
            MC::mix_root(channel, layer.root());
            let folding_alpha = channel.draw_secure_felt();
            layer_evaluation = layer
                .evaluation()
                .fold(folding_alpha, config.line_fold_step);
            layers.push(layer);
        }

        let last_fold_step = layer_evaluation.domain().log_size() - last_layer_log_domain_size;
        let layer = MetalFriInnerLayerRow::new(layer_evaluation, last_fold_step);
        MC::mix_root(channel, layer.root());
        let folding_alpha = channel.draw_secure_felt();
        layer_evaluation = layer.evaluation().fold(folding_alpha, last_fold_step);
        layers.push(layer);

        (layers, layer_evaluation)
    }

    fn commit_last_layer(
        channel: &mut MC::C,
        config: FriConfig,
        evaluation: &MetalLineEvaluation,
    ) -> LinePoly {
        let last_layer_poly = interpolate_last_layer(config, evaluation);
        channel.mix_felts(&last_layer_poly);
        last_layer_poly
    }
}
