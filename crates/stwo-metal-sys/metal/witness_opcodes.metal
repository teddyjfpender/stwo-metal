// witness_opcodes.metal
//
// Metal compute kernels for opcode trace generation:
//   - add_opcode_small (39 trace columns)
//   - assert_eq_opcode_double_deref (19 trace columns)
//
// These kernels mirror the CPU write_trace_simd() logic from stwo-cairo.
// Each thread processes one row, performing:
//   1. Memory lookups via address_to_id and id_to_big tables
//   2. Instruction field decoding (bit manipulation on Felt252 limbs)
//   3. Column filling in column-major output layout
//
// Input buffers:
//   - inputs:         row-major [n_rows][3] uint (pc, ap, fp per row)
//   - address_to_id:  [addr_table_size] uint — address -> raw memory ID
//   - big_values:     row-major [n_big][8] uint — F252 values as 8 x u32
//   - small_values:   row-major [n_small][4] uint — small values as 4 x u32 (u128)
//   - trace_out:      column-major [N_TRACE_COLUMNS][column_length] uint
//   - n_rows:         scalar, number of real rows
//   - column_length:  scalar, padded power-of-two column length
//
// The M31 prime is P = 2^31 - 1. All trace values are stored as raw u32 < P.

#include <metal_stdlib>
using namespace metal;

// M31 prime
constant uint M31_P = 0x7FFFFFFFu;

// Tag bits for EncodedMemoryValueId
constant uint LARGE_ID_BASE = 0x40000000u; // bit 30

// Felt252 constants
constant uint FELT252_N_WORDS = 28u;
constant uint BITS_PER_LIMB   = 9u;
constant uint LIMB_MASK       = (1u << BITS_PER_LIMB) - 1u; // 0x1FF

// ---------------------------------------------------------------------------
// M31 field arithmetic helpers
// ---------------------------------------------------------------------------

static inline uint m31_add(uint a, uint b) {
    uint s = a + b;
    // Reduce: if s >= P, subtract P
    return select(s, s - M31_P, s >= M31_P);
}

static inline uint m31_sub(uint a, uint b) {
    // a - b mod P
    return select(a - b, a - b + M31_P, a < b);
}

static inline uint m31_mul(uint a, uint b) {
    ulong prod = (ulong)a * (ulong)b;
    // Barrett-style reduction for M31: prod mod (2^31 - 1)
    uint lo = (uint)(prod & 0x7FFFFFFFull);
    uint hi = (uint)(prod >> 31u);
    uint s = lo + hi;
    return select(s, s - M31_P, s >= M31_P);
}

// ---------------------------------------------------------------------------
// Split 8 x 32-bit words into 28 x 9-bit limbs (Felt252 decomposition).
// ---------------------------------------------------------------------------
static void split_felt252(
    thread const uint (&input)[8],
    thread uint (&output)[28]
) {
    uint n_bits = 32u;
    uint wi = 0u;
    uint word = input[0];
    for (uint e = 0u; e < FELT252_N_WORDS; ++e) {
        if (n_bits > BITS_PER_LIMB) {
            output[e] = word & LIMB_MASK;
            word >>= BITS_PER_LIMB;
            n_bits -= BITS_PER_LIMB;
            continue;
        }
        uint val = word;
        wi += 1u;
        word = (wi < 8u) ? input[wi] : 0u;
        if (n_bits < BITS_PER_LIMB) {
            val |= (word << n_bits) & LIMB_MASK;
            word >>= (BITS_PER_LIMB - n_bits);
        }
        n_bits += 32u - BITS_PER_LIMB;
        output[e] = val;
    }
}

// ---------------------------------------------------------------------------
// Lookup: address_to_id — returns the raw memory ID for a given address.
// The address is an M31 value used as a direct index.
// ---------------------------------------------------------------------------
static inline uint lookup_addr_to_id(
    device const uint *address_to_id,
    uint addr
) {
    return address_to_id[addr];
}

// ---------------------------------------------------------------------------
// Lookup: id_to_big — given an encoded memory value ID, returns 28 x 9-bit
// limbs of the Felt252 value.
// ---------------------------------------------------------------------------
static void lookup_id_to_big(
    device const uint *big_values,
    device const uint *small_values,
    uint encoded_id,
    thread uint (&limbs)[28]
) {
    uint tag = encoded_id >> 30u;
    uint val = encoded_id & 0x3FFFFFFFu;

    uint words[8];
    if (tag == 1u) {
        // F252 value
        uint base = val * 8u;
        for (uint i = 0u; i < 8u; ++i) words[i] = big_values[base + i];
    } else {
        // Small value (u128 as 4 limbs)
        uint base = val * 4u;
        for (uint i = 0u; i < 4u; ++i) words[i] = small_values[base + i];
        for (uint i = 4u; i < 8u; ++i) words[i] = 0u;
    }
    split_felt252(words, limbs);
}

