#![cfg(all(feature = "prover", target_os = "macos", target_arch = "aarch64"))]

use stwo::core::channel::Blake2sM31Channel;
use stwo::core::fields::m31::BaseField;
use stwo::core::proof_of_work::GrindOps;
use stwo::core::vcs::blake2_hash::Blake2sHash;
use stwo::core::vcs_lifted::blake2_merkle::{Blake2sM31MerkleHasher, Blake2sMerkleHasher};
use stwo::core::vcs_lifted::merkle_hasher::MerkleHasherLifted;
use stwo::prover::backend::CpuBackend;
use stwo::prover::vcs_lifted::ops::MerkleOpsLifted;
use stwo_metal::{metal_runtime_support, MetalBackend, MetalBaseFieldVec, MetalRuntimeSupport};

fn require_metal_runtime() {
    assert_eq!(
        metal_runtime_support(),
        MetalRuntimeSupport::Available,
        "Metal runtime must be available for the native Apple Silicon parity test"
    );
}

fn base_column(len: usize, seed: u32) -> Vec<BaseField> {
    (0..len)
        .map(|i| BaseField::from_u32_unchecked(seed + i as u32))
        .collect()
}

fn assert_blake2s_merkle_bridge_matches_cpu<H>()
where
    H: MerkleHasherLifted<Hash = Blake2sHash>,
    CpuBackend: MerkleOpsLifted<H>,
    MetalBackend: MerkleOpsLifted<H>,
{
    let cpu_columns = vec![
        base_column(8, 3),
        base_column(16, 101),
        base_column(16, 211),
    ];
    let metal_columns = cpu_columns
        .iter()
        .cloned()
        .map(MetalBaseFieldVec::from_vec)
        .collect::<Vec<_>>();
    let cpu_column_refs = cpu_columns.iter().collect::<Vec<_>>();
    let metal_column_refs = metal_columns.iter().collect::<Vec<_>>();

    let expected = <CpuBackend as MerkleOpsLifted<H>>::build_leaves(&cpu_column_refs, 4);
    let actual = <MetalBackend as MerkleOpsLifted<H>>::build_leaves(&metal_column_refs, 4);
    assert_eq!(actual, expected);

    let expected_next = <CpuBackend as MerkleOpsLifted<H>>::build_next_layer(&expected);
    let actual_next = <MetalBackend as MerkleOpsLifted<H>>::build_next_layer(&actual);
    assert_eq!(actual_next, expected_next);
}

#[test]
fn metal_blake2s_merkle_cpu_bridge_matches_cpu_for_both_blake2s_hash_modes() {
    require_metal_runtime();

    assert_blake2s_merkle_bridge_matches_cpu::<Blake2sMerkleHasher>();
    assert_blake2s_merkle_bridge_matches_cpu::<Blake2sM31MerkleHasher>();
}

#[test]
fn metal_blake2s_merkle_native_wide_leaf_path_matches_cpu() {
    require_metal_runtime();

    let cpu_columns = (0..32usize)
        .map(|column_index| base_column(1 << 8, 17 + (column_index as u32) * 257))
        .collect::<Vec<_>>();
    let metal_columns = cpu_columns
        .iter()
        .cloned()
        .map(MetalBaseFieldVec::from_vec)
        .collect::<Vec<_>>();
    let cpu_column_refs = cpu_columns.iter().collect::<Vec<_>>();
    let metal_column_refs = metal_columns.iter().collect::<Vec<_>>();

    let expected =
        <CpuBackend as MerkleOpsLifted<Blake2sMerkleHasher>>::build_leaves(&cpu_column_refs, 8);
    let actual =
        <MetalBackend as MerkleOpsLifted<Blake2sMerkleHasher>>::build_leaves(&metal_column_refs, 8);

    assert_eq!(actual, expected);
}

#[test]
fn metal_blake2s_merkle_native_next_layer_matches_cpu_on_large_standard_layer() {
    require_metal_runtime();

    let prev_layer = (0..(1 << 12))
        .map(|index| {
            let mut bytes = [0u8; 32];
            for (word_index, chunk) in bytes.chunks_exact_mut(4).enumerate() {
                let value = (index as u32)
                    .wrapping_mul(17)
                    .wrapping_add((word_index as u32).wrapping_mul(257));
                chunk.copy_from_slice(&value.to_le_bytes());
            }
            Blake2sHash(bytes)
        })
        .collect::<Vec<_>>();

    let expected =
        <CpuBackend as MerkleOpsLifted<Blake2sMerkleHasher>>::build_next_layer(&prev_layer);
    let actual =
        <MetalBackend as MerkleOpsLifted<Blake2sMerkleHasher>>::build_next_layer(&prev_layer);

    assert_eq!(actual, expected);
}

#[test]
fn metal_blake2s_grind_cpu_bridge_matches_cpu() {
    require_metal_runtime();

    let channel = Blake2sM31Channel::default();
    let pow_bits = 8;

    let expected = CpuBackend::grind(&channel, pow_bits);
    let actual = MetalBackend::grind(&channel, pow_bits);

    assert_eq!(actual, expected);
}
