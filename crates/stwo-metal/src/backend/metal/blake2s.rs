use std::time::Instant;

use itertools::Itertools;
#[cfg(feature = "parallel")]
use rayon::prelude::*;
use stwo::core::channel::{Blake2sChannelGeneric, Channel};
use stwo::core::fields::m31::BaseField;
use stwo::core::proof_of_work::GrindOps;
use stwo::core::utils::bit_reverse;
use stwo::core::vcs::blake2_hash::Blake2sHash;
use stwo::core::vcs_lifted::blake2_merkle::{
    Blake2sM31MerkleChannel, Blake2sMerkleChannel, Blake2sMerkleHasherGeneric,
};
use stwo::core::vcs_lifted::merkle_hasher::MerkleHasherLifted;
use stwo::prover::backend::{BackendForChannel, ColumnOps};
use stwo::prover::vcs_lifted::ops::MerkleOpsLifted;
use stwo_metal_sys::metal::U32Buffer;

use super::{MetalBackend, MetalBaseFieldVec};

const HOST_BLAKE2S_LEAF_CHUNK_COLUMNS: usize = 16;

fn materialize_leaf_columns<'a>(columns: &'a [&'a MetalBaseFieldVec]) -> Vec<&'a [BaseField]> {
    columns.iter().map(|column| column.host_slice()).collect()
}

fn decode_packed_blake2s_hashes(words: Vec<u32>) -> Vec<Blake2sHash> {
    words
        .chunks_exact(8)
        .map(|chunk| {
            let mut bytes = [0u8; 32];
            for (word_index, word) in chunk.iter().enumerate() {
                bytes[word_index * 4..(word_index + 1) * 4].copy_from_slice(&word.to_le_bytes());
            }
            Blake2sHash(bytes)
        })
        .collect()
}

fn build_leaves_native_standard(
    columns: &[&MetalBaseFieldVec],
    lifting_log_size: u32,
) -> Option<Vec<Blake2sHash>> {
    if columns.is_empty() || lifting_log_size < 18 || columns.len() > 8 {
        return None;
    }

    let total_len = columns.iter().map(|column| column.len()).sum::<usize>();
    let mut flat_columns = U32Buffer::uninitialized(total_len).ok()?;
    let mut column_offsets = Vec::with_capacity(columns.len());
    let mut column_log_sizes = Vec::with_capacity(columns.len());
    let mut offset = 0usize;
    for column in columns {
        column_offsets.push(offset.try_into().ok()?);
        column_log_sizes.push(column.len().ilog2());
        flat_columns
            .copy_range_from(&column.buffer, 0, column.len(), offset)
            .ok()?;
        offset += column.len();
    }

    let column_offsets = U32Buffer::from_slice(&column_offsets).ok()?;
    let column_log_sizes = U32Buffer::from_slice(&column_log_sizes).ok()?;
    let packed_hashes = U32Buffer::blake2s_build_leaves_lifted(
        &flat_columns,
        &column_offsets,
        &column_log_sizes,
        lifting_log_size,
    )
    .ok()?;
    Some(decode_packed_blake2s_hashes(packed_hashes.to_vec().ok()?))
}

fn build_leaves_native_wide(
    columns: &[&MetalBaseFieldVec],
    lifting_log_size: u32,
) -> Option<Vec<Blake2sHash>> {
    if columns.is_empty() {
        return None;
    }

    let column_buffers = columns.iter().map(|column| &column.buffer).collect_vec();
    let column_log_sizes = columns
        .iter()
        .map(|column| column.len().ilog2())
        .collect_vec();
    let packed_hashes = U32Buffer::blake2s_build_leaves_lifted_wide(
        &column_buffers,
        &column_log_sizes,
        lifting_log_size,
    )
    .ok()?;
    Some(decode_packed_blake2s_hashes(packed_hashes.to_vec().ok()?))
}

impl ColumnOps<Blake2sHash> for MetalBackend {
    type Column = Vec<Blake2sHash>;

    fn bit_reverse_column(column: &mut Self::Column) {
        bit_reverse(column);
    }
}

impl<const IS_M31_OUTPUT: bool> MerkleOpsLifted<Blake2sMerkleHasherGeneric<IS_M31_OUTPUT>>
    for MetalBackend
{
    fn build_leaves(columns: &[&MetalBaseFieldVec], lifting_log_size: u32) -> Vec<Blake2sHash> {
        let profile_merkle = std::env::var_os("STWO_METAL_PROFILE_MERKLE").is_some();
        let build_leaves_start = Instant::now();
        if !IS_M31_OUTPUT {
            if columns.len() > 8 {
                if let Some(leaves) = build_leaves_native_wide(columns, lifting_log_size) {
                    if profile_merkle {
                        eprintln!(
                            "metal_merkle_timing phase=build_leaves columns={} lifting_log_size={} ms={}",
                            columns.len(),
                            lifting_log_size,
                            build_leaves_start.elapsed().as_secs_f64() * 1000.0
                        );
                    }
                    return leaves;
                }
            }
            if let Some(leaves) = build_leaves_native_standard(columns, lifting_log_size) {
                if profile_merkle {
                    eprintln!(
                        "metal_merkle_timing phase=build_leaves columns={} lifting_log_size={} ms={}",
                        columns.len(),
                        lifting_log_size,
                        build_leaves_start.elapsed().as_secs_f64() * 1000.0
                    );
                }
                return leaves;
            }
        }
        let hasher = Blake2sMerkleHasherGeneric::<IS_M31_OUTPUT>::default();
        if columns.is_empty() {
            return vec![hasher.finalize()];
        }

        let host_columns = materialize_leaf_columns(columns);

        assert!(columns[0].len() >= 2, "A column must be of length >= 2.");
        let mut prev_layer = Vec::new();
        let mut prev_layer_log_size: u32 = 0;

        for (log_size, group) in host_columns
            .iter()
            .group_by(|column| column.len().ilog2())
            .into_iter()
        {
            let group_columns = group.into_iter().collect_vec();
            if prev_layer.is_empty() {
                prev_layer = vec![hasher.clone(); 1 << log_size];
            } else {
                let log_ratio = log_size - prev_layer_log_size;
                prev_layer = (0..1 << log_size)
                    .map(|idx| prev_layer[(idx >> (log_ratio + 1) << 1) + (idx & 1)].clone())
                    .collect();
            }

            for chunk_columns in group_columns.chunks(HOST_BLAKE2S_LEAF_CHUNK_COLUMNS) {
                #[cfg(not(feature = "parallel"))]
                let iter = prev_layer.iter_mut().enumerate();
                #[cfg(feature = "parallel")]
                let iter = prev_layer.par_iter_mut().enumerate();

                iter.for_each(|(i, hasher)| {
                    let mut chunk_bytes = [0u8; HOST_BLAKE2S_LEAF_CHUNK_COLUMNS * 4];
                    let mut used = 0usize;
                    for column in chunk_columns {
                        chunk_bytes[used..used + 4].copy_from_slice(&column[i].0.to_le_bytes());
                        used += 4;
                    }
                    hasher.update(&chunk_bytes[..used]);
                });
            }
            prev_layer_log_size = log_size;
        }

        let log_ratio = lifting_log_size - prev_layer_log_size;
        if log_ratio > 0 {
            prev_layer = (0..1 << lifting_log_size)
                .map(|idx| prev_layer[(idx >> (log_ratio + 1) << 1) + (idx & 1)].clone())
                .collect();
        }

        #[cfg(not(feature = "parallel"))]
        let iter = prev_layer.into_iter();
        #[cfg(feature = "parallel")]
        let iter = prev_layer.into_par_iter();

        let leaves = iter.map(|x| x.finalize()).collect();

        if profile_merkle {
            eprintln!(
                "metal_merkle_timing phase=build_leaves columns={} lifting_log_size={} ms={}",
                columns.len(),
                lifting_log_size,
                build_leaves_start.elapsed().as_secs_f64() * 1000.0
            );
        }
        leaves
    }

    fn build_next_layer(prev_layer: &Vec<Blake2sHash>) -> Vec<Blake2sHash> {
        let profile_merkle = std::env::var_os("STWO_METAL_PROFILE_MERKLE").is_some();
        let build_next_layer_start = Instant::now();
        let log_size: u32 = prev_layer.len().ilog2() - 1;
        let next = stwo::parallel_iter!(0..(1 << log_size))
            .map(|i| {
                Blake2sMerkleHasherGeneric::<IS_M31_OUTPUT>::hash_children((
                    prev_layer[2 * i],
                    prev_layer[2 * i + 1],
                ))
            })
            .collect();
        if profile_merkle {
            eprintln!(
                "metal_merkle_timing phase=build_next_layer log_size={} ms={}",
                log_size,
                build_next_layer_start.elapsed().as_secs_f64() * 1000.0
            );
        }
        next
    }
}

impl<const IS_M31_OUTPUT: bool> GrindOps<Blake2sChannelGeneric<IS_M31_OUTPUT>> for MetalBackend {
    fn grind(channel: &Blake2sChannelGeneric<IS_M31_OUTPUT>, pow_bits: u32) -> u64 {
        let mut nonce = 0;
        loop {
            let channel = channel.clone();
            if channel.verify_pow_nonce(pow_bits, nonce) {
                return nonce;
            }
            nonce += 1;
        }
    }
}

impl BackendForChannel<Blake2sMerkleChannel> for MetalBackend {}
impl BackendForChannel<Blake2sM31MerkleChannel> for MetalBackend {}
