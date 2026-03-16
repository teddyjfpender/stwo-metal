// interaction_trace_opcodes_from_trace.metal
//
// Metal compute kernels that read GPU-resident opcode trace columns and produce
// flat LookupData-equivalent values for the generic interaction trace kernel.
//
// Each kernel takes:
//   buffer(0) — trace_cols: column-major [N_TRACE_COLS][col_len] uint
//   buffer(1) — output:     column-major [N_OUTPUT_FIELDS][col_len] uint
//   buffer(2) — n_rows:     scalar (number of real rows, for enabler)
//   buffer(3) — col_len:    scalar (padded power-of-two column length)
//
// One thread per row (over col_len, including padding rows).

#include <metal_stdlib>
using namespace metal;

constant uint M31_P = 0x7FFFFFFFu;

// ---------------------------------------------------------------------------
// M31 field helpers
// ---------------------------------------------------------------------------
static inline uint m31_add(uint a, uint b) {
    uint s = a + b;
    return select(s, s - M31_P, s >= M31_P);
}

static inline uint m31_sub(uint a, uint b) {
    return select(a - b, a - b + M31_P, a < b);
}

static inline uint m31_mul(uint a, uint b) {
    ulong prod = (ulong)a * (ulong)b;
    uint lo = (uint)(prod & 0x7FFFFFFFull);
    uint hi = (uint)(prod >> 31u);
    uint s = lo + hi;
    return select(s, s - M31_P, s >= M31_P);
}

// Read trace column value at (col, row) in column-major layout.
static inline uint tc(device const uint *trace_cols, uint col, uint row, uint col_len) {
    return trace_cols[col * col_len + row];
}

// Write output value at (field, row) in column-major layout.
static inline void out_set(device uint *output, uint field, uint row, uint col_len, uint val) {
    output[field * col_len + row] = val;
}

// ---------------------------------------------------------------------------
// Relation ID constants (hashes of relation names)
// ---------------------------------------------------------------------------
constant uint REL_VERIFY_INSTRUCTION = 1719106205u;
constant uint REL_MEMORY_ADDR_TO_ID = 1444891767u;
constant uint REL_MEMORY_ID_TO_BIG  = 1662111297u;
constant uint REL_OPCODES           = 428564188u;

// Common M31 constants
constant uint M31_0   = 0u;
constant uint M31_1   = 1u;
constant uint M31_2   = 2u;
constant uint M31_4   = 4u;
constant uint M31_8   = 8u;
constant uint M31_16  = 16u;
constant uint M31_32  = 32u;
constant uint M31_56  = 56u;
constant uint M31_68  = 68u;
constant uint M31_88  = 88u;
constant uint M31_130 = 130u;
constant uint M31_136 = 136u;
constant uint M31_256 = 256u;
constant uint M31_508 = 508u;
constant uint M31_511 = 511u;
constant uint M31_512 = 512u;
constant uint M31_262144 = 262144u;
constant uint M31_134217728 = 134217728u;
constant uint M31_536870912 = 536870912u;
constant uint M31_32766 = 32766u;
constant uint M31_32767 = 32767u;
constant uint M31_32768 = 32768u;
constant uint M31_32769 = 32769u;

// Negations mod P = 2^31 - 1
constant uint M31_NEG_1 = 2147483646u; // P - 1
constant uint M31_NEG_2 = 2147483645u; // P - 2

