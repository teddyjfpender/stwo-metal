// interaction_trace_fused_opcodes.metal
//
// Single-pass fused Metal kernels for opcode interaction traces.
// Each kernel reads trace columns AND computes logup fractions in one dispatch,
// eliminating the intermediate LookupData buffer and the second kernel dispatch.
//
// LookupData values are computed in registers from trace columns, then immediately
// consumed by the logup combine. No intermediate global memory traffic.
//
// Output: [n_logup_cols * 4][col_len] u32 — running-sum QM31 columns.
// The caller must still do prefix-sum + cumsum_shift on the last column.

#include "secure_field_support.h"

// ---------------------------------------------------------------------------
// M31 field helpers (local copies matching interaction_trace_opcodes_from_trace)
// ---------------------------------------------------------------------------
constant uint M31_P_FUSED = 0x7FFFFFFFu;

static inline uint m31_add_f(uint a, uint b) {
    uint s = a + b;
    return select(s, s - M31_P_FUSED, s >= M31_P_FUSED);
}

static inline uint m31_sub_f(uint a, uint b) {
    return select(a - b, a - b + M31_P_FUSED, a < b);
}

static inline uint m31_mul_f(uint a, uint b) {
    ulong prod = (ulong)a * (ulong)b;
    uint lo = (uint)(prod & 0x7FFFFFFFull);
    uint hi = (uint)(prod >> 31u);
    uint s = lo + hi;
    return select(s, s - M31_P_FUSED, s >= M31_P_FUSED);
}

// Read trace column value at (col, row) in column-major layout.
static inline uint tc_f(device const uint *trace_cols, uint col, uint row, uint col_len) {
    return trace_cols[col * col_len + row];
}

// ---------------------------------------------------------------------------
// Relation ID constants
// ---------------------------------------------------------------------------
constant uint FUSED_REL_VERIFY_INSTRUCTION = 1719106205u;
constant uint FUSED_REL_MEMORY_ADDR_TO_ID  = 1444891767u;
constant uint FUSED_REL_MEMORY_ID_TO_BIG   = 1662111297u;
constant uint FUSED_REL_OPCODES            = 428564188u;

// Common M31 constants
constant uint F_M31_0   = 0u;
constant uint F_M31_1   = 1u;
constant uint F_M31_2   = 2u;
constant uint F_M31_4   = 4u;
constant uint F_M31_8   = 8u;
constant uint F_M31_16  = 16u;
constant uint F_M31_32  = 32u;
constant uint F_M31_56  = 56u;
constant uint F_M31_68  = 68u;
constant uint F_M31_88  = 88u;
constant uint F_M31_130 = 130u;
constant uint F_M31_136 = 136u;
constant uint F_M31_256 = 256u;
constant uint F_M31_508 = 508u;
constant uint F_M31_511 = 511u;
constant uint F_M31_512 = 512u;
constant uint F_M31_262144 = 262144u;
constant uint F_M31_134217728 = 134217728u;
constant uint F_M31_536870912 = 536870912u;
constant uint F_M31_32766 = 32766u;
constant uint F_M31_32767 = 32767u;
constant uint F_M31_32768 = 32768u;
constant uint F_M31_32769 = 32769u;

// ---------------------------------------------------------------------------
// Shared logup helpers
// ---------------------------------------------------------------------------

// Load a QM31 from a contiguous array of u32 at index.
static inline StwoMetalQm31 load_ap(device const uint *alpha_powers, uint idx) {
    uint base = idx * 4u;
    return StwoMetalQm31 {
        alpha_powers[base + 0u],
        alpha_powers[base + 1u],
        alpha_powers[base + 2u],
        alpha_powers[base + 3u],
    };
}

// combine(values[0..n]) = sum(alpha_powers[i] * values[i]) - z
// where values[i] are M31 base-field elements in thread-local storage.
static inline StwoMetalQm31 fused_combine(
    device const uint *alpha_powers,
    const thread uint *values,
    uint n_values,
    StwoMetalQm31 z
) {
    StwoMetalQm31 acc = { 0u, 0u, 0u, 0u };
    for (uint i = 0u; i < n_values; ++i) {
        StwoMetalQm31 ap = load_ap(alpha_powers, i);
        acc = stwo_metal_qm31_add(acc, stwo_metal_qm31_mul_base(ap, values[i]));
    }
    return stwo_metal_qm31_sub(acc, z);
}

// Write a QM31 running sum to the output buffer.
// Output layout: [n_logup_cols * 4][col_len] column-major u32.
static inline void write_running_sum(
    device uint *output,
    uint logup_col,
    uint col_len,
    uint row,
    StwoMetalQm31 val
) {
    uint base = logup_col * 4u;
    output[(base + 0u) * col_len + row] = val.a;
    output[(base + 1u) * col_len + row] = val.b;
    output[(base + 2u) * col_len + row] = val.c;
    output[(base + 3u) * col_len + row] = val.d;
}

// Compute a pair_sum logup fraction:
//   denom0 = combine(vals0, n0)
//   denom1 = combine(vals1, n1)
//   numer  = denom0 + denom1
//   frac   = numer / (denom0 * denom1)
static inline StwoMetalQm31 fused_pair_sum_frac(
    device const uint *alpha_powers,
    const thread uint *vals0, uint n0,
    const thread uint *vals1, uint n1,
    StwoMetalQm31 z
) {
    StwoMetalQm31 d0 = fused_combine(alpha_powers, vals0, n0, z);
    StwoMetalQm31 d1 = fused_combine(alpha_powers, vals1, n1, z);
    StwoMetalQm31 numer = stwo_metal_qm31_add(d0, d1);
    StwoMetalQm31 denom = stwo_metal_qm31_mul(d0, d1);
    return stwo_metal_qm31_mul(numer, stwo_metal_qm31_inverse(denom));
}