// ---------------------------------------------------------------------------
// Helper: extract instruction fields from the 28 x 9-bit limbs of the
// instruction word at [pc].
//
// The instruction encoding packs 3 x 16-bit offsets and flag bits into
// the lower limbs of a Felt252.  The extraction mirrors the CPU code.
// ---------------------------------------------------------------------------

struct DecodedInstruction {
    uint offset0;        // 16-bit
    uint offset1;        // 16-bit
    uint offset2;        // 16-bit
    uint dst_base_fp;    // 1 bit
    uint op0_base_fp;    // 1 bit
    uint op1_imm;        // 1 bit
    uint op1_base_fp;    // 1 bit
    uint ap_update_add_1; // 1 bit
    // Derived: flags_word for extracting more bits
    uint flags_word;
};

static DecodedInstruction decode_instruction(thread const uint (&limbs)[28]) {
    DecodedInstruction d;

    // offset0 = limbs[0] + ((limbs[1] & 0x7F) << 9)
    d.offset0 = limbs[0] + ((limbs[1] & 0x7Fu) << 9u);

    // offset1 = (limbs[1] >> 7) + (limbs[2] << 2) + ((limbs[3] & 0x1F) << 11)
    d.offset1 = (limbs[1] >> 7u) + (limbs[2] << 2u) + ((limbs[3] & 0x1Fu) << 11u);

    // offset2 = (limbs[3] >> 5) + (limbs[4] << 4) + ((limbs[5] & 0x7) << 13)
    d.offset2 = (limbs[3] >> 5u) + (limbs[4] << 4u) + ((limbs[5] & 0x7u) << 13u);

    // flags_word = (limbs[5] >> 3) + (limbs[6] << 6)
    d.flags_word = (limbs[5] >> 3u) + (limbs[6] << 6u);

    d.dst_base_fp    = (d.flags_word >> 0u) & 1u;
    d.op0_base_fp    = (d.flags_word >> 1u) & 1u;
    d.op1_imm        = (d.flags_word >> 2u) & 1u;
    d.op1_base_fp    = (d.flags_word >> 3u) & 1u;
    d.ap_update_add_1 = (d.flags_word >> 11u) & 1u;

    return d;
}

// ---------------------------------------------------------------------------
// "Decode small sign" — extracts sign information from a small Felt252 value
// for the "Read Small" subroutine.
// Returns: [msb, mid_limbs_set, mid_limbs_set*508, mid_limbs_set*511,
//           msb*136 - mid_limbs_set, msb*256]
// ---------------------------------------------------------------------------
struct SmallSign {
    uint msb;
    uint mid_limbs_set;
    uint val2; // mid_limbs_set * 508
    uint val3; // mid_limbs_set * 511
    uint val4; // msb*136 - mid_limbs_set
    uint val5; // msb * 256
};

static SmallSign decode_small_sign(thread const uint (&limbs)[28]) {
    SmallSign s;
    s.msb = (limbs[27] == 256u) ? 1u : 0u;
    uint limb20_is_511 = (limbs[20] == 511u) ? 1u : 0u;
    s.mid_limbs_set = limb20_is_511 & s.msb;
    s.val2 = m31_mul(s.mid_limbs_set, 508u);
    s.val3 = m31_mul(s.mid_limbs_set, 511u);
    s.val4 = m31_sub(m31_mul(s.msb, 136u), s.mid_limbs_set);
    s.val5 = m31_mul(s.msb, 256u);
    return s;
}

// ---------------------------------------------------------------------------
// "Read Small" — reconstructs a small felt value from limbs + sign info.
// Returns the reconstructed value as an M31.
// ---------------------------------------------------------------------------
static uint read_small_value(
    uint limb0, uint limb1, uint limb2, uint remainder_bits,
    uint msb, uint mid_limbs_set
) {
    // value = limb0 + limb1*512 + limb2*262144 + remainder_bits*134217728
    //         - msb - 536870912 * mid_limbs_set
    uint val = m31_add(limb0, m31_mul(limb1, 512u));
    val = m31_add(val, m31_mul(limb2, 262144u));
    val = m31_add(val, m31_mul(remainder_bits, 134217728u));
    val = m31_sub(val, msb);
    val = m31_sub(val, m31_mul(536870912u, mid_limbs_set));
    return val;
}