// ===========================================================================
// ret_opcode: 16 trace cols -> 82 output values
//
// LookupData layout (flat output):
//   [0..8)   verify_instruction_0  (8 values)
//   [8..11)  memory_address_to_id_0 (3 values)
//   [11..41) memory_id_to_big_0    (30 values)
//   [41..44) memory_address_to_id_1 (3 values)
//   [44..74) memory_id_to_big_1    (30 values)
//   [74..78) opcodes_0             (4 values)
//   [78..82) opcodes_1             (4 values)
// Total: 82
// ===========================================================================
kernel void interaction_values_ret_opcode(
    device const uint *trace_cols [[buffer(0)]],
    device       uint *output     [[buffer(1)]],
    constant     uint &n_rows     [[buffer(2)]],
    constant     uint &col_len    [[buffer(3)]],
    uint row [[thread_position_in_grid]]
) {
    if (row >= col_len) return;

    // Trace columns:
    // col0=pc, col1=ap, col2=fp, col3=next_pc_id, col4..7=next_pc_limbs,
    // col8=partial_limb_msb, col9=next_fp_id, col10..13=next_fp_limbs,
    // col14=partial_limb_msb2, col15=enabler
    uint input_pc  = tc(trace_cols, 0, row, col_len);
    uint input_ap  = tc(trace_cols, 1, row, col_len);
    uint input_fp  = tc(trace_cols, 2, row, col_len);
    uint next_pc_id = tc(trace_cols, 3, row, col_len);
    uint npc0 = tc(trace_cols, 4, row, col_len);
    uint npc1 = tc(trace_cols, 5, row, col_len);
    uint npc2 = tc(trace_cols, 6, row, col_len);
    uint npc3 = tc(trace_cols, 7, row, col_len);
    uint next_fp_id = tc(trace_cols, 9, row, col_len);
    uint nfp0 = tc(trace_cols, 10, row, col_len);
    uint nfp1 = tc(trace_cols, 11, row, col_len);
    uint nfp2 = tc(trace_cols, 12, row, col_len);
    uint nfp3 = tc(trace_cols, 13, row, col_len);

    uint idx = 0;

    // verify_instruction_0: [rel_id, pc, 32766, 32767, 32767, 88, 130, 0]
    out_set(output, idx++, row, col_len, REL_VERIFY_INSTRUCTION);
    out_set(output, idx++, row, col_len, input_pc);
    out_set(output, idx++, row, col_len, M31_32766);
    out_set(output, idx++, row, col_len, M31_32767);
    out_set(output, idx++, row, col_len, M31_32767);
    out_set(output, idx++, row, col_len, M31_88);
    out_set(output, idx++, row, col_len, M31_130);
    out_set(output, idx++, row, col_len, M31_0);

    // memory_address_to_id_0: [rel_id, fp - 1, next_pc_id]
    out_set(output, idx++, row, col_len, REL_MEMORY_ADDR_TO_ID);
    out_set(output, idx++, row, col_len, m31_sub(input_fp, M31_1));
    out_set(output, idx++, row, col_len, next_pc_id);

    // memory_id_to_big_0: [rel_id, next_pc_id, npc0..3, 0*24]
    out_set(output, idx++, row, col_len, REL_MEMORY_ID_TO_BIG);
    out_set(output, idx++, row, col_len, next_pc_id);
    out_set(output, idx++, row, col_len, npc0);
    out_set(output, idx++, row, col_len, npc1);
    out_set(output, idx++, row, col_len, npc2);
    out_set(output, idx++, row, col_len, npc3);
    for (uint i = 0; i < 24; i++) out_set(output, idx++, row, col_len, M31_0);

    // memory_address_to_id_1: [rel_id, fp - 2, next_fp_id]
    out_set(output, idx++, row, col_len, REL_MEMORY_ADDR_TO_ID);
    out_set(output, idx++, row, col_len, m31_sub(input_fp, M31_2));
    out_set(output, idx++, row, col_len, next_fp_id);

    // memory_id_to_big_1: [rel_id, next_fp_id, nfp0..3, 0*24]
    out_set(output, idx++, row, col_len, REL_MEMORY_ID_TO_BIG);
    out_set(output, idx++, row, col_len, next_fp_id);
    out_set(output, idx++, row, col_len, nfp0);
    out_set(output, idx++, row, col_len, nfp1);
    out_set(output, idx++, row, col_len, nfp2);
    out_set(output, idx++, row, col_len, nfp3);
    for (uint i = 0; i < 24; i++) out_set(output, idx++, row, col_len, M31_0);

    // opcodes_0: [rel_id, pc, ap, fp]
    out_set(output, idx++, row, col_len, REL_OPCODES);
    out_set(output, idx++, row, col_len, input_pc);
    out_set(output, idx++, row, col_len, input_ap);
    out_set(output, idx++, row, col_len, input_fp);

    // opcodes_1: [rel_id, reconstructed_next_pc, ap, reconstructed_next_fp]
    // next_pc = npc0 + npc1*512 + npc2*262144 + npc3*134217728
    uint next_pc_val = m31_add(m31_add(npc0, m31_mul(npc1, M31_512)),
                       m31_add(m31_mul(npc2, M31_262144), m31_mul(npc3, M31_134217728)));
    uint next_fp_val = m31_add(m31_add(nfp0, m31_mul(nfp1, M31_512)),
                       m31_add(m31_mul(nfp2, M31_262144), m31_mul(nfp3, M31_134217728)));
    out_set(output, idx++, row, col_len, REL_OPCODES);
    out_set(output, idx++, row, col_len, next_pc_val);
    out_set(output, idx++, row, col_len, input_ap);
    out_set(output, idx++, row, col_len, next_fp_val);
    // Total: 82
}

