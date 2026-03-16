//! GPU-accelerated Merkle tree decommitment.
//!
//! Replaces the generic `MerkleProverLifted::decommit` path with a version that
//! pre-computes the full traversal plan on CPU (cheap integer arithmetic on
//! ~32-64 queries), dispatches a **single** GPU bulk gather across all Merkle
//! tree layers in one Metal command buffer, then assembles the result on CPU.
//!
//! This eliminates per-layer GPU readback round-trips that dominate the
//! `prove_values` decommit phase at large tree sizes.

use hashbrown::HashMap;
use itertools::Itertools;
use stwo::core::fields::m31::BaseField;
use stwo::core::vcs::blake2_hash::Blake2sHash;
use stwo::core::vcs_lifted::merkle_hasher::MerkleHasherLifted;
use stwo::core::vcs_lifted::verifier::{
    ExtendedMerkleDecommitmentLifted, MerkleDecommitmentLifted, MerkleDecommitmentLiftedAux,
};
use stwo::core::ColumnVec;

use super::MetalBaseFieldVec;
use crate::stwo_metal::blake2s_hash_vec::Blake2sHashVec as MetalBlake2sHashVec;

/// Describes one hash gather operation within a single layer.
#[derive(Clone, Debug)]
struct LayerGatherPlan {
    /// All node indices to read from this layer.  Layout:
    ///   For each chunk of `prev_layer_queries.chunk_by(|a, b| a ^ 1 == *b)`:
    ///     - if needs_sibling: [sibling_index, left_child, right_child]
    ///     - else:             [left_child, right_child]
    gather_indices: Vec<u32>,
    /// For each chunk: `(needs_sibling, parent_index)`.
    chunk_plan: Vec<(bool, usize)>,
}

/// Pre-compute the full decommitment traversal plan across all layers.
///
/// Returns `(per_layer_plans, propagated_queries_per_layer)` where the plans
/// are ordered from the second-to-bottom layer upward (matching the traversal
/// order in `MerkleProverLifted::decommit`).
fn compute_decommit_plan(
    query_positions: &[usize],
    n_layers: usize,
) -> Vec<LayerGatherPlan> {
    let mut prev_layer_queries = query_positions.to_vec();
    prev_layer_queries.dedup();

    let mut plans = Vec::with_capacity(n_layers.saturating_sub(1));

    // layers[0] = root (1 node), layers[n_layers-1] = leaves.
    // We iterate from layer_log_size = n_layers-2 down to 0.
    for _layer_log_size in (0..n_layers.saturating_sub(1)).rev() {
        let mut gather_indices = Vec::new();
        let mut chunk_plan = Vec::new();
        let mut curr_layer_queries = Vec::new();

        for queries_chunk in prev_layer_queries.as_slice().chunk_by(|a, b| a ^ 1 == *b) {
            let first = queries_chunk[0];
            let needs_sibling = queries_chunk.len() == 1;
            let curr_index = first >> 1;

            if needs_sibling {
                gather_indices.push((first ^ 1) as u32);
            }
            gather_indices.push((2 * curr_index) as u32);
            gather_indices.push((2 * curr_index + 1) as u32);

            chunk_plan.push((needs_sibling, curr_index));
            curr_layer_queries.push(curr_index);
        }

        plans.push(LayerGatherPlan {
            gather_indices,
            chunk_plan,
        });
        prev_layer_queries = curr_layer_queries;
    }

    plans
}

/// Decode 8 consecutive u32 words into a `Blake2sHash`.
fn decode_hash_words(words: &[u32]) -> Blake2sHash {
    debug_assert!(words.len() >= 8);
    let mut bytes = [0u8; 32];
    for (i, &word) in words[..8].iter().enumerate() {
        bytes[i * 4..(i + 1) * 4].copy_from_slice(&word.to_le_bytes());
    }
    Blake2sHash(bytes)
}