// Compute an enabler_weighted logup fraction:
//   numer = denom0 * enabler + denom1
//   frac  = numer / (denom0 * denom1)
static inline StwoMetalQm31 fused_enabler_weighted_frac(
    device const uint *alpha_powers,
    const thread uint *vals0, uint n0,
    const thread uint *vals1, uint n1,
    StwoMetalQm31 z,
    uint enabler_val
) {
    StwoMetalQm31 d0 = fused_combine(alpha_powers, vals0, n0, z);
    StwoMetalQm31 d1 = fused_combine(alpha_powers, vals1, n1, z);
    StwoMetalQm31 weighted_d0 = stwo_metal_qm31_mul_base(d0, enabler_val);
    StwoMetalQm31 numer = stwo_metal_qm31_add(weighted_d0, d1);
    StwoMetalQm31 denom = stwo_metal_qm31_mul(d0, d1);
    return stwo_metal_qm31_mul(numer, stwo_metal_qm31_inverse(denom));
}

// Compute a single_neg logup fraction:
//   numer = -enabler
//   frac  = numer / denom0
static inline StwoMetalQm31 fused_single_neg_frac(
    device const uint *alpha_powers,
    const thread uint *vals0, uint n0,
    StwoMetalQm31 z,
    uint enabler_val
) {
    StwoMetalQm31 d0 = fused_combine(alpha_powers, vals0, n0, z);
    StwoMetalQm31 numer = stwo_metal_qm31_from_base(stwo_metal_m31_neg(enabler_val));
    return stwo_metal_qm31_mul(numer, stwo_metal_qm31_inverse(d0));
}

// ===========================================================================
// ret_opcode: 16 trace cols -> 4 logup columns
//
// Logup layout:
//   col0: pair_sum(verify_inst[8], addr_to_id_0[3])
//   col1: pair_sum(id_to_big_0[30], addr_to_id_1[3])
//   col2: enabler_weighted(id_to_big_1[30], opcodes_0[4])
//   col3: single_neg(opcodes_1[4])
// ===========================================================================
kernel void interaction_trace_fused_ret_opcode(
    device const uint *trace_cols   [[buffer(0)]],
    device const uint *alpha_powers [[buffer(1)]],
    device const uint *z_buf        [[buffer(2)]],
    device       uint *output       [[buffer(3)]],
    constant     uint &n_rows       [[buffer(4)]],
    constant     uint &col_len      [[buffer(5)]],
    uint row [[thread_position_in_grid]]
) {
    if (row >= col_len) return;

    StwoMetalQm31 z = { z_buf[0], z_buf[1], z_buf[2], z_buf[3] };
    uint enabler_val = (row < n_rows) ? 1u : 0u;

    // --- Read trace columns ---
    uint input_pc  = tc_f(trace_cols, 0, row, col_len);
    uint input_ap  = tc_f(trace_cols, 1, row, col_len);
    uint input_fp  = tc_f(trace_cols, 2, row, col_len);
    uint next_pc_id = tc_f(trace_cols, 3, row, col_len);
    uint npc0 = tc_f(trace_cols, 4, row, col_len);
    uint npc1 = tc_f(trace_cols, 5, row, col_len);
    uint npc2 = tc_f(trace_cols, 6, row, col_len);
    uint npc3 = tc_f(trace_cols, 7, row, col_len);
    uint next_fp_id = tc_f(trace_cols, 9, row, col_len);
    uint nfp0 = tc_f(trace_cols, 10, row, col_len);
    uint nfp1 = tc_f(trace_cols, 11, row, col_len);
    uint nfp2 = tc_f(trace_cols, 12, row, col_len);
    uint nfp3 = tc_f(trace_cols, 13, row, col_len);

    StwoMetalQm31 running_sum = { 0u, 0u, 0u, 0u };

    // --- Logup col 0: pair_sum(verify_instruction[8], addr_to_id_0[3]) ---
    {
        uint vi[8] = {
            FUSED_REL_VERIFY_INSTRUCTION, input_pc,
            F_M31_32766, F_M31_32767, F_M31_32767,
            F_M31_88, F_M31_130, F_M31_0
        };
        uint a2i[3] = {
            FUSED_REL_MEMORY_ADDR_TO_ID,
            m31_sub_f(input_fp, F_M31_1),
            next_pc_id
        };
        running_sum = stwo_metal_qm31_add(running_sum,
            fused_pair_sum_frac(alpha_powers, vi, 8u, a2i, 3u, z));
    }
    write_running_sum(output, 0u, col_len, row, running_sum);

    // --- Logup col 1: pair_sum(id_to_big_0[30], addr_to_id_1[3]) ---
    {
        uint i2b[30] = {
            FUSED_REL_MEMORY_ID_TO_BIG, next_pc_id,
            npc0, npc1, npc2, npc3,
            0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0
        };
        uint a2i[3] = {
            FUSED_REL_MEMORY_ADDR_TO_ID,
            m31_sub_f(input_fp, F_M31_2),
            next_fp_id
        };
        running_sum = stwo_metal_qm31_add(running_sum,
            fused_pair_sum_frac(alpha_powers, i2b, 30u, a2i, 3u, z));
    }
    write_running_sum(output, 1u, col_len, row, running_sum);

    // --- Logup col 2: enabler_weighted(id_to_big_1[30], opcodes_0[4]) ---
    {
        uint i2b[30] = {
            FUSED_REL_MEMORY_ID_TO_BIG, next_fp_id,
            nfp0, nfp1, nfp2, nfp3,
            0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0
        };
        uint opc[4] = {
            FUSED_REL_OPCODES, input_pc, input_ap, input_fp
        };
        running_sum = stwo_metal_qm31_add(running_sum,
            fused_enabler_weighted_frac(alpha_powers, i2b, 30u, opc, 4u, z, enabler_val));
    }
    write_running_sum(output, 2u, col_len, row, running_sum);

    // --- Logup col 3: single_neg(opcodes_1[4]) ---
    {
        uint next_pc_val = m31_add_f(m31_add_f(npc0, m31_mul_f(npc1, F_M31_512)),
                           m31_add_f(m31_mul_f(npc2, F_M31_262144), m31_mul_f(npc3, F_M31_134217728)));
        uint next_fp_val = m31_add_f(m31_add_f(nfp0, m31_mul_f(nfp1, F_M31_512)),
                           m31_add_f(m31_mul_f(nfp2, F_M31_262144), m31_mul_f(nfp3, F_M31_134217728)));
        uint opc[4] = {
            FUSED_REL_OPCODES, next_pc_val, input_ap, next_fp_val
        };
        running_sum = stwo_metal_qm31_add(running_sum,
            fused_single_neg_frac(alpha_powers, opc, 4u, z, enabler_val));
    }
    write_running_sum(output, 3u, col_len, row, running_sum);
}

