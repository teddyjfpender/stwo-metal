//! Generic GPU-accelerated interaction trace generation.
//!
//! This module provides a single data-driven Metal kernel that replaces ~25
//! component-specific CPU interaction trace generators. Instead of hard-coding
//! the column layout for each component, callers build a descriptor table
//! describing the logup column structure and a flat values buffer containing
//! all lookup data. The GPU kernel then processes all rows in parallel using
//! the descriptor table.
//!
//! # Numerator types
//!
//! Each logup column uses one of four numerator patterns:
//!
//! | Type | Name               | Formula                                                |
//! |------|--------------------|--------------------------------------------------------|
//! | 0    | `pair_sum`         | `numer = denom0 + denom1`                              |
//! | 1    | `enabler_weighted` | `numer = denom0 * enabler(row) + denom1`               |
//! | 2    | `mults_weighted`   | `numer = -(denom0 * weight1 + denom1 * weight0)`       |
//! | 3    | `single_neg`       | `numer = -enabler(row)`, single denominator only       |
//!
//! The `enabler(row)` is 1 for `row < n_rows_real`, 0 for padding rows.

use stwo::core::fields::m31::{BaseField, M31};
use stwo::core::fields::qm31::{SecureField, SECURE_EXTENSION_DEGREE};
use stwo::core::poly::circle::CanonicCoset;
use stwo::prover::backend::simd::m31::{PackedBaseField, N_LANES};
use stwo::prover::backend::simd::prefix_sum::inclusive_prefix_sum;
use stwo::prover::poly::circle::CircleEvaluation;
use stwo::prover::poly::BitReversedOrder;
use stwo_metal_sys::metal::U32Buffer;

use crate::backend::metal::MetalBackend;
use crate::stwo_metal::base_field_vec::BaseFieldVec;

use super::interaction_trace_id_to_big::{
    InteractionLookupElements, InteractionTraceError,
};

/// Numerator type constants matching the Metal kernel.
pub const NUMERATOR_PAIR_SUM: u32 = 0;
pub const NUMERATOR_ENABLER_WEIGHTED: u32 = 1;
pub const NUMERATOR_MULTS_WEIGHTED: u32 = 2;
pub const NUMERATOR_SINGLE_NEG: u32 = 3;

/// Descriptor for one logup column in the generic interaction trace kernel.
///
/// Must match the Metal-side `ColumnDescriptor` struct layout exactly.
/// Each field is a u32 (8 fields = 8 u32 = 32 bytes per descriptor).
#[derive(Clone, Debug)]
#[repr(C)]
pub struct ColumnDescriptor {
    /// Column index into the flat values buffer for the first combine's data.
    pub values0_offset: u32,
    /// Number of value columns for combine0 (excluding the relation_id which
    /// is prepended automatically).
    pub values0_count: u32,
    /// Column index into the flat values buffer for the second combine's data.
    /// Unused for `single_neg` type.
    pub values1_offset: u32,
    /// Number of value columns for combine1.
    pub values1_count: u32,
    /// Relation ID prepended as the first value in combine0.
    pub relation_id0: u32,
    /// Relation ID prepended as the first value in combine1.
    pub relation_id1: u32,
    /// Numerator type: 0=pair_sum, 1=enabler_weighted, 2=mults_weighted, 3=single_neg.
    pub numerator_type: u32,
    /// Column index into values buffer for the weight column(s).
    /// For `mults_weighted`: weight0 is at `weight_offset`, weight1 at `weight_offset+1`.
    /// Unused for other types.
    pub weight_offset: u32,
}

impl ColumnDescriptor {
    /// Serialize a descriptor to 8 u32 values matching the Metal struct layout.
    pub fn to_u32_array(&self) -> [u32; 8] {
        [
            self.values0_offset,
            self.values0_count,
            self.values1_offset,
            self.values1_count,
            self.relation_id0,
            self.relation_id1,
            self.numerator_type,
            self.weight_offset,
        ]
    }
}

/// Builder for constructing the flat values buffer and descriptor table
/// for the generic interaction trace kernel.
#[derive(Default)]
pub struct GenericInteractionTraceBuilder {
    /// Number of value columns accumulated so far.
    n_value_columns: u32,
    /// Column descriptors for each logup column.
    descriptors: Vec<ColumnDescriptor>,
    /// Flat values data: Vec of (column_data_u32) to be assembled into
    /// column-major layout [n_value_columns][n_rows].
    columns: Vec<Vec<u32>>,
}

