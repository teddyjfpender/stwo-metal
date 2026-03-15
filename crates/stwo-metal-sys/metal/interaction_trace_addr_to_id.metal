// interaction_trace_addr_to_id.metal
//
// Metal compute kernel for generating the memory_address_to_id interaction trace.
//
// The memory_address_to_id component uses MEMORY_ADDRESS_TO_ID_SPLIT chunks
// (typically 16), paired into SPLIT/2 logup columns. Each column combines two
// chunks of (address, id, multiplicity) into a single logup fraction:
//
//   p0 = combine([RELATION_ID, addr0, id0])
//   p1 = combine([RELATION_ID, addr1, id1])
//   numer = p0 * (-mult1) + p1 * (-mult0)
//   denom = p1 * p0
//   fraction = numer * denom^{-1}
//
// The running sum accumulates across all SPLIT/2 columns per row.
// The last column still needs prefix-sum finalization on CPU.
//
// Input layout:
//   buffer(0)  — ids: column-major [SPLIT][n_rows] u32 (id values per chunk)
//   buffer(1)  — mults: column-major [SPLIT][n_rows] u32 (multiplicities per chunk)
//   buffer(2)  — alpha_powers: [3] QM31 = [12] u32 (only 3 powers needed for combine of 3 values)
//   buffer(3)  — z: QM31 = [4] u32
//   buffer(4)  — relation_id: [1] u32 (MEMORY_ADDRESS_TO_ID_RELATION_ID)
//   buffer(5)  — trace_out: column-major [SPLIT/2 * 4][n_rows] u32 (SPLIT/2 QM31 cols)
//   buffer(6)  — n_rows: scalar
//   buffer(7)  — split: scalar (MEMORY_ADDRESS_TO_ID_SPLIT, must be even)
//
// Thread mapping: one thread per row.

#include "secure_field_support.h"

// Load a QM31 from a contiguous array of u32 at index.
static inline StwoMetalQm31 load_alpha_power_a2i(
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
static inline StwoMetalQm31 logup_combine_a2i(
    device const uint *alpha_powers,
    const thread uint *values,
    uint n_values,
    StwoMetalQm31 z
) {
    StwoMetalQm31 acc = { 0u, 0u, 0u, 0u };
    for (uint i = 0u; i < n_values; ++i) {
        StwoMetalQm31 ap = load_alpha_power_a2i(alpha_powers, i);
        acc = stwo_metal_qm31_add(acc, stwo_metal_qm31_mul_base(ap, values[i]));
    }
    return stwo_metal_qm31_sub(acc, z);
}

kernel void interaction_trace_addr_to_id_fractions(
    device const uint  *ids                [[buffer(0)]],
    device const uint  *mults              [[buffer(1)]],
    device const uint  *alpha_powers       [[buffer(2)]],
    device const uint  *z_buf              [[buffer(3)]],
    device const uint  *relation_id_buf    [[buffer(4)]],
    device       uint  *trace_out          [[buffer(5)]],
    constant     uint  &n_rows             [[buffer(6)]],
    constant     uint  &split              [[buffer(7)]],
    uint               row                [[thread_position_in_grid]]
) {
    if (row >= n_rows) {
        return;
    }

    StwoMetalQm31 z = {
        z_buf[0], z_buf[1], z_buf[2], z_buf[3]
    };

    uint rel_id = relation_id_buf[0];
    uint n_logup_cols = split / 2u;

    StwoMetalQm31 running_sum = { 0u, 0u, 0u, 0u };

    for (uint col = 0u; col < n_logup_cols; ++col) {
        uint chunk0 = col * 2u;
        uint chunk1 = col * 2u + 1u;

        // Load ids for this row from each chunk.
        uint id0 = ids[chunk0 * n_rows + row];
        uint id1 = ids[chunk1 * n_rows + row];

        // Load multiplicities for this row from each chunk.
        uint mult0 = mults[chunk0 * n_rows + row];
        uint mult1 = mults[chunk1 * n_rows + row];

        // Compute addresses: addr = row + 1 + chunk * n_rows
        // (Seq::packed_at(vec_row) produces sequential row indices,
        //  +1 because of the `+ PackedM31::broadcast(M31(1))`,
        //  + chunk * n_rows for the chunk offset)
        // Values stay well within M31 range (max ~16 * 2^25 = 2^29 < 2^31-1).
        uint addr0 = row + 1u + chunk0 * n_rows;
        uint addr1 = row + 1u + chunk1 * n_rows;

        // p0 = combine([relation_id, addr0, id0])
        uint vals0[3] = { rel_id, addr0, id0 };
        StwoMetalQm31 p0 = logup_combine_a2i(alpha_powers, vals0, 3u, z);

        // p1 = combine([relation_id, addr1, id1])
        uint vals1[3] = { rel_id, addr1, id1 };
        StwoMetalQm31 p1 = logup_combine_a2i(alpha_powers, vals1, 3u, z);

        // numer = p0 * (-mult1) + p1 * (-mult0)
        StwoMetalQm31 neg_mult0 = stwo_metal_qm31_from_base(stwo_metal_m31_neg(mult0));
        StwoMetalQm31 neg_mult1 = stwo_metal_qm31_from_base(stwo_metal_m31_neg(mult1));
        StwoMetalQm31 numer = stwo_metal_qm31_add(
            stwo_metal_qm31_mul(p0, neg_mult1),
            stwo_metal_qm31_mul(p1, neg_mult0)
        );

        // denom = p1 * p0
        StwoMetalQm31 denom = stwo_metal_qm31_mul(p1, p0);

        // fraction = numer * denom^{-1}
        StwoMetalQm31 denom_inv = stwo_metal_qm31_inverse(denom);
        StwoMetalQm31 frac = stwo_metal_qm31_mul(numer, denom_inv);

        // Accumulate running sum.
        running_sum = stwo_metal_qm31_add(running_sum, frac);

        // Write this column's cumulative value.
        uint out_base = col * 4u;
        trace_out[(out_base + 0u) * n_rows + row] = running_sum.a;
        trace_out[(out_base + 1u) * n_rows + row] = running_sum.b;
        trace_out[(out_base + 2u) * n_rows + row] = running_sum.c;
        trace_out[(out_base + 3u) * n_rows + row] = running_sum.d;
    }
}