// ===========================================================================
// jump_opcode_rel_imm: 13 trace cols -> 3 logup columns
//
// Logup layout:
//   col0: pair_sum(verify_inst[8], addr_to_id_0[3])
//   col1: enabler_weighted(id_to_big_0[30], opcodes_0[4])
//   col2: single_neg(opcodes_1[4])
// ===========================================================================
kernel void interaction_trace_fused_jump_opcode_rel_imm(
    device const uint *trace_cols   [[buffer(0)]],
    device const uint *alpha_powers [[buffer(1)]],
    device const uint *z_buf        [[buffer(2)]],
    device       uint *output       [[buffer(3)]],
    constant     uint &n_rows       [[buffer(4)]],
    constant     uint &col_len      [[buffer(5)]],
    uint row [[thread_position_in_grid]]
) {
    if (row >= col_len) return;

    StwoMetalQm31 z = { z_buf[0], z_buf[1], z_buf[2], z_buf[3] };
    uint enabler_val = (row < n_rows) ? 1u : 0u;

    // --- Read trace columns ---
    uint input_pc  = tc_f(trace_cols, 0, row, col_len);
    uint input_ap  = tc_f(trace_cols, 1, row, col_len);
    uint input_fp  = tc_f(trace_cols, 2, row, col_len);
    uint ap_update = tc_f(trace_cols, 3, row, col_len);
    uint next_pc_id = tc_f(trace_cols, 4, row, col_len);
    uint msb       = tc_f(trace_cols, 5, row, col_len);
    uint mid_set   = tc_f(trace_cols, 6, row, col_len);
    uint limb0     = tc_f(trace_cols, 7, row, col_len);
    uint limb1     = tc_f(trace_cols, 8, row, col_len);
    uint limb2     = tc_f(trace_cols, 9, row, col_len);
    uint rem_bits  = tc_f(trace_cols, 10, row, col_len);

    // decode_small_sign outputs
    uint dss2 = m31_mul_f(mid_set, F_M31_508);
    uint dss3 = m31_mul_f(mid_set, F_M31_511);
    uint dss4 = m31_sub_f(m31_mul_f(msb, F_M31_136), mid_set);
    uint dss5 = m31_mul_f(msb, F_M31_256);

    StwoMetalQm31 running_sum = { 0u, 0u, 0u, 0u };

    // --- Logup col 0: pair_sum(verify_inst[8], addr_to_id_0[3]) ---
    {
        uint vi[8] = {
            FUSED_REL_VERIFY_INSTRUCTION, input_pc,
            F_M31_32767, F_M31_32767, F_M31_32769,
            F_M31_56, m31_add_f(F_M31_4, m31_mul_f(ap_update, F_M31_32)), F_M31_0
        };
        uint a2i[3] = {
            FUSED_REL_MEMORY_ADDR_TO_ID,
            m31_add_f(input_pc, F_M31_1),
            next_pc_id
        };
        running_sum = stwo_metal_qm31_add(running_sum,
            fused_pair_sum_frac(alpha_powers, vi, 8u, a2i, 3u, z));
    }
    write_running_sum(output, 0u, col_len, row, running_sum);

    // --- Logup col 1: enabler_weighted(id_to_big_0[30], opcodes_0[4]) ---
    {
        uint i2b[30] = {
            FUSED_REL_MEMORY_ID_TO_BIG, next_pc_id,
            limb0, limb1, limb2,
            m31_add_f(rem_bits, dss2),
            dss3, dss3, dss3, dss3, dss3, dss3, dss3, dss3, dss3,
            dss3, dss3, dss3, dss3, dss3, dss3, dss3, dss3,
            dss4,
            F_M31_0, F_M31_0, F_M31_0, F_M31_0, F_M31_0,
            dss5
        };
        uint opc[4] = {
            FUSED_REL_OPCODES, input_pc, input_ap, input_fp
        };
        running_sum = stwo_metal_qm31_add(running_sum,
            fused_enabler_weighted_frac(alpha_powers, i2b, 30u, opc, 4u, z, enabler_val));
    }
    write_running_sum(output, 1u, col_len, row, running_sum);

    // --- Logup col 2: single_neg(opcodes_1[4]) ---
    {
        uint read_small = m31_sub_f(
            m31_sub_f(
                m31_add_f(m31_add_f(limb0, m31_mul_f(limb1, F_M31_512)),
                          m31_add_f(m31_mul_f(limb2, F_M31_262144), m31_mul_f(rem_bits, F_M31_134217728))),
                msb),
            m31_mul_f(F_M31_536870912, mid_set));

        uint opc[4] = {
            FUSED_REL_OPCODES,
            m31_add_f(input_pc, read_small),
            m31_add_f(input_ap, ap_update),
            input_fp
        };
        running_sum = stwo_metal_qm31_add(running_sum,
            fused_single_neg_frac(alpha_powers, opc, 4u, z, enabler_val));
    }
    write_running_sum(output, 2u, col_len, row, running_sum);
}