impl GenericInteractionTraceBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a value column (n_rows u32 values in row-major order).
    /// Returns the column index that was assigned.
    pub fn add_column(&mut self, data: Vec<u32>) -> u32 {
        let idx = self.n_value_columns;
        self.n_value_columns += 1;
        self.columns.push(data);
        idx
    }

    /// Add a logup column descriptor.
    pub fn add_logup_column(&mut self, desc: ColumnDescriptor) {
        self.descriptors.push(desc);
    }

    /// Get the number of logup columns.
    pub fn n_logup_cols(&self) -> usize {
        self.descriptors.len()
    }

    /// Dispatch the generic kernel and return Metal-native evaluations.
    ///
    /// # Arguments
    /// * `n_rows` - Total number of rows (power of two, including padding).
    /// * `n_rows_real` - Actual number of rows (before padding).
    /// * `lookup_elements` - Alpha powers and z from the Fiat-Shamir channel.
    ///
    /// # Returns
    /// Tuple of (trace_evaluations, claimed_sum).
    pub fn dispatch_metal(
        self,
        n_rows: usize,
        n_rows_real: usize,
        lookup_elements: &InteractionLookupElements,
    ) -> Result<
        (
            Vec<CircleEvaluation<MetalBackend, BaseField, BitReversedOrder>>,
            SecureField,
        ),
        InteractionTraceError,
    > {
        let n_logup_cols = self.descriptors.len();
        assert!(n_logup_cols > 0, "must have at least one logup column");
        let log_size = n_rows.ilog2();

        // Determine max alpha powers needed.
        let max_combine_size = self.descriptors.iter().map(|d| {
            // combine uses relation_id (1) + values_count, so total = 1 + count.
            let s0 = 1 + d.values0_count;
            let s1 = if d.numerator_type != NUMERATOR_SINGLE_NEG {
                1 + d.values1_count
            } else {
                0
            };
            s0.max(s1)
        }).max().unwrap_or(1) as usize;

        assert!(
            lookup_elements.alpha_powers.len() >= max_combine_size,
            "Need at least {} alpha powers, got {}",
            max_combine_size,
            lookup_elements.alpha_powers.len()
        );

        // Build flat values buffer: column-major [n_value_columns][n_rows].
        let mut flat_values = vec![0u32; self.n_value_columns as usize * n_rows];
        for (col_idx, col_data) in self.columns.iter().enumerate() {
            assert_eq!(
                col_data.len(), n_rows,
                "Column {} has {} elements, expected {}",
                col_idx, col_data.len(), n_rows
            );
            let offset = col_idx * n_rows;
            flat_values[offset..offset + n_rows].copy_from_slice(col_data);
        }

        // Flatten descriptors: 8 u32 per descriptor.
        let mut flat_descriptors = Vec::with_capacity(n_logup_cols * 8);
        for desc in &self.descriptors {
            flat_descriptors.extend_from_slice(&desc.to_u32_array());
        }

        // Flatten alpha powers.
        let mut flat_alpha: Vec<u32> = Vec::with_capacity(max_combine_size * 4);
        for ap in &lookup_elements.alpha_powers[..max_combine_size] {
            flat_alpha.extend_from_slice(ap);
        }

        // Upload to GPU.
        let values_buf = U32Buffer::from_slice(&flat_values)?;
        let desc_buf = U32Buffer::from_slice(&flat_descriptors)?;
        let alpha_buf = U32Buffer::from_slice(&flat_alpha)?;
        let z_buf = U32Buffer::from_slice(&lookup_elements.z)?;

        // Dispatch.
        let trace_buf = U32Buffer::interaction_trace_generic(
            &values_buf,
            &desc_buf,
            &alpha_buf,
            &z_buf,
            n_rows as u32,
            n_logup_cols as u32,
            n_rows_real as u32,
        )?;

        // Parse output into CircleEvaluations.
        let domain = CanonicCoset::new(log_size).circle_domain();
        let mut all_evals: Vec<CircleEvaluation<MetalBackend, BaseField, BitReversedOrder>> =
            Vec::new();

        // Non-last columns: zero-copy from GPU buffer.
        for logup_col in 0..(n_logup_cols - 1) {
            let base_m31_col = logup_col * 4;
            for coord in 0..SECURE_EXTENSION_DEGREE {
                let col_idx = base_m31_col + coord;
                let offset = col_idx * n_rows;
                let col_buf = trace_buf.clone_range(offset, n_rows)?;
                all_evals.push(CircleEvaluation::new(
                    domain,
                    BaseFieldVec::from_buffer(col_buf),
                ));
            }
        }

        // Last column: read back for prefix-sum finalization.
        {
            let last_logup = n_logup_cols - 1;
            let base_m31_col = last_logup * 4;
            let packed_len = n_rows / N_LANES;

            let mut coord_columns: [stwo::prover::backend::simd::column::BaseColumn;
                SECURE_EXTENSION_DEGREE] = std::array::from_fn(|coord| {
                let col_idx = base_m31_col + coord;
                let gpu_offset = col_idx * n_rows;
                let col_raw = trace_buf
                    .clone_range(gpu_offset, n_rows)
                    .expect("clone_range for prefix-sum column")
                    .to_vec()
                    .expect("to_vec for prefix-sum column");
                let mut col_data = Vec::with_capacity(packed_len);
                for vec_row in 0..packed_len {
                    let mut arr = [M31(0); N_LANES];
                    for lane in 0..N_LANES {
                        arr[lane] = M31(col_raw[vec_row * N_LANES + lane]);
                    }
                    col_data.push(PackedBaseField::from_array(arr));
                }
                stwo::prover::backend::simd::column::BaseColumn::from_simd(col_data)
            });

            // Compute cumsum_shift = sum_of_all_elements / domain_size.
            let coordinate_sums: [BaseField; SECURE_EXTENSION_DEGREE] =
                std::array::from_fn(|i| {
                    coord_columns[i]
                        .data
                        .iter()
                        .copied()
                        .sum::<PackedBaseField>()
                        .pointwise_sum()
                });
            let claimed_sum = SecureField::from_m31_array(coordinate_sums);
            let cumsum_shift =
                claimed_sum / BaseField::from_u32_unchecked(1 << log_size);
            let packed_cumsum_shift =
                stwo::prover::backend::simd::qm31::PackedSecureField::broadcast(cumsum_shift);

            // Subtract cumsum_shift from each element.
            for (i, col) in coord_columns.iter_mut().enumerate() {
                for x in col.data.iter_mut() {
                    *x -= packed_cumsum_shift.into_packed_m31s()[i];
                }
            }

            // Inclusive prefix sum on each coordinate.
            let coord_prefix_sum = coord_columns.map(inclusive_prefix_sum);

            // Re-upload prefix-summed columns to Metal.
            for col in coord_prefix_sum {
                let raw: Vec<u32> = col.data.iter()
                    .flat_map(|packed| packed.to_array().map(|v| v.0))
                    .collect();
                let col_buf = U32Buffer::from_slice(&raw)?;
                all_evals.push(CircleEvaluation::new(
                    domain,
                    BaseFieldVec::from_buffer(col_buf),
                ));
            }

            return Ok((all_evals, claimed_sum));
        }
    }
}