// ===========================================================================
// jump_opcode_rel_imm: 13 trace cols -> 49 output values
//
// LookupData layout:
//   [0..8)   verify_instruction_0  (8)
//   [8..11)  memory_address_to_id_0 (3)
//   [11..41) memory_id_to_big_0    (30)
//   [41..45) opcodes_0             (4)
//   [45..49) opcodes_1             (4)
// Total: 49
// ===========================================================================
kernel void interaction_values_jump_opcode_rel_imm(
    device const uint *trace_cols [[buffer(0)]],
    device       uint *output     [[buffer(1)]],
    constant     uint &n_rows     [[buffer(2)]],
    constant     uint &col_len    [[buffer(3)]],
    uint row [[thread_position_in_grid]]
) {
    if (row >= col_len) return;

    // Trace columns:
    // col0=pc, col1=ap, col2=fp, col3=ap_update_add_1, col4=next_pc_id,
    // col5=msb, col6=mid_limbs_set, col7=limb0, col8=limb1, col9=limb2,
    // col10=remainder_bits, col11=partial_limb_msb, col12=enabler
    uint input_pc  = tc(trace_cols, 0, row, col_len);
    uint input_ap  = tc(trace_cols, 1, row, col_len);
    uint input_fp  = tc(trace_cols, 2, row, col_len);
    uint ap_update = tc(trace_cols, 3, row, col_len);
    uint next_pc_id = tc(trace_cols, 4, row, col_len);
    uint msb       = tc(trace_cols, 5, row, col_len);
    uint mid_set   = tc(trace_cols, 6, row, col_len);
    uint limb0     = tc(trace_cols, 7, row, col_len);
    uint limb1     = tc(trace_cols, 8, row, col_len);
    uint limb2     = tc(trace_cols, 9, row, col_len);
    uint rem_bits  = tc(trace_cols, 10, row, col_len);

    // decode_small_sign outputs
    uint dss2 = m31_mul(mid_set, M31_508);  // mid_limbs_set * 508
    uint dss3 = m31_mul(mid_set, M31_511);  // mid_limbs_set * 511
    uint dss4 = m31_sub(m31_mul(msb, M31_136), mid_set); // msb*136 - mid_limbs_set
    uint dss5 = m31_mul(msb, M31_256);      // msb * 256

    uint idx = 0;

    // verify_instruction_0: [rel_id, pc, 32767, 32767, 32769, 56, 4+ap_update*32, 0]
    out_set(output, idx++, row, col_len, REL_VERIFY_INSTRUCTION);
    out_set(output, idx++, row, col_len, input_pc);
    out_set(output, idx++, row, col_len, M31_32767);
    out_set(output, idx++, row, col_len, M31_32767);
    out_set(output, idx++, row, col_len, M31_32769);
    out_set(output, idx++, row, col_len, M31_56);
    out_set(output, idx++, row, col_len, m31_add(M31_4, m31_mul(ap_update, M31_32)));
    out_set(output, idx++, row, col_len, M31_0);

    // memory_address_to_id_0: [rel_id, pc+1, next_pc_id]
    out_set(output, idx++, row, col_len, REL_MEMORY_ADDR_TO_ID);
    out_set(output, idx++, row, col_len, m31_add(input_pc, M31_1));
    out_set(output, idx++, row, col_len, next_pc_id);

    // memory_id_to_big_0: [rel_id, next_pc_id, limb0, limb1, limb2,
    //   remainder_bits+dss2, dss3*17, dss4, 0*5, dss5]
    out_set(output, idx++, row, col_len, REL_MEMORY_ID_TO_BIG);
    out_set(output, idx++, row, col_len, next_pc_id);
    out_set(output, idx++, row, col_len, limb0);
    out_set(output, idx++, row, col_len, limb1);
    out_set(output, idx++, row, col_len, limb2);
    out_set(output, idx++, row, col_len, m31_add(rem_bits, dss2));
    // 17 copies of dss3 (indices 6..22 of memory_id_to_big_0)
    for (uint i = 0; i < 17; i++) out_set(output, idx++, row, col_len, dss3);
    out_set(output, idx++, row, col_len, dss4); // index 23
    out_set(output, idx++, row, col_len, M31_0);
    out_set(output, idx++, row, col_len, M31_0);
    out_set(output, idx++, row, col_len, M31_0);
    out_set(output, idx++, row, col_len, M31_0);
    out_set(output, idx++, row, col_len, M31_0);
    out_set(output, idx++, row, col_len, dss5); // index 29

    // opcodes_0: [rel_id, pc, ap, fp]
    out_set(output, idx++, row, col_len, REL_OPCODES);
    out_set(output, idx++, row, col_len, input_pc);
    out_set(output, idx++, row, col_len, input_ap);
    out_set(output, idx++, row, col_len, input_fp);

    // opcodes_1: [rel_id, pc + read_small_value, ap + ap_update, fp]
    // read_small = limb0 + limb1*512 + limb2*262144 + rem_bits*134217728 - msb - 536870912*mid_set
    uint read_small = m31_sub(
        m31_sub(
            m31_add(m31_add(limb0, m31_mul(limb1, M31_512)),
                    m31_add(m31_mul(limb2, M31_262144), m31_mul(rem_bits, M31_134217728))),
            msb),
        m31_mul(M31_536870912, mid_set));

    out_set(output, idx++, row, col_len, REL_OPCODES);
    out_set(output, idx++, row, col_len, m31_add(input_pc, read_small));
    out_set(output, idx++, row, col_len, m31_add(input_ap, ap_update));
    out_set(output, idx++, row, col_len, input_fp);
    // Total: 49
}

