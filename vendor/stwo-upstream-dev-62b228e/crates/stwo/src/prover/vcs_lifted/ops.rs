use serde::{Deserialize, Serialize};

use itertools::Itertools;

use crate::core::fields::m31::BaseField;
use crate::core::vcs_lifted::merkle_hasher::MerkleHasherLifted;
use crate::prover::backend::{Col, Column, ColumnOps};

/// Trait for performing Merkle operations on a commitment scheme.
pub trait MerkleOpsLifted<H: MerkleHasherLifted>:
    ColumnOps<BaseField> + ColumnOps<H::Hash> + for<'de> Deserialize<'de> + Serialize
{
    /// Computes the leaves of the lifted Merkle commitment.
    fn build_leaves(columns: &[&Col<Self, BaseField>], lifting_log_size: u32)
        -> Col<Self, H::Hash>;

    /// Given a layer of hashes as input, computes a new layer by hashing pairs
    /// of adjacent elements of the input, as in a standard Merkle tree.
    fn build_next_layer(prev_layer: &Col<Self, H::Hash>) -> Col<Self, H::Hash>;

    /// Computes the full Merkle tree layers for a lifted commitment.
    ///
    /// Laws:
    /// - returns layers sorted by increasing depth, with the root at index `0`
    /// - preserves the same root and all intermediate layer values as repeated
    ///   `build_leaves` plus `build_next_layer`
    /// - may batch or keep intermediate representation private as long as the
    ///   returned layer values are identical
    fn build_merkle_layers(
        columns: Vec<&Col<Self, BaseField>>,
        lifting_log_size: u32,
    ) -> Vec<Col<Self, H::Hash>> {
        if columns.is_empty() {
            return vec![Self::build_leaves(&[], lifting_log_size)];
        }

        let columns = &mut columns.into_iter().sorted_by_key(|c| c.len()).collect_vec();
        let mut layers = vec![Self::build_leaves(columns, lifting_log_size)];
        for _ in 0..lifting_log_size {
            let next = Self::build_next_layer(layers.last().unwrap());
            layers.push(next);
        }
        layers.reverse();
        layers
    }
}
