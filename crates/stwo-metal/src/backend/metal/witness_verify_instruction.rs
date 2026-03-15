//! GPU-accelerated witness trace generation for the `verify_instruction` component.
//!
//! This module provides a Metal-backed version of the verify_instruction
//! trace generator from stwo-cairo.
//!
//! # Trace layout
//!
//! The trace table has 17 columns in column-major layout:
//!
//!   col 0:  input_pc
//!   col 1:  input_offset0
//!   col 2:  input_offset1
//!   col 3:  input_offset2
//!   col 4:  input_inst_felt5_high
//!   col 5:  input_inst_felt6
//!   col 6:  input_opcode_extension
//!   col 7:  offset0_low           (offset0 & 0x1FF)
//!   col 8:  offset0_mid           (offset0 >> 9)
//!   col 9:  offset1_low           (offset1 & 0x3)
//!   col 10: offset1_mid           ((offset1 >> 2) & 0x1FF)
//!   col 11: offset1_high          (offset1 >> 11)
//!   col 12: offset2_low           (offset2 & 0xF)
//!   col 13: offset2_mid           ((offset2 >> 4) & 0x1FF)
//!   col 14: offset2_high          (offset2 >> 13)
//!   col 15: instruction_id        (addr_to_id[pc])
//!   col 16: multiplicity
//!
//! This mirrors the CPU trace generation in stwo-cairo's
//! `verify_instruction::ClaimGenerator::write_trace()`.

use stwo::core::fields::m31::BaseField;
use stwo::core::poly::circle::CanonicCoset;
use stwo::prover::backend::CpuBackend;
use stwo::prover::poly::circle::CircleEvaluation;
use stwo::prover::poly::BitReversedOrder;
use stwo_metal_sys::metal::{MetalError, U32Buffer};

use super::MetalBackend;
use crate::stwo_metal::base_field_vec::BaseFieldVec;

/// Number of trace columns for verify_instruction (matches cairo-air).
pub const N_TRACE_COLUMNS: usize = 17;

/// Number of input fields per row: (pc, offset0, offset1, offset2, felt5_high, felt6, opcode_ext).
pub const N_INPUT_FIELDS: usize = 7;

/// Error type for verify_instruction witness generation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VerifyInstructionTraceError {
    /// The column length is not a power of two.
    ColumnLengthNotPowerOfTwo { column_length: usize },
    /// An error occurred in the Metal runtime.
    Runtime { message: String },
}

impl core::fmt::Display for VerifyInstructionTraceError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::ColumnLengthNotPowerOfTwo { column_length } => {
                write!(
                    f,
                    "verify_instruction trace requires a power-of-two column length, got {column_length}"
                )
            }
            Self::Runtime { message } => f.write_str(message),
        }
    }
}

impl std::error::Error for VerifyInstructionTraceError {}

impl From<MetalError> for VerifyInstructionTraceError {
    fn from(value: MetalError) -> Self {
        Self::Runtime {
            message: value.message().to_string(),
        }
    }
}

/// Result of Metal GPU trace generation for the verify_instruction component.
///
/// Holds the raw GPU buffer and provides methods for extracting column values
/// or converting to `CircleEvaluation`s.
#[derive(Debug)]
pub struct MetalVerifyInstructionTrace {
    values: U32Buffer,
    n_rows: usize,
    column_length: usize,
}

impl MetalVerifyInstructionTrace {
    /// The number of real (non-padding) rows.
    pub fn n_rows(&self) -> usize {
        self.n_rows
    }

    /// The power-of-two padded column length.
    pub fn column_length(&self) -> usize {
        self.column_length
    }

    /// Total number of trace columns (always 17).
    pub fn n_columns(&self) -> usize {
        N_TRACE_COLUMNS
    }

    /// Read a single cell from the trace.
    pub fn value(&self, column_index: usize, row_index: usize) -> BaseField {
        assert!(column_index < N_TRACE_COLUMNS, "column out of bounds");
        assert!(row_index < self.column_length, "row out of bounds");
        BaseField::from_u32_unchecked(
            self.values.get(column_index * self.column_length + row_index),
        )
    }

    /// Extract all values for a single column.
    pub fn column_values(&self, column_index: usize) -> Vec<BaseField> {
        assert!(column_index < N_TRACE_COLUMNS, "column out of bounds");
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
        (0..N_TRACE_COLUMNS)
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
        (0..N_TRACE_COLUMNS)
            .map(|col| {
                let start = col * self.column_length;
                let col_buf = self
                    .values
                    .clone_range(start, self.column_length)
                    .expect("Metal verify-instruction trace column slicing should succeed");
                CircleEvaluation::<MetalBackend, BaseField, BitReversedOrder>::new(
                    domain,
                    BaseFieldVec::from_buffer(col_buf),
                )
            })
            .collect()
    }
}