// ===========================================================================
// assert_eq_opcode_double_deref: 19 trace cols -> 55 output values
//
// LookupData layout:
//   [0..8)   verify_instruction_0     (8)
//   [8..11)  memory_address_to_id_0   (3)
//   [11..41) memory_id_to_big_0       (30)
//   [41..44) memory_address_to_id_1   (3)
//   [44..47) memory_address_to_id_2   (3)
//   [47..51) opcodes_0                (4)
//   [51..55) opcodes_1                (4)
// Total: 55
// ===========================================================================
kernel void interaction_values_assert_eq_double_deref(
    device const uint *trace_cols [[buffer(0)]],
    device       uint *output     [[buffer(1)]],
    constant     uint &n_rows     [[buffer(2)]],
    constant     uint &col_len    [[buffer(3)]],
    uint row [[thread_position_in_grid]]
) {
    if (row >= col_len) return;

    // Trace columns:
    // col0=pc, col1=ap, col2=fp, col3=offset0, col4=offset1, col5=offset2,
    // col6=dst_base_fp, col7=op0_base_fp, col8=ap_update_add_1,
    // col9=mem_dst_base, col10=mem0_base, col11=mem1_base_id,
    // col12..15=mem1_base_limbs[0..3], col16=partial_limb_msb,
    // col17=dst_id, col18=enabler
    uint input_pc  = tc(trace_cols, 0, row, col_len);
    uint input_ap  = tc(trace_cols, 1, row, col_len);
    uint input_fp  = tc(trace_cols, 2, row, col_len);
    uint offset0   = tc(trace_cols, 3, row, col_len);
    uint offset1   = tc(trace_cols, 4, row, col_len);
    uint offset2   = tc(trace_cols, 5, row, col_len);
    uint dst_base_fp = tc(trace_cols, 6, row, col_len);
    uint op0_base_fp = tc(trace_cols, 7, row, col_len);
    uint ap_update   = tc(trace_cols, 8, row, col_len);
    uint mem_dst_base = tc(trace_cols, 9, row, col_len);
    uint mem0_base   = tc(trace_cols, 10, row, col_len);
    uint mem1_base_id = tc(trace_cols, 11, row, col_len);
    uint mb0 = tc(trace_cols, 12, row, col_len);
    uint mb1 = tc(trace_cols, 13, row, col_len);
    uint mb2 = tc(trace_cols, 14, row, col_len);
    uint mb3 = tc(trace_cols, 15, row, col_len);
    uint dst_id = tc(trace_cols, 17, row, col_len);

    // Decoded offsets (signed)
    uint off0_signed = m31_sub(offset0, M31_32768);
    uint off1_signed = m31_sub(offset1, M31_32768);
    uint off2_signed = m31_sub(offset2, M31_32768);

    // Reconstructed mem1_base value
    uint mem1_base_val = m31_add(m31_add(mb0, m31_mul(mb1, M31_512)),
                         m31_add(m31_mul(mb2, M31_262144), m31_mul(mb3, M31_134217728)));

    uint idx = 0;

    // verify_instruction_0: [rel_id, pc, offset0, offset1, offset2,
    //   dst_base_fp*8 + op0_base_fp*16, ap_update*32 + 256, 0]
    out_set(output, idx++, row, col_len, REL_VERIFY_INSTRUCTION);
    out_set(output, idx++, row, col_len, input_pc);
    out_set(output, idx++, row, col_len, offset0);
    out_set(output, idx++, row, col_len, offset1);
    out_set(output, idx++, row, col_len, offset2);
    out_set(output, idx++, row, col_len, m31_add(m31_mul(dst_base_fp, M31_8), m31_mul(op0_base_fp, M31_16)));
    out_set(output, idx++, row, col_len, m31_add(m31_mul(ap_update, M31_32), M31_256));
    out_set(output, idx++, row, col_len, M31_0);

    // memory_address_to_id_0: [rel_id, mem0_base + off1_signed, mem1_base_id]
    out_set(output, idx++, row, col_len, REL_MEMORY_ADDR_TO_ID);
    out_set(output, idx++, row, col_len, m31_add(mem0_base, off1_signed));
    out_set(output, idx++, row, col_len, mem1_base_id);

    // memory_id_to_big_0: [rel_id, mem1_base_id, mb0..3, 0*24]
    out_set(output, idx++, row, col_len, REL_MEMORY_ID_TO_BIG);
    out_set(output, idx++, row, col_len, mem1_base_id);
    out_set(output, idx++, row, col_len, mb0);
    out_set(output, idx++, row, col_len, mb1);
    out_set(output, idx++, row, col_len, mb2);
    out_set(output, idx++, row, col_len, mb3);
    for (uint i = 0; i < 24; i++) out_set(output, idx++, row, col_len, M31_0);

    // memory_address_to_id_1: [rel_id, mem_dst_base + off0_signed, dst_id]
    out_set(output, idx++, row, col_len, REL_MEMORY_ADDR_TO_ID);
    out_set(output, idx++, row, col_len, m31_add(mem_dst_base, off0_signed));
    out_set(output, idx++, row, col_len, dst_id);

    // memory_address_to_id_2: [rel_id, mem1_base_val + off2_signed, dst_id]
    out_set(output, idx++, row, col_len, REL_MEMORY_ADDR_TO_ID);
    out_set(output, idx++, row, col_len, m31_add(mem1_base_val, off2_signed));
    out_set(output, idx++, row, col_len, dst_id);

    // opcodes_0: [rel_id, pc, ap, fp]
    out_set(output, idx++, row, col_len, REL_OPCODES);
    out_set(output, idx++, row, col_len, input_pc);
    out_set(output, idx++, row, col_len, input_ap);
    out_set(output, idx++, row, col_len, input_fp);

    // opcodes_1: [rel_id, pc+1, ap+ap_update, fp]
    out_set(output, idx++, row, col_len, REL_OPCODES);
    out_set(output, idx++, row, col_len, m31_add(input_pc, M31_1));
    out_set(output, idx++, row, col_len, m31_add(input_ap, ap_update));
    out_set(output, idx++, row, col_len, input_fp);
    // Total: 55
}

