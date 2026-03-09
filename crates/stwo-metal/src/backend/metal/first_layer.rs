use std::array;

use hashbrown::HashMap;
use stwo::core::fields::m31::BaseField;
use stwo::core::fields::qm31::{SecureField, QM31};
use stwo::core::fri::{ExtendedFriLayerProof, FriLayerProof, FriLayerProofAux};
use stwo::core::poly::circle::CircleDomain;
use stwo::core::queries::Queries;
use stwo::core::vcs_lifted::merkle_hasher::MerkleHasherLifted;
use stwo::core::vcs_lifted::verifier::{
    ExtendedMerkleDecommitmentLifted, MerkleDecommitmentLifted, MerkleDecommitmentLiftedAux,
};

use super::line::MetalLineEvaluation;
use crate::stwo_metal::secure_field_vec::SecureFieldVec;

#[derive(Clone, Debug)]
pub struct MetalFriFirstLayer<H: MerkleHasherLifted> {
    values: SecureFieldVec,
    domain: CircleDomain,
    layers: Vec<Vec<H::Hash>>,
}

impl<H: MerkleHasherLifted> MetalFriFirstLayer<H> {
    pub fn new(domain: CircleDomain, values: SecureFieldVec) -> Self {
        assert_eq!(
            values.len(),
            domain.size(),
            "Metal FRI first layer requires one secure-field value per circle-domain point"
        );
        let mut layer = build_first_layer_merkle_leaves::<H>(&values);
        let mut layers = vec![layer.clone()];
        while layer.len() > 1 {
            layer = layer
                .chunks_exact(2)
                .map(|chunk| H::hash_children((chunk[0], chunk[1])))
                .collect();
            layers.push(layer.clone());
        }
        layers.reverse();

        Self {
            values,
            domain,
            layers,
        }
    }

    pub fn root(&self) -> H::Hash {
        self.layers[0][0]
    }

    pub fn domain(&self) -> CircleDomain {
        self.domain
    }

    pub fn values(&self) -> &SecureFieldVec {
        &self.values
    }

    pub fn fold(&self, alpha: SecureField) -> MetalLineEvaluation {
        MetalLineEvaluation::from_first_circle_fold(&self.values, self.domain, alpha)
    }

    pub fn decommit(&self, queries: &Queries) -> ExtendedFriLayerProof<H> {
        assert_eq!(
            queries.log_domain_size,
            self.domain.log_size(),
            "FRI first-layer decommit queries must target the circle-evaluation domain"
        );

        let (decommitment_positions, fri_witness, value_map) =
            compute_first_layer_positions_and_witness(&self.values, &queries.positions);
        let decommitment = self.decommit_merkle(&decommitment_positions);

        ExtendedFriLayerProof {
            proof: FriLayerProof {
                fri_witness,
                decommitment: decommitment.decommitment,
                commitment: self.root(),
            },
            aux: FriLayerProofAux {
                all_values: vec![value_map],
                decommitment: decommitment.aux,
            },
        }
    }

    fn decommit_merkle(&self, query_positions: &[usize]) -> ExtendedMerkleDecommitmentLifted<H> {
        assert!(
            query_positions.is_sorted(),
            "Merkle decommitment requires sorted query positions"
        );
        let mut decommitment = MerkleDecommitmentLifted::default();
        let mut all_node_values = Vec::new();
        let mut prev_layer_queries = query_positions.to_vec();
        prev_layer_queries.dedup();

        for layer_log_size in (0..self.layers.len() - 1).rev() {
            let prev_layer_hashes = &self.layers[layer_log_size + 1];
            let mut curr_layer_queries = Vec::new();
            let mut all_node_values_for_layer = HashMap::new();

            for queries_chunk in prev_layer_queries.as_slice().chunk_by(|a, b| a ^ 1 == *b) {
                let first = queries_chunk[0];
                let curr_index = first >> 1;
                if queries_chunk.len() == 1 {
                    decommitment.hash_witness.push(prev_layer_hashes[first ^ 1]);
                }
                all_node_values_for_layer.insert(2 * curr_index, prev_layer_hashes[2 * curr_index]);
                all_node_values_for_layer
                    .insert(2 * curr_index + 1, prev_layer_hashes[2 * curr_index + 1]);
                curr_layer_queries.push(curr_index);
            }

            prev_layer_queries = curr_layer_queries;
            all_node_values.push(all_node_values_for_layer);
        }

        ExtendedMerkleDecommitmentLifted {
            decommitment,
            aux: MerkleDecommitmentLiftedAux { all_node_values },
        }
    }
}

fn build_first_layer_merkle_leaves<H: MerkleHasherLifted>(values: &SecureFieldVec) -> Vec<H::Hash> {
    assert!(
        values.len().is_power_of_two() && values.len() >= 2,
        "Metal FRI first layer requires a power-of-two circle evaluation of length at least two"
    );

    values
        .to_vec()
        .into_iter()
        .map(|value| {
            let coords = value.to_m31_array();
            let mut hasher = H::default();
            let base_coords: [BaseField; 4] =
                array::from_fn(|i| BaseField::from_u32_unchecked(coords[i].0));
            hasher.update_leaf(&base_coords);
            hasher.finalize()
        })
        .collect()
}

fn compute_first_layer_positions_and_witness(
    values: &SecureFieldVec,
    query_positions: &[usize],
) -> (Vec<usize>, Vec<SecureField>, HashMap<usize, QM31>) {
    assert!(
        query_positions.is_sorted(),
        "FRI first-layer decommitment requires sorted query positions"
    );
    assert!(
        query_positions
            .iter()
            .all(|position| *position < values.len()),
        "FRI first-layer decommitment query is out of bounds"
    );

    let mut decommitment_positions = Vec::new();
    let mut fri_witness = Vec::new();
    let mut value_map = HashMap::new();

    for subset_queries in query_positions.chunk_by(|a, b| a >> 1 == b >> 1) {
        let subset_start = (subset_queries[0] >> 1) << 1;
        let subset_decommitment_positions = subset_start..subset_start + 2;
        let mut subset_queries_iter = subset_queries.iter().peekable();

        for position in subset_decommitment_positions {
            decommitment_positions.push(position);

            let eval = values.get_data(position);
            value_map.insert(position, eval);

            if subset_queries_iter.next_if_eq(&&position).is_none() {
                fri_witness.push(eval);
            }
        }
    }

    (decommitment_positions, fri_witness, value_map)
}