// ===========================================================================
// assert_eq_opcode_double_deref: 19 trace cols -> 4 logup columns
//
// Logup layout:
//   col0: pair_sum(verify_inst[8], addr_to_id_0[3])
//   col1: pair_sum(id_to_big_0[30], addr_to_id_1[3])
//   col2: enabler_weighted(addr_to_id_2[3], opcodes_0[4])
//   col3: single_neg(opcodes_1[4])
// ===========================================================================
kernel void interaction_trace_fused_assert_eq_double_deref(
    device const uint *trace_cols   [[buffer(0)]],
    device const uint *alpha_powers [[buffer(1)]],
    device const uint *z_buf        [[buffer(2)]],
    device       uint *output       [[buffer(3)]],
    constant     uint &n_rows       [[buffer(4)]],
    constant     uint &col_len      [[buffer(5)]],
    uint row [[thread_position_in_grid]]
) {
    if (row >= col_len) return;

    StwoMetalQm31 z = { z_buf[0], z_buf[1], z_buf[2], z_buf[3] };
    uint enabler_val = (row < n_rows) ? 1u : 0u;

    // --- Read trace columns ---
    uint input_pc  = tc_f(trace_cols, 0, row, col_len);
    uint input_ap  = tc_f(trace_cols, 1, row, col_len);
    uint input_fp  = tc_f(trace_cols, 2, row, col_len);
    uint offset0   = tc_f(trace_cols, 3, row, col_len);
    uint offset1   = tc_f(trace_cols, 4, row, col_len);
    uint offset2   = tc_f(trace_cols, 5, row, col_len);
    uint dst_base_fp = tc_f(trace_cols, 6, row, col_len);
    uint op0_base_fp = tc_f(trace_cols, 7, row, col_len);
    uint ap_update   = tc_f(trace_cols, 8, row, col_len);
    uint mem_dst_base = tc_f(trace_cols, 9, row, col_len);
    uint mem0_base   = tc_f(trace_cols, 10, row, col_len);
    uint mem1_base_id = tc_f(trace_cols, 11, row, col_len);
    uint mb0 = tc_f(trace_cols, 12, row, col_len);
    uint mb1 = tc_f(trace_cols, 13, row, col_len);
    uint mb2 = tc_f(trace_cols, 14, row, col_len);
    uint mb3 = tc_f(trace_cols, 15, row, col_len);
    uint dst_id = tc_f(trace_cols, 17, row, col_len);

    // Decoded offsets (signed)
    uint off0_signed = m31_sub_f(offset0, F_M31_32768);
    uint off1_signed = m31_sub_f(offset1, F_M31_32768);
    uint off2_signed = m31_sub_f(offset2, F_M31_32768);

    // Reconstructed mem1_base value
    uint mem1_base_val = m31_add_f(m31_add_f(mb0, m31_mul_f(mb1, F_M31_512)),
                         m31_add_f(m31_mul_f(mb2, F_M31_262144), m31_mul_f(mb3, F_M31_134217728)));

    StwoMetalQm31 running_sum = { 0u, 0u, 0u, 0u };

    // --- Logup col 0: pair_sum(verify_inst[8], addr_to_id_0[3]) ---
    {
        uint vi[8] = {
            FUSED_REL_VERIFY_INSTRUCTION, input_pc,
            offset0, offset1, offset2,
            m31_add_f(m31_mul_f(dst_base_fp, F_M31_8), m31_mul_f(op0_base_fp, F_M31_16)),
            m31_add_f(m31_mul_f(ap_update, F_M31_32), F_M31_256),
            F_M31_0
        };
        uint a2i[3] = {
            FUSED_REL_MEMORY_ADDR_TO_ID,
            m31_add_f(mem0_base, off1_signed),
            mem1_base_id
        };
        running_sum = stwo_metal_qm31_add(running_sum,
            fused_pair_sum_frac(alpha_powers, vi, 8u, a2i, 3u, z));
    }
    write_running_sum(output, 0u, col_len, row, running_sum);

    // --- Logup col 1: pair_sum(id_to_big_0[30], addr_to_id_1[3]) ---
    {
        uint i2b[30] = {
            FUSED_REL_MEMORY_ID_TO_BIG, mem1_base_id,
            mb0, mb1, mb2, mb3,
            0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0
        };
        uint a2i[3] = {
            FUSED_REL_MEMORY_ADDR_TO_ID,
            m31_add_f(mem_dst_base, off0_signed),
            dst_id
        };
        running_sum = stwo_metal_qm31_add(running_sum,
            fused_pair_sum_frac(alpha_powers, i2b, 30u, a2i, 3u, z));
    }
    write_running_sum(output, 1u, col_len, row, running_sum);

    // --- Logup col 2: enabler_weighted(addr_to_id_2[3], opcodes_0[4]) ---
    {
        uint a2i[3] = {
            FUSED_REL_MEMORY_ADDR_TO_ID,
            m31_add_f(mem1_base_val, off2_signed),
            dst_id
        };
        uint opc[4] = {
            FUSED_REL_OPCODES, input_pc, input_ap, input_fp
        };
        running_sum = stwo_metal_qm31_add(running_sum,
            fused_enabler_weighted_frac(alpha_powers, a2i, 3u, opc, 4u, z, enabler_val));
    }
    write_running_sum(output, 2u, col_len, row, running_sum);

    // --- Logup col 3: single_neg(opcodes_1[4]) ---
    {
        uint opc[4] = {
            FUSED_REL_OPCODES,
            m31_add_f(input_pc, F_M31_1),
            m31_add_f(input_ap, ap_update),
            input_fp
        };
        running_sum = stwo_metal_qm31_add(running_sum,
            fused_single_neg_frac(alpha_powers, opc, 4u, z, enabler_val));
    }
    write_running_sum(output, 3u, col_len, row, running_sum);
}