// ===========================================================================
// jnz_opcode_taken: 47 trace cols -> 82 output values
//
// LookupData layout:
//   [0..8)   verify_instruction_0     (8)
//   [8..11)  memory_address_to_id_0   (3)
//   [11..41) memory_id_to_big_0       (30) — full 252-bit dst
//   [41..44) memory_address_to_id_1   (3)
//   [44..74) memory_id_to_big_1       (30) — small-sign next_pc
//   [74..78) opcodes_0                (4)
//   [78..82) opcodes_1                (4)
// Total: 82
// ===========================================================================
kernel void interaction_values_jnz_opcode_taken(
    device const uint *trace_cols [[buffer(0)]],
    device       uint *output     [[buffer(1)]],
    constant     uint &n_rows     [[buffer(2)]],
    constant     uint &col_len    [[buffer(3)]],
    uint row [[thread_position_in_grid]]
) {
    if (row >= col_len) return;

    // Trace columns:
    // col0=pc, col1=ap, col2=fp, col3=offset0, col4=dst_base_fp,
    // col5=ap_update_add_1, col6=mem_dst_base, col7=dst_id,
    // col8..35=dst_limb_0..27,
    // col36=dst_sum_inv, col37=dst_sum_squares_inv,
    // col38=next_pc_id, col39=msb, col40=mid_limbs_set,
    // col41..43=next_pc_limbs[0..2], col44=remainder_bits,
    // col45=partial_limb_msb, col46=enabler
    uint input_pc  = tc(trace_cols, 0, row, col_len);
    uint input_ap  = tc(trace_cols, 1, row, col_len);
    uint input_fp  = tc(trace_cols, 2, row, col_len);
    uint offset0   = tc(trace_cols, 3, row, col_len);
    uint dst_base_fp = tc(trace_cols, 4, row, col_len);
    uint ap_update = tc(trace_cols, 5, row, col_len);
    uint mem_dst_base = tc(trace_cols, 6, row, col_len);
    uint dst_id    = tc(trace_cols, 7, row, col_len);

    // 28 dst limbs at cols 8..35
    uint dst_limbs[28];
    for (uint i = 0; i < 28; i++) {
        dst_limbs[i] = tc(trace_cols, 8 + i, row, col_len);
    }

    uint next_pc_id = tc(trace_cols, 38, row, col_len);
    uint msb       = tc(trace_cols, 39, row, col_len);
    uint mid_set   = tc(trace_cols, 40, row, col_len);
    uint npc_limb0 = tc(trace_cols, 41, row, col_len);
    uint npc_limb1 = tc(trace_cols, 42, row, col_len);
    uint npc_limb2 = tc(trace_cols, 43, row, col_len);
    uint rem_bits  = tc(trace_cols, 44, row, col_len);

    uint off0_signed = m31_sub(offset0, M31_32768);

    // decode_small_sign
    uint dss2 = m31_mul(mid_set, M31_508);
    uint dss3 = m31_mul(mid_set, M31_511);
    uint dss4 = m31_sub(m31_mul(msb, M31_136), mid_set);
    uint dss5 = m31_mul(msb, M31_256);

    uint idx = 0;

    // verify_instruction_0: [rel_id, pc, offset0, 32767, 32769,
    //   dst_base_fp*8+16+32, 8+ap_update*32, 0]
    out_set(output, idx++, row, col_len, REL_VERIFY_INSTRUCTION);
    out_set(output, idx++, row, col_len, input_pc);
    out_set(output, idx++, row, col_len, offset0);
    out_set(output, idx++, row, col_len, M31_32767);
    out_set(output, idx++, row, col_len, M31_32769);
    out_set(output, idx++, row, col_len, m31_add(m31_add(m31_mul(dst_base_fp, M31_8), M31_16), M31_32));
    out_set(output, idx++, row, col_len, m31_add(M31_8, m31_mul(ap_update, M31_32)));
    out_set(output, idx++, row, col_len, M31_0);

    // memory_address_to_id_0: [rel_id, mem_dst_base + off0_signed, dst_id]
    out_set(output, idx++, row, col_len, REL_MEMORY_ADDR_TO_ID);
    out_set(output, idx++, row, col_len, m31_add(mem_dst_base, off0_signed));
    out_set(output, idx++, row, col_len, dst_id);

    // memory_id_to_big_0: [rel_id, dst_id, dst_limbs[0..27]]
    out_set(output, idx++, row, col_len, REL_MEMORY_ID_TO_BIG);
    out_set(output, idx++, row, col_len, dst_id);
    for (uint i = 0; i < 28; i++) {
        out_set(output, idx++, row, col_len, dst_limbs[i]);
    }

    // memory_address_to_id_1: [rel_id, pc+1, next_pc_id]
    out_set(output, idx++, row, col_len, REL_MEMORY_ADDR_TO_ID);
    out_set(output, idx++, row, col_len, m31_add(input_pc, M31_1));
    out_set(output, idx++, row, col_len, next_pc_id);

    // memory_id_to_big_1: small-sign pattern
    // [rel_id, next_pc_id, limb0, limb1, limb2,
    //  rem_bits+dss2, dss3*17, dss4, 0*5, dss5]
    out_set(output, idx++, row, col_len, REL_MEMORY_ID_TO_BIG);
    out_set(output, idx++, row, col_len, next_pc_id);
    out_set(output, idx++, row, col_len, npc_limb0);
    out_set(output, idx++, row, col_len, npc_limb1);
    out_set(output, idx++, row, col_len, npc_limb2);
    out_set(output, idx++, row, col_len, m31_add(rem_bits, dss2));
    for (uint i = 0; i < 17; i++) out_set(output, idx++, row, col_len, dss3);
    out_set(output, idx++, row, col_len, dss4);
    for (uint i = 0; i < 5; i++) out_set(output, idx++, row, col_len, M31_0);
    out_set(output, idx++, row, col_len, dss5);

    // opcodes_0: [rel_id, pc, ap, fp]
    out_set(output, idx++, row, col_len, REL_OPCODES);
    out_set(output, idx++, row, col_len, input_pc);
    out_set(output, idx++, row, col_len, input_ap);
    out_set(output, idx++, row, col_len, input_fp);

    // opcodes_1: [rel_id, pc + read_small, ap + ap_update, fp]
    uint read_small = m31_sub(
        m31_sub(
            m31_add(m31_add(npc_limb0, m31_mul(npc_limb1, M31_512)),
                    m31_add(m31_mul(npc_limb2, M31_262144), m31_mul(rem_bits, M31_134217728))),
            msb),
        m31_mul(M31_536870912, mid_set));

    out_set(output, idx++, row, col_len, REL_OPCODES);
    out_set(output, idx++, row, col_len, m31_add(input_pc, read_small));
    out_set(output, idx++, row, col_len, m31_add(input_ap, ap_update));
    out_set(output, idx++, row, col_len, input_fp);
    // Total: 82
}

