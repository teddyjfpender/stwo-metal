use std::array;

use stwo::core::fields::m31::BaseField;
use stwo::core::fields::qm31::SecureField;
use stwo::core::poly::circle::CircleDomain;
use stwo::core::poly::line::LineDomain;
use stwo::core::vcs_lifted::merkle_hasher::MerkleHasherLifted;

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

impl<H: MerkleHasherLifted> MetalLineCommitment<H> {
    pub fn root(&self) -> H::Hash {
        self.layers[0][0]
    }

    pub fn layer_count(&self) -> usize {
        self.layers.len()
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