// ===========================================================================
// jnz_opcode_taken: 47 trace cols -> 4 logup columns
//
// Logup layout:
//   col0: pair_sum(verify_inst[8], addr_to_id_0[3])
//   col1: pair_sum(id_to_big_0[30], addr_to_id_1[3])
//   col2: enabler_weighted(id_to_big_1[30], opcodes_0[4])
//   col3: single_neg(opcodes_1[4])
// ===========================================================================
kernel void interaction_trace_fused_jnz_opcode_taken(
    device const uint *trace_cols   [[buffer(0)]],
    device const uint *alpha_powers [[buffer(1)]],
    device const uint *z_buf        [[buffer(2)]],
    device       uint *output       [[buffer(3)]],
    constant     uint &n_rows       [[buffer(4)]],
    constant     uint &col_len      [[buffer(5)]],
    uint row [[thread_position_in_grid]]
) {
    if (row >= col_len) return;

    StwoMetalQm31 z = { z_buf[0], z_buf[1], z_buf[2], z_buf[3] };
    uint enabler_val = (row < n_rows) ? 1u : 0u;

    // --- Read trace columns ---
    uint input_pc  = tc_f(trace_cols, 0, row, col_len);
    uint input_ap  = tc_f(trace_cols, 1, row, col_len);
    uint input_fp  = tc_f(trace_cols, 2, row, col_len);
    uint offset0   = tc_f(trace_cols, 3, row, col_len);
    uint dst_base_fp = tc_f(trace_cols, 4, row, col_len);
    uint ap_update = tc_f(trace_cols, 5, row, col_len);
    uint mem_dst_base = tc_f(trace_cols, 6, row, col_len);
    uint dst_id    = tc_f(trace_cols, 7, row, col_len);

    // 28 dst limbs at cols 8..35
    uint dst_limbs[28];
    for (uint i = 0; i < 28; i++) {
        dst_limbs[i] = tc_f(trace_cols, 8 + i, row, col_len);
    }

    uint next_pc_id = tc_f(trace_cols, 38, row, col_len);
    uint msb       = tc_f(trace_cols, 39, row, col_len);
    uint mid_set   = tc_f(trace_cols, 40, row, col_len);
    uint npc_limb0 = tc_f(trace_cols, 41, row, col_len);
    uint npc_limb1 = tc_f(trace_cols, 42, row, col_len);
    uint npc_limb2 = tc_f(trace_cols, 43, row, col_len);
    uint rem_bits  = tc_f(trace_cols, 44, row, col_len);

    uint off0_signed = m31_sub_f(offset0, F_M31_32768);

    // decode_small_sign
    uint dss2 = m31_mul_f(mid_set, F_M31_508);
    uint dss3 = m31_mul_f(mid_set, F_M31_511);
    uint dss4 = m31_sub_f(m31_mul_f(msb, F_M31_136), mid_set);
    uint dss5 = m31_mul_f(msb, F_M31_256);

    StwoMetalQm31 running_sum = { 0u, 0u, 0u, 0u };

    // --- Logup col 0: pair_sum(verify_inst[8], addr_to_id_0[3]) ---
    {
        uint vi[8] = {
            FUSED_REL_VERIFY_INSTRUCTION, input_pc,
            offset0, F_M31_32767, F_M31_32769,
            m31_add_f(m31_add_f(m31_mul_f(dst_base_fp, F_M31_8), F_M31_16), F_M31_32),
            m31_add_f(F_M31_8, m31_mul_f(ap_update, F_M31_32)),
            F_M31_0
        };
        uint a2i[3] = {
            FUSED_REL_MEMORY_ADDR_TO_ID,
            m31_add_f(mem_dst_base, off0_signed),
            dst_id
        };
        running_sum = stwo_metal_qm31_add(running_sum,
            fused_pair_sum_frac(alpha_powers, vi, 8u, a2i, 3u, z));
    }
    write_running_sum(output, 0u, col_len, row, running_sum);

    // --- Logup col 1: pair_sum(id_to_big_0[30], addr_to_id_1[3]) ---
    {
        uint i2b[30];
        i2b[0] = FUSED_REL_MEMORY_ID_TO_BIG;
        i2b[1] = dst_id;
        for (uint i = 0; i < 28; i++) i2b[2 + i] = dst_limbs[i];

        uint a2i[3] = {
            FUSED_REL_MEMORY_ADDR_TO_ID,
            m31_add_f(input_pc, F_M31_1),
            next_pc_id
        };
        running_sum = stwo_metal_qm31_add(running_sum,
            fused_pair_sum_frac(alpha_powers, i2b, 30u, a2i, 3u, z));
    }
    write_running_sum(output, 1u, col_len, row, running_sum);

    // --- Logup col 2: enabler_weighted(id_to_big_1[30], opcodes_0[4]) ---
    {
        uint i2b[30] = {
            FUSED_REL_MEMORY_ID_TO_BIG, next_pc_id,
            npc_limb0, npc_limb1, npc_limb2,
            m31_add_f(rem_bits, dss2),
            dss3, dss3, dss3, dss3, dss3, dss3, dss3, dss3, dss3,
            dss3, dss3, dss3, dss3, dss3, dss3, dss3, dss3,
            dss4,
            F_M31_0, F_M31_0, F_M31_0, F_M31_0, F_M31_0,
            dss5
        };
        uint opc[4] = {
            FUSED_REL_OPCODES, input_pc, input_ap, input_fp
        };
        running_sum = stwo_metal_qm31_add(running_sum,
            fused_enabler_weighted_frac(alpha_powers, i2b, 30u, opc, 4u, z, enabler_val));
    }
    write_running_sum(output, 2u, col_len, row, running_sum);

    // --- Logup col 3: single_neg(opcodes_1[4]) ---
    {
        uint read_small = m31_sub_f(
            m31_sub_f(
                m31_add_f(m31_add_f(npc_limb0, m31_mul_f(npc_limb1, F_M31_512)),
                          m31_add_f(m31_mul_f(npc_limb2, F_M31_262144), m31_mul_f(rem_bits, F_M31_134217728))),
                msb),
            m31_mul_f(F_M31_536870912, mid_set));

        uint opc[4] = {
            FUSED_REL_OPCODES,
            m31_add_f(input_pc, read_small),
            m31_add_f(input_ap, ap_update),
            input_fp
        };
        running_sum = stwo_metal_qm31_add(running_sum,
            fused_single_neg_frac(alpha_powers, opc, 4u, z, enabler_val));
    }
    write_running_sum(output, 3u, col_len, row, running_sum);
}

