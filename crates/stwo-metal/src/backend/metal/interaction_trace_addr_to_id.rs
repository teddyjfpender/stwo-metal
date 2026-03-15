//! GPU-accelerated interaction trace generation for the `memory_address_to_id` component.
//!
//! This module provides Metal-backed logup interaction trace generation,
//! replacing the CPU-bound `InteractionClaimGenerator::write_interaction_trace`
//! from stwo-cairo.
//!
//! # Architecture
//!
//! The GPU kernel computes all per-row logup fractions in parallel.
//! MEMORY_ADDRESS_TO_ID_SPLIT chunks are paired into SPLIT/2 logup columns.
//! Each column combines two chunks:
//!
//!   p0 = combine([RELATION_ID, addr0, id0])
//!   p1 = combine([RELATION_ID, addr1, id1])
//!   numer = p0 * (-mult1) + p1 * (-mult0)
//!   denom = p1 * p0
//!   fraction = numer / denom
//!
//! The CPU then finalizes the last column with prefix-sum + cumsum_shift.
//!
//! # Output layout
//!
//! SPLIT/2 QM31 columns = SPLIT/2 * 4 M31 columns (column-major).

use stwo::core::fields::m31::{BaseField, M31};
use stwo::core::fields::qm31::{SecureField, SECURE_EXTENSION_DEGREE};
use stwo::core::poly::circle::CanonicCoset;
use stwo::prover::backend::simd::m31::{PackedBaseField, N_LANES};
use stwo::prover::backend::simd::prefix_sum::inclusive_prefix_sum;
use stwo::prover::backend::simd::SimdBackend;
use stwo::prover::poly::circle::CircleEvaluation;
use stwo::prover::poly::BitReversedOrder;
use stwo_metal_sys::metal::U32Buffer;

use super::interaction_trace_id_to_big::{
    InteractionLookupElements, InteractionTraceError,
};

/// Generate the interaction trace for memory_address_to_id on the GPU.
///
/// # Arguments
/// * `ids`             - [SPLIT] arrays of PackedM31 id values per chunk.
/// * `multiplicities`  - [SPLIT] arrays of PackedM31 multiplicities per chunk.
/// * `lookup_elements` - QM31 alpha_powers and z from the Fiat-Shamir channel.
/// * `relation_id`     - MEMORY_ADDRESS_TO_ID_RELATION_ID as u32.
/// * `split`           - MEMORY_ADDRESS_TO_ID_SPLIT (must be even).
///
/// # Returns
/// Tuple of (trace_evaluations, claimed_sum).
/// The trace has SPLIT/2 * 4 M31 columns formatted as CircleEvaluations.
pub fn gpu_gen_addr_to_id_interaction_trace(
    ids: &[Vec<PackedBaseField>],
    multiplicities: &[Vec<PackedBaseField>],
    lookup_elements: &InteractionLookupElements,
    relation_id: u32,
    split: usize,
) -> Result<
    (
        Vec<CircleEvaluation<SimdBackend, BaseField, BitReversedOrder>>,
        SecureField,
    ),
    InteractionTraceError,
> {
    assert!(split % 2 == 0, "split must be even");
    assert_eq!(ids.len(), split);
    assert_eq!(multiplicities.len(), split);

    let packed_len = ids[0].len();
    let n_rows = packed_len * N_LANES;
    let log_size = n_rows.ilog2();
    let n_logup_cols = split / 2;

    // Prepare GPU input: ids in column-major layout [SPLIT][n_rows].
    let mut flat_ids = vec![0u32; split * n_rows];
    for (chunk, id_col) in ids.iter().enumerate() {
        for (vec_row, packed) in id_col.iter().enumerate() {
            let vals = packed.to_array();
            for (lane, &v) in vals.iter().enumerate() {
                flat_ids[chunk * n_rows + vec_row * N_LANES + lane] = v.0;
            }
        }
    }

    // Prepare multiplicities: column-major [SPLIT][n_rows].
    let mut flat_mults = vec![0u32; split * n_rows];
    for (chunk, mult_col) in multiplicities.iter().enumerate() {
        for (vec_row, packed) in mult_col.iter().enumerate() {
            let vals = packed.to_array();
            for (lane, &v) in vals.iter().enumerate() {
                flat_mults[chunk * n_rows + vec_row * N_LANES + lane] = v.0;
            }
        }
    }

    // Alpha powers: need at least 3 for combine([relation_id, addr, id]).
    assert!(
        lookup_elements.alpha_powers.len() >= 3,
        "Need at least 3 alpha powers for addr_to_id interaction trace"
    );
    let mut flat_alpha: Vec<u32> = Vec::with_capacity(12);
    for ap in &lookup_elements.alpha_powers[..3] {
        flat_alpha.extend_from_slice(ap);
    }

    // Upload to GPU.
    let ids_buf = U32Buffer::from_slice(&flat_ids)?;
    let mults_buf = U32Buffer::from_slice(&flat_mults)?;
    let alpha_buf = U32Buffer::from_slice(&flat_alpha)?;
    let z_buf = U32Buffer::from_slice(&lookup_elements.z)?;
    let rel_id_buf = U32Buffer::from_slice(&[relation_id])?;

    // Dispatch GPU kernel.
    let trace_buf = U32Buffer::interaction_trace_addr_to_id(
        &ids_buf,
        &mults_buf,
        &alpha_buf,
        &z_buf,
        &rel_id_buf,
        n_rows as u32,
        split as u32,
    )?;

    // Read back results: n_logup_cols * 4 M31 columns * n_rows u32 values.
    let raw = trace_buf.to_vec()?;

    let domain = CanonicCoset::new(log_size).circle_domain();
    let mut all_evals: Vec<CircleEvaluation<SimdBackend, BaseField, BitReversedOrder>> = Vec::new();

    // Process columns: first n_logup_cols - 1 are direct, last needs prefix-sum.
    for logup_col in 0..n_logup_cols {
        let base_m31_col = logup_col * 4;
        if logup_col < n_logup_cols - 1 {
            // Direct output columns.
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
            // Last column: needs cumsum_shift subtraction + prefix sum.
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