/// Flatten a Vec<[PackedM31; N]> into a Vec<u32> of n_rows elements.
///
/// Each `[PackedM31; N]` row contains N values, but we need column-major
/// layout where each column is a separate flat array. This function extracts
/// a single column `col_idx` from the packed lookup data.
pub fn flatten_packed_column<const N: usize>(
    data: &[[stwo::prover::backend::simd::m31::PackedM31; N]],
    col_idx: usize,
) -> Vec<u32> {
    let n_packed = data.len();
    let n_rows = n_packed * N_LANES;
    let mut result = vec![0u32; n_rows];
    for (vec_row, packed_row) in data.iter().enumerate() {
        let vals = packed_row[col_idx].to_array();
        for (lane, &v) in vals.iter().enumerate() {
            result[vec_row * N_LANES + lane] = v.0;
        }
    }
    result
}

/// Extract `InteractionLookupElements` from QM31 z and alpha_powers.
///
/// Re-exported from the id_to_big module for convenience.
pub use super::interaction_trace_id_to_big::extract_lookup_elements_for_gpu;

/// Generate the interaction trace for an opcode-style component using the
/// generic GPU kernel.
///
/// This function is the primary entry point for GPU-accelerating opcode
/// interaction traces. It takes:
/// - The component's `LookupData` decomposed into column arrays
/// - Lookup elements (alpha powers and z)
/// - A descriptor table describing the logup column layout
///
/// # Arguments
/// * `builder` - A fully-configured `GenericInteractionTraceBuilder` with
///               all value columns and logup column descriptors added.
/// * `n_rows`  - Total number of rows (power of two).
/// * `n_rows_real` - Actual row count before padding.
/// * `lookup_elements` - QM31 alpha powers and z.
///
/// # Returns
/// Tuple of (trace_evaluations, claimed_sum).
pub fn gpu_gen_generic_interaction_trace(
    builder: GenericInteractionTraceBuilder,
    n_rows: usize,
    n_rows_real: usize,
    lookup_elements: &InteractionLookupElements,
) -> Result<
    (
        Vec<CircleEvaluation<MetalBackend, BaseField, BitReversedOrder>>,
        SecureField,
    ),
    InteractionTraceError,
