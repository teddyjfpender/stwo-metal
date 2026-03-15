//! GPU-accelerated witness trace generation for all range-check components.
//!
//! This module provides a Metal-backed version of the range-check trace
//! generators from stwo-cairo. All range-check components share the same
//! pattern: the trace columns are direct copies of multiplicity counters.
//!
//! Supported components:
//! - `range_check_9_9` (8 columns)
//! - `range_check_7_2_5` (1 column)
//! - `range_check_4_3` (1 column)
//! - `range_check_4_4` (1 column)
//! - `range_check_6` (1 column)
//! - `range_check_8` (1 column)
//! - `range_check_11` (1 column)
//! - `range_check_12` (1 column)
//! - `range_check_18` (2 columns)
//! - `range_check_20` (8 columns)
//! - `range_check_3_6_6_3` (1 column)
//! - `range_check_3_3_3_3_3` (1 column)
//! - `range_check_4_4_4_4` (1 column)
//!
//! # Trace layout
//!
//! Each range-check component has `N_TRACE_COLUMNS` columns, one per
//! multiplicity sub-relation. The trace is column-major:
//! `[mult_0[0..col_len], mult_1[0..col_len], ..., mult_N[0..col_len]]`
//!
//! This mirrors the CPU trace generation where `row[i] = mults[i][row_index]`.

use stwo::core::fields::m31::BaseField;
use stwo::core::poly::circle::CanonicCoset;
use stwo::prover::backend::CpuBackend;
use stwo::prover::poly::circle::CircleEvaluation;
use stwo::prover::poly::BitReversedOrder;
use stwo_metal_sys::metal::{MetalError, U32Buffer};

use super::MetalBackend;
use crate::stwo_metal::base_field_vec::BaseFieldVec;

/// Error type for range-check witness trace generation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RangeCheckTraceError {
    /// The column length is not a power of two.
    ColumnLengthNotPowerOfTwo { column_length: usize },
    /// An error occurred in the Metal runtime.
    Runtime { message: String },
}

impl core::fmt::Display for RangeCheckTraceError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::ColumnLengthNotPowerOfTwo { column_length } => {
                write!(
                    f,
                    "range_check trace requires a power-of-two column length, got {column_length}"
                )
            }
            Self::Runtime { message } => f.write_str(message),
        }
    }
}

impl std::error::Error for RangeCheckTraceError {}

impl From<MetalError> for RangeCheckTraceError {
    fn from(value: MetalError) -> Self {
        Self::Runtime {
            message: value.message().to_string(),
        }
    }
}

/// Result of Metal GPU trace generation for a range-check component.
///
/// Holds the raw GPU buffer and provides methods for extracting column values
/// or converting to `CircleEvaluation`s.
#[derive(Debug)]
pub struct MetalRangeCheckTrace {
    values: U32Buffer,
    n_columns: usize,
    column_length: usize,
}

impl MetalRangeCheckTrace {
    /// Number of trace columns (= number of multiplicity sub-relations).
    pub fn n_columns(&self) -> usize {
        self.n_columns
    }

    /// The power-of-two column length (number of rows).
    pub fn column_length(&self) -> usize {
        self.column_length
    }

    /// Read a single cell from the trace.
    pub fn value(&self, column_index: usize, row_index: usize) -> BaseField {
        assert!(column_index < self.n_columns, "column out of bounds");
        assert!(row_index < self.column_length, "row out of bounds");
        BaseField::from_u32_unchecked(
            self.values.get(column_index * self.column_length + row_index),
        )
    }

    /// Extract all values for a single column.
    pub fn column_values(&self, column_index: usize) -> Vec<BaseField> {
        assert!(column_index < self.n_columns, "column out of bounds");
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
        (0..self.n_columns)
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
        (0..self.n_columns)
            .map(|col| {
                let start = col * self.column_length;
                let col_buf = self
                    .values
                    .clone_range(start, self.column_length)
                    .expect("Metal range-check trace column slicing should succeed");
                CircleEvaluation::<MetalBackend, BaseField, BitReversedOrder>::new(
                    domain,
                    BaseFieldVec::from_buffer(col_buf),
                )
            })
            .collect()
    }
}

