//! GPU-accelerated witness trace generation for the `memory_id_to_big` component.
//!
//! This module provides Metal-backed versions of the "big" and "small" trace
//! generators from stwo-cairo's `memory_id_to_big` witness component.
//!
//! # Trace layout
//!
//! **Big trace** (F252 values):
//! - 28 value columns, each holding one 9-bit limb of a Felt252.
//! - 1 multiplicity column.
//! - Total: 29 columns per trace component.
//!
//! **Small trace** (u128 values):
//! - 8 value columns (N_M31_IN_SMALL_FELT252).
//! - 1 multiplicity column.
//! - Total: 9 columns.
//!
//! Both kernels produce column-major output suitable for direct conversion to
//! `CircleEvaluation` columns.

use stwo::core::fields::m31::BaseField;
use stwo::core::poly::circle::CanonicCoset;
use stwo::prover::backend::CpuBackend;
use stwo::prover::poly::circle::CircleEvaluation;
use stwo::prover::poly::BitReversedOrder;
use stwo_metal_sys::metal::{MetalError, U32Buffer};

/// Number of 9-bit limbs in a Felt252 (matches FELT252_N_WORDS).
const FELT252_N_WORDS: usize = 28;

/// Number of 9-bit limbs used for "small" memory values
/// (matches N_M31_IN_SMALL_FELT252).
const N_M31_IN_SMALL_FELT252: usize = 8;

/// Number of trace columns for the big-value component (28 limbs + 1 multiplicity).
const BIG_N_COLUMNS: usize = FELT252_N_WORDS + 1;

/// Number of trace columns for the small-value component (8 limbs + 1 multiplicity).
const SMALL_N_COLUMNS: usize = N_M31_IN_SMALL_FELT252 + 1;

/// Error type for memory_id_to_big witness generation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MemoryIdToBigTraceError {
    /// The number of values is not aligned to the SIMD lane width.
    ValuesNotAligned { n_values: usize },
    /// The column length is not a power of two.
    ColumnLengthNotPowerOfTwo { column_length: usize },
    /// An error occurred in the Metal runtime.
    Runtime { message: String },
}

impl core::fmt::Display for MemoryIdToBigTraceError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::ValuesNotAligned { n_values } => {
                write!(
                    f,
                    "memory_id_to_big trace generation requires n_values to be > 0, got {n_values}"
                )
            }
            Self::ColumnLengthNotPowerOfTwo { column_length } => {
                write!(
                    f,
                    "memory_id_to_big trace generation requires a power-of-two column length, got {column_length}"
                )
            }
            Self::Runtime { message } => f.write_str(message),
        }
    }
}

impl std::error::Error for MemoryIdToBigTraceError {}

impl From<MetalError> for MemoryIdToBigTraceError {
    fn from(value: MetalError) -> Self {
        Self::Runtime {
            message: value.message().to_string(),
        }
    }
}

/// Result of Metal GPU trace generation for the memory_id_to_big "big" component.
///
/// Holds the raw GPU buffer and provides methods for extracting column values
/// or converting to `CircleEvaluation`s.
#[derive(Debug)]
pub struct MetalMemoryIdToBigTrace {
    values: U32Buffer,
    n_values: usize,
    column_length: usize,
}

impl MetalMemoryIdToBigTrace {
    /// The number of real (non-padding) rows.
    pub fn n_values(&self) -> usize {
        self.n_values
    }

    /// The power-of-two padded column length.
    pub fn column_length(&self) -> usize {
        self.column_length
    }

    /// Read a single cell from the trace.
    pub fn value(&self, column_index: usize, row_index: usize) -> BaseField {
        assert!(column_index < BIG_N_COLUMNS, "column out of bounds");
        assert!(row_index < self.column_length, "row out of bounds");
        BaseField::from_u32_unchecked(
            self.values.get(column_index * self.column_length + row_index),
        )
    }

    /// Extract all values for a single column.
    pub fn column_values(&self, column_index: usize) -> Vec<BaseField> {
        assert!(column_index < BIG_N_COLUMNS, "column out of bounds");
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
        (0..BIG_N_COLUMNS)
            .map(|col| {
                CircleEvaluation::<CpuBackend, BaseField, BitReversedOrder>::new(
                    domain,
                    self.column_values(col),
                )
            })
            .collect()
    }
}

/// Result of Metal GPU trace generation for the memory_id_to_big "small" component.
#[derive(Debug)]
pub struct MetalMemoryIdToBigSmallTrace {
    values: U32Buffer,
    n_values: usize,
    column_length: usize,
}