> {
    builder.dispatch_metal(n_rows, n_rows_real, lookup_elements)
}

/// Helper: build a generic interaction trace for a component that follows
/// the typical opcode pattern (pairs of combine lookups + final single_neg).
///
/// This covers components like ret_opcode, add_opcode_small, etc. that have:
/// - N-1 paired logup columns (pair_sum or enabler_weighted)
/// - 1 final single_neg column
///
/// The caller provides the LookupData columns and a configuration describing
/// each logup column's layout.
pub struct LogupColumnConfig {
    /// Relation ID for the first combine in this logup column.
    pub relation_id0: u32,
    /// Column indices and count for values0.
    pub values0_col_indices: Vec<usize>,
    /// Relation ID for the second combine (ignored for single_neg).
    pub relation_id1: u32,
    /// Column indices and count for values1 (ignored for single_neg).
    pub values1_col_indices: Vec<usize>,
    /// Numerator type.
    pub numerator_type: u32,
    /// Weight column indices for mults_weighted (2 cols) or ignored.
    pub weight_col_indices: Vec<usize>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use stwo::core::channel::{Blake2sChannel, Channel};
    use stwo::core::fields::m31::M31;
    use stwo::core::fields::qm31::QM31;
    #[allow(unused_imports)]
    use stwo::prover::backend::simd::m31::PackedM31;

    #[allow(unused_imports)]
    use stwo::core::fields::FieldExpOps;

    /// Build dummy lookup elements for testing.
    fn make_test_lookup_elements(n_powers: usize) -> InteractionLookupElements {
        let mut channel = Blake2sChannel::default();
        let felts = channel.draw_secure_felts(2);
        let z = felts[0];
        let alpha = felts[1];

        let mut cur = QM31::from_u32_unchecked(1, 0, 0, 0);
        let mut powers = Vec::new();
        for _ in 0..n_powers {
            powers.push(cur);
            cur *= alpha;
        }
        extract_lookup_elements_for_gpu(z, &powers)
    }

    /// CPU reference: compute combine(values) = sum(alpha_powers[i] * values[i]) - z
    fn cpu_combine(alpha_powers: &[QM31], values: &[M31], z: QM31) -> QM31 {
        let mut acc = QM31::from_u32_unchecked(0, 0, 0, 0);
        for (i, &v) in values.iter().enumerate() {
            acc += alpha_powers[i] * v;
        }
        acc - z
    }

    fn reconstruct_qm31(lookup: &InteractionLookupElements) -> (QM31, Vec<QM31>) {
        let z = QM31::from_m31_array([
            M31(lookup.z[0]),
            M31(lookup.z[1]),
            M31(lookup.z[2]),
            M31(lookup.z[3]),
        ]);
        let powers: Vec<QM31> = lookup
            .alpha_powers
            .iter()
            .map(|ap| {
                QM31::from_m31_array([M31(ap[0]), M31(ap[1]), M31(ap[2]), M31(ap[3])])
            })
            .collect();
        (z, powers)
    }

