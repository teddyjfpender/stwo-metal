//! GPU-accelerated interaction trace generation for the `memory_id_to_big` component.
//!
//! This module provides Metal-backed logup interaction trace generation,
//! replacing the CPU-bound `InteractionClaimGenerator::write_interaction_trace`
//! from stwo-cairo.
//!
//! # Architecture
//!
//! The GPU kernel computes all per-row logup fractions in parallel:
//! - 7 range-check 9-9 columns (big) or 2 (small)
//! - 1 yield column (memory_id_to_big lookup)
//!
//! For each row, it computes `combine()`, per-element QM31 inverse, and
//! accumulates running sums across columns. The CPU then finalizes the last
//! column with a prefix-sum + cumsum_shift.
//!
//! # Output layout
//!
//! **Big variant**: 8 QM31 columns = 32 M31 columns (column-major).
//! **Small variant**: 3 QM31 columns = 12 M31 columns (column-major).

use stwo::core::fields::m31::{BaseField, M31};
use stwo::core::fields::qm31::{SecureField, QM31, SECURE_EXTENSION_DEGREE};
use stwo::core::poly::circle::CanonicCoset;
use stwo::prover::backend::simd::m31::{PackedBaseField, N_LANES};
use stwo::prover::backend::simd::prefix_sum::inclusive_prefix_sum;
use stwo::prover::backend::simd::SimdBackend;
use stwo::prover::poly::circle::CircleEvaluation;
use stwo::prover::poly::BitReversedOrder;
use stwo_metal_sys::metal::{MetalError, U32Buffer};

/// Relation IDs used for the range-check 9-9 lookups and the memory_id_to_big
/// yield lookup. Must match cairo_air::relations constants.
pub struct InteractionRelationIds {
    /// RANGE_CHECK_9_9, _B, _C, _D, _E, _F, _G, _H relation IDs.
    pub range_check_9_9_ids: [u32; 8],
    /// MEMORY_ID_TO_BIG_RELATION_ID.
    pub memory_id_to_big_id: u32,
}

/// QM31 lookup elements (alpha_powers and z) needed for `combine()`.
pub struct InteractionLookupElements {
    /// Pre-computed alpha powers: alpha^0, alpha^1, ..., alpha^(N-1) as QM31.
    /// For big: need at least 30 powers (1 relation_id + 1 id + 28 limbs).
    /// For small: need at least 10 powers (1 relation_id + 1 id + 8 limbs).
    pub alpha_powers: Vec<[u32; 4]>,
    /// The z element (subtracted in combine).
    pub z: [u32; 4],
}

/// Error type for interaction trace generation.
#[derive(Clone, Debug)]
pub enum InteractionTraceError {
    Runtime { message: String },
}

impl core::fmt::Display for InteractionTraceError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Runtime { message } => f.write_str(message),
        }
    }
}

impl std::error::Error for InteractionTraceError {}

impl From<MetalError> for InteractionTraceError {
    fn from(value: MetalError) -> Self {
        Self::Runtime {
            message: value.message().to_string(),
        }
    }
}

/// Generate the interaction trace for one memory_id_to_big "big" chunk on the GPU.
///
/// # Arguments
/// * `limb_columns` - 28 column vectors of PackedM31 limb values (the base trace values).
/// * `multiplicities` - Packed multiplicities for this chunk.
/// * `lookup_elements` - QM31 alpha_powers and z from the Fiat-Shamir channel.
/// * `relation_ids` - Relation IDs for range-check and memory_id_to_big.
/// * `id_offset` - Offset for the yield column id (e.g., chunk_index * chunk_size).
/// * `large_id_base` - LARGE_MEMORY_VALUE_ID_BASE, OR'd into the id.
///
/// # Returns
/// Tuple of (trace_evaluations, claimed_sum).
/// The trace has 8*4 = 32 M31 columns formatted as CircleEvaluations.
pub fn gpu_gen_big_memory_interaction_trace(
    limb_columns: &[Vec<PackedBaseField>; 28],
    multiplicities: &[PackedBaseField],
    lookup_elements: &InteractionLookupElements,
    relation_ids: &InteractionRelationIds,
    id_offset: u32,
    large_id_base: u32,
) -> Result<
    (
        Vec<CircleEvaluation<SimdBackend, BaseField, BitReversedOrder>>,
        SecureField,
    ),
    InteractionTraceError,
