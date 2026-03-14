use std::time::Instant;

use itertools::Itertools;
#[cfg(feature = "parallel")]
use rayon::prelude::*;
use stwo::core::channel::Blake2sChannelGeneric;
use stwo::core::fields::m31::{BaseField, P};
use stwo::core::proof_of_work::GrindOps;
use stwo::core::vcs::blake2_hash::Blake2sHasherGeneric;
use stwo::core::vcs_lifted::blake2_merkle::{
    Blake2sM31MerkleChannel, Blake2sMerkleChannel, Blake2sMerkleHasherGeneric,
};
use stwo::core::vcs_lifted::merkle_hasher::MerkleHasherLifted;
use stwo::prover::backend::BackendForChannel;
use stwo::prover::vcs_lifted::ops::MerkleOpsLifted;
use stwo_metal_sys::metal::U32Buffer;

use super::{MetalBackend, MetalBaseFieldVec};
use crate::stwo_metal::blake2s_hash_vec::Blake2sHashVec as MetalBlake2sHashVec;

const HOST_BLAKE2S_LEAF_CHUNK_COLUMNS: usize = 16;

fn materialize_leaf_columns<'a>(columns: &'a [&'a MetalBaseFieldVec]) -> Vec<&'a [BaseField]> {
    columns.iter().map(|column| column.host_slice()).collect()
}

fn build_leaves_native_standard(
    columns: &[&MetalBaseFieldVec],
    lifting_log_size: u32,
) -> Option<MetalBlake2sHashVec> {
    if columns.is_empty() || lifting_log_size < 12 || columns.len() > 8 {
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
    Some(MetalBlake2sHashVec::from_buffer(packed_hashes))
}

fn build_leaves_native_wide(
    columns: &[&MetalBaseFieldVec],
    lifting_log_size: u32,
) -> Option<MetalBlake2sHashVec> {
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
    Some(MetalBlake2sHashVec::from_buffer(packed_hashes))
}

fn build_next_layer_native_standard(
    prev_layer: &MetalBlake2sHashVec,
) -> Option<MetalBlake2sHashVec> {
    if prev_layer.is_empty() || !prev_layer.len().is_power_of_two() || prev_layer.len() < (1 << 12)
    {
        return None;
    }

    let packed_next_layer = U32Buffer::blake2s_build_next_layer(&prev_layer.buffer).ok()?;
    Some(MetalBlake2sHashVec::from_buffer(packed_next_layer))
}

fn build_leaves_native_standard_packed(
    columns: &[&MetalBaseFieldVec],
    lifting_log_size: u32,
) -> Option<U32Buffer> {
    if columns.is_empty() || lifting_log_size < 12 || columns.len() > 8 {
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
    U32Buffer::blake2s_build_leaves_lifted(
        &flat_columns,
        &column_offsets,
        &column_log_sizes,
        lifting_log_size,
    )
    .ok()
}

fn build_leaves_native_wide_packed(
    columns: &[&MetalBaseFieldVec],
    lifting_log_size: u32,
) -> Option<U32Buffer> {
    if columns.is_empty() {
        return None;
    }

    let column_buffers = columns.iter().map(|column| &column.buffer).collect_vec();
    let column_log_sizes = columns
        .iter()
        .map(|column| column.len().ilog2())
        .collect_vec();
    U32Buffer::blake2s_build_leaves_lifted_wide(
        &column_buffers,
        &column_log_sizes,
        lifting_log_size,
    )
    .ok()
}

fn build_merkle_layers_native_standard(
    columns: &[&MetalBaseFieldVec],
    lifting_log_size: u32,
) -> Option<Vec<MetalBlake2sHashVec>> {
    let packed_layer = if columns.len() > 8 {
        build_leaves_native_wide_packed(columns, lifting_log_size)?
    } else {
        build_leaves_native_standard_packed(columns, lifting_log_size)?
    };
    let packed_layers = U32Buffer::blake2s_build_merkle_layers_from_leaves(packed_layer).ok()?;

    let mut layers = packed_layers
        .into_iter()
        .map(MetalBlake2sHashVec::from_buffer)
        .collect::<Vec<_>>();
    layers.reverse();
    Some(layers)
}

impl<const IS_M31_OUTPUT: bool> MerkleOpsLifted<Blake2sMerkleHasherGeneric<IS_M31_OUTPUT>>
    for MetalBackend
{
    fn build_leaves(columns: &[&MetalBaseFieldVec], lifting_log_size: u32) -> MetalBlake2sHashVec {
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
            return MetalBlake2sHashVec::from_vec(vec![hasher.finalize()]);
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
        MetalBlake2sHashVec::from_vec(leaves)
    }

    fn build_next_layer(prev_layer: &MetalBlake2sHashVec) -> MetalBlake2sHashVec {
        let profile_merkle = std::env::var_os("STWO_METAL_PROFILE_MERKLE").is_some();
        let build_next_layer_start = Instant::now();
        let next = if !IS_M31_OUTPUT {
            if let Some(next) = build_next_layer_native_standard(prev_layer) {
                next
            } else {
                let log_size: u32 = prev_layer.len().ilog2() - 1;
                let prev_layer_cpu = prev_layer.to_vec();
                let hashes = stwo::parallel_iter!(0..(1 << log_size))
                    .map(|i| {
                        Blake2sMerkleHasherGeneric::<IS_M31_OUTPUT>::hash_children((
                            prev_layer_cpu[2 * i],
                            prev_layer_cpu[2 * i + 1],
                        ))
                    })
                    .collect::<Vec<_>>();
                MetalBlake2sHashVec::from_vec(hashes)
            }
        } else {
            let log_size: u32 = prev_layer.len().ilog2() - 1;
            let prev_layer_cpu = prev_layer.to_vec();
            let hashes = stwo::parallel_iter!(0..(1 << log_size))
                .map(|i| {
                    Blake2sMerkleHasherGeneric::<IS_M31_OUTPUT>::hash_children((
                        prev_layer_cpu[2 * i],
                        prev_layer_cpu[2 * i + 1],
                    ))
                })
                .collect::<Vec<_>>();
            MetalBlake2sHashVec::from_vec(hashes)
        };
        if profile_merkle {
            eprintln!(
                "metal_merkle_timing phase=build_next_layer log_size={} ms={}",
                prev_layer.len().ilog2() - 1,
                build_next_layer_start.elapsed().as_secs_f64() * 1000.0
            );
        }
        next
    }

    fn build_merkle_layers(
        columns: Vec<&MetalBaseFieldVec>,
        lifting_log_size: u32,
    ) -> Vec<MetalBlake2sHashVec> {
        let profile_merkle = std::env::var_os("STWO_METAL_PROFILE_MERKLE").is_some();
        let build_tree_start = Instant::now();
        let sorted_columns = columns.into_iter().sorted_by_key(|c| c.len()).collect_vec();
        if !IS_M31_OUTPUT {
            if let Some(layers) =
                build_merkle_layers_native_standard(&sorted_columns, lifting_log_size)
            {
                if profile_merkle {
                    eprintln!(
                        "metal_merkle_timing phase=build_merkle_layers columns={} lifting_log_size={} ms={}",
                        sorted_columns.len(),
                        lifting_log_size,
                        build_tree_start.elapsed().as_secs_f64() * 1000.0
                    );
                }
                return layers;
            }
        }

        let mut layers = vec![<Self as MerkleOpsLifted<
            Blake2sMerkleHasherGeneric<IS_M31_OUTPUT>,
        >>::build_leaves(&sorted_columns, lifting_log_size)];
        for _ in 0..lifting_log_size {
            let next =
                <Self as MerkleOpsLifted<Blake2sMerkleHasherGeneric<IS_M31_OUTPUT>>>::build_next_layer(
                    layers.last().unwrap(),
                );
            layers.push(next);
        }
        layers.reverse();
        if profile_merkle {
            eprintln!(
                "metal_merkle_timing phase=build_merkle_layers columns={} lifting_log_size={} ms={}",
                sorted_columns.len(),
                lifting_log_size,
                build_tree_start.elapsed().as_secs_f64() * 1000.0
            );
        }
        layers
    }
}

/// Number of nonce_lo candidates per GPU batch dispatch.
/// 2^24 = 16,777,216 threads — covers the expected search space for 24-bit PoW
/// in a single dispatch, minimizing command buffer overhead.
const GPU_GRIND_BATCH_BITS: u32 = 24;

impl<const IS_M31_OUTPUT: bool> GrindOps<Blake2sChannelGeneric<IS_M31_OUTPUT>> for MetalBackend {
    fn grind(channel: &Blake2sChannelGeneric<IS_M31_OUTPUT>, pow_bits: u32) -> u64 {
        assert!(pow_bits <= 32, "pow_bits > 32 is not supported");

        // Compute the prefix digest: H(POW_PREFIX, [0_u8; 12], digest, pow_bits).
        let digest = channel.digest();
        let mut hasher = Blake2sHasherGeneric::<IS_M31_OUTPUT>::default();
        hasher.update(&Blake2sChannelGeneric::<IS_M31_OUTPUT>::POW_PREFIX.to_le_bytes());
        hasher.update(&[0_u8; 12]);
        hasher.update(&digest.0[..]);
        hasher.update(&pow_bits.to_le_bytes());
        let prefixed_digest = hasher.finalize();
        let prefix_array: [u32; 8] = std::array::from_fn(|i| {
            u32::from_le_bytes(
                prefixed_digest.0[i * 4..(i + 1) * 4]
                    .try_into()
                    .unwrap(),
            )
        });

        let batch_size = 1u32 << GPU_GRIND_BATCH_BITS;

        // Iterate over nonce_hi values, dispatching GPU batches.
        for hi in 0..P {
            let nonce_lo = U32Buffer::blake2s_grind_batch(
                &prefix_array,
                pow_bits,
                hi,
                batch_size,
            )
            .expect("Metal GPU grind dispatch failed");

            if nonce_lo != u32::MAX {
                let nonce = ((hi as u64) << 32) | (nonce_lo as u64);
                assert!(
                    (nonce as u32) < P,
                    "The 32 low bits of the nonce are not reduced modulo the M31 prime."
                );
                return nonce;
            }
        }

        panic!("GPU grind failed to find a solution.");
    }
}

impl BackendForChannel<Blake2sMerkleChannel> for MetalBackend {}
impl BackendForChannel<Blake2sM31MerkleChannel> for MetalBackend {}

#[cfg(test)]
mod tests {
    use stwo::core::channel::{Blake2sChannel, Channel};
    use stwo::core::proof_of_work::GrindOps;
    use stwo::prover::backend::simd::SimdBackend;

    use super::MetalBackend;

    #[test]
    fn gpu_grind_matches_simd() {
        let mut channel = Blake2sChannel::default();
        channel.mix_u64(0xDEADBEEF_CAFEBABE);
        let pow_bits = 10; // Low PoW for fast test.

        let gpu_nonce = MetalBackend::grind(&channel, pow_bits);
        assert!(
            channel.clone().verify_pow_nonce(pow_bits, gpu_nonce),
            "GPU grind nonce {} failed verification",
            gpu_nonce
        );

        // Verify the same channel state gives a valid SIMD nonce too.
        let simd_nonce = SimdBackend::grind(&channel, pow_bits);
        assert!(
            channel.verify_pow_nonce(pow_bits, simd_nonce),
            "SIMD grind nonce {} failed verification",
            simd_nonce
        );

        // Both should find a valid nonce (may differ but both must verify).
        println!("GPU nonce: {}, SIMD nonce: {}", gpu_nonce, simd_nonce);
    }

    #[test]
    fn gpu_grind_deterministic() {
        let mut channel = Blake2sChannel::default();
        channel.mix_u64(0x1111222233334444);
        let pow_bits = 8;

        let nonce1 = MetalBackend::grind(&channel, pow_bits);
        let nonce2 = MetalBackend::grind(&channel, pow_bits);
        assert_eq!(nonce1, nonce2, "GPU grind should be deterministic");
    }

    #[test]
    fn gpu_grind_high_pow_bits() {
        let mut channel = Blake2sChannel::default();
        channel.mix_u64(0xAABBCCDDEEFF0011);
        let pow_bits = 20; // ~1M nonces expected.

        let gpu_nonce = MetalBackend::grind(&channel, pow_bits);
        assert!(
            channel.verify_pow_nonce(pow_bits, gpu_nonce),
            "GPU grind nonce {} failed verification at pow_bits={}",
            gpu_nonce,
            pow_bits
        );
    }
}