    #[test]
    #[allow(unused_variables)]
    fn test_generic_pair_sum() {
        // Test pair_sum numerator type:
        // Two logup columns, each with pair_sum, then verify against CPU.
        let n_rows = 16usize;
        let lookup = make_test_lookup_elements(8);

        let rel_id_a = 12345u32;
        let rel_id_b = 67890u32;

        // Build value columns: 4 columns, 3 values per combine.
        // col 0,1 -> combine0 for logup col 0
        // col 2,3 -> combine1 for logup col 0
        let col0: Vec<u32> = (0..n_rows).map(|r| (r as u32 * 7 + 3) % 512).collect();
        let col1: Vec<u32> = (0..n_rows).map(|r| (r as u32 * 11 + 5) % 512).collect();
        let col2: Vec<u32> = (0..n_rows).map(|r| (r as u32 * 13 + 7) % 512).collect();
        let col3: Vec<u32> = (0..n_rows).map(|r| (r as u32 * 17 + 11) % 512).collect();

        let mut builder = GenericInteractionTraceBuilder::new();
        let c0 = builder.add_column(col0.clone());
        let _c1 = builder.add_column(col1.clone());
        let c2 = builder.add_column(col2.clone());
        let _c3 = builder.add_column(col3.clone());

        builder.add_logup_column(ColumnDescriptor {
            values0_offset: c0,
            values0_count: 2,
            values1_offset: c2,
            values1_count: 2,
            relation_id0: rel_id_a,
            relation_id1: rel_id_b,
            numerator_type: NUMERATOR_PAIR_SUM,
            weight_offset: 0,
        });

        let (evals, _claimed_sum) = builder
            .dispatch_metal(n_rows, n_rows, &lookup)
            .expect("GPU dispatch failed");

        // CPU reference for the first (and only non-prefix-summed) column:
        // Actually there's only 1 logup col, so the output goes through prefix-sum.
        // With 1 logup col, the output is the prefix-summed last column.
        // We can verify the claimed_sum instead.

        // To properly test, let's add a second logup column so the first one
        // is not prefix-summed.
        // For this minimal test, just verify it runs and produces non-zero output.
        assert_eq!(evals.len(), 4); // 1 logup col * 4 coords
        assert_ne!(_claimed_sum, SecureField::from_u32_unchecked(0, 0, 0, 0));
    }

    #[test]
    fn test_generic_two_columns_pair_sum_correctness() {
        let n_rows = 16usize;
        let lookup = make_test_lookup_elements(4);
        let (z, powers) = reconstruct_qm31(&lookup);

        let rel_id_a = 100u32;
        let rel_id_b = 200u32;
        let rel_id_c = 300u32;
        let rel_id_d = 400u32;

        let col0: Vec<u32> = (0..n_rows).map(|r| (r as u32 * 7 + 3) % 512).collect();
        let col1: Vec<u32> = (0..n_rows).map(|r| (r as u32 * 11 + 5) % 512).collect();
        let col2: Vec<u32> = (0..n_rows).map(|r| (r as u32 * 13 + 7) % 512).collect();
        let col3: Vec<u32> = (0..n_rows).map(|r| (r as u32 * 17 + 11) % 512).collect();

        let mut builder = GenericInteractionTraceBuilder::new();
        let c0 = builder.add_column(col0.clone());
        let c1 = builder.add_column(col1.clone());
        let c2 = builder.add_column(col2.clone());
        let c3 = builder.add_column(col3.clone());

        // Two pair_sum logup columns.
        builder.add_logup_column(ColumnDescriptor {
            values0_offset: c0,
            values0_count: 1, // 1 value column after relation_id
            values1_offset: c1,
            values1_count: 1,
            relation_id0: rel_id_a,
            relation_id1: rel_id_b,
            numerator_type: NUMERATOR_PAIR_SUM,
            weight_offset: 0,
        });
        builder.add_logup_column(ColumnDescriptor {
            values0_offset: c2,
            values0_count: 1,
            values1_offset: c3,
            values1_count: 1,
            relation_id0: rel_id_c,
            relation_id1: rel_id_d,
            numerator_type: NUMERATOR_PAIR_SUM,
            weight_offset: 0,
        });

        let (evals, _claimed_sum) = builder
            .dispatch_metal(n_rows, n_rows, &lookup)
            .expect("GPU dispatch failed");

        // 2 logup cols * 4 coords = 8 M31 columns
        assert_eq!(evals.len(), 8);

        // Check column 0 (not prefix-summed) against CPU reference.
        for row in 0..n_rows {
            // denom0 = combine([rel_id_a, col0[row]]) = alpha^0 * rel_id_a + alpha^1 * col0[row] - z
            let d0 = cpu_combine(&powers, &[M31(rel_id_a), M31(col0[row])], z);
            let d1 = cpu_combine(&powers, &[M31(rel_id_b), M31(col1[row])], z);
            let numer = d0 + d1;
            let denom = d0 * d1;
            let frac = numer * denom.inverse();

            let [ea, eb, ec, ed] = frac.to_m31_array();
            use stwo::prover::backend::Column;
            assert_eq!(evals[0].values.at(row), ea, "row {row} coord 0");
            assert_eq!(evals[1].values.at(row), eb, "row {row} coord 1");
            assert_eq!(evals[2].values.at(row), ec, "row {row} coord 2");
            assert_eq!(evals[3].values.at(row), ed, "row {row} coord 3");
        }
    }