/// GPU-accelerated Merkle decommitment.
///
/// Mirrors the semantics of `MerkleProverLifted::decommit` but uses a single
/// GPU bulk gather instead of per-layer `batch_at` calls.
pub fn gpu_decommit<H: MerkleHasherLifted<Hash = Blake2sHash>>(
    layers: &[MetalBlake2sHashVec],
    query_positions: &[usize],
    columns: Vec<&MetalBaseFieldVec>,
) -> (
    ColumnVec<Vec<BaseField>>,
    ExtendedMerkleDecommitmentLifted<H>,
) {
    let n_layers = layers.len();

    // --- Phase 1: Queried column values (same logic as upstream) ---
    let max_log_size = n_layers.saturating_sub(1);
    let mut queried_values: ColumnVec<Vec<BaseField>> = vec![];
    let mut query_read_indices_by_shift = HashMap::<usize, Vec<usize>>::new();
    for col in columns.iter() {
        let log_size = col.len().ilog2() as usize;
        let shift = max_log_size - log_size;
        let read_indices = query_read_indices_by_shift
            .entry(shift)
            .or_insert_with(|| {
                query_positions
                    .iter()
                    .map(|pos| (pos >> (shift + 1) << 1) + (pos & 1))
                    .collect_vec()
            });
        let res = col.batch_get(read_indices);
        queried_values.push(res);
    }

    // --- Phase 2: Pre-compute the full traversal plan on CPU ---
    let plans = compute_decommit_plan(query_positions, n_layers);

    // --- Phase 3: Single GPU bulk gather across all layers ---
    // Build per-layer gather index arrays.  `plans[i]` corresponds to
    // `layers[layer_log_size + 1]` where layer_log_size goes from
    // n_layers-2 down to 0.  So plans[0] reads from layers[n_layers-1]
    // (the leaves), plans[1] from layers[n_layers-2], etc.
    //
    // We organize the gather call by *layer buffer* so the indices for
    // layers[k] are collected in order.
    let mut per_layer_indices: Vec<Vec<u32>> = vec![Vec::new(); n_layers];
    for (plan_idx, plan) in plans.iter().enumerate() {
        // plan_idx=0 corresponds to layer_log_size=(n_layers-2), reading
        // from layers[n_layers-1].  Generally, layer buffer index =
        // (n_layers - 1) - plan_idx.
        let layer_buf_idx = (n_layers - 1) - plan_idx;
        per_layer_indices[layer_buf_idx] = plan.gather_indices.clone();
    }

    let total_gathers: usize = per_layer_indices.iter().map(|v| v.len()).sum();

    // Attempt GPU gather.  Fall back to CPU if GPU is unavailable or fails.
    let gathered_words = if total_gathers > 0 {
        let layer_refs: Vec<&stwo_metal_sys::metal::U32Buffer> =
            layers.iter().map(|l| &l.buffer).collect();
        let index_slices: Vec<&[u32]> = per_layer_indices.iter().map(|v| v.as_slice()).collect();

        match stwo_metal_sys::metal::U32Buffer::merkle_decommit_gather(
            &layer_refs,
            &index_slices,
        ) {
            Ok(words) => words,
            Err(_) => {
                // Fallback: gather via CPU host pointer reads.
                cpu_fallback_gather(layers, &per_layer_indices)
            }
        }
    } else {
        Vec::new()
    };

    // --- Phase 4: Assemble hash_witness and all_node_values on CPU ---
    let mut decommitment = MerkleDecommitmentLifted::<H>::default();
    let mut all_node_values: Vec<HashMap<usize, H::Hash>> = Vec::with_capacity(plans.len());

    // Build an offset map: for each layer buffer, the word offset into
    // `gathered_words` where its gathered hashes start.
    let mut layer_word_offsets = vec![0usize; n_layers];
    {
        let mut running_offset = 0usize;
        for k in 0..n_layers {
            layer_word_offsets[k] = running_offset;
            running_offset += per_layer_indices[k].len() * 8;
        }
    }

    for (plan_idx, plan) in plans.iter().enumerate() {
        let layer_buf_idx = (n_layers - 1) - plan_idx;
        let base_word_offset = layer_word_offsets[layer_buf_idx];

        let mut all_node_values_for_layer = HashMap::<usize, H::Hash>::new();
        let mut read_cursor = 0usize;

        for &(needs_sibling, curr_index) in &plan.chunk_plan {
            if needs_sibling {
                let word_start = base_word_offset + read_cursor * 8;
                decommitment
                    .hash_witness
                    .push(decode_hash_words(&gathered_words[word_start..word_start + 8]));
                read_cursor += 1;
            }
            // Left child.
            let left_start = base_word_offset + read_cursor * 8;
            all_node_values_for_layer
                .insert(2 * curr_index, decode_hash_words(&gathered_words[left_start..left_start + 8]));
            read_cursor += 1;
            // Right child.
            let right_start = base_word_offset + read_cursor * 8;
            all_node_values_for_layer
                .insert(2 * curr_index + 1, decode_hash_words(&gathered_words[right_start..right_start + 8]));
            read_cursor += 1;
        }

        all_node_values.push(all_node_values_for_layer);
    }

    (
        queried_values,
        ExtendedMerkleDecommitmentLifted {
            decommitment,
            aux: MerkleDecommitmentLiftedAux { all_node_values },
        },
    )
}