// ===========================================================================
// KERNEL: add_opcode_small trace generation
//
// 39 trace columns. Three memory reads (dst, op0, op1), all "Read Small".
// ===========================================================================

kernel void witness_add_opcode_small_trace(
    device const uint  *inputs          [[buffer(0)]],  // [n_rows][3]: pc,ap,fp
    device const uint  *address_to_id   [[buffer(1)]],  // address -> id table
    device const uint  *big_values      [[buffer(2)]],  // [n_big][8] u32
    device const uint  *small_values    [[buffer(3)]],  // [n_small][4] u32
    device       uint  *trace_out       [[buffer(4)]],  // column-major output
    constant     uint  &n_rows          [[buffer(5)]],
    constant     uint  &column_length   [[buffer(6)]],
    uint               row              [[thread_position_in_grid]]
) {
    if (row >= column_length) return;

    // Load input (pc, ap, fp). Pad rows beyond n_rows with row 0 values.
    uint src_row = (row < n_rows) ? row : 0u;
    uint input_pc = inputs[src_row * 3u + 0u];
    uint input_ap = inputs[src_row * 3u + 1u];
    uint input_fp = inputs[src_row * 3u + 2u];

    // col0 = pc, col1 = ap, col2 = fp
    trace_out[0u * column_length + row] = input_pc;
    trace_out[1u * column_length + row] = input_ap;
    trace_out[2u * column_length + row] = input_fp;

    // --- Decode Instruction ---
    // Look up the instruction word at pc
    uint instr_id = lookup_addr_to_id(address_to_id, input_pc);
    uint instr_limbs[28];
    lookup_id_to_big(big_values, small_values, instr_id, instr_limbs);

    DecodedInstruction di = decode_instruction(instr_limbs);

    uint offset0 = di.offset0;
    uint offset1 = di.offset1;
    uint offset2 = di.offset2;
    uint dst_base_fp = di.dst_base_fp;
    uint op0_base_fp = di.op0_base_fp;
    uint op1_imm     = di.op1_imm;
    uint op1_base_fp = di.op1_base_fp;
    uint op1_base_ap = m31_sub(m31_sub(1u, op1_imm), op1_base_fp);
    uint ap_update_add_1 = di.ap_update_add_1;

    // col3..col10
    trace_out[3u  * column_length + row] = offset0;
    trace_out[4u  * column_length + row] = offset1;
    trace_out[5u  * column_length + row] = offset2;
    trace_out[6u  * column_length + row] = dst_base_fp;
    trace_out[7u  * column_length + row] = op0_base_fp;
    trace_out[8u  * column_length + row] = op1_imm;
    trace_out[9u  * column_length + row] = op1_base_fp;
    trace_out[10u * column_length + row] = ap_update_add_1;

    // Signed offsets
    uint off0_signed = m31_sub(offset0, 32768u);
    uint off1_signed = m31_sub(offset1, 32768u);
    uint off2_signed = m31_sub(offset2, 32768u);

    // mem_dst_base = dst_base_fp * fp + (1 - dst_base_fp) * ap
    uint mem_dst_base = m31_add(
        m31_mul(dst_base_fp, input_fp),
        m31_mul(m31_sub(1u, dst_base_fp), input_ap)
    );
    // mem0_base = op0_base_fp * fp + (1 - op0_base_fp) * ap
    uint mem0_base = m31_add(
        m31_mul(op0_base_fp, input_fp),
        m31_mul(m31_sub(1u, op0_base_fp), input_ap)
    );
    // mem1_base = op1_imm*pc + op1_base_fp*fp + op1_base_ap*ap
    uint mem1_base = m31_add(
        m31_add(m31_mul(op1_imm, input_pc), m31_mul(op1_base_fp, input_fp)),
        m31_mul(op1_base_ap, input_ap)
    );

    // col11..col13
    trace_out[11u * column_length + row] = mem_dst_base;
    trace_out[12u * column_length + row] = mem0_base;
    trace_out[13u * column_length + row] = mem1_base;

    // --- Read Small: dst ---
    uint dst_addr = m31_add(mem_dst_base, off0_signed);
    uint dst_id = lookup_addr_to_id(address_to_id, dst_addr);
    trace_out[14u * column_length + row] = dst_id;  // col14

    uint dst_limbs[28];
    lookup_id_to_big(big_values, small_values, dst_id, dst_limbs);

    SmallSign dst_sign = decode_small_sign(dst_limbs);
    trace_out[15u * column_length + row] = dst_sign.msb;           // col15
    trace_out[16u * column_length + row] = dst_sign.mid_limbs_set; // col16

    uint dst_limb_0 = dst_limbs[0];
    uint dst_limb_1 = dst_limbs[1];
    uint dst_limb_2 = dst_limbs[2];
    uint dst_remainder_bits = dst_limbs[3] & 3u;
    uint dst_partial_limb_msb = (dst_remainder_bits >> 1u) & 1u;

    trace_out[17u * column_length + row] = dst_limb_0;          // col17
    trace_out[18u * column_length + row] = dst_limb_1;          // col18
    trace_out[19u * column_length + row] = dst_limb_2;          // col19
    trace_out[20u * column_length + row] = dst_remainder_bits;  // col20
    trace_out[21u * column_length + row] = dst_partial_limb_msb; // col21

    // --- Read Small: op0 ---
    uint op0_addr = m31_add(mem0_base, off1_signed);
    uint op0_id = lookup_addr_to_id(address_to_id, op0_addr);
    trace_out[22u * column_length + row] = op0_id;  // col22

    uint op0_limbs[28];
    lookup_id_to_big(big_values, small_values, op0_id, op0_limbs);

    SmallSign op0_sign = decode_small_sign(op0_limbs);
    trace_out[23u * column_length + row] = op0_sign.msb;           // col23
    trace_out[24u * column_length + row] = op0_sign.mid_limbs_set; // col24

    uint op0_limb_0 = op0_limbs[0];
    uint op0_limb_1 = op0_limbs[1];
    uint op0_limb_2 = op0_limbs[2];
    uint op0_remainder_bits = op0_limbs[3] & 3u;
    uint op0_partial_limb_msb = (op0_remainder_bits >> 1u) & 1u;

    trace_out[25u * column_length + row] = op0_limb_0;           // col25
    trace_out[26u * column_length + row] = op0_limb_1;           // col26
    trace_out[27u * column_length + row] = op0_limb_2;           // col27
    trace_out[28u * column_length + row] = op0_remainder_bits;   // col28
    trace_out[29u * column_length + row] = op0_partial_limb_msb; // col29

    // --- Read Small: op1 ---
    uint op1_addr = m31_add(mem1_base, off2_signed);
    uint op1_id = lookup_addr_to_id(address_to_id, op1_addr);
    trace_out[30u * column_length + row] = op1_id;  // col30

    uint op1_limbs[28];
    lookup_id_to_big(big_values, small_values, op1_id, op1_limbs);

    SmallSign op1_sign = decode_small_sign(op1_limbs);
    trace_out[31u * column_length + row] = op1_sign.msb;           // col31
    trace_out[32u * column_length + row] = op1_sign.mid_limbs_set; // col32

    uint op1_limb_0 = op1_limbs[0];
    uint op1_limb_1 = op1_limbs[1];
    uint op1_limb_2 = op1_limbs[2];
    uint op1_remainder_bits = op1_limbs[3] & 3u;
    uint op1_partial_limb_msb = (op1_remainder_bits >> 1u) & 1u;

    trace_out[33u * column_length + row] = op1_limb_0;           // col33
    trace_out[34u * column_length + row] = op1_limb_1;           // col34
    trace_out[35u * column_length + row] = op1_limb_2;           // col35
    trace_out[36u * column_length + row] = op1_remainder_bits;   // col36
    trace_out[37u * column_length + row] = op1_partial_limb_msb; // col37

    // --- Enabler column (col38) ---
    // 1 for real rows, 0 for padding
    trace_out[38u * column_length + row] = (row < n_rows) ? 1u : 0u;
}