    #[test]
    fn test_generic_single_neg() {
        let n_rows = 16usize;
        let n_rows_real = 10usize; // partial enabler
        let lookup = make_test_lookup_elements(4);
        let (z, powers) = reconstruct_qm31(&lookup);

        let rel_id = 999u32;
        let col0: Vec<u32> = (0..n_rows).map(|r| (r as u32 * 3 + 1) % 512).collect();
        let col1: Vec<u32> = (0..n_rows).map(|r| (r as u32 * 5 + 2) % 512).collect();

        // To get a non-prefix-summed column, we need at least 2 logup cols.
        // Col 0: pair_sum, Col 1: single_neg.
        let mut builder = GenericInteractionTraceBuilder::new();
        let c0 = builder.add_column(col0.clone());
        let c1 = builder.add_column(col1.clone());

        builder.add_logup_column(ColumnDescriptor {
            values0_offset: c0,
            values0_count: 1,
            values1_offset: c1,
            values1_count: 1,
            relation_id0: rel_id,
            relation_id1: rel_id,
            numerator_type: NUMERATOR_PAIR_SUM,
            weight_offset: 0,
        });
        builder.add_logup_column(ColumnDescriptor {
            values0_offset: c0,
            values0_count: 2, // relation_id + col0 + col1
            values1_offset: 0,
            values1_count: 0,
            relation_id0: rel_id,
            relation_id1: 0,
            numerator_type: NUMERATOR_SINGLE_NEG,
            weight_offset: 0,
        });

        let (evals, _claimed_sum) = builder
            .dispatch_metal(n_rows, n_rows_real, &lookup)
            .expect("GPU dispatch failed");

        assert_eq!(evals.len(), 8); // 2 logup * 4

        // Verify first logup column (pair_sum, not prefix-summed).
        for row in 0..n_rows {
            let d0 = cpu_combine(&powers, &[M31(rel_id), M31(col0[row])], z);
            let d1 = cpu_combine(&powers, &[M31(rel_id), M31(col1[row])], z);
            let numer = d0 + d1;
            let denom = d0 * d1;
            let frac = numer * denom.inverse();

            let [ea, eb, ec, ed] = frac.to_m31_array();
            use stwo::prover::backend::Column;
            assert_eq!(evals[0].values.at(row), ea, "pair_sum row {row} coord 0");
            assert_eq!(evals[1].values.at(row), eb, "pair_sum row {row} coord 1");
            assert_eq!(evals[2].values.at(row), ec, "pair_sum row {row} coord 2");
            assert_eq!(evals[3].values.at(row), ed, "pair_sum row {row} coord 3");
        }
    }

    #[test]
    fn test_generic_enabler_weighted() {
        let n_rows = 16usize;
        let n_rows_real = 12usize;
        let lookup = make_test_lookup_elements(4);
        let (z, powers) = reconstruct_qm31(&lookup);

        let rel_id_a = 111u32;
        let rel_id_b = 222u32;

        let col0: Vec<u32> = (0..n_rows).map(|r| (r as u32 + 10) % 512).collect();
        let col1: Vec<u32> = (0..n_rows).map(|r| (r as u32 + 20) % 512).collect();

        let mut builder = GenericInteractionTraceBuilder::new();
        let c0 = builder.add_column(col0.clone());
        let c1 = builder.add_column(col1.clone());

        // 2 logup cols: enabler_weighted + pair_sum (need 2 to get non-prefix-summed col 0)
        builder.add_logup_column(ColumnDescriptor {
            values0_offset: c0,
            values0_count: 1,
            values1_offset: c1,
            values1_count: 1,
            relation_id0: rel_id_a,
            relation_id1: rel_id_b,
            numerator_type: NUMERATOR_ENABLER_WEIGHTED,
            weight_offset: 0,
        });
        builder.add_logup_column(ColumnDescriptor {
            values0_offset: c0,
            values0_count: 1,
            values1_offset: c1,
            values1_count: 1,
            relation_id0: rel_id_a,
            relation_id1: rel_id_b,
            numerator_type: NUMERATOR_PAIR_SUM,
            weight_offset: 0,
        });

        let (evals, _claimed_sum) = builder
            .dispatch_metal(n_rows, n_rows_real, &lookup)
            .expect("GPU dispatch failed");

        assert_eq!(evals.len(), 8);

        // Check the enabler_weighted column (col 0, not prefix-summed).
        for row in 0..n_rows {
            let d0 = cpu_combine(&powers, &[M31(rel_id_a), M31(col0[row])], z);
            let d1 = cpu_combine(&powers, &[M31(rel_id_b), M31(col1[row])], z);
            let enabler = if row < n_rows_real { M31(1) } else { M31(0) };
            // numer = d0 * enabler + d1
            let numer = d0 * QM31::from(enabler) + d1;
            let denom = d0 * d1;
            let frac = numer * denom.inverse();

            let [ea, eb, ec, ed] = frac.to_m31_array();
            use stwo::prover::backend::Column;
            assert_eq!(evals[0].values.at(row), ea, "row {row} coord 0");
            assert_eq!(evals[1].values.at(row), eb, "row {row} coord 1");
            assert_eq!(evals[2].values.at(row), ec, "row {row} coord 2");
            assert_eq!(evals[3].values.at(row), ed, "row {row} coord 3");
        }
    }

