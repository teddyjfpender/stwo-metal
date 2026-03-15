//! GPU-accelerated witness trace generation for the `memory_address_to_id` component.
//!
//! This module provides a Metal-backed version of the memory_address_to_id
//! trace generator from stwo-cairo.
//!
//! # Trace layout
//!
//! The trace table has `SPLIT * 2` columns, organized as pairs of
//! (id, multiplicity) for each chunk. The address-to-id mapping is split
//! into `SPLIT` chunks of `column_length` rows each.
//!
//! Column layout (column-major):
//!   [id_chunk0, mult_chunk0, id_chunk1, mult_chunk1, ..., id_chunk15, mult_chunk15]
//!
//! This mirrors the CPU trace generation in stwo-cairo's
//! `memory_address_to_id::ClaimGenerator::write_trace()`.

use stwo::core::fields::m31::BaseField;
use stwo::core::poly::circle::CanonicCoset;
use stwo::prover::backend::CpuBackend;
use stwo::prover::poly::circle::CircleEvaluation;
use stwo::prover::poly::BitReversedOrder;
use stwo_metal_sys::metal::{MetalError, U32Buffer};

use super::MetalBackend;
use crate::stwo_metal::base_field_vec::BaseFieldVec;

/// Default split factor matching stwo-cairo's MEMORY_ADDRESS_TO_ID_SPLIT.
/// This is 1 << (LOG_MEMORY_ADDRESS_BOUND - MAX_SEQUENCE_LOG_SIZE) = 1 << (29 - 25) = 16.
pub const DEFAULT_SPLIT: u32 = 16;

/// Error type for memory_address_to_id witness generation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MemoryAddrToIdTraceError {
    /// The column length is not a power of two.
    ColumnLengthNotPowerOfTwo { column_length: usize },
    /// An error occurred in the Metal runtime.
    Runtime { message: String },
}

impl core::fmt::Display for MemoryAddrToIdTraceError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::ColumnLengthNotPowerOfTwo { column_length } => {
                write!(
                    f,
                    "memory_address_to_id trace requires a power-of-two column length, got {column_length}"
                )
            }
            Self::Runtime { message } => f.write_str(message),
        }
    }
}

impl std::error::Error for MemoryAddrToIdTraceError {}

impl From<MetalError> for MemoryAddrToIdTraceError {
    fn from(value: MetalError) -> Self {
        Self::Runtime {
            message: value.message().to_string(),
        }
    }
}

/// Result of Metal GPU trace generation for the memory_address_to_id component.
///
/// Holds the raw GPU buffer and provides methods for extracting column values
/// or converting to `CircleEvaluation`s.
#[derive(Debug)]
pub struct MetalMemoryAddrToIdTrace {
    values: U32Buffer,
    n_ids: usize,
    column_length: usize,
    split: usize,
}

impl MetalMemoryAddrToIdTrace {
    /// The number of real (non-padding) rows across all chunks.
    pub fn n_ids(&self) -> usize {
        self.n_ids
    }

    /// The power-of-two padded column length per chunk.
    pub fn column_length(&self) -> usize {
        self.column_length
    }

    /// The split factor (number of chunks).
    pub fn split(&self) -> usize {
        self.split
    }

    /// Total number of trace columns (split * 2).
    pub fn n_columns(&self) -> usize {
        self.split * 2
    }

    /// Read a single cell from the trace.
    pub fn value(&self, column_index: usize, row_index: usize) -> BaseField {
        assert!(column_index < self.n_columns(), "column out of bounds");
        assert!(row_index < self.column_length, "row out of bounds");
        BaseField::from_u32_unchecked(
            self.values.get(column_index * self.column_length + row_index),
        )
    }

    /// Extract all values for a single column.
    pub fn column_values(&self, column_index: usize) -> Vec<BaseField> {
        assert!(column_index < self.n_columns(), "column out of bounds");
        let start = column_index * self.column_length;
        (start..start + self.column_length)
            .map(|i| BaseField::from_u32_unchecked(self.values.get(i)))
            .collect()
    }

    /// Convert the GPU trace into CPU `CircleEvaluation`s on the canonical domain.
    pub fn to_cpu_evaluations(
        &self,
    ) -> Vec<CircleEvaluation<CpuBackend, BaseField, BitReversedOrder>> {
        let log_size = self.column_length.ilog2();
        let domain = CanonicCoset::new(log_size).circle_domain();
        (0..self.n_columns())
            .map(|col| {
                CircleEvaluation::<CpuBackend, BaseField, BitReversedOrder>::new(
                    domain,
                    self.column_values(col),
                )
            })
            .collect()
    }