> {
    let packed_len = limb_columns[0].len();
    let n_rows = packed_len * N_LANES;
    let log_size = n_rows.ilog2();

    // Prepare GPU input: limb columns in column-major layout [28][n_rows].
    let mut flat_limbs = vec![0u32; 28 * n_rows];
    for (col, limb_col) in limb_columns.iter().enumerate() {
        for (vec_row, packed) in limb_col.iter().enumerate() {
            let vals = packed.to_array();
            for (lane, &v) in vals.iter().enumerate() {
                flat_limbs[col * n_rows + vec_row * N_LANES + lane] = v.0;
            }
        }
    }

    // Prepare multiplicities: flat [n_rows].
    let mut flat_mults = vec![0u32; n_rows];
    for (vec_row, packed) in multiplicities.iter().enumerate() {
        let vals = packed.to_array();
        for (lane, &v) in vals.iter().enumerate() {
            flat_mults[vec_row * N_LANES + lane] = v.0;
        }
    }

    // Flatten alpha_powers: [30][4] -> [120] u32.
    assert!(
        lookup_elements.alpha_powers.len() >= 30,
        "Need at least 30 alpha powers for big interaction trace"
    );
    let mut flat_alpha: Vec<u32> = Vec::with_capacity(120);
    for ap in &lookup_elements.alpha_powers[..30] {
        flat_alpha.extend_from_slice(ap);
    }

    // Relation IDs: [9] u32.
    let mut rel_ids = [0u32; 9];
    rel_ids[..8].copy_from_slice(&relation_ids.range_check_9_9_ids);
    rel_ids[8] = relation_ids.memory_id_to_big_id;

    // Upload to GPU.
    let limbs_buf = U32Buffer::from_slice(&flat_limbs)?;
    let mults_buf = U32Buffer::from_slice(&flat_mults)?;
    let alpha_buf = U32Buffer::from_slice(&flat_alpha)?;
    let z_buf = U32Buffer::from_slice(&lookup_elements.z)?;
    let rel_buf = U32Buffer::from_slice(&rel_ids)?;

    // Dispatch GPU kernel.
    let trace_buf = U32Buffer::interaction_trace_id_to_big(
        &limbs_buf,
        &mults_buf,
        &alpha_buf,
        &z_buf,
        &rel_buf,
        n_rows as u32,
        id_offset,
        large_id_base,
    )?;

    // Read back results: 32 columns * n_rows u32 values.
    let raw = trace_buf.to_vec()?;

    // Parse into 8 SecureColumnByCoords (each has 4 M31 coordinate columns).
    let n_logup_cols = 8;
    let domain = CanonicCoset::new(log_size).circle_domain();
    let mut all_evals: Vec<CircleEvaluation<SimdBackend, BaseField, BitReversedOrder>> =
        Vec::new();

    // The last logup column (column 7) needs prefix-sum finalization.
    // Columns 0-6 are output as-is (per-row running sums, not prefix-summed).
    for logup_col in 0..n_logup_cols {
        let base_m31_col = logup_col * 4;
        if logup_col < n_logup_cols - 1 {
            // Columns 0-6: direct output, convert raw u32 -> BaseColumn.
            for coord in 0..SECURE_EXTENSION_DEGREE {
                let col_idx = base_m31_col + coord;
                let offset = col_idx * n_rows;
                let mut col_data = Vec::with_capacity(packed_len);
                for vec_row in 0..packed_len {
                    let mut arr = [M31(0); N_LANES];
                    for lane in 0..N_LANES {
                        arr[lane] = M31(raw[offset + vec_row * N_LANES + lane]);
                    }
                    col_data.push(PackedBaseField::from_array(arr));
                }
                let base_col =
                    stwo::prover::backend::simd::column::BaseColumn::from_simd(col_data);
                all_evals.push(CircleEvaluation::new(domain, base_col));
            }
        } else {
            // Column 7 (last): needs cumsum_shift subtraction + prefix sum.
            let mut coord_columns: [stwo::prover::backend::simd::column::BaseColumn;
                SECURE_EXTENSION_DEGREE] = std::array::from_fn(|coord| {
                let col_idx = base_m31_col + coord;
                let offset = col_idx * n_rows;
                let mut col_data = Vec::with_capacity(packed_len);
                for vec_row in 0..packed_len {
                    let mut arr = [M31(0); N_LANES];
                    for lane in 0..N_LANES {
                        arr[lane] = M31(raw[offset + vec_row * N_LANES + lane]);
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

            for col in coord_prefix_sum {
                all_evals.push(CircleEvaluation::new(domain, col));
            }

            return Ok((all_evals, claimed_sum));
        }
    }

    unreachable!("should have returned from the last column branch")
}

/// Generate the interaction trace for the memory_id_to_big "small" chunk on the GPU.
///
/// Same structure as big but with 8 limbs, 2 RC columns + 1 yield column = 3 logup columns.
pub fn gpu_gen_small_memory_interaction_trace(
    limb_columns: &[Vec<PackedBaseField>; 8],
    multiplicities: &[PackedBaseField],
    lookup_elements: &InteractionLookupElements,
    relation_ids: &InteractionRelationIds,
) -> Result<
    (
        Vec<CircleEvaluation<SimdBackend, BaseField, BitReversedOrder>>,
        SecureField,
    ),
    InteractionTraceError,
> {
    let packed_len = limb_columns[0].len();
    let n_rows = packed_len * N_LANES;
    let log_size = n_rows.ilog2();

    // Prepare GPU input: limb columns in column-major layout [8][n_rows].
    let mut flat_limbs = vec![0u32; 8 * n_rows];
    for (col, limb_col) in limb_columns.iter().enumerate() {
        for (vec_row, packed) in limb_col.iter().enumerate() {
            let vals = packed.to_array();
            for (lane, &v) in vals.iter().enumerate() {
                flat_limbs[col * n_rows + vec_row * N_LANES + lane] = v.0;
            }
        }
    }

    // Prepare multiplicities.
    let mut flat_mults = vec![0u32; n_rows];
    for (vec_row, packed) in multiplicities.iter().enumerate() {
        let vals = packed.to_array();
        for (lane, &v) in vals.iter().enumerate() {
            flat_mults[vec_row * N_LANES + lane] = v.0;
        }
    }

    // Alpha powers: need at least 10.
    assert!(
        lookup_elements.alpha_powers.len() >= 10,
        "Need at least 10 alpha powers for small interaction trace"
    );
    let mut flat_alpha: Vec<u32> = Vec::with_capacity(40);
    for ap in &lookup_elements.alpha_powers[..10] {
        flat_alpha.extend_from_slice(ap);
    }

    // Relation IDs: [5] u32 (RC_9_9, _B, _C, _D, MEMORY_ID_TO_BIG).
    let rel_ids: [u32; 5] = [
        relation_ids.range_check_9_9_ids[0],
        relation_ids.range_check_9_9_ids[1],
        relation_ids.range_check_9_9_ids[2],
        relation_ids.range_check_9_9_ids[3],
        relation_ids.memory_id_to_big_id,
    ];

    // Upload to GPU.
    let limbs_buf = U32Buffer::from_slice(&flat_limbs)?;
    let mults_buf = U32Buffer::from_slice(&flat_mults)?;
    let alpha_buf = U32Buffer::from_slice(&flat_alpha)?;
    let z_buf = U32Buffer::from_slice(&lookup_elements.z)?;
    let rel_buf = U32Buffer::from_slice(&rel_ids)?;

    // Dispatch GPU kernel.
    let trace_buf = U32Buffer::interaction_trace_id_to_big_small(
        &limbs_buf,
        &mults_buf,
        &alpha_buf,
        &z_buf,
        &rel_buf,
        n_rows as u32,
    )?;

    // Read back results: 12 columns * n_rows.
    let raw = trace_buf.to_vec()?;

    let n_logup_cols = 3;
    let domain = CanonicCoset::new(log_size).circle_domain();
    let mut all_evals: Vec<CircleEvaluation<SimdBackend, BaseField, BitReversedOrder>> =
        Vec::new();

    for logup_col in 0..n_logup_cols {
        let base_m31_col = logup_col * 4;
        if logup_col < n_logup_cols - 1 {
            for coord in 0..SECURE_EXTENSION_DEGREE {
                let col_idx = base_m31_col + coord;
                let offset = col_idx * n_rows;
                let mut col_data = Vec::with_capacity(packed_len);
                for vec_row in 0..packed_len {
                    let mut arr = [M31(0); N_LANES];
                    for lane in 0..N_LANES {
                        arr[lane] = M31(raw[offset + vec_row * N_LANES + lane]);
                    }
                    col_data.push(PackedBaseField::from_array(arr));
                }
                let base_col =
                    stwo::prover::backend::simd::column::BaseColumn::from_simd(col_data);
                all_evals.push(CircleEvaluation::new(domain, base_col));
            }
        } else {
            // Last column: prefix-sum finalization.
            let mut coord_columns: [stwo::prover::backend::simd::column::BaseColumn;
                SECURE_EXTENSION_DEGREE] = std::array::from_fn(|coord| {
                let col_idx = base_m31_col + coord;
                let offset = col_idx * n_rows;
                let mut col_data = Vec::with_capacity(packed_len);
                for vec_row in 0..packed_len {
                    let mut arr = [M31(0); N_LANES];
                    for lane in 0..N_LANES {
                        arr[lane] = M31(raw[offset + vec_row * N_LANES + lane]);
                    }
                    col_data.push(PackedBaseField::from_array(arr));
                }
                stwo::prover::backend::simd::column::BaseColumn::from_simd(col_data)
            });

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

            for (i, col) in coord_columns.iter_mut().enumerate() {
                for x in col.data.iter_mut() {
                    *x -= packed_cumsum_shift.into_packed_m31s()[i];
                }
            }

            let coord_prefix_sum = coord_columns.map(inclusive_prefix_sum);

            for col in coord_prefix_sum {
                all_evals.push(CircleEvaluation::new(domain, col));
            }

            return Ok((all_evals, claimed_sum));
        }
    }

    unreachable!("should have returned from the last column branch")
}

/// Extract `InteractionLookupElements` from a `CommonLookupElements` relation.
///
/// This converts the QM31 alpha_powers and z into the flat u32 format needed
/// by the Metal kernel.
pub fn extract_lookup_elements_for_gpu(
    z: QM31,
    alpha_powers: &[QM31],
) -> InteractionLookupElements {
    let powers: Vec<[u32; 4]> = alpha_powers
        .iter()
        .map(|qm| {
            let [a, b, c, d] = qm.to_m31_array();
            [a.0, b.0, c.0, d.0]
        })
        .collect();
    let [za, zb, zc, zd] = z.to_m31_array();
    InteractionLookupElements {
        alpha_powers: powers,
        z: [za.0, zb.0, zc.0, zd.0],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use stwo::core::channel::{Blake2sChannel, Channel};
    use stwo::core::fields::m31::M31;
    use stwo::core::fields::qm31::QM31;
    use stwo::prover::backend::simd::m31::PackedM31;

    #[allow(unused_imports)]
    use stwo::core::fields::FieldExpOps;

    /// Build dummy lookup elements for testing.
    fn make_test_lookup_elements() -> InteractionLookupElements {
        let mut channel = Blake2sChannel::default();
        let felts = channel.draw_secure_felts(2);
        let z = felts[0];
        let alpha = felts[1];

        let mut cur = QM31::from_u32_unchecked(1, 0, 0, 0);
        let mut powers = Vec::new();
        for _ in 0..30 {
            powers.push(cur);
            cur *= alpha;
        }
        extract_lookup_elements_for_gpu(z, &powers)
    }

    fn make_test_relation_ids() -> InteractionRelationIds {
        InteractionRelationIds {
            range_check_9_9_ids: [
                517791011,   // RANGE_CHECK_9_9
                1897792095,  // _B
                1881014476,  // _C
                1864236857,  // _D
                1847459238,  // _E
                1830681619,  // _F
                1813904000,  // _G
                2065568285,  // _H
            ],
            memory_id_to_big_id: 1662111297,
        }
    }

    #[test]
    fn test_big_interaction_trace_runs() {
        // Minimal test: verify the GPU kernel runs without error on a small input.
        let n_packed = 1; // 16 rows
        let limb_columns: [Vec<PackedM31>; 28] = std::array::from_fn(|col| {
            vec![PackedM31::broadcast(M31(col as u32 + 1)); n_packed]
        });
        // Use varying multiplicities to ensure a non-zero claimed sum.
        let multiplicities = vec![PackedM31::from_array(std::array::from_fn(|i| {
            M31((i as u32 + 1) * 3)
        })); n_packed];

        let lookup = make_test_lookup_elements();
        let rel_ids = make_test_relation_ids();

        let result = gpu_gen_big_memory_interaction_trace(
            &limb_columns,
            &multiplicities,
            &lookup,
            &rel_ids,
            0,
            0x4000_0000, // LARGE_MEMORY_VALUE_ID_BASE
        );
        assert!(result.is_ok(), "GPU interaction trace failed: {:?}", result.err());
        let (trace, _claimed_sum) = result.unwrap();
        // 8 logup columns * 4 coords = 32 M31 columns
        assert_eq!(trace.len(), 32, "Expected 32 M31 columns");
    }

    /// CPU reference: compute combine(values) = sum(alpha_powers[i] * values[i]) - z
    fn cpu_combine(alpha_powers: &[QM31], values: &[M31], z: QM31) -> QM31 {
        let mut acc = QM31::from_u32_unchecked(0, 0, 0, 0);
        for (i, &v) in values.iter().enumerate() {
            acc += alpha_powers[i] * v;
        }
        acc - z
    }

    /// CPU reference for the big interaction trace (single row).
    fn cpu_big_interaction_single_row(
        limbs: &[M31; 28],
        mult: M31,
        alpha_powers: &[QM31],
        z: QM31,
        rel_ids: &InteractionRelationIds,
        row_id: u32,
        large_id_base: u32,
    ) -> [QM31; 8] {
        let mut running_sum = QM31::from_u32_unchecked(0, 0, 0, 0);
        let mut cols = [QM31::from_u32_unchecked(0, 0, 0, 0); 8];

        // 7 range-check columns
        for col in 0..7 {
            let base = col * 4;
            let rc_pair = col % 4;
            let rid0 = M31(rel_ids.range_check_9_9_ids[rc_pair * 2]);
            let rid1 = M31(rel_ids.range_check_9_9_ids[rc_pair * 2 + 1]);

            let denom0 = cpu_combine(
                alpha_powers,
                &[rid0, limbs[base], limbs[base + 1]],
                z,
            );
            let denom1 = cpu_combine(
                alpha_powers,
                &[rid1, limbs[base + 2], limbs[base + 3]],
                z,
            );
            let numer = denom0 + denom1;
            let denom = denom0 * denom1;
            let frac = numer * denom.inverse();
            running_sum += frac;
            cols[col] = running_sum;
        }

        // Yield column
        {
            let id_value = M31((row_id | large_id_base) % (1 << 31));
            let mut values = vec![M31(rel_ids.memory_id_to_big_id), id_value];
            values.extend_from_slice(limbs);
            let yield_denom = cpu_combine(alpha_powers, &values, z);
            let yield_numer = QM31::from(-mult); // -mult
            let frac = yield_numer * yield_denom.inverse();
            running_sum += frac;
            cols[7] = running_sum;
        }

        cols
    }

    #[test]
    fn test_big_interaction_trace_correctness_vs_cpu() {
        let n_packed = 1; // 16 rows
        // Use varied limb values
        let limb_columns: [Vec<PackedM31>; 28] = std::array::from_fn(|col| {
            vec![PackedM31::from_array(std::array::from_fn(|lane| {
                M31(((col * 16 + lane) as u32 * 7 + 3) % 512) // varied 9-bit values
            })); n_packed]
        });
        let multiplicities = vec![PackedM31::from_array(std::array::from_fn(|i| {
            M31((i as u32 + 1) * 5)
        })); n_packed];

        // Build lookup elements
        let mut channel = Blake2sChannel::default();
        let felts = channel.draw_secure_felts(2);
        let z = felts[0];
        let alpha = felts[1];
        let mut cur = QM31::from_u32_unchecked(1, 0, 0, 0);
        let mut powers = Vec::new();
        for _ in 0..30 {
            powers.push(cur);
            cur *= alpha;
        }
        let lookup = extract_lookup_elements_for_gpu(z, &powers);
        let rel_ids = make_test_relation_ids();
        let large_id_base = 0x4000_0000u32;

        let (gpu_trace, _claimed_sum) = gpu_gen_big_memory_interaction_trace(
            &limb_columns,
            &multiplicities,
            &lookup,
            &rel_ids,
            0,
            large_id_base,
        )
        .expect("GPU trace generation failed");

        // Compare the first 7 logup columns (non-prefix-summed) per row.
        for row in 0..N_LANES {
            let mut limbs = [M31(0); 28];
            for col in 0..28 {
                limbs[col] = limb_columns[col][0].to_array()[row];
            }
            let mult = multiplicities[0].to_array()[row];

            let expected = cpu_big_interaction_single_row(
                &limbs,
                mult,
                &powers,
                z,
                &rel_ids,
                row as u32,
                large_id_base,
            );

            // Check columns 0-6 (not prefix-summed).
            for logup_col in 0..7 {
                let exp = expected[logup_col];
                let [ea, eb, ec, ed] = exp.to_m31_array();
                for coord in 0..4 {
                    let col_idx = logup_col * 4 + coord;
                    use stwo::prover::backend::Column;
                    let got = gpu_trace[col_idx].values.at(row);
                    let exp_coord = [ea, eb, ec, ed][coord];
                    assert_eq!(
                        got, exp_coord,
                        "mismatch at row {row}, logup_col {logup_col}, coord {coord}: \
                         got {got:?}, expected {exp_coord:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn test_small_interaction_trace_runs() {
        let n_packed = 1; // 16 rows
        let limb_columns: [Vec<PackedM31>; 8] = std::array::from_fn(|col| {
            vec![PackedM31::broadcast(M31(col as u32 + 1)); n_packed]
        });
        let multiplicities = vec![PackedM31::broadcast(M31(1)); n_packed];

        let lookup = make_test_lookup_elements();
        let rel_ids = make_test_relation_ids();

        let result = gpu_gen_small_memory_interaction_trace(
            &limb_columns,
            &multiplicities,
            &lookup,
            &rel_ids,
        );
        assert!(result.is_ok(), "GPU small interaction trace failed: {:?}", result.err());
        let (trace, claimed_sum) = result.unwrap();
        // 3 logup columns * 4 coords = 12 M31 columns
        assert_eq!(trace.len(), 12, "Expected 12 M31 columns");
        assert_ne!(claimed_sum, SecureField::from_u32_unchecked(0, 0, 0, 0));
    }
}
