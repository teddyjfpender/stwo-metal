// interaction_trace_generic.metal
//
// Generic Metal compute kernel for interaction trace generation.
// Replaces ~25 component-specific CPU interaction trace generators with a
// single data-driven GPU kernel.
//
// Each component's interaction trace follows the logup pattern:
//   For each logup column (described by a ColumnDescriptor):
//     denom0 = combine(values0) = sum(alpha^i * val_i) - z
//     denom1 = combine(values1) = sum(alpha^i * val_i) - z  (if dual-term)
//     numerator depends on numerator_type:
//       0 (pair_sum):        numer = denom0 + denom1
//       1 (enabler_weighted): numer = denom0 * enabler + denom1
//       2 (mults_weighted):  numer = -(denom0 * weight1 + denom1 * weight0)
//       3 (single_neg):      numer = -enabler, denominator = denom0 only
//     denominator = denom0 * denom1  (or just denom0 for single_neg)
//     fraction = numer * inverse(denom)
//     running_sum += fraction
//     output[col][row] = running_sum
//
// Input layout:
//   buffer(0) — values: flat column-major [n_value_columns][n_rows] u32
//               All lookup data for all columns packed contiguously.
//   buffer(1) — descriptors: array of ColumnDescriptor structs
//   buffer(2) — alpha_powers: [max_combine_size] QM31 = [max_combine_size*4] u32
//   buffer(3) — z: QM31 = [4] u32
//   buffer(4) — trace_out: column-major [n_logup_cols*4][n_rows] u32
//   buffer(5) — n_rows: scalar
//   buffer(6) — n_logup_cols: scalar
//   buffer(7) — n_rows_real: scalar (actual row count for enabler computation)
//
// Thread mapping: one thread per row.

#include "secure_field_support.h"

// Column descriptor struct — must match the Rust-side repr(C) layout.
struct ColumnDescriptor {
    uint values0_offset;    // column index into values buffer for first combine
    uint values0_count;     // number of values for combine0
    uint values1_offset;    // column index for second combine (unused if single_neg)
    uint values1_count;     // number of values for combine1
    uint relation_id0;      // first value passed to combine0 (prepended)
    uint relation_id1;      // first value passed to combine1 (prepended)
    uint numerator_type;    // 0=pair_sum, 1=enabler_weighted, 2=mults_weighted, 3=single_neg
    uint weight_offset;     // column index for weight (enabler/mults) in values buffer
};

