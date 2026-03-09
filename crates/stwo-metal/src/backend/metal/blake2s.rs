use stwo::core::channel::Blake2sChannelGeneric;
use stwo::core::proof_of_work::GrindOps;
use stwo::core::utils::bit_reverse;
use stwo::core::vcs::blake2_hash::Blake2sHash;
use stwo::core::vcs_lifted::blake2_merkle::{
    Blake2sM31MerkleChannel, Blake2sMerkleChannel, Blake2sMerkleHasherGeneric,
};
use stwo::prover::backend::{BackendForChannel, Column, ColumnOps, CpuBackend};
use stwo::prover::vcs_lifted::ops::MerkleOpsLifted;

use super::{MetalBackend, MetalBaseFieldVec};

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
        let cpu_columns = columns
            .iter()
            .map(|column| column.to_cpu())
            .collect::<Vec<_>>();
        let cpu_column_refs = cpu_columns.iter().collect::<Vec<_>>();
        <CpuBackend as MerkleOpsLifted<Blake2sMerkleHasherGeneric<IS_M31_OUTPUT>>>::build_leaves(
            &cpu_column_refs,
            lifting_log_size,
        )
    }

    fn build_next_layer(prev_layer: &Vec<Blake2sHash>) -> Vec<Blake2sHash> {
        <CpuBackend as MerkleOpsLifted<Blake2sMerkleHasherGeneric<IS_M31_OUTPUT>>>::build_next_layer(
            prev_layer,
        )
    }
}

impl<const IS_M31_OUTPUT: bool> GrindOps<Blake2sChannelGeneric<IS_M31_OUTPUT>> for MetalBackend {
    fn grind(channel: &Blake2sChannelGeneric<IS_M31_OUTPUT>, pow_bits: u32) -> u64 {
        <CpuBackend as GrindOps<Blake2sChannelGeneric<IS_M31_OUTPUT>>>::grind(channel, pow_bits)
    }
}

impl BackendForChannel<Blake2sMerkleChannel> for MetalBackend {}
impl BackendForChannel<Blake2sM31MerkleChannel> for MetalBackend {}
