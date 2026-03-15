// witness_verify_instruction.metal
//
// Metal compute kernel for generating the verify_instruction trace.
//
// The trace table has N_TRACE_COLUMNS = 17 columns in column-major layout.
//
// Each row decodes a Cairo instruction into its constituent fields:
//   col 0:  input_pc              (pass-through)
//   col 1:  input_offset0         (pass-through)
//   col 2:  input_offset1         (pass-through)
//   col 3:  input_offset2         (pass-through)
//   col 4:  input_inst_felt5_high (pass-through)
//   col 5:  input_inst_felt6      (pass-through)
//   col 6:  input_opcode_ext      (pass-through)
//   col 7:  offset0_low           = offset0 & 0x1FF
//   col 8:  offset0_mid           = offset0 >> 9
//   col 9:  offset1_low           = offset1 & 0x3
//   col 10: offset1_mid           = (offset1 >> 2) & 0x1FF
//   col 11: offset1_high          = offset1 >> 11
//   col 12: offset2_low           = offset2 & 0xF
//   col 13: offset2_mid           = (offset2 >> 4) & 0x1FF
//   col 14: offset2_high          = offset2 >> 13
//   col 15: instruction_id        = address_to_id[pc]
//   col 16: multiplicity          (pass-through)
//
// Input buffers:
//   buffer(0) — inputs:  [n_rows * 7] u32, row-major (pc, off0, off1, off2, felt5h, felt6, opext)
//   buffer(1) — mults:   [n_rows] u32 multiplicities
//   buffer(2) — addr_to_id: [addr_to_id_len] u32 address-to-raw-id lookup table
//   buffer(3) — trace_out: column-major [17][column_length] u32 output
//   buffer(4) — n_rows:  scalar, number of real rows
//   buffer(5) — column_length: scalar, padded column length (power-of-two)
//   buffer(6) — addr_to_id_len: scalar, length of the addr_to_id table
//
// Thread mapping: one thread per row index (0..column_length).

#include <metal_stdlib>
using namespace metal;

#define N_TRACE_COLUMNS 17u
#define N_INPUT_FIELDS  7u

kernel void witness_verify_instruction_trace(
    device const uint  *inputs        [[buffer(0)]],
    device const uint  *mults         [[buffer(1)]],
    device const uint  *addr_to_id    [[buffer(2)]],
    device       uint  *trace_out     [[buffer(3)]],
    constant     uint  &n_rows        [[buffer(4)]],
    constant     uint  &column_length [[buffer(5)]],
    constant     uint  &addr_to_id_len [[buffer(6)]],
    uint               tid            [[thread_position_in_grid]]
) {
    if (tid >= column_length) {
        return;
    }

    uint row = tid;

    // Default values for padding rows.
    uint pc          = 0u;
    uint offset0     = 0u;
    uint offset1     = 0u;
    uint offset2     = 0u;
    uint felt5_high  = 0u;
    uint felt6       = 0u;
    uint opcode_ext  = 0u;
    uint mult        = 0u;

    if (row < n_rows) {
        uint base = row * N_INPUT_FIELDS;
        pc         = inputs[base + 0u];
        offset0    = inputs[base + 1u];
        offset1    = inputs[base + 2u];
        offset2    = inputs[base + 3u];
        felt5_high = inputs[base + 4u];
        felt6      = inputs[base + 5u];
        opcode_ext = inputs[base + 6u];
        mult       = mults[row];
    }

    // Encode offsets — bit-field decomposition.
    uint offset0_low  = offset0 & 0x1FFu;           // bits [0..8]
    uint offset0_mid  = offset0 >> 9u;               // bits [9..15]

    uint offset1_low  = offset1 & 0x3u;              // bits [0..1]
    uint offset1_mid  = (offset1 >> 2u) & 0x1FFu;    // bits [2..10]
    uint offset1_high = offset1 >> 11u;               // bits [11..15]

    uint offset2_low  = offset2 & 0xFu;              // bits [0..3]
    uint offset2_mid  = (offset2 >> 4u) & 0x1FFu;    // bits [4..12]
    uint offset2_high = offset2 >> 13u;               // bits [13..15]

    // Memory lookup: instruction_id = addr_to_id[pc].
    uint instruction_id = 0u;
    if (pc < addr_to_id_len) {
        instruction_id = addr_to_id[pc];
    }

    // Write columns in column-major layout: trace_out[col * column_length + row].
    trace_out[ 0u * column_length + row] = pc;
    trace_out[ 1u * column_length + row] = offset0;
    trace_out[ 2u * column_length + row] = offset1;
    trace_out[ 3u * column_length + row] = offset2;
    trace_out[ 4u * column_length + row] = felt5_high;
    trace_out[ 5u * column_length + row] = felt6;
    trace_out[ 6u * column_length + row] = opcode_ext;
    trace_out[ 7u * column_length + row] = offset0_low;
    trace_out[ 8u * column_length + row] = offset0_mid;
    trace_out[ 9u * column_length + row] = offset1_low;
    trace_out[10u * column_length + row] = offset1_mid;
    trace_out[11u * column_length + row] = offset1_high;
    trace_out[12u * column_length + row] = offset2_low;
    trace_out[13u * column_length + row] = offset2_mid;
    trace_out[14u * column_length + row] = offset2_high;
    trace_out[15u * column_length + row] = instruction_id;
    trace_out[16u * column_length + row] = mult;
}
