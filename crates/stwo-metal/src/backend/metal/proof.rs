use stwo::core::fields::qm31::SecureField;
use stwo::core::fri::{FriConfig, FriLayerProof, FriLayerProofAux};
use stwo::core::poly::circle::CircleDomain;
use stwo::core::poly::line::LinePoly;
use stwo::core::queries::Queries;
use stwo::core::vcs_lifted::merkle_hasher::MerkleHasherLifted;

use super::commitment_slice::MetalFriCommitmentSlice;
use crate::stwo_metal::secure_field_vec::SecureFieldVec;

#[derive(Clone, Debug)]
pub struct MetalInnerFriProof<H: MerkleHasherLifted> {
    pub inner_layers: Vec<FriLayerProof<H>>,
    pub last_layer_poly: LinePoly,
}

#[derive(Clone, Debug)]
pub struct MetalInnerFriProofAux<H: MerkleHasherLifted> {
    pub inner_layers: Vec<FriLayerProofAux<H>>,
}

#[derive(Clone, Debug)]
pub struct MetalExtendedInnerFriProof<H: MerkleHasherLifted> {
    pub proof: MetalInnerFriProof<H>,
    pub aux: MetalInnerFriProofAux<H>,
}

#[derive(Clone, Debug)]
pub struct MetalFriInnerProofSlice<H: MerkleHasherLifted> {
    commitment_slice: MetalFriCommitmentSlice<H>,
}

impl<H: MerkleHasherLifted> MetalFriInnerProofSlice<H> {
    pub fn from_first_circle_fold(
        src: &SecureFieldVec,
        domain: CircleDomain,
        config: FriConfig,
        first_layer_alpha: SecureField,
        inner_layer_alphas: &[SecureField],
    ) -> Self {
        Self {
            commitment_slice: MetalFriCommitmentSlice::from_first_circle_fold(
                src,
                domain,
                config,
                first_layer_alpha,
                inner_layer_alphas,
            ),
        }
    }

    pub fn commitment_slice(&self) -> &MetalFriCommitmentSlice<H> {
        &self.commitment_slice
    }

    pub fn decommit_on_queries(&self, queries: &Queries) -> MetalExtendedInnerFriProof<H> {
        let (inner_layers, inner_layers_aux): (Vec<_>, Vec<_>) = self
            .commitment_slice
            .decommit_on_queries(queries)
            .into_iter()
            .map(|layer| (layer.proof, layer.aux))
            .unzip();

        MetalExtendedInnerFriProof {
            proof: MetalInnerFriProof {
                inner_layers,
                last_layer_poly: self.commitment_slice.last_layer_poly().clone(),
            },
            aux: MetalInnerFriProofAux {
                inner_layers: inner_layers_aux,
            },
        }
    }
}