    #[test]
    fn test_generic_mults_weighted() {
        let n_rows = 16usize;
        let lookup = make_test_lookup_elements(4);
        let (z, powers) = reconstruct_qm31(&lookup);

        let rel_id = 555u32;

        let col0: Vec<u32> = (0..n_rows).map(|r| (r as u32 + 1) % 512).collect();
        let col1: Vec<u32> = (0..n_rows).map(|r| (r as u32 + 100) % 512).collect();
        let wt0: Vec<u32> = (0..n_rows).map(|r| (r as u32 * 3 + 1) % 100).collect();
        let wt1: Vec<u32> = (0..n_rows).map(|r| (r as u32 * 5 + 2) % 100).collect();

        let mut builder = GenericInteractionTraceBuilder::new();
        let c0 = builder.add_column(col0.clone());
        let c1 = builder.add_column(col1.clone());
        let w0 = builder.add_column(wt0.clone());
        let _w1 = builder.add_column(wt1.clone());

        // 2 logup cols to get non-prefix-summed first col
        builder.add_logup_column(ColumnDescriptor {
            values0_offset: c0,
            values0_count: 1,
            values1_offset: c1,
            values1_count: 1,
            relation_id0: rel_id,
            relation_id1: rel_id,
            numerator_type: NUMERATOR_MULTS_WEIGHTED,
            weight_offset: w0, // weight0 at w0, weight1 at w0+1
        });
        builder.add_logup_column(ColumnDescriptor {
            values0_offset: c0,
            values0_count: 1,
            values1_offset: c1,
            values1_count: 1,
            relation_id0: rel_id,
            relation_id1: rel_id,
            numerator_type: NUMERATOR_PAIR_SUM,
            weight_offset: 0,
        });

        let (evals, _claimed_sum) = builder
            .dispatch_metal(n_rows, n_rows, &lookup)
            .expect("GPU dispatch failed");

        assert_eq!(evals.len(), 8);

        // Check mults_weighted column.
        for row in 0..n_rows {
            let d0 = cpu_combine(&powers, &[M31(rel_id), M31(col0[row])], z);
            let d1 = cpu_combine(&powers, &[M31(rel_id), M31(col1[row])], z);
            // numer = -(d0 * weight1 + d1 * weight0) = d0 * (-weight1) + d1 * (-weight0)
            let neg_w0 = -M31(wt0[row]);
            let neg_w1 = -M31(wt1[row]);
            let numer = d0 * QM31::from(neg_w1) + d1 * QM31::from(neg_w0);
            let denom = d0 * d1;
            let frac = numer * denom.inverse();

            let [ea, eb, ec, ed] = frac.to_m31_array();
            use stwo::prover::backend::Column;
            assert_eq!(evals[0].values.at(row), ea, "row {row} coord 0");
            assert_eq!(evals[1].values.at(row), eb, "row {row} coord 1");
            assert_eq!(evals[2].values.at(row), ec, "row {row} coord 2");
            assert_eq!(evals[3].values.at(row), ed, "row {row} coord 3");
        }
    }
}