// ===========================================================================
// KERNEL: assert_eq_opcode_double_deref trace generation
//
// 19 trace columns. One memory read via op0 (Read Positive Num Bits 29),
// then a "Mem Verify Equal" pattern with double dereference.
// ===========================================================================

kernel void witness_assert_eq_double_deref_trace(
    device const uint  *inputs          [[buffer(0)]],  // [n_rows][3]: pc,ap,fp
    device const uint  *address_to_id   [[buffer(1)]],  // address -> id table
    device const uint  *big_values      [[buffer(2)]],  // [n_big][8] u32
    device const uint  *small_values    [[buffer(3)]],  // [n_small][4] u32
    device       uint  *trace_out       [[buffer(4)]],  // column-major output
    constant     uint  &n_rows          [[buffer(5)]],
    constant     uint  &column_length   [[buffer(6)]],
    uint               row              [[thread_position_in_grid]]
) {
    if (row >= column_length) return;

    // Load input
    uint src_row = (row < n_rows) ? row : 0u;
    uint input_pc = inputs[src_row * 3u + 0u];
    uint input_ap = inputs[src_row * 3u + 1u];
    uint input_fp = inputs[src_row * 3u + 2u];

    // col0..col2
    trace_out[0u * column_length + row] = input_pc;
    trace_out[1u * column_length + row] = input_ap;
    trace_out[2u * column_length + row] = input_fp;

    // --- Decode Instruction ---
    uint instr_id = lookup_addr_to_id(address_to_id, input_pc);
    uint instr_limbs[28];
    lookup_id_to_big(big_values, small_values, instr_id, instr_limbs);

    DecodedInstruction di = decode_instruction(instr_limbs);

    uint offset0 = di.offset0;
    uint offset1 = di.offset1;
    uint offset2 = di.offset2;
    uint dst_base_fp = di.dst_base_fp;
    uint op0_base_fp = di.op0_base_fp;
    uint ap_update_add_1 = di.ap_update_add_1;

    // col3..col8
    trace_out[3u * column_length + row] = offset0;
    trace_out[4u * column_length + row] = offset1;
    trace_out[5u * column_length + row] = offset2;
    trace_out[6u * column_length + row] = dst_base_fp;
    trace_out[7u * column_length + row] = op0_base_fp;
    trace_out[8u * column_length + row] = ap_update_add_1;

    // Signed offsets
    uint off0_signed = m31_sub(offset0, 32768u);
    uint off1_signed = m31_sub(offset1, 32768u);
    uint off2_signed = m31_sub(offset2, 32768u);

    // mem_dst_base = dst_base_fp * fp + (1 - dst_base_fp) * ap
    uint mem_dst_base = m31_add(
        m31_mul(dst_base_fp, input_fp),
        m31_mul(m31_sub(1u, dst_base_fp), input_ap)
    );
    // mem0_base = op0_base_fp * fp + (1 - op0_base_fp) * ap
    uint mem0_base = m31_add(
        m31_mul(op0_base_fp, input_fp),
        m31_mul(m31_sub(1u, op0_base_fp), input_ap)
    );

    // col9..col10
    trace_out[9u  * column_length + row] = mem_dst_base;
    trace_out[10u * column_length + row] = mem0_base;

    // --- Read Positive Num Bits 29: read [mem0_base + off1_signed] ---
    uint mem1_addr = m31_add(mem0_base, off1_signed);
    uint mem1_base_id = lookup_addr_to_id(address_to_id, mem1_addr);
    trace_out[11u * column_length + row] = mem1_base_id; // col11

    // Get the Felt252 value
    uint mem1_limbs[28];
    lookup_id_to_big(big_values, small_values, mem1_base_id, mem1_limbs);

    uint mem1_base_limb_0 = mem1_limbs[0];
    uint mem1_base_limb_1 = mem1_limbs[1];
    uint mem1_base_limb_2 = mem1_limbs[2];
    uint mem1_base_limb_3 = mem1_limbs[3];
    uint partial_limb_msb = (mem1_base_limb_3 >> 1u) & 1u;

    // col12..col16
    trace_out[12u * column_length + row] = mem1_base_limb_0;
    trace_out[13u * column_length + row] = mem1_base_limb_1;
    trace_out[14u * column_length + row] = mem1_base_limb_2;
    trace_out[15u * column_length + row] = mem1_base_limb_3;
    trace_out[16u * column_length + row] = partial_limb_msb;

    // --- Mem Verify Equal ---
    // Reconstruct the address from limbs:
    // addr = limb0 + limb1*512 + limb2*262144 + limb3*134217728 + off2_signed
    // Then look up that address's ID — it should equal dst_id.
    uint dst_addr = m31_add(mem_dst_base, off0_signed);
    uint dst_id = lookup_addr_to_id(address_to_id, dst_addr);
    trace_out[17u * column_length + row] = dst_id; // col17

    // --- Enabler column (col18) ---
    trace_out[18u * column_length + row] = (row < n_rows) ? 1u : 0u;
}