impl MetalMemoryIdToBigSmallTrace {
    /// The number of real (non-padding) rows.
    pub fn n_values(&self) -> usize {
        self.n_values
    }

    /// The power-of-two padded column length.
    pub fn column_length(&self) -> usize {
        self.column_length
    }

    /// Read a single cell from the trace.
    pub fn value(&self, column_index: usize, row_index: usize) -> BaseField {
        assert!(column_index < SMALL_N_COLUMNS, "column out of bounds");
        assert!(row_index < self.column_length, "row out of bounds");
        BaseField::from_u32_unchecked(
            self.values.get(column_index * self.column_length + row_index),
        )
    }

    /// Extract all values for a single column.
    pub fn column_values(&self, column_index: usize) -> Vec<BaseField> {
        assert!(column_index < SMALL_N_COLUMNS, "column out of bounds");
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
        (0..SMALL_N_COLUMNS)
            .map(|col| {
                CircleEvaluation::<CpuBackend, BaseField, BitReversedOrder>::new(
                    domain,
                    self.column_values(col),
                )
            })
            .collect()
    }
}

/// Generate the "big" memory_id_to_big trace on the Metal GPU.
///
/// This is the GPU equivalent of `gen_single_big_memory_trace` from stwo-cairo.
///
/// # Arguments
///
/// * `big_values` - Row-major slice of `[u32; 8]` values (one 256-bit F252 per row).
/// * `multiplicities` - One `u32` multiplicity per row (M31 values stored as raw u32).
///
/// # Returns
///
/// A `MetalMemoryIdToBigTrace` containing 29 columns (28 x 9-bit limbs + multiplicity).
pub fn generate_memory_id_to_big_trace(
    big_values: &[[u32; 8]],
    multiplicities: &[u32],
) -> Result<MetalMemoryIdToBigTrace, MemoryIdToBigTraceError> {
    let n_values = big_values.len();
    if n_values == 0 {
        return Err(MemoryIdToBigTraceError::ValuesNotAligned { n_values: 0 });
    }
    assert_eq!(
        n_values,
        multiplicities.len(),
        "big_values and multiplicities must have the same length"
    );

    let column_length = n_values.next_power_of_two();

    // Flatten big_values into a contiguous u32 buffer.
    let flat_values: Vec<u32> = big_values
        .iter()
        .flat_map(|v| v.iter().copied())
        .collect();
    let values_buf = U32Buffer::from_slice(&flat_values)?;
    let mults_buf = U32Buffer::from_slice(multiplicities)?;

    let trace_buf = U32Buffer::witness_memory_id_to_big_trace(
        &values_buf,
        &mults_buf,
        n_values as u32,
        column_length as u32,
    )?;

    Ok(MetalMemoryIdToBigTrace {
        values: trace_buf,
        n_values,
        column_length,
    })
}