/// Generate the verify_instruction trace on the Metal GPU.
///
/// This is the GPU equivalent of `verify_instruction::ClaimGenerator::write_trace()`
/// from stwo-cairo, producing the same 17-column trace output.
///
/// # Arguments
///
/// * `inputs` - Flattened row-major array of instruction inputs. Each row has 7 fields:
///   `(pc, offset0, offset1, offset2, inst_felt5_high, inst_felt6, opcode_extension)`.
///   Total length must be `n_rows * 7`.
/// * `multiplicities` - One u32 multiplicity per instruction row.
/// * `address_to_id` - The memory address-to-raw-id lookup table. `address_to_id[pc]`
///   gives the memory ID for the instruction at address `pc`.
///
/// # Returns
///
/// A `MetalVerifyInstructionTrace` containing 17 columns of `column_length` rows.
pub fn generate_verify_instruction_trace(
    inputs: &[u32],
    multiplicities: &[u32],
    address_to_id: &[u32],
) -> Result<MetalVerifyInstructionTrace, VerifyInstructionTraceError> {
    let n_rows = multiplicities.len();
    assert_eq!(
        inputs.len(),
        n_rows * N_INPUT_FIELDS,
        "inputs length must be n_rows * 7"
    );
    assert_ne!(n_rows, 0, "must have at least one instruction row");

    // Column length = next power of two >= n_rows, but at least N_LANES (16).
    let column_length = std::cmp::max(n_rows.next_power_of_two(), 16);

    let inputs_buf = U32Buffer::from_slice(inputs)?;
    let mults_buf = U32Buffer::from_slice(multiplicities)?;
    let addr_to_id_buf = U32Buffer::from_slice(address_to_id)?;

    let trace_buf = U32Buffer::witness_verify_instruction_trace(
        &inputs_buf,
        &mults_buf,
        &addr_to_id_buf,
        n_rows as u32,
        column_length as u32,
    )?;

    Ok(MetalVerifyInstructionTrace {
        values: trace_buf,
        n_rows,
        column_length,
    })
}

#[cfg(test)]
mod tests {
    use stwo::prover::backend::Column;

    use super::*;

    /// CPU reference implementation for verify_instruction trace generation.
    /// Mirrors the write_trace_simd logic from stwo-cairo.
    fn cpu_reference_trace(
        inputs: &[(u32, [u32; 3], [u32; 2], u32)],
        mults: &[u32],
        addr_to_id: &[u32],
    ) -> Vec<Vec<u32>> {
        let n_rows = inputs.len();
        let size = std::cmp::max(n_rows.next_power_of_two(), 16);

        let mut trace = vec![vec![0u32; size]; N_TRACE_COLUMNS];

        for row in 0..n_rows {
            let (pc, offsets, felts, opcode_ext) = inputs[row];
            let offset0 = offsets[0];
            let offset1 = offsets[1];
            let offset2 = offsets[2];
            let felt5_high = felts[0];
            let felt6 = felts[1];

            // Pass-through columns.
            trace[0][row] = pc;
            trace[1][row] = offset0;
            trace[2][row] = offset1;
            trace[3][row] = offset2;
            trace[4][row] = felt5_high;
            trace[5][row] = felt6;
            trace[6][row] = opcode_ext;

            // Encode offsets.
            trace[7][row] = offset0 & 0x1FF;
            trace[8][row] = offset0 >> 9;
            trace[9][row] = offset1 & 0x3;
            trace[10][row] = (offset1 >> 2) & 0x1FF;
            trace[11][row] = offset1 >> 11;
            trace[12][row] = offset2 & 0xF;
            trace[13][row] = (offset2 >> 4) & 0x1FF;
            trace[14][row] = offset2 >> 13;

            // Memory lookup.
            trace[15][row] = if (pc as usize) < addr_to_id.len() {
                addr_to_id[pc as usize]
            } else {
                0
            };

            // Multiplicity.
            trace[16][row] = mults[row];
        }

        trace
    }

    /// Flatten structured inputs into the row-major u32 array expected by the GPU.
    fn flatten_inputs(inputs: &[(u32, [u32; 3], [u32; 2], u32)]) -> Vec<u32> {
        let mut flat = Vec::with_capacity(inputs.len() * N_INPUT_FIELDS);
        for &(pc, offsets, felts, opcode_ext) in inputs {
            flat.push(pc);
            flat.push(offsets[0]);
            flat.push(offsets[1]);
            flat.push(offsets[2]);
            flat.push(felts[0]);
            flat.push(felts[1]);
            flat.push(opcode_ext);
        }
        flat
    }