/// CPU fallback: gather hash words via shared-memory host pointers.
fn cpu_fallback_gather(
    layers: &[MetalBlake2sHashVec],
    per_layer_indices: &[Vec<u32>],
) -> Vec<u32> {
    let total_words: usize = per_layer_indices.iter().map(|v| v.len() * 8).sum();
    let mut result = Vec::with_capacity(total_words);
    for (layer, indices) in layers.iter().zip(per_layer_indices.iter()) {
        if indices.is_empty() {
            continue;
        }
        // Use batch_get which reads from host cache or shared memory.
        let hashes = layer.batch_get(
            &indices.iter().map(|&i| i as usize).collect::<Vec<_>>(),
        );
        for hash in &hashes {
            for chunk in hash.0.chunks_exact(4) {
                result.push(u32::from_le_bytes(
                    chunk.try_into().expect("Blake2s hash word is 4 bytes"),
                ));
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use stwo::core::fields::m31::M31;
    use stwo::core::vcs_lifted::blake2_merkle::Blake2sMerkleHasher;
    use stwo::prover::vcs_lifted::ops::MerkleOpsLifted;

    use super::*;
    use crate::backend::metal::MetalBackend;

    type MetalBaseFieldVec = crate::stwo_metal::base_field_vec::BaseFieldVec;

    /// Build a small Merkle tree on GPU and verify the GPU decommit matches
    /// the upstream CPU decommit.
    #[test]
    fn gpu_decommit_matches_upstream() {
        let log_size: u32 = 8;
        let n = 1u32 << log_size;

        // Create columns with deterministic data.
        let col_data: Vec<BaseField> = (0..n).map(M31::from_u32_unchecked).collect();
        let col = MetalBaseFieldVec::from_iter(col_data.clone());
        let columns = vec![&col];

        // Build Merkle tree layers on GPU.
        let layers: Vec<MetalBlake2sHashVec> =
            <MetalBackend as MerkleOpsLifted<Blake2sMerkleHasher>>::build_merkle_layers(
                columns.iter().map(|c| *c).collect(),
                log_size,
            );

        // Query positions (sorted, deduplicated).
        let query_positions: Vec<usize> = vec![0, 3, 7, 15, 42, 100, 200, 255];

        // GPU decommit.
        let (gpu_values, gpu_decommit_result) = gpu_decommit::<Blake2sMerkleHasher>(
            &layers,
            &query_positions,
            vec![&col],
        );

        // Upstream CPU decommit (via MerkleProverLifted).
        let upstream_prover =
            stwo::prover::vcs_lifted::prover::MerkleProverLifted::<MetalBackend, Blake2sMerkleHasher> {
                layers,
            };
        let (upstream_values, upstream_decommit_result) =
            upstream_prover.decommit(&query_positions, vec![&col]);

        // Verify queried values match.
        assert_eq!(
            gpu_values.len(),
            upstream_values.len(),
            "queried_values column count mismatch"
        );
        for (col_idx, (gpu_col, upstream_col)) in
            gpu_values.iter().zip(upstream_values.iter()).enumerate()
        {
            assert_eq!(
                gpu_col, upstream_col,
                "queried_values mismatch at column {col_idx}"
            );
        }

        // Verify hash_witness matches.
        assert_eq!(
            gpu_decommit_result.decommitment.hash_witness,
            upstream_decommit_result.decommitment.hash_witness,
            "hash_witness mismatch"
        );

        // Verify all_node_values matches.
        assert_eq!(
            gpu_decommit_result.aux.all_node_values.len(),
            upstream_decommit_result.aux.all_node_values.len(),
            "all_node_values layer count mismatch"
        );
        for (layer_idx, (gpu_layer, upstream_layer)) in gpu_decommit_result
            .aux
            .all_node_values
            .iter()
            .zip(upstream_decommit_result.aux.all_node_values.iter())
            .enumerate()
        {
            assert_eq!(
                gpu_layer, upstream_layer,
                "all_node_values mismatch at layer {layer_idx}"
            );
        }
    }

    /// Verify that the GPU decommit handles edge cases: adjacent query pairs
    /// where both siblings are queried (no sibling hash emitted).
    #[test]
    fn gpu_decommit_adjacent_queries() {
        let log_size: u32 = 6;
        let n = 1u32 << log_size;

        let col_data: Vec<BaseField> = (0..n).map(M31::from_u32_unchecked).collect();
        let col = MetalBaseFieldVec::from_iter(col_data);
        let columns = vec![&col];

        let layers: Vec<MetalBlake2sHashVec> =
            <MetalBackend as MerkleOpsLifted<Blake2sMerkleHasher>>::build_merkle_layers(
                columns.iter().map(|c| *c).collect(),
                log_size,
            );

        // Adjacent pairs: 0,1 and 4,5 are sibling pairs.
        let query_positions: Vec<usize> = vec![0, 1, 4, 5, 10, 63];

        let (gpu_values, gpu_result) = gpu_decommit::<Blake2sMerkleHasher>(
            &layers,
            &query_positions,
            vec![&col],
        );

        let upstream_prover =
            stwo::prover::vcs_lifted::prover::MerkleProverLifted::<MetalBackend, Blake2sMerkleHasher> {
                layers,
            };
        let (upstream_values, upstream_result) =
            upstream_prover.decommit(&query_positions, vec![&col]);

        assert_eq!(gpu_values, upstream_values);
        assert_eq!(
            gpu_result.decommitment.hash_witness,
            upstream_result.decommitment.hash_witness,
        );
        for (gpu_layer, upstream_layer) in gpu_result
            .aux
            .all_node_values
            .iter()
            .zip(upstream_result.aux.all_node_values.iter())
        {
            assert_eq!(gpu_layer, upstream_layer);
        }
    }
}
