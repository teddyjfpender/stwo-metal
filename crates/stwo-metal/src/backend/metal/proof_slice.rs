use stwo::core::fields::qm31::SecureField;
use stwo::core::fri::{
    ExtendedFriProof, FriConfig, FriProof, FriProofAux, CIRCLE_TO_LINE_FOLD_STEP,
};
use stwo::core::poly::circle::CircleDomain;
use stwo::core::queries::Queries;
use stwo::core::vcs_lifted::merkle_hasher::MerkleHasherLifted;

use super::first_layer::MetalFriFirstLayer;
use super::proof::MetalFriInnerProofSlice;
use crate::stwo_metal::secure_field_vec::SecureFieldVec;

#[derive(Clone, Debug)]
pub struct MetalFriProofSlice<H: MerkleHasherLifted> {
    first_layer: MetalFriFirstLayer<H>,
    inner_proof_slice: MetalFriInnerProofSlice<H>,
}

impl<H: MerkleHasherLifted> MetalFriProofSlice<H> {
    pub fn from_first_circle_fold(
        src: &SecureFieldVec,
        domain: CircleDomain,
        config: FriConfig,
        first_layer_alpha: SecureField,
        inner_layer_alphas: &[SecureField],
    ) -> Self {
        Self {
            first_layer: MetalFriFirstLayer::new(domain, SecureFieldVec::from_vec(src.to_vec())),
            inner_proof_slice: MetalFriInnerProofSlice::from_first_circle_fold(
                src,
                domain,
                config,
                first_layer_alpha,
                inner_layer_alphas,
            ),
        }
    }

    pub fn first_layer(&self) -> &MetalFriFirstLayer<H> {
        &self.first_layer
    }

    pub fn inner_proof_slice(&self) -> &MetalFriInnerProofSlice<H> {
        &self.inner_proof_slice
    }

    pub fn decommit_on_queries(&self, queries: &Queries) -> ExtendedFriProof<H> {
        assert_eq!(
            queries.log_domain_size,
            self.first_layer.domain().log_size(),
            "bounded Metal FRI proof queries must target the first-layer circle domain"
        );

        let first_layer = self.first_layer.decommit(queries);
        let inner_layers = self
            .inner_proof_slice
            .decommit_on_queries(&queries.fold(CIRCLE_TO_LINE_FOLD_STEP));

        ExtendedFriProof {
            proof: FriProof {
                first_layer: first_layer.proof,
                inner_layers: inner_layers.proof.inner_layers,
                last_layer_poly: inner_layers.proof.last_layer_poly,
            },
            aux: FriProofAux {
                first_layer: first_layer.aux,
                inner_layers: inner_layers.aux.inner_layers,
            },
        }
    }
}