// ===========================================================================
// call_opcode_rel_imm: 24 trace cols -> 5 logup columns
//
// Logup layout:
//   col0: pair_sum(verify_inst[8], addr_to_id_0[3])
//   col1: pair_sum(id_to_big_0[30], addr_to_id_1[3])
//   col2: pair_sum(id_to_big_1[30], addr_to_id_2[3])
//   col3: enabler_weighted(id_to_big_2[30], opcodes_0[4])
//   col4: single_neg(opcodes_1[4])
// ===========================================================================
kernel void interaction_trace_fused_call_opcode_rel_imm(
    device const uint *trace_cols   [[buffer(0)]],
    device const uint *alpha_powers [[buffer(1)]],
    device const uint *z_buf        [[buffer(2)]],
    device       uint *output       [[buffer(3)]],
    constant     uint &n_rows       [[buffer(4)]],
    constant     uint &col_len      [[buffer(5)]],
    uint row [[thread_position_in_grid]]
) {
    if (row >= col_len) return;

    StwoMetalQm31 z = { z_buf[0], z_buf[1], z_buf[2], z_buf[3] };
    uint enabler_val = (row < n_rows) ? 1u : 0u;

    // --- Read trace columns ---
    uint input_pc  = tc_f(trace_cols, 0, row, col_len);
    uint input_ap  = tc_f(trace_cols, 1, row, col_len);
    uint input_fp  = tc_f(trace_cols, 2, row, col_len);
    uint stored_fp_id = tc_f(trace_cols, 3, row, col_len);
    uint sfp0 = tc_f(trace_cols, 4, row, col_len);
    uint sfp1 = tc_f(trace_cols, 5, row, col_len);
    uint sfp2 = tc_f(trace_cols, 6, row, col_len);
    uint sfp3 = tc_f(trace_cols, 7, row, col_len);
    uint stored_ret_pc_id = tc_f(trace_cols, 9, row, col_len);
    uint srp0 = tc_f(trace_cols, 10, row, col_len);
    uint srp1 = tc_f(trace_cols, 11, row, col_len);
    uint srp2 = tc_f(trace_cols, 12, row, col_len);
    uint srp3 = tc_f(trace_cols, 13, row, col_len);
    uint dist_id = tc_f(trace_cols, 15, row, col_len);
    uint msb     = tc_f(trace_cols, 16, row, col_len);
    uint mid_set = tc_f(trace_cols, 17, row, col_len);
    uint dl0     = tc_f(trace_cols, 18, row, col_len);
    uint dl1     = tc_f(trace_cols, 19, row, col_len);
    uint dl2     = tc_f(trace_cols, 20, row, col_len);
    uint rem_bits = tc_f(trace_cols, 21, row, col_len);

    // decode_small_sign
    uint dss2 = m31_mul_f(mid_set, F_M31_508);
    uint dss3 = m31_mul_f(mid_set, F_M31_511);
    uint dss4 = m31_sub_f(m31_mul_f(msb, F_M31_136), mid_set);
    uint dss5 = m31_mul_f(msb, F_M31_256);

    StwoMetalQm31 running_sum = { 0u, 0u, 0u, 0u };

    // --- Logup col 0: pair_sum(verify_inst[8], addr_to_id_0[3]) ---
    {
        uint vi[8] = {
            FUSED_REL_VERIFY_INSTRUCTION, input_pc,
            F_M31_32768, F_M31_32769, F_M31_32769,
            F_M31_32, F_M31_68, F_M31_0
        };
        uint a2i[3] = {
            FUSED_REL_MEMORY_ADDR_TO_ID,
            input_ap,
            stored_fp_id
        };
        running_sum = stwo_metal_qm31_add(running_sum,
            fused_pair_sum_frac(alpha_powers, vi, 8u, a2i, 3u, z));
    }
    write_running_sum(output, 0u, col_len, row, running_sum);

    // --- Logup col 1: pair_sum(id_to_big_0[30], addr_to_id_1[3]) ---
    {
        uint i2b[30] = {
            FUSED_REL_MEMORY_ID_TO_BIG, stored_fp_id,
            sfp0, sfp1, sfp2, sfp3,
            0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0
        };
        uint a2i[3] = {
            FUSED_REL_MEMORY_ADDR_TO_ID,
            m31_add_f(input_ap, F_M31_1),
            stored_ret_pc_id
        };
        running_sum = stwo_metal_qm31_add(running_sum,
            fused_pair_sum_frac(alpha_powers, i2b, 30u, a2i, 3u, z));
    }
    write_running_sum(output, 1u, col_len, row, running_sum);

    // --- Logup col 2: pair_sum(id_to_big_1[30], addr_to_id_2[3]) ---
    {
        uint i2b[30] = {
            FUSED_REL_MEMORY_ID_TO_BIG, stored_ret_pc_id,
            srp0, srp1, srp2, srp3,
            0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0
        };
        uint a2i[3] = {
            FUSED_REL_MEMORY_ADDR_TO_ID,
            m31_add_f(input_pc, F_M31_1),
            dist_id
        };
        running_sum = stwo_metal_qm31_add(running_sum,
            fused_pair_sum_frac(alpha_powers, i2b, 30u, a2i, 3u, z));
    }
    write_running_sum(output, 2u, col_len, row, running_sum);

    // --- Logup col 3: enabler_weighted(id_to_big_2[30], opcodes_0[4]) ---
    {
        uint i2b[30] = {
            FUSED_REL_MEMORY_ID_TO_BIG, dist_id,
            dl0, dl1, dl2,
            m31_add_f(rem_bits, dss2),
            dss3, dss3, dss3, dss3, dss3, dss3, dss3, dss3, dss3,
            dss3, dss3, dss3, dss3, dss3, dss3, dss3, dss3,
            dss4,
            F_M31_0, F_M31_0, F_M31_0, F_M31_0, F_M31_0,
            dss5
        };
        uint opc[4] = {
            FUSED_REL_OPCODES, input_pc, input_ap, input_fp
        };
        running_sum = stwo_metal_qm31_add(running_sum,
            fused_enabler_weighted_frac(alpha_powers, i2b, 30u, opc, 4u, z, enabler_val));
    }
    write_running_sum(output, 3u, col_len, row, running_sum);

    // --- Logup col 4: single_neg(opcodes_1[4]) ---
    {
        uint read_small = m31_sub_f(
            m31_sub_f(
                m31_add_f(m31_add_f(dl0, m31_mul_f(dl1, F_M31_512)),
                          m31_add_f(m31_mul_f(dl2, F_M31_262144), m31_mul_f(rem_bits, F_M31_134217728))),
                msb),
            m31_mul_f(F_M31_536870912, mid_set));

        uint opc[4] = {
            FUSED_REL_OPCODES,
            m31_add_f(input_pc, read_small),
            m31_add_f(input_ap, F_M31_2),
            m31_add_f(input_ap, F_M31_2)
        };
        running_sum = stwo_metal_qm31_add(running_sum,
            fused_single_neg_frac(alpha_powers, opc, 4u, z, enabler_val));
    }
    write_running_sum(output, 4u, col_len, row, running_sum);
}