// Load a QM31 from a contiguous array of u32 at index.
static inline StwoMetalQm31 load_alpha_power_gen(
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

// combine(relation_id, values[0..n]) = alpha^0 * relation_id + sum_{i=0}^{n-1}(alpha^{i+1} * values[i]) - z
// The relation_id is the first element, values follow starting at alpha^1.
static inline StwoMetalQm31 logup_combine_gen(
    device const uint *alpha_powers,
    uint relation_id,
    device const uint *values,   // column-major: values[col_idx * n_rows + row]
    uint n_value_cols,
    uint row,
    uint n_rows,
    StwoMetalQm31 z
) {
    // alpha^0 * relation_id
    StwoMetalQm31 acc = stwo_metal_qm31_mul_base(
        load_alpha_power_gen(alpha_powers, 0u), relation_id);

    // alpha^{i+1} * values[col_i]
    for (uint i = 0u; i < n_value_cols; ++i) {
        StwoMetalQm31 ap = load_alpha_power_gen(alpha_powers, i + 1u);
        uint val = values[i * n_rows + row];
        acc = stwo_metal_qm31_add(acc, stwo_metal_qm31_mul_base(ap, val));
    }
    return stwo_metal_qm31_sub(acc, z);
}

kernel void interaction_trace_generic_fractions(
    device const uint               *values           [[buffer(0)]],
    device const ColumnDescriptor   *descriptors      [[buffer(1)]],
    device const uint               *alpha_powers     [[buffer(2)]],
    device const uint               *z_buf            [[buffer(3)]],
    device       uint               *trace_out        [[buffer(4)]],
    constant     uint               &n_rows           [[buffer(5)]],
    constant     uint               &n_logup_cols     [[buffer(6)]],
    constant     uint               &n_rows_real      [[buffer(7)]],
    uint                             row              [[thread_position_in_grid]]
) {
    if (row >= n_rows) {
        return;
    }

    StwoMetalQm31 z = {
        z_buf[0], z_buf[1], z_buf[2], z_buf[3]
    };

    // Enabler value: 1 for row < n_rows_real, 0 for padding rows.
    uint enabler_val = (row < n_rows_real) ? 1u : 0u;

    StwoMetalQm31 running_sum = { 0u, 0u, 0u, 0u };

    for (uint col = 0u; col < n_logup_cols; ++col) {
        ColumnDescriptor desc = descriptors[col];

        StwoMetalQm31 frac;

        if (desc.numerator_type == 3u) {
            // single_neg: numer = -enabler, denom = combine(values0)
            StwoMetalQm31 denom0 = logup_combine_gen(
                alpha_powers, desc.relation_id0,
                values + desc.values0_offset * n_rows,
                desc.values0_count, row, n_rows, z
            );

            StwoMetalQm31 numer = stwo_metal_qm31_from_base(stwo_metal_m31_neg(enabler_val));
            StwoMetalQm31 denom_inv = stwo_metal_qm31_inverse(denom0);
            frac = stwo_metal_qm31_mul(numer, denom_inv);
        } else {
            // Dual-term columns: compute both denominators.
            StwoMetalQm31 denom0 = logup_combine_gen(
                alpha_powers, desc.relation_id0,
                values + desc.values0_offset * n_rows,
                desc.values0_count, row, n_rows, z
            );
            StwoMetalQm31 denom1 = logup_combine_gen(
                alpha_powers, desc.relation_id1,
                values + desc.values1_offset * n_rows,
                desc.values1_count, row, n_rows, z
            );

            StwoMetalQm31 numer;
            StwoMetalQm31 denom = stwo_metal_qm31_mul(denom0, denom1);

            if (desc.numerator_type == 0u) {
                // pair_sum: numer = denom0 + denom1
                numer = stwo_metal_qm31_add(denom0, denom1);
            } else if (desc.numerator_type == 1u) {
                // enabler_weighted: numer = denom0 * enabler + denom1
                StwoMetalQm31 weighted_d0 = stwo_metal_qm31_mul_base(denom0, enabler_val);
                numer = stwo_metal_qm31_add(weighted_d0, denom1);
            } else {
                // mults_weighted (type 2): numer = -(denom0 * weight1 + denom1 * weight0)
                // weight0 = values[weight_offset * n_rows + row] (for first chunk)
                // weight1 = values[(weight_offset+1) * n_rows + row] (for second chunk)
                uint weight0 = values[desc.weight_offset * n_rows + row];
                uint weight1 = values[(desc.weight_offset + 1u) * n_rows + row];
                StwoMetalQm31 term0 = stwo_metal_qm31_mul_base(denom0, stwo_metal_m31_neg(weight1));
                StwoMetalQm31 term1 = stwo_metal_qm31_mul_base(denom1, stwo_metal_m31_neg(weight0));
                numer = stwo_metal_qm31_add(term0, term1);
            }

            StwoMetalQm31 denom_inv = stwo_metal_qm31_inverse(denom);
            frac = stwo_metal_qm31_mul(numer, denom_inv);
        }

        // Accumulate running sum.
        running_sum = stwo_metal_qm31_add(running_sum, frac);

        // Write this column's cumulative value (running_sum).
        uint out_base = col * 4u;
        trace_out[(out_base + 0u) * n_rows + row] = running_sum.a;
        trace_out[(out_base + 1u) * n_rows + row] = running_sum.b;
        trace_out[(out_base + 2u) * n_rows + row] = running_sum.c;
        trace_out[(out_base + 3u) * n_rows + row] = running_sum.d;
    }
}