// ===========================================================================
// call_opcode_rel_imm: 24 trace cols -> 108 output values
//
// LookupData layout:
//   [0..8)    verify_instruction_0    (8)
//   [8..11)   memory_address_to_id_0  (3)
//   [11..41)  memory_id_to_big_0      (30) — stored_fp
//   [41..44)  memory_address_to_id_1  (3)
//   [44..74)  memory_id_to_big_1      (30) — stored_ret_pc
//   [74..77)  memory_address_to_id_2  (3)
//   [77..107) memory_id_to_big_2      (30) — distance_to_next_pc (small-sign)
//   [107..111) opcodes_0              (4)
//   [111..115) opcodes_1              (4)
// Total: 115
// ===========================================================================
kernel void interaction_values_call_opcode_rel_imm(
    device const uint *trace_cols [[buffer(0)]],
    device       uint *output     [[buffer(1)]],
    constant     uint &n_rows     [[buffer(2)]],
    constant     uint &col_len    [[buffer(3)]],
    uint row [[thread_position_in_grid]]
) {
    if (row >= col_len) return;

    // Trace columns:
    // col0=pc, col1=ap, col2=fp,
    // col3=stored_fp_id, col4..7=stored_fp_limbs[0..3],
    // col8=partial_limb_msb,
    // col9=stored_ret_pc_id, col10..13=stored_ret_pc_limbs[0..3],
    // col14=partial_limb_msb2,
    // col15=distance_to_next_pc_id, col16=msb, col17=mid_limbs_set,
    // col18..20=dist_limbs[0..2], col21=remainder_bits,
    // col22=partial_limb_msb3, col23=enabler
    uint input_pc  = tc(trace_cols, 0, row, col_len);
    uint input_ap  = tc(trace_cols, 1, row, col_len);
    uint input_fp  = tc(trace_cols, 2, row, col_len);
    uint stored_fp_id = tc(trace_cols, 3, row, col_len);
    uint sfp0 = tc(trace_cols, 4, row, col_len);
    uint sfp1 = tc(trace_cols, 5, row, col_len);
    uint sfp2 = tc(trace_cols, 6, row, col_len);
    uint sfp3 = tc(trace_cols, 7, row, col_len);
    uint stored_ret_pc_id = tc(trace_cols, 9, row, col_len);
    uint srp0 = tc(trace_cols, 10, row, col_len);
    uint srp1 = tc(trace_cols, 11, row, col_len);
    uint srp2 = tc(trace_cols, 12, row, col_len);
    uint srp3 = tc(trace_cols, 13, row, col_len);
    uint dist_id = tc(trace_cols, 15, row, col_len);
    uint msb     = tc(trace_cols, 16, row, col_len);
    uint mid_set = tc(trace_cols, 17, row, col_len);
    uint dl0     = tc(trace_cols, 18, row, col_len);
    uint dl1     = tc(trace_cols, 19, row, col_len);
    uint dl2     = tc(trace_cols, 20, row, col_len);
    uint rem_bits = tc(trace_cols, 21, row, col_len);

    // decode_small_sign
    uint dss2 = m31_mul(mid_set, M31_508);
    uint dss3 = m31_mul(mid_set, M31_511);
    uint dss4 = m31_sub(m31_mul(msb, M31_136), mid_set);
    uint dss5 = m31_mul(msb, M31_256);

    uint idx = 0;

    // verify_instruction_0: [rel_id, pc, 32768, 32769, 32769, 32, 68, 0]
    out_set(output, idx++, row, col_len, REL_VERIFY_INSTRUCTION);
    out_set(output, idx++, row, col_len, input_pc);
    out_set(output, idx++, row, col_len, M31_32768);
    out_set(output, idx++, row, col_len, M31_32769);
    out_set(output, idx++, row, col_len, M31_32769);
    out_set(output, idx++, row, col_len, M31_32);
    out_set(output, idx++, row, col_len, M31_68);
    out_set(output, idx++, row, col_len, M31_0);

    // memory_address_to_id_0: [rel_id, ap, stored_fp_id]
    out_set(output, idx++, row, col_len, REL_MEMORY_ADDR_TO_ID);
    out_set(output, idx++, row, col_len, input_ap);
    out_set(output, idx++, row, col_len, stored_fp_id);

    // memory_id_to_big_0: [rel_id, stored_fp_id, sfp0..3, 0*24]
    out_set(output, idx++, row, col_len, REL_MEMORY_ID_TO_BIG);
    out_set(output, idx++, row, col_len, stored_fp_id);
    out_set(output, idx++, row, col_len, sfp0);
    out_set(output, idx++, row, col_len, sfp1);
    out_set(output, idx++, row, col_len, sfp2);
    out_set(output, idx++, row, col_len, sfp3);
    for (uint i = 0; i < 24; i++) out_set(output, idx++, row, col_len, M31_0);

    // memory_address_to_id_1: [rel_id, ap+1, stored_ret_pc_id]
    out_set(output, idx++, row, col_len, REL_MEMORY_ADDR_TO_ID);
    out_set(output, idx++, row, col_len, m31_add(input_ap, M31_1));
    out_set(output, idx++, row, col_len, stored_ret_pc_id);

    // memory_id_to_big_1: [rel_id, stored_ret_pc_id, srp0..3, 0*24]
    out_set(output, idx++, row, col_len, REL_MEMORY_ID_TO_BIG);
    out_set(output, idx++, row, col_len, stored_ret_pc_id);
    out_set(output, idx++, row, col_len, srp0);
    out_set(output, idx++, row, col_len, srp1);
    out_set(output, idx++, row, col_len, srp2);
    out_set(output, idx++, row, col_len, srp3);
    for (uint i = 0; i < 24; i++) out_set(output, idx++, row, col_len, M31_0);

    // memory_address_to_id_2: [rel_id, pc+1, dist_id]
    out_set(output, idx++, row, col_len, REL_MEMORY_ADDR_TO_ID);
    out_set(output, idx++, row, col_len, m31_add(input_pc, M31_1));
    out_set(output, idx++, row, col_len, dist_id);

    // memory_id_to_big_2: small-sign pattern
    out_set(output, idx++, row, col_len, REL_MEMORY_ID_TO_BIG);
    out_set(output, idx++, row, col_len, dist_id);
    out_set(output, idx++, row, col_len, dl0);
    out_set(output, idx++, row, col_len, dl1);
    out_set(output, idx++, row, col_len, dl2);
    out_set(output, idx++, row, col_len, m31_add(rem_bits, dss2));
    for (uint i = 0; i < 17; i++) out_set(output, idx++, row, col_len, dss3);
    out_set(output, idx++, row, col_len, dss4);
    for (uint i = 0; i < 5; i++) out_set(output, idx++, row, col_len, M31_0);
    out_set(output, idx++, row, col_len, dss5);

    // opcodes_0: [rel_id, pc, ap, fp]
    out_set(output, idx++, row, col_len, REL_OPCODES);
    out_set(output, idx++, row, col_len, input_pc);
    out_set(output, idx++, row, col_len, input_ap);
    out_set(output, idx++, row, col_len, input_fp);

    // opcodes_1: [rel_id, pc + read_small, ap + 2, ap + 2]
    uint read_small = m31_sub(
        m31_sub(
            m31_add(m31_add(dl0, m31_mul(dl1, M31_512)),
                    m31_add(m31_mul(dl2, M31_262144), m31_mul(rem_bits, M31_134217728))),
            msb),
        m31_mul(M31_536870912, mid_set));

    out_set(output, idx++, row, col_len, REL_OPCODES);
    out_set(output, idx++, row, col_len, m31_add(input_pc, read_small));
    out_set(output, idx++, row, col_len, m31_add(input_ap, M31_2));
    out_set(output, idx++, row, col_len, m31_add(input_ap, M31_2));
    // Total: 115
}