// ===========================================================================
// add_opcode_small: 39 trace cols -> 5 logup columns
//
// Trace columns (from witness_opcodes.metal):
//   col0=pc, col1=ap, col2=fp
//   col3=offset0, col4=offset1, col5=offset2
//   col6=dst_base_fp, col7=op0_base_fp, col8=op1_imm, col9=op1_base_fp,
//   col10=ap_update_add_1
//   col11=mem_dst_base, col12=mem0_base, col13=mem1_base
//   col14=dst_id, col15=dst_msb, col16=dst_mid_limbs_set
//   col17..19=dst_limb[0..2], col20=dst_rem_bits, col21=dst_partial_limb_msb
//   col22=op0_id, col23=op0_msb, col24=op0_mid_limbs_set
//   col25..27=op0_limb[0..2], col28=op0_rem_bits, col29=op0_partial_limb_msb
//   col30=op1_id, col31=op1_msb, col32=op1_mid_limbs_set
//   col33..35=op1_limb[0..2], col36=op1_rem_bits, col37=op1_partial_limb_msb
//   col38=enabler
//
// Logup layout:
//   col0: pair_sum(verify_inst[8], addr_to_id_0[3])
//   col1: pair_sum(id_to_big_0[30], addr_to_id_1[3])
//   col2: pair_sum(id_to_big_1[30], addr_to_id_2[3])
//   col3: enabler_weighted(id_to_big_2[30], opcodes_0[4])
//   col4: single_neg(opcodes_1[4])
// ===========================================================================

constant uint F_M31_64  = 64u;
constant uint F_M31_128 = 128u;

