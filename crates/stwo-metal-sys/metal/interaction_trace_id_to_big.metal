// interaction_trace_id_to_big.metal
//
// Metal compute kernel for generating the memory_id_to_big interaction trace
// (big values variant).
//
// This kernel fuses the entire per-row logup computation:
//   1. Compute combine() for 7 range-check-9-9 pairs (alpha_powers dot limbs - z)
//   2. Compute combine() for 1 yield column (id + 28 limbs)
//   3. Compute numerator/denominator for each logup column
//   4. Per-element QM31 inverse of each denominator
//   5. Multiply numerator * denom_inverse = fraction
//   6. Accumulate partial sums across the 8 columns per row
//
// The output is 8 QM31 columns in coordinate layout (32 M31 columns).
// The caller must still do the prefix-sum + cumsum_shift on the last column.
//
// Input layout:
//   buffer(0)  — limbs: column-major [28][n_rows] u32 (the 28 x 9-bit limb values)
//   buffer(1)  — mults: [n_rows] u32 (multiplicities)
//   buffer(2)  — alpha_powers: [30] QM31 = [120] u32 (lookup element powers)
//   buffer(3)  — z: QM31 = [4] u32 (lookup element z)
//   buffer(4)  — relation_ids: [9] u32 (RANGE_CHECK_9_9, _B, _C, _D, _E, _F, _G, _H, MEMORY_ID_TO_BIG)
//   buffer(5)  — trace_out: column-major [32][n_rows] u32 (8 QM31 cols = 32 M31 cols)
//   buffer(6)  — n_rows: scalar
//   buffer(7)  — id_offset: scalar (offset added to row index for the yield column id)
//   buffer(8)  — large_memory_value_id_base: scalar (OR'd into id for large values)
//
// Thread mapping: one thread per row.

#include "secure_field_support.h"

// Load a QM31 from a contiguous array of u32 at index.
static inline StwoMetalQm31 load_alpha_power(
    device const uint *alpha_powers,
    uint idx
) {
    uint base = idx * 4u;
    return StwoMetalQm31 {
        alpha_powers[base + 0u],
        alpha_powers[base + 1u],
        alpha_powers[base + 2u],
        alpha_powers[base + 3u],
    };
}

// combine(values[0..n]) = sum(alpha_powers[i] * values[i]) - z
// where values[i] are M31 base-field elements.
// alpha_powers are QM31, values are scalar M31 -> uses qm31_mul_base.
static inline StwoMetalQm31 logup_combine(
    device const uint *alpha_powers,
    const thread uint *values,
    uint n_values,
    StwoMetalQm31 z
) {
    StwoMetalQm31 acc = { 0u, 0u, 0u, 0u };
    for (uint i = 0u; i < n_values; ++i) {
        StwoMetalQm31 ap = load_alpha_power(alpha_powers, i);
        acc = stwo_metal_qm31_add(acc, stwo_metal_qm31_mul_base(ap, values[i]));
    }
    return stwo_metal_qm31_sub(acc, z);
}