    /// Convert the GPU trace into Metal-backed `CircleEvaluation`s on the canonical
    /// domain, without copying data through the CPU.
    pub fn to_metal_evaluations(
        &self,
    ) -> Vec<CircleEvaluation<MetalBackend, BaseField, BitReversedOrder>> {
        let log_size = self.column_length.ilog2();
        let domain = CanonicCoset::new(log_size).circle_domain();
        (0..self.n_columns())
            .map(|col| {
                let start = col * self.column_length;
                let col_buf = self
                    .values
                    .clone_range(start, self.column_length)
                    .expect("Metal addr-to-id trace column slicing should succeed");
                CircleEvaluation::<MetalBackend, BaseField, BitReversedOrder>::new(
                    domain,
                    BaseFieldVec::from_buffer(col_buf),
                )
            })
            .collect()
    }
}

/// Generate the memory_address_to_id trace on the Metal GPU.
///
/// This is the GPU equivalent of `memory_address_to_id::ClaimGenerator::write_trace()`
/// from stwo-cairo, producing the same column-major trace output.
///
/// # Arguments
///
/// * `ids` - Raw memory ID values (one u32 per address, addresses offset by 1).
/// * `multiplicities` - One u32 multiplicity per address.
/// * `split` - Number of chunks (MEMORY_ADDRESS_TO_ID_SPLIT, typically 16).
///
/// # Returns
///
/// A `MetalMemoryAddrToIdTrace` containing `split * 2` columns of `column_length` rows.
pub fn generate_memory_addr_to_id_trace(
    ids: &[u32],
    multiplicities: &[u32],
    split: u32,
) -> Result<MetalMemoryAddrToIdTrace, MemoryAddrToIdTraceError> {
    let n_ids = ids.len();
    assert_eq!(
        n_ids,
        multiplicities.len(),
        "ids and multiplicities must have the same length"
    );
    assert!(split > 0, "split must be > 0");

    // Column length = ceil(n_ids / split) rounded up to next power of two,
    // but at least N_LANES (16).
    let chunk_size = n_ids.div_ceil(split as usize);
    let column_length = std::cmp::max(chunk_size.next_power_of_two(), 16);

    let ids_buf = U32Buffer::from_slice(ids)?;
    let mults_buf = U32Buffer::from_slice(multiplicities)?;

    let trace_buf = U32Buffer::witness_memory_addr_to_id_trace(
        &ids_buf,
        &mults_buf,
        n_ids as u32,
        column_length as u32,
        split,
    )?;

    Ok(MetalMemoryAddrToIdTrace {
        values: trace_buf,
        n_ids,
        column_length,
        split: split as usize,
    })
}

#[cfg(test)]
mod tests {
    use stwo::prover::backend::Column;

    use super::*;

    #[test]
    fn test_basic_trace_generation() {
        // 64 IDs with split=2 -> chunk_size=32, column_length=32.
        // n_packed_rows = 32/16 = 2.
        // IDs 0..31 go to chunk 0, IDs 32..63 go to chunk 1.
        let n = 64;
        let ids: Vec<u32> = (1..=n as u32).collect();
        let mults: Vec<u32> = (0..n as u32).map(|i| i * 2).collect();
        let split = 2u32;

        let result = generate_memory_addr_to_id_trace(&ids, &mults, split).unwrap();
        assert_eq!(result.split(), 2);
        assert_eq!(result.n_columns(), 4); // 2 chunks * 2 cols each
        assert!(result.column_length().is_power_of_two());
        let col_len = result.column_length();
        assert_eq!(col_len, 32);

        // Verify chunk 0 IDs and mults (rows 0..31).
        for row in 0..col_len {
            let global_idx = row;
            assert_eq!(
                result.value(0, row).0,
                ids[global_idx],
                "id mismatch at chunk 0, row {row}"
            );
            assert_eq!(
                result.value(1, row).0,
                mults[global_idx],
                "mult mismatch at chunk 0, row {row}"
            );
        }

        // Verify chunk 1 IDs and mults (rows 0..31).
        for row in 0..col_len {
            let global_idx = col_len + row;
            assert_eq!(
                result.value(2, row).0,
                ids[global_idx],
                "id mismatch at chunk 1, row {row}"
            );
            assert_eq!(
                result.value(3, row).0,
                mults[global_idx],
                "mult mismatch at chunk 1, row {row}"
            );
        }
    }