    #[test]
    fn test_basic_trace_generation() {
        // 32 instructions with simple values.
        let n = 32;
        let addr_to_id: Vec<u32> = (100..100 + n as u32 + 50).collect();

        let inputs: Vec<(u32, [u32; 3], [u32; 2], u32)> = (0..n)
            .map(|i| {
                let pc = i as u32;
                let offset0 = (i as u32 * 3 + 7) & 0xFFFF;
                let offset1 = (i as u32 * 5 + 13) & 0xFFFF;
                let offset2 = (i as u32 * 7 + 19) & 0xFFFF;
                let felt5_high = (i as u32) & 0x1F;
                let felt6 = (i as u32 * 2) & 0xFF;
                let opcode_ext = (i as u32) & 0x7;
                (pc, [offset0, offset1, offset2], [felt5_high, felt6], opcode_ext)
            })
            .collect();
        let mults: Vec<u32> = (0..n).map(|i| i as u32 + 1).collect();

        let flat_inputs = flatten_inputs(&inputs);
        let result = generate_verify_instruction_trace(&flat_inputs, &mults, &addr_to_id).unwrap();

        assert_eq!(result.n_columns(), 17);
        assert_eq!(result.n_rows(), n);
        assert!(result.column_length().is_power_of_two());
        assert!(result.column_length() >= n);

        let cpu_ref = cpu_reference_trace(&inputs, &mults, &addr_to_id);

        for col in 0..N_TRACE_COLUMNS {
            for row in 0..result.column_length() {
                assert_eq!(
                    result.value(col, row).0,
                    cpu_ref[col][row],
                    "mismatch at col {col}, row {row}: GPU={} CPU={}",
                    result.value(col, row).0,
                    cpu_ref[col][row],
                );
            }
        }
    }

    #[test]
    fn test_offset_decomposition() {
        // Single instruction with known offset values for exact bit-field checking.
        let offset0: u32 = 0b1010101_101010101; // mid=0b1010101=85, low=0b101010101=341
        let offset1: u32 = 0b10101_101010101_01; // high=0b10101=21, mid=0b101010101=341, low=0b01=1
        let offset2: u32 = 0b101_101010101_0101; // high=0b101=5, mid=0b101010101=341, low=0b0101=5

        let inputs: Vec<(u32, [u32; 3], [u32; 2], u32)> = vec![
            (0, [offset0, offset1, offset2], [3, 7], 2),
        ];
        let mults = vec![42u32];
        let addr_to_id = vec![999u32]; // addr_to_id[0] = 999

        let flat = flatten_inputs(&inputs);
        let result = generate_verify_instruction_trace(&flat, &mults, &addr_to_id).unwrap();

        // offset0 decomposition
        assert_eq!(result.value(7, 0).0, 341, "offset0_low");
        assert_eq!(result.value(8, 0).0, 85, "offset0_mid");

        // offset1 decomposition
        assert_eq!(result.value(9, 0).0, 1, "offset1_low");
        assert_eq!(result.value(10, 0).0, 341, "offset1_mid");
        assert_eq!(result.value(11, 0).0, 21, "offset1_high");

        // offset2 decomposition
        assert_eq!(result.value(12, 0).0, 5, "offset2_low");
        assert_eq!(result.value(13, 0).0, 341, "offset2_mid");
        assert_eq!(result.value(14, 0).0, 5, "offset2_high");

        // Memory lookup
        assert_eq!(result.value(15, 0).0, 999, "instruction_id");

        // Multiplicity
        assert_eq!(result.value(16, 0).0, 42, "multiplicity");
    }

    #[test]
    fn test_padding_zeroes() {
        // 20 instructions -> column_length = 32, rows 20..31 should be padding zeros.
        let n = 20;
        let addr_to_id: Vec<u32> = (0..100).collect();
        let inputs: Vec<(u32, [u32; 3], [u32; 2], u32)> = (0..n)
            .map(|i| (i as u32, [100, 200, 300], [1, 2], 3))
            .collect();
        let mults: Vec<u32> = vec![1; n];

        let flat = flatten_inputs(&inputs);
        let result = generate_verify_instruction_trace(&flat, &mults, &addr_to_id).unwrap();

        assert_eq!(result.column_length(), 32);

        // Padding rows should be all zeros.
        for row in n..result.column_length() {
            for col in 0..N_TRACE_COLUMNS {
                assert_eq!(
                    result.value(col, row).0,
                    0,
                    "padding should be 0 at col {col}, row {row}"
                );
            }
        }
    }

    #[test]
    fn test_to_metal_evaluations() {
        let n = 64;
        let addr_to_id: Vec<u32> = (0..200).collect();
        let inputs: Vec<(u32, [u32; 3], [u32; 2], u32)> = (0..n)
            .map(|i| (i as u32, [i as u32 * 10, i as u32 * 20, i as u32 * 30], [1, 2], 0))
            .collect();
        let mults: Vec<u32> = vec![1; n];

        let flat = flatten_inputs(&inputs);
        let result = generate_verify_instruction_trace(&flat, &mults, &addr_to_id).unwrap();

        let metal_evals = result.to_metal_evaluations();
        assert_eq!(metal_evals.len(), N_TRACE_COLUMNS);

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
}