/// Generate a range-check witness trace on the Metal GPU.
///
/// This is the GPU equivalent of range-check `ClaimGenerator::write_trace()`
/// from stwo-cairo. It copies multiplicity columns into the trace output.
///
/// # Arguments
///
/// * `multiplicities` - One flat array of multiplicities in column-major order:
///   `[mult_col_0[0..column_length], mult_col_1[0..column_length], ...]`.
///   Length must be `n_columns * column_length`.
/// * `n_columns` - Number of multiplicity / trace columns (= N_TRACE_COLUMNS for the component).
/// * `column_length` - Number of rows per column (must be a power of two, = 1 << LOG_SIZE).
///
/// # Returns
///
/// A `MetalRangeCheckTrace` with `n_columns` columns of `column_length` rows.
pub fn generate_range_check_trace(
    multiplicities: &[u32],
    n_columns: u32,
    column_length: u32,
) -> Result<MetalRangeCheckTrace, RangeCheckTraceError> {
    assert!(n_columns > 0, "n_columns must be > 0");
    if !column_length.is_power_of_two() {
        return Err(RangeCheckTraceError::ColumnLengthNotPowerOfTwo {
            column_length: column_length as usize,
        });
    }

    let expected_len = n_columns as usize * column_length as usize;
    assert_eq!(
        multiplicities.len(),
        expected_len,
        "multiplicities length must be n_columns * column_length"
    );

    let mults_buf = U32Buffer::from_slice(multiplicities)?;
    let trace_buf = U32Buffer::witness_range_check_trace(&mults_buf, n_columns, column_length)?;

    Ok(MetalRangeCheckTrace {
        values: trace_buf,
        n_columns: n_columns as usize,
        column_length: column_length as usize,
    })
}

#[cfg(test)]
mod tests {
    use stwo::prover::backend::Column;

    use super::*;

    #[test]
    fn test_single_column_range_check() {
        // Simulate range_check_4_3 (1 column, LOG_SIZE=7 -> 128 rows).
        let column_length = 128u32;
        let n_columns = 1u32;
        let mults: Vec<u32> = (0..column_length).map(|i| i * 3 + 1).collect();

        let result = generate_range_check_trace(&mults, n_columns, column_length).unwrap();
        assert_eq!(result.n_columns(), 1);
        assert_eq!(result.column_length(), 128);

        for row in 0..column_length as usize {
            assert_eq!(
                result.value(0, row).0,
                mults[row],
                "mismatch at row {row}"
            );
        }
    }

    #[test]
    fn test_multi_column_range_check() {
        // Simulate range_check_9_9 (8 columns, LOG_SIZE=18 -> use 256 for test).
        let column_length = 256u32;
        let n_columns = 8u32;
        let mults: Vec<u32> = (0..n_columns * column_length)
            .map(|i| (i * 7 + 13) % 1000)
            .collect();

        let result = generate_range_check_trace(&mults, n_columns, column_length).unwrap();
        assert_eq!(result.n_columns(), 8);
        assert_eq!(result.column_length(), 256);

        for col in 0..n_columns as usize {
            for row in 0..column_length as usize {
                let expected = mults[col * column_length as usize + row];
                assert_eq!(
                    result.value(col, row).0,
                    expected,
                    "mismatch at col {col}, row {row}"
                );
            }
        }
    }

    #[test]
    fn test_two_column_range_check() {
        // Simulate range_check_18 (2 columns).
        let column_length = 64u32;
        let n_columns = 2u32;
        let mults: Vec<u32> = (0..n_columns * column_length).map(|i| i + 100).collect();

        let result = generate_range_check_trace(&mults, n_columns, column_length).unwrap();
        assert_eq!(result.n_columns(), 2);

        for col in 0..n_columns as usize {
            for row in 0..column_length as usize {
                let expected = mults[col * column_length as usize + row];
                assert_eq!(
                    result.value(col, row).0,
                    expected,
                    "mismatch at col {col}, row {row}"
                );
            }
        }
    }

    #[test]
    fn test_to_metal_evaluations() {
        let column_length = 64u32;
        let n_columns = 2u32;
        let mults: Vec<u32> = (0..n_columns * column_length).map(|i| i % 50).collect();

        let result = generate_range_check_trace(&mults, n_columns, column_length).unwrap();

        let metal_evals = result.to_metal_evaluations();
        assert_eq!(metal_evals.len(), 2);

        let cpu_evals = result.to_cpu_evaluations();
        assert_eq!(cpu_evals.len(), metal_evals.len());
        for (col, (cpu, metal)) in cpu_evals.iter().zip(metal_evals.iter()).enumerate() {
            assert_eq!(cpu.domain, metal.domain, "domain mismatch at col {col}");
            let metal_cpu: Vec<BaseField> = metal.values.to_cpu();
            assert_eq!(cpu.values, metal_cpu, "value mismatch at col {col}");
        }
    }

    #[test]
    fn test_zeros_passthrough() {
        // All-zero mults should produce all-zero trace.
        let column_length = 32u32;
        let n_columns = 4u32;
        let mults = vec![0u32; (n_columns * column_length) as usize];

        let result = generate_range_check_trace(&mults, n_columns, column_length).unwrap();

        for col in 0..n_columns as usize {
            for row in 0..column_length as usize {
                assert_eq!(result.value(col, row).0, 0, "expected 0 at col {col}, row {row}");
            }
        }
    }
}