kernel void interaction_trace_id_to_big_fractions(
    device const uint  *limbs              [[buffer(0)]],
    device const uint  *mults              [[buffer(1)]],
    device const uint  *alpha_powers       [[buffer(2)]],
    device const uint  *z_buf              [[buffer(3)]],
    device const uint  *relation_ids       [[buffer(4)]],
    device       uint  *trace_out          [[buffer(5)]],
    constant     uint  &n_rows             [[buffer(6)]],
    constant     uint  &id_offset          [[buffer(7)]],
    constant     uint  &large_id_base      [[buffer(8)]],
    uint               row                [[thread_position_in_grid]]
) {
    if (row >= n_rows) {
        return;
    }

    StwoMetalQm31 z = {
        z_buf[0], z_buf[1], z_buf[2], z_buf[3]
    };

    // Load the 28 limb values for this row.
    uint limb_vals[28];
    for (uint c = 0u; c < 28u; ++c) {
        limb_vals[c] = limbs[c * n_rows + row];
    }

    // Load multiplicity.
    uint mult = mults[row];

    // ------------------------------------------------------------------
    // Columns 0-6: Range-check 9-9 pairs.
    // Each column pairs 4 consecutive limbs (limb0, limb1) and (limb2, limb3).
    // denom0 = combine([relation_id_X, limb0, limb1])
    // denom1 = combine([relation_id_Y, limb2, limb3])
    // numerator = denom0 + denom1
    // denominator = denom0 * denom1
    // fraction = numerator / denominator
    // ------------------------------------------------------------------
    StwoMetalQm31 running_sum = { 0u, 0u, 0u, 0u };

    for (uint col = 0u; col < 7u; ++col) {
        uint base_limb = col * 4u;
        // Two relation IDs per column: col maps to relation_ids[2*col%8] and [2*col%8+1]
        // The pattern from the CPU code: col 0 -> (RC_9_9, RC_9_9_B),
        //   col 1 -> (RC_9_9_C, RC_9_9_D), col 2 -> (RC_9_9_E, RC_9_9_F),
        //   col 3 -> (RC_9_9_G, RC_9_9_H), col 4 -> (RC_9_9, RC_9_9_B), ...
        // Actually from the CPU: i = col, match i%4 for the relation ID pair.
        // But the relation_ids buffer stores them in order: [A, B, C, D, E, F, G, H, MEM_ID_TO_BIG]
        uint rc_pair_idx = col % 4u;
        uint rid0_idx = rc_pair_idx * 2u;
        uint rid1_idx = rc_pair_idx * 2u + 1u;

        uint vals0[3] = { relation_ids[rid0_idx], limb_vals[base_limb + 0u], limb_vals[base_limb + 1u] };
        uint vals1[3] = { relation_ids[rid1_idx], limb_vals[base_limb + 2u], limb_vals[base_limb + 3u] };

        StwoMetalQm31 denom0 = logup_combine(alpha_powers, vals0, 3u, z);
        StwoMetalQm31 denom1 = logup_combine(alpha_powers, vals1, 3u, z);

        // numerator = denom0 + denom1, denominator = denom0 * denom1
        StwoMetalQm31 numer = stwo_metal_qm31_add(denom0, denom1);
        StwoMetalQm31 denom = stwo_metal_qm31_mul(denom0, denom1);

        // fraction = numer / denom = numer * denom^{-1}
        StwoMetalQm31 denom_inv = stwo_metal_qm31_inverse(denom);
        StwoMetalQm31 frac = stwo_metal_qm31_mul(numer, denom_inv);

        // Accumulate running sum.
        running_sum = stwo_metal_qm31_add(running_sum, frac);

        // Write this column's cumulative value (running_sum).
        uint out_base = col * 4u;
        trace_out[(out_base + 0u) * n_rows + row] = running_sum.a;
        trace_out[(out_base + 1u) * n_rows + row] = running_sum.b;
        trace_out[(out_base + 2u) * n_rows + row] = running_sum.c;
        trace_out[(out_base + 3u) * n_rows + row] = running_sum.d;
    }

    // ------------------------------------------------------------------
    // Column 7: Yield column (memory_id_to_big lookup).
    // combine([MEMORY_ID_TO_BIG_RELATION_ID, id, limb0..limb27])
    // where id = (row + id_offset) | large_id_base
    // numerator = -multiplicity (as QM31 from base field)
    // denominator = combine result
    // ------------------------------------------------------------------
    {
        uint id_value = (row + id_offset) | large_id_base;
        uint yield_vals[29]; // 1 relation_id + 1 id + 27 limbs... wait, it's 1+1+28=30
        // Actually: [relation_id, id, limb0, limb1, ..., limb27] = 30 values total
        // relation_id is relation_ids[8] = MEMORY_ID_TO_BIG_RELATION_ID
        yield_vals[0] = relation_ids[8u];
        yield_vals[1] = id_value;
        for (uint i = 0u; i < 28u; ++i) {
            yield_vals[2u + i] = limb_vals[i];
        }

        StwoMetalQm31 yield_denom = logup_combine(alpha_powers, yield_vals, 30u, z);
        StwoMetalQm31 yield_numer = stwo_metal_qm31_from_base(stwo_metal_m31_neg(mult));
        StwoMetalQm31 yield_denom_inv = stwo_metal_qm31_inverse(yield_denom);
        StwoMetalQm31 yield_frac = stwo_metal_qm31_mul(yield_numer, yield_denom_inv);

        running_sum = stwo_metal_qm31_add(running_sum, yield_frac);

        uint out_base = 7u * 4u;
        trace_out[(out_base + 0u) * n_rows + row] = running_sum.a;
        trace_out[(out_base + 1u) * n_rows + row] = running_sum.b;
        trace_out[(out_base + 2u) * n_rows + row] = running_sum.c;
        trace_out[(out_base + 3u) * n_rows + row] = running_sum.d;
    }
}

