use std::array;

use hashbrown::HashMap;
use stwo::core::fields::m31::BaseField;
use stwo::core::fields::qm31::{SecureField, QM31};
use stwo::core::fri::{ExtendedFriLayerProof, FriLayerProof, FriLayerProofAux};
use stwo::core::poly::circle::CircleDomain;
use stwo::core::poly::line::LineDomain;
use stwo::core::queries::Queries;
use stwo::core::vcs_lifted::merkle_hasher::MerkleHasherLifted;
use stwo::core::vcs_lifted::verifier::{
    ExtendedMerkleDecommitmentLifted, MerkleDecommitmentLifted, MerkleDecommitmentLiftedAux,
};

use super::fri;
use crate::stwo_metal::secure_field_vec::SecureFieldVec;

#[derive(Clone, Debug)]
pub struct MetalLineEvaluation {
    values: SecureFieldVec,
    domain: LineDomain,
}

impl MetalLineEvaluation {
    pub fn new(domain: LineDomain, values: SecureFieldVec) -> Self {
        assert_eq!(
            values.len(),
            domain.size(),
            "Metal line evaluation requires one secure-field value per line-domain point"
        );
        Self { values, domain }
    }

    pub fn from_first_circle_fold(
        src: &SecureFieldVec,
        domain: CircleDomain,
        alpha: SecureField,
    ) -> Self {
        Self::new(
            LineDomain::new(domain.half_coset),
            fri::fold_circle_into_line_first_layer(src, domain, alpha),
        )
    }

    pub fn fold(&self, alpha: SecureField, fold_step: u32) -> Self {
        Self::new(
            self.domain.repeated_double(fold_step),
            fri::fold_line(&self.values, self.domain, alpha, fold_step),
        )
    }

    pub fn domain(&self) -> LineDomain {
        self.domain
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn values(&self) -> &SecureFieldVec {
        &self.values
    }

    pub fn commitment<H: MerkleHasherLifted>(&self) -> MetalLineCommitment<H> {
        MetalLineCommitment::from_line_evaluation(self)
    }
}

#[derive(Clone, Debug)]
pub struct MetalLineCommitment<H: MerkleHasherLifted> {
    layers: Vec<Vec<H::Hash>>,
}

#[derive(Clone, Debug)]
pub struct MetalFriLayerDecommitment<H: MerkleHasherLifted> {
    pub decommitment_positions: Vec<usize>,
    pub proof: ExtendedFriLayerProof<H>,
}

impl<H: MerkleHasherLifted> MetalLineCommitment<H> {
    pub fn root(&self) -> H::Hash {
        self.layers[0][0]
    }

    pub fn layer_count(&self) -> usize {
        self.layers.len()
    }

    pub fn decommit_fri_layer(
        &self,
        evaluation: &MetalLineEvaluation,
        queries: &Queries,
        fold_step: u32,
    ) -> MetalFriLayerDecommitment<H> {
        assert_eq!(
            queries.log_domain_size,
            evaluation.domain.log_size(),
            "FRI layer decommit queries must target the line-evaluation domain"
        );
        let (decommitment_positions, fri_witness, value_map) =
            compute_decommitment_positions_and_witness_evals(
                evaluation,
                &queries.positions,
                fold_step,
            );
        let decommitment = self.decommit_merkle(&decommitment_positions);

        MetalFriLayerDecommitment {
            decommitment_positions,
            proof: ExtendedFriLayerProof {
                proof: FriLayerProof {
                    fri_witness,
                    decommitment: decommitment.decommitment,
                    commitment: self.root(),
                },
                aux: FriLayerProofAux {
                    all_values: vec![value_map],
                    decommitment: decommitment.aux,
                },
            },
        }
    }

    fn from_line_evaluation(evaluation: &MetalLineEvaluation) -> Self {
        let mut layer = build_line_merkle_leaves::<H>(&evaluation.values);
        let mut layers = vec![layer.clone()];
        while layer.len() > 1 {
            layer = layer
                .chunks_exact(2)
                .map(|chunk| H::hash_children((chunk[0], chunk[1])))
                .collect();
            layers.push(layer.clone());
        }
        layers.reverse();
        Self { layers }
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

fn build_line_merkle_leaves<H: MerkleHasherLifted>(values: &SecureFieldVec) -> Vec<H::Hash> {
    assert!(
        values.len().is_power_of_two() && values.len() >= 2,
        "Metal line commitment requires a power-of-two line evaluation of length at least two"
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

fn compute_decommitment_positions_and_witness_evals(
    evaluation: &MetalLineEvaluation,
    query_positions: &[usize],
    fold_step: u32,
) -> (Vec<usize>, Vec<SecureField>, HashMap<usize, QM31>) {
    assert!(
        query_positions.is_sorted(),
        "FRI layer decommitment requires sorted query positions"
    );
    assert!(
        query_positions
            .iter()
            .all(|position| *position < evaluation.len()),
        "FRI layer decommitment query is out of bounds"
    );

    let mut decommitment_positions = Vec::new();
    let mut fri_witness = Vec::new();
    let mut value_map = HashMap::new();

    for subset_queries in query_positions.chunk_by(|a, b| a >> fold_step == b >> fold_step) {
        let subset_start = (subset_queries[0] >> fold_step) << fold_step;
        let subset_decommitment_positions = subset_start..subset_start + (1 << fold_step);
        let mut subset_queries_iter = subset_queries.iter().peekable();

        for position in subset_decommitment_positions {
            decommitment_positions.push(position);

            let eval = evaluation.values.get_data(position);
            value_map.insert(position, eval);

            if subset_queries_iter.next_if_eq(&&position).is_none() {
                fri_witness.push(eval);
            }
        }
    }

    (decommitment_positions, fri_witness, value_map)
}