kernel void interaction_trace_fused_add_opcode_small(
    device const uint *trace_cols   [[buffer(0)]],
    device const uint *alpha_powers [[buffer(1)]],
    device const uint *z_buf        [[buffer(2)]],
    device       uint *output       [[buffer(3)]],
    constant     uint &n_rows       [[buffer(4)]],
    constant     uint &col_len      [[buffer(5)]],
    uint row [[thread_position_in_grid]]
) {
    if (row >= col_len) return;

    StwoMetalQm31 z = { z_buf[0], z_buf[1], z_buf[2], z_buf[3] };
    uint enabler_val = (row < n_rows) ? 1u : 0u;

    // --- Read trace columns ---
    uint input_pc  = tc_f(trace_cols, 0, row, col_len);
    uint input_ap  = tc_f(trace_cols, 1, row, col_len);
    uint input_fp  = tc_f(trace_cols, 2, row, col_len);
    uint offset0   = tc_f(trace_cols, 3, row, col_len);
    uint offset1   = tc_f(trace_cols, 4, row, col_len);
    uint offset2   = tc_f(trace_cols, 5, row, col_len);
    uint dst_base_fp = tc_f(trace_cols, 6, row, col_len);
    uint op0_base_fp = tc_f(trace_cols, 7, row, col_len);
    uint op1_imm     = tc_f(trace_cols, 8, row, col_len);
    uint op1_base_fp = tc_f(trace_cols, 9, row, col_len);
    uint ap_update   = tc_f(trace_cols, 10, row, col_len);
    uint mem_dst_base = tc_f(trace_cols, 11, row, col_len);
    uint mem0_base   = tc_f(trace_cols, 12, row, col_len);
    uint mem1_base   = tc_f(trace_cols, 13, row, col_len);
    uint dst_id      = tc_f(trace_cols, 14, row, col_len);
    uint dst_msb     = tc_f(trace_cols, 15, row, col_len);
    uint dst_mid_set = tc_f(trace_cols, 16, row, col_len);
    uint dst_limb0   = tc_f(trace_cols, 17, row, col_len);
    uint dst_limb1   = tc_f(trace_cols, 18, row, col_len);
    uint dst_limb2   = tc_f(trace_cols, 19, row, col_len);
    uint dst_rem     = tc_f(trace_cols, 20, row, col_len);
    uint op0_id      = tc_f(trace_cols, 22, row, col_len);
    uint op0_msb     = tc_f(trace_cols, 23, row, col_len);
    uint op0_mid_set = tc_f(trace_cols, 24, row, col_len);
    uint op0_limb0   = tc_f(trace_cols, 25, row, col_len);
    uint op0_limb1   = tc_f(trace_cols, 26, row, col_len);
    uint op0_limb2   = tc_f(trace_cols, 27, row, col_len);
    uint op0_rem     = tc_f(trace_cols, 28, row, col_len);
    uint op1_id      = tc_f(trace_cols, 30, row, col_len);
    uint op1_msb     = tc_f(trace_cols, 31, row, col_len);
    uint op1_mid_set = tc_f(trace_cols, 32, row, col_len);
    uint op1_limb0   = tc_f(trace_cols, 33, row, col_len);
    uint op1_limb1   = tc_f(trace_cols, 34, row, col_len);
    uint op1_limb2   = tc_f(trace_cols, 35, row, col_len);
    uint op1_rem     = tc_f(trace_cols, 36, row, col_len);

    // Decoded offsets (signed)
    uint off0_signed = m31_sub_f(offset0, F_M31_32768);
    uint off1_signed = m31_sub_f(offset1, F_M31_32768);
    uint off2_signed = m31_sub_f(offset2, F_M31_32768);

    // decode_small_sign for dst
    uint dst_dss2 = m31_mul_f(dst_mid_set, F_M31_508);
    uint dst_dss3 = m31_mul_f(dst_mid_set, F_M31_511);
    uint dst_dss4 = m31_sub_f(m31_mul_f(dst_msb, F_M31_136), dst_mid_set);
    uint dst_dss5 = m31_mul_f(dst_msb, F_M31_256);

    // decode_small_sign for op0
    uint op0_dss2 = m31_mul_f(op0_mid_set, F_M31_508);
    uint op0_dss3 = m31_mul_f(op0_mid_set, F_M31_511);
    uint op0_dss4 = m31_sub_f(m31_mul_f(op0_msb, F_M31_136), op0_mid_set);
    uint op0_dss5 = m31_mul_f(op0_msb, F_M31_256);

    // decode_small_sign for op1
    uint op1_dss2 = m31_mul_f(op1_mid_set, F_M31_508);
    uint op1_dss3 = m31_mul_f(op1_mid_set, F_M31_511);
    uint op1_dss4 = m31_sub_f(m31_mul_f(op1_msb, F_M31_136), op1_mid_set);
    uint op1_dss5 = m31_mul_f(op1_msb, F_M31_256);

    // op1_base_ap = 1 - op1_imm - op1_base_fp
    uint op1_base_ap = m31_sub_f(m31_sub_f(F_M31_1, op1_imm), op1_base_fp);

    StwoMetalQm31 running_sum = { 0u, 0u, 0u, 0u };

    // --- Logup col 0: pair_sum(verify_inst[8], addr_to_id_0[3]) ---
    // verify_instruction_0: [rel_id, pc, offset0, offset1, offset2,
    //   dst_base_fp*8 + op0_base_fp*16 + op1_imm*32 + op1_base_fp*64 + op1_base_ap*128 + 256,
    //   ap_update_add_1*32 + 256, 0]
    {
        uint flags_val = m31_add_f(
            m31_add_f(
                m31_add_f(m31_mul_f(dst_base_fp, F_M31_8), m31_mul_f(op0_base_fp, F_M31_16)),
                m31_add_f(m31_mul_f(op1_imm, F_M31_32), m31_mul_f(op1_base_fp, F_M31_64))
            ),
            m31_add_f(m31_mul_f(op1_base_ap, F_M31_128), F_M31_256)
        );
        uint vi[8] = {
            FUSED_REL_VERIFY_INSTRUCTION, input_pc,
            offset0, offset1, offset2,
            flags_val,
            m31_add_f(m31_mul_f(ap_update, F_M31_32), F_M31_256),
            F_M31_0
        };
        uint a2i[3] = {
            FUSED_REL_MEMORY_ADDR_TO_ID,
            m31_add_f(mem_dst_base, off0_signed),
            dst_id
        };
        running_sum = stwo_metal_qm31_add(running_sum,
            fused_pair_sum_frac(alpha_powers, vi, 8u, a2i, 3u, z));
    }
    write_running_sum(output, 0u, col_len, row, running_sum);

    // --- Logup col 1: pair_sum(id_to_big_0[30], addr_to_id_1[3]) ---
    // id_to_big_0 for dst (small-sign pattern):
    //   [rel_id, dst_id, dst_limb0..2, dst_rem+dss2, dss3*17, dss4, 0*5, dss5]
    {
        uint i2b[30] = {
            FUSED_REL_MEMORY_ID_TO_BIG, dst_id,
            dst_limb0, dst_limb1, dst_limb2,
            m31_add_f(dst_rem, dst_dss2),
            dst_dss3, dst_dss3, dst_dss3, dst_dss3, dst_dss3,
            dst_dss3, dst_dss3, dst_dss3, dst_dss3, dst_dss3,
            dst_dss3, dst_dss3, dst_dss3, dst_dss3, dst_dss3,
            dst_dss3, dst_dss3,
            dst_dss4,
            F_M31_0, F_M31_0, F_M31_0, F_M31_0, F_M31_0,
            dst_dss5
        };
        uint a2i[3] = {
            FUSED_REL_MEMORY_ADDR_TO_ID,
            m31_add_f(mem0_base, off1_signed),
            op0_id
        };
        running_sum = stwo_metal_qm31_add(running_sum,
            fused_pair_sum_frac(alpha_powers, i2b, 30u, a2i, 3u, z));
    }
    write_running_sum(output, 1u, col_len, row, running_sum);

    // --- Logup col 2: pair_sum(id_to_big_1[30], addr_to_id_2[3]) ---
    // id_to_big_1 for op0 (small-sign pattern):
    {
        uint i2b[30] = {
            FUSED_REL_MEMORY_ID_TO_BIG, op0_id,
            op0_limb0, op0_limb1, op0_limb2,
            m31_add_f(op0_rem, op0_dss2),
            op0_dss3, op0_dss3, op0_dss3, op0_dss3, op0_dss3,
            op0_dss3, op0_dss3, op0_dss3, op0_dss3, op0_dss3,
            op0_dss3, op0_dss3, op0_dss3, op0_dss3, op0_dss3,
            op0_dss3, op0_dss3,
            op0_dss4,
            F_M31_0, F_M31_0, F_M31_0, F_M31_0, F_M31_0,
            op0_dss5
        };
        uint a2i[3] = {
            FUSED_REL_MEMORY_ADDR_TO_ID,
            m31_add_f(mem1_base, off2_signed),
            op1_id
        };
        running_sum = stwo_metal_qm31_add(running_sum,
            fused_pair_sum_frac(alpha_powers, i2b, 30u, a2i, 3u, z));
    }
    write_running_sum(output, 2u, col_len, row, running_sum);

    // --- Logup col 3: enabler_weighted(id_to_big_2[30], opcodes_0[4]) ---
    // id_to_big_2 for op1 (small-sign pattern):
    {
        uint i2b[30] = {
            FUSED_REL_MEMORY_ID_TO_BIG, op1_id,
            op1_limb0, op1_limb1, op1_limb2,
            m31_add_f(op1_rem, op1_dss2),
            op1_dss3, op1_dss3, op1_dss3, op1_dss3, op1_dss3,
            op1_dss3, op1_dss3, op1_dss3, op1_dss3, op1_dss3,
            op1_dss3, op1_dss3, op1_dss3, op1_dss3, op1_dss3,
            op1_dss3, op1_dss3,
            op1_dss4,
            F_M31_0, F_M31_0, F_M31_0, F_M31_0, F_M31_0,
            op1_dss5
        };
        uint opc[4] = {
            FUSED_REL_OPCODES, input_pc, input_ap, input_fp
        };
        running_sum = stwo_metal_qm31_add(running_sum,
            fused_enabler_weighted_frac(alpha_powers, i2b, 30u, opc, 4u, z, enabler_val));
    }
    write_running_sum(output, 3u, col_len, row, running_sum);

    // --- Logup col 4: single_neg(opcodes_1[4]) ---
    // opcodes_1: [rel_id, pc + 1 + op1_imm, ap + ap_update_add_1, fp]
    {
        uint opc[4] = {
            FUSED_REL_OPCODES,
            m31_add_f(m31_add_f(input_pc, F_M31_1), op1_imm),
            m31_add_f(input_ap, ap_update),
            input_fp
        };
        running_sum = stwo_metal_qm31_add(running_sum,
            fused_single_neg_frac(alpha_powers, opc, 4u, z, enabler_val));
    }
    write_running_sum(output, 4u, col_len, row, running_sum);
}