    #[test]
    fn test_to_metal_evaluations() {
        let n = 64;
        let ids: Vec<u32> = (1..=n as u32).collect();
        let mults: Vec<u32> = vec![1u32; n];
        let split = 4u32;

        let result = generate_memory_addr_to_id_trace(&ids, &mults, split).unwrap();

        let metal_evals = result.to_metal_evaluations();
        assert_eq!(metal_evals.len(), 8); // 4 chunks * 2 cols

        let cpu_evals = result.to_cpu_evaluations();
        assert_eq!(cpu_evals.len(), metal_evals.len());
        for (col, (cpu, metal)) in cpu_evals.iter().zip(metal_evals.iter()).enumerate() {
            assert_eq!(cpu.domain, metal.domain, "domain mismatch at col {col}");
            let metal_cpu: Vec<BaseField> = metal.values.to_cpu();
            assert_eq!(
                cpu.values, metal_cpu,
                "value mismatch at col {col}"
            );
        }
    }

    #[test]
    fn test_padding_zeroes() {
        // 48 IDs with split=2 -> chunk_size=ceil(48/2)=24, column_length=32.
        // n_packed_rows = 32/16 = 2.
        // First 32 IDs go to chunk 0 (packed rows 0..1).
        // Next 16 IDs go to chunk 1 (packed row 0), rows 16..31 of chunk 1 are padding.
        let n = 48;
        let ids: Vec<u32> = (1..=n as u32).collect();
        let mults: Vec<u32> = vec![42u32; n];

        let result = generate_memory_addr_to_id_trace(&ids, &mults, 2).unwrap();
        assert_eq!(result.column_length(), 32);

        // Chunk 0 should have all 32 real values.
        for row in 0..32 {
            assert_eq!(
                result.value(0, row).0,
                ids[row],
                "id mismatch at chunk 0, row {row}"
            );
        }

        // Chunk 1 should have 16 real values then 16 zeros.
        for row in 0..16 {
            assert_eq!(
                result.value(2, row).0,
                ids[32 + row],
                "id mismatch at chunk 1, row {row}"
            );
        }
        // Rows 16..31 of chunk 1 should be padded with zeros.
        for row in 16..32 {
            assert_eq!(
                result.value(2, row).0,
                0,
                "padding should be 0 at chunk 1, row {row}"
            );
            assert_eq!(
                result.value(3, row).0,
                0,
                "padding mult should be 0 at chunk 1, row {row}"
            );
        }
    }

    /// Reference implementation of the CPU trace generation for verification.
    /// This mimics stwo-cairo's memory_address_to_id::ClaimGenerator::write_trace().
    fn cpu_reference_trace(ids: &[u32], mults: &[u32], split: usize) -> Vec<Vec<u32>> {
        let n = ids.len();
        let size = std::cmp::max((n.div_ceil(split)).next_power_of_two(), 16);
        let n_trace_columns = split * 2;
        let n_packed_rows = size / 16; // N_LANES = 16

        // Create zero-initialized trace.
        let mut trace = vec![vec![0u32; size]; n_trace_columns];

        // Pad inputs to multiple of 16.
        let padded_n = n.next_multiple_of(16);
        let mut padded_ids = ids.to_vec();
        padded_ids.resize(padded_n, 0);
        let mut padded_mults = mults.to_vec();
        padded_mults.resize(padded_n, 0);

        // Iterate in packs of 16.
        for (i, (id_chunk, mult_chunk)) in padded_ids
            .chunks(16)
            .zip(padded_mults.chunks(16))
            .enumerate()
        {
            let chunk_idx = i / n_packed_rows;
            let local_i = i % n_packed_rows;
            if chunk_idx >= split {
                break;
            }
            for lane in 0..16 {
                let row = local_i * 16 + lane;
                trace[chunk_idx * 2][row] = id_chunk[lane];
                trace[chunk_idx * 2 + 1][row] = mult_chunk[lane];
            }
        }

        trace
    }

    #[test]
    fn test_matches_cpu_reference() {
        // Use realistic-ish parameters: 256 IDs with split=4.
        let n = 256;
        let ids: Vec<u32> = (100..100 + n as u32).collect();
        let mults: Vec<u32> = (0..n as u32).map(|i| (i * 3 + 7) % 100).collect();
        let split = 4u32;

        let gpu_result = generate_memory_addr_to_id_trace(&ids, &mults, split).unwrap();
        let cpu_ref = cpu_reference_trace(&ids, &mults, split as usize);

        assert_eq!(cpu_ref.len(), gpu_result.n_columns());
        let col_len = gpu_result.column_length();
        assert_eq!(cpu_ref[0].len(), col_len);

        for col in 0..gpu_result.n_columns() {
            for row in 0..col_len {
                assert_eq!(
                    gpu_result.value(col, row).0,
                    cpu_ref[col][row],
                    "mismatch at col {col}, row {row}: GPU={} CPU={}",
                    gpu_result.value(col, row).0,
                    cpu_ref[col][row],
                );
            }
        }
    }
}