/// Generate the "small" memory_id_to_big trace on the Metal GPU.
///
/// This is the GPU equivalent of `gen_small_memory_trace` from stwo-cairo.
///
/// # Arguments
///
/// * `small_values` - Slice of `u128` values (one per row).
/// * `multiplicities` - One `u32` multiplicity per row (M31 values stored as raw u32).
///
/// # Returns
///
/// A `MetalMemoryIdToBigSmallTrace` containing 9 columns (8 x 9-bit limbs + multiplicity).
pub fn generate_memory_id_to_big_small_trace(
    small_values: &[u128],
    multiplicities: &[u32],
) -> Result<MetalMemoryIdToBigSmallTrace, MemoryIdToBigTraceError> {
    let n_values = small_values.len();
    if n_values == 0 {
        return Err(MemoryIdToBigTraceError::ValuesNotAligned { n_values: 0 });
    }
    assert_eq!(
        n_values,
        multiplicities.len(),
        "small_values and multiplicities must have the same length"
    );

    let column_length = n_values.next_power_of_two();

    // Convert u128 values to flat [u32; 4] representation.
    let flat_values: Vec<u32> = small_values
        .iter()
        .flat_map(|&v| {
            [
                v as u32,
                (v >> 32) as u32,
                (v >> 64) as u32,
                (v >> 96) as u32,
            ]
        })
        .collect();
    let values_buf = U32Buffer::from_slice(&flat_values)?;
    let mults_buf = U32Buffer::from_slice(multiplicities)?;

    let trace_buf = U32Buffer::witness_memory_id_to_big_small_trace(
        &values_buf,
        &mults_buf,
        n_values as u32,
        column_length as u32,
    )?;

    Ok(MetalMemoryIdToBigSmallTrace {
        values: trace_buf,
        n_values,
        column_length,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reference implementation of the split_f252 algorithm (scalar, for testing).
    fn split_f252_reference(input: [u32; 8]) -> [u32; FELT252_N_WORDS] {
        const BITS_PER_WORD: usize = 9;
        const MASK: u32 = (1 << BITS_PER_WORD) - 1;

        let mut result = [0u32; FELT252_N_WORDS];
        let mut n_bits_in_word: usize = 32;
        let mut word_i: usize = 0;
        let mut word = input[0];

        for e in result.iter_mut() {
            if n_bits_in_word > BITS_PER_WORD {
                *e = word & MASK;
                word >>= BITS_PER_WORD;
                n_bits_in_word -= BITS_PER_WORD;
                continue;
            }

            let mut val = word;
            word_i += 1;
            word = if word_i < 8 { input[word_i] } else { 0 };

            if n_bits_in_word < BITS_PER_WORD {
                val |= (word << n_bits_in_word) & MASK;
                word >>= BITS_PER_WORD - n_bits_in_word;
            }

            n_bits_in_word += 32 - BITS_PER_WORD;
            *e = val;
        }

        result
    }

    #[test]
    fn test_big_trace_single_value() {
        // A single non-trivial 256-bit value.
        let value: [u32; 8] = [
            0xDEAD_BEEF,
            0xCAFE_BABE,
            0x1234_5678,
            0x9ABC_DEF0,
            0x1111_2222,
            0x3333_4444,
            0x5555_6666,
            0x0800_0000, // fits in 252 bits
        ];
        let mult: u32 = 42;

        let result = generate_memory_id_to_big_trace(&[value], &[mult]).unwrap();
        let expected_limbs = split_f252_reference(value);

        // The column_length should be 1 rounded up to next power of two = 1.
        assert_eq!(result.column_length(), 1);

        // Verify all 28 limb columns.
        for col in 0..FELT252_N_WORDS {
            let got = result.value(col, 0);
            assert_eq!(
                got.0, expected_limbs[col],
                "mismatch at limb column {col}: got {}, expected {}",
                got.0, expected_limbs[col]
            );
        }

        // Verify multiplicity column.
        assert_eq!(result.value(FELT252_N_WORDS, 0).0, mult);
    }

    #[test]
    fn test_big_trace_multiple_values_with_padding() {
        // 3 values => column_length = 4 (next power of two).
        let values: Vec<[u32; 8]> = vec![
            [1, 2, 3, 4, 5, 6, 7, 8],
            [0xFF, 0, 0, 0, 0, 0, 0, 0],
            [0, 0, 0, 0, 0, 0, 0, 0],
        ];
        let mults: Vec<u32> = vec![10, 20, 30];

        let result = generate_memory_id_to_big_trace(&values, &mults).unwrap();
        assert_eq!(result.column_length(), 4);

        // Verify each real row.
        for (row, value) in values.iter().enumerate() {
            let expected = split_f252_reference(*value);
            for col in 0..FELT252_N_WORDS {
                let got = result.value(col, row);
                assert_eq!(
                    got.0, expected[col],
                    "mismatch at row {row}, col {col}"
                );
            }
            assert_eq!(
                result.value(FELT252_N_WORDS, row).0,
                mults[row],
                "mismatch at multiplicity row {row}"
            );
        }

        // Padding row (row 3) should be all zeros.
        for col in 0..BIG_N_COLUMNS {
            assert_eq!(
                result.value(col, 3).0,
                0,
                "padding row 3, col {col} should be 0"
            );
        }
    }

    #[test]
    fn test_small_trace_single_value() {
        let value: u128 = 0x1234_5678_9ABC_DEF0_CAFE_BABE_DEAD_BEEFu128;
        let mult: u32 = 7;

        let result = generate_memory_id_to_big_small_trace(&[value], &[mult]).unwrap();
        assert_eq!(result.column_length(), 1);

        // Compute expected limbs using the same split logic as big values
        // but with only the lower 4 u32 words.
        let input_words: [u32; 8] = [
            value as u32,
            (value >> 32) as u32,
            (value >> 64) as u32,
            (value >> 96) as u32,
            0,
            0,
            0,
            0,
        ];
        let expected = split_f252_reference(input_words);

        for col in 0..N_M31_IN_SMALL_FELT252 {
            let got = result.value(col, 0);
            assert_eq!(
                got.0, expected[col],
                "mismatch at small limb column {col}"
            );
        }

        assert_eq!(result.value(N_M31_IN_SMALL_FELT252, 0).0, mult);
    }
}