// ---------------------------------------------------------------------------
// Small-value variant.
//
// Same structure but with 8 limbs and 2 range-check columns + 1 yield column
// = 3 logup columns = 12 M31 output columns.
//
// Input layout:
//   buffer(0) — limbs: column-major [8][n_rows] u32
//   buffer(1) — mults: [n_rows] u32
//   buffer(2) — alpha_powers: [10] QM31 = [40] u32 (need at most 10 powers for combine of 10 values)
//   buffer(3) — z: QM31 = [4] u32
//   buffer(4) — relation_ids: [5] u32 (RC_9_9, RC_9_9_B, RC_9_9_C, RC_9_9_D, MEMORY_ID_TO_BIG)
//   buffer(5) — trace_out: column-major [12][n_rows] u32
//   buffer(6) — n_rows: scalar
// ---------------------------------------------------------------------------
kernel void interaction_trace_id_to_big_small_fractions(
    device const uint  *limbs              [[buffer(0)]],
    device const uint  *mults              [[buffer(1)]],
    device const uint  *alpha_powers       [[buffer(2)]],
    device const uint  *z_buf              [[buffer(3)]],
    device const uint  *relation_ids       [[buffer(4)]],
    device       uint  *trace_out          [[buffer(5)]],
    constant     uint  &n_rows             [[buffer(6)]],
    uint               row                [[thread_position_in_grid]]
) {
    if (row >= n_rows) {
        return;
    }

    StwoMetalQm31 z = {
        z_buf[0], z_buf[1], z_buf[2], z_buf[3]
    };

    // Load the 8 limb values for this row.
    uint limb_vals[8];
    for (uint c = 0u; c < 8u; ++c) {
        limb_vals[c] = limbs[c * n_rows + row];
    }

    uint mult = mults[row];

    StwoMetalQm31 running_sum = { 0u, 0u, 0u, 0u };

    // ------------------------------------------------------------------
    // Columns 0-1: Range-check 9-9 pairs (2 columns for 8 limbs / 4 per col).
    // col 0 -> i%2==0 -> (RC_9_9, RC_9_9_B)
    // col 1 -> i%2==1 -> (RC_9_9_C, RC_9_9_D)
    // ------------------------------------------------------------------
    for (uint col = 0u; col < 2u; ++col) {
        uint base_limb = col * 4u;
        uint rid0_idx = col * 2u;
        uint rid1_idx = col * 2u + 1u;

        uint vals0[3] = { relation_ids[rid0_idx], limb_vals[base_limb + 0u], limb_vals[base_limb + 1u] };
        uint vals1[3] = { relation_ids[rid1_idx], limb_vals[base_limb + 2u], limb_vals[base_limb + 3u] };

        StwoMetalQm31 denom0 = logup_combine(alpha_powers, vals0, 3u, z);
        StwoMetalQm31 denom1 = logup_combine(alpha_powers, vals1, 3u, z);

        StwoMetalQm31 numer = stwo_metal_qm31_add(denom0, denom1);
        StwoMetalQm31 denom = stwo_metal_qm31_mul(denom0, denom1);

        StwoMetalQm31 denom_inv = stwo_metal_qm31_inverse(denom);
        StwoMetalQm31 frac = stwo_metal_qm31_mul(numer, denom_inv);

        running_sum = stwo_metal_qm31_add(running_sum, frac);

        uint out_base = col * 4u;
        trace_out[(out_base + 0u) * n_rows + row] = running_sum.a;
        trace_out[(out_base + 1u) * n_rows + row] = running_sum.b;
        trace_out[(out_base + 2u) * n_rows + row] = running_sum.c;
        trace_out[(out_base + 3u) * n_rows + row] = running_sum.d;
    }

    // ------------------------------------------------------------------
    // Column 2: Yield column.
    // combine([MEMORY_ID_TO_BIG_RELATION_ID, id, limb0..limb7])
    // id = row (small values start at 0, no large_id_base)
    // ------------------------------------------------------------------
    {
        uint yield_vals[10]; // 1 + 1 + 8 = 10
        yield_vals[0] = relation_ids[4u]; // MEMORY_ID_TO_BIG_RELATION_ID
        yield_vals[1] = row;
        for (uint i = 0u; i < 8u; ++i) {
            yield_vals[2u + i] = limb_vals[i];
        }

        StwoMetalQm31 yield_denom = logup_combine(alpha_powers, yield_vals, 10u, z);
        StwoMetalQm31 yield_numer = stwo_metal_qm31_from_base(stwo_metal_m31_neg(mult));
        StwoMetalQm31 yield_denom_inv = stwo_metal_qm31_inverse(yield_denom);
        StwoMetalQm31 yield_frac = stwo_metal_qm31_mul(yield_numer, yield_denom_inv);

        running_sum = stwo_metal_qm31_add(running_sum, yield_frac);

        uint out_base = 2u * 4u;
        trace_out[(out_base + 0u) * n_rows + row] = running_sum.a;
        trace_out[(out_base + 1u) * n_rows + row] = running_sum.b;
        trace_out[(out_base + 2u) * n_rows + row] = running_sum.c;
        trace_out[(out_base + 3u) * n_rows + row] = running_sum.d;
    }
}
