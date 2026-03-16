// witness_mults_accumulate.metal
//
// GPU-native multiplicity accumulation via Metal atomic operations.
// Each kernel recomputes the same addresses/IDs as the corresponding trace
// kernel, but instead of writing trace columns it atomically increments
// the three multiplicity buffers:
//
//   addr_to_id_mults[addr - 1] += 1   (for each address looked up)
//   id_to_big_mults[id]        += 1   (for each F252 encoded id)
//   id_to_small_mults[id]      += 1   (for each small encoded id)
//
// The verify_instruction multiplicities remain on CPU (DashMap-based).
//
// Buffer layout per kernel:
//   buffer(0): inputs            [n_rows * 3]  uint (pc, ap, fp)
//   buffer(1): address_to_id     [table_size]  uint
//   buffer(2): big_values        [n_big * 8]   uint
//   buffer(3): small_values      [n_small * 4] uint
//   buffer(4): addr_to_id_mults  [table_size]  atomic_uint
//   buffer(5): id_to_big_mults   [n_big]       atomic_uint
//   buffer(6): id_to_small_mults [n_small]     atomic_uint
//   buffer(7): n_rows            uint constant
//
// Only real rows (gid < n_rows) do atomic increments -- padding rows are skipped.

#include <metal_stdlib>
using namespace metal;

// M31 prime
constant uint M31_P = 0x7FFFFFFFu;

// Tag bits for EncodedMemoryValueId
constant uint LARGE_ID_BASE = 0x40000000u; // bit 30

// ---------------------------------------------------------------------------
// M31 field arithmetic helpers (same as witness_opcodes.metal)
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

// ---------------------------------------------------------------------------
// Lookup: address_to_id
// ---------------------------------------------------------------------------
static inline uint lookup_addr_to_id(
    device const uint *address_to_id,
    uint addr
) {
    return address_to_id[addr - 1u];
}

// ---------------------------------------------------------------------------
// Increment addr_to_id multiplicity and return the looked-up ID.
// ---------------------------------------------------------------------------
static inline uint lookup_and_inc_addr(
    device const uint *address_to_id,
    device atomic_uint *addr_to_id_mults,
    uint addr
) {
    uint idx = addr - 1u;
    uint id = address_to_id[idx];
    atomic_fetch_add_explicit(&addr_to_id_mults[idx], 1u, memory_order_relaxed);
    return id;
}

// ---------------------------------------------------------------------------
// Increment id_to_big or id_to_small multiplicity based on encoded ID.
// ---------------------------------------------------------------------------
static inline void inc_id_mults(
    device atomic_uint *id_to_big_mults,
    device atomic_uint *id_to_small_mults,
    uint encoded_id
) {
    uint tag = encoded_id >> 30u;
    uint val = encoded_id & 0x3FFFFFFFu;
    if (tag == 1u) {
        atomic_fetch_add_explicit(&id_to_big_mults[val], 1u, memory_order_relaxed);
    } else {
        atomic_fetch_add_explicit(&id_to_small_mults[val], 1u, memory_order_relaxed);
    }
}

// ---------------------------------------------------------------------------
// Felt252 split: 8 x 32-bit words -> 28 x 9-bit limbs
// ---------------------------------------------------------------------------
constant uint BITS_PER_LIMB   = 9u;
constant uint LIMB_MASK       = (1u << BITS_PER_LIMB) - 1u;
constant uint FELT252_N_WORDS = 28u;

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
// Lookup: id_to_big -- decode Felt252 from big or small value tables.
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
        uint base = val * 8u;
        for (uint i = 0u; i < 8u; ++i) words[i] = big_values[base + i];
    } else {
        uint base = val * 4u;
        for (uint i = 0u; i < 4u; ++i) words[i] = small_values[base + i];
        for (uint i = 4u; i < 8u; ++i) words[i] = 0u;
    }
    split_felt252(words, limbs);
}

// ---------------------------------------------------------------------------
// Instruction decode
// ---------------------------------------------------------------------------
struct DecodedInstruction {
    uint offset0;
    uint offset1;
    uint offset2;
    uint dst_base_fp;
    uint op0_base_fp;
    uint op1_imm;
    uint op1_base_fp;
    uint ap_update_add_1;
    uint flags_word;
};

static DecodedInstruction decode_instruction(thread const uint (&limbs)[28]) {
    DecodedInstruction d;
    d.offset0 = limbs[0] + ((limbs[1] & 0x7Fu) << 9u);
    d.offset1 = (limbs[1] >> 7u) + (limbs[2] << 2u) + ((limbs[3] & 0x1Fu) << 11u);
    d.offset2 = (limbs[3] >> 5u) + (limbs[4] << 4u) + ((limbs[5] & 0x7u) << 13u);
    d.flags_word = (limbs[5] >> 3u) + (limbs[6] << 6u);
    d.dst_base_fp     = (d.flags_word >> 0u) & 1u;
    d.op0_base_fp     = (d.flags_word >> 1u) & 1u;
    d.op1_imm         = (d.flags_word >> 2u) & 1u;
    d.op1_base_fp     = (d.flags_word >> 3u) & 1u;
    d.ap_update_add_1 = (d.flags_word >> 11u) & 1u;
    return d;
}


// ===========================================================================
// KERNEL: add_opcode_small multiplicity accumulation
//
// Memory lookups: dst, op0, op1 -- 3 addr_to_id + 3 id_to_big/small
// ===========================================================================

kernel void mults_add_opcode_small(
    device const uint   *inputs            [[buffer(0)]],
    device const uint   *address_to_id     [[buffer(1)]],
    device const uint   *big_values        [[buffer(2)]],
    device const uint   *small_values      [[buffer(3)]],
    device atomic_uint  *addr_to_id_mults  [[buffer(4)]],
    device atomic_uint  *id_to_big_mults   [[buffer(5)]],
    device atomic_uint  *id_to_small_mults [[buffer(6)]],
    constant uint       &n_rows            [[buffer(7)]],
    uint                 gid               [[thread_position_in_grid]]
) {
    if (gid >= n_rows) return;

    uint input_pc = inputs[gid * 3u + 0u];
    uint input_ap = inputs[gid * 3u + 1u];
    uint input_fp = inputs[gid * 3u + 2u];

    // Decode instruction at pc (no mult increment -- verify_instruction handles that)
    uint instr_id = lookup_addr_to_id(address_to_id, input_pc);
    uint instr_limbs[28];
    lookup_id_to_big(big_values, small_values, instr_id, instr_limbs);

    DecodedInstruction di = decode_instruction(instr_limbs);

    uint off0_signed = m31_sub(di.offset0, 32768u);
    uint off1_signed = m31_sub(di.offset1, 32768u);
    uint off2_signed = m31_sub(di.offset2, 32768u);

    uint op1_base_ap = m31_sub(m31_sub(1u, di.op1_imm), di.op1_base_fp);

    uint mem_dst_base = m31_add(
        m31_mul(di.dst_base_fp, input_fp),
        m31_mul(m31_sub(1u, di.dst_base_fp), input_ap)
    );
    uint mem0_base = m31_add(
        m31_mul(di.op0_base_fp, input_fp),
        m31_mul(m31_sub(1u, di.op0_base_fp), input_ap)
    );
    uint mem1_base = m31_add(
        m31_add(m31_mul(di.op1_imm, input_pc), m31_mul(di.op1_base_fp, input_fp)),
        m31_mul(op1_base_ap, input_ap)
    );

    // dst
    uint dst_addr = m31_add(mem_dst_base, off0_signed);
    uint dst_id = lookup_and_inc_addr(address_to_id, addr_to_id_mults, dst_addr);
    inc_id_mults(id_to_big_mults, id_to_small_mults, dst_id);

    // op0
    uint op0_addr = m31_add(mem0_base, off1_signed);
    uint op0_id = lookup_and_inc_addr(address_to_id, addr_to_id_mults, op0_addr);
    inc_id_mults(id_to_big_mults, id_to_small_mults, op0_id);

    // op1
    uint op1_addr = m31_add(mem1_base, off2_signed);
    uint op1_id = lookup_and_inc_addr(address_to_id, addr_to_id_mults, op1_addr);
    inc_id_mults(id_to_big_mults, id_to_small_mults, op1_id);
}


// ===========================================================================
// KERNEL: assert_eq_opcode_double_deref multiplicity accumulation
//
// SubComponentInputs: 3 addr_to_id, 1 id_to_big
// ===========================================================================

kernel void mults_assert_eq_double_deref(
    device const uint   *inputs            [[buffer(0)]],
    device const uint   *address_to_id     [[buffer(1)]],
    device const uint   *big_values        [[buffer(2)]],
    device const uint   *small_values      [[buffer(3)]],
    device atomic_uint  *addr_to_id_mults  [[buffer(4)]],
    device atomic_uint  *id_to_big_mults   [[buffer(5)]],
    device atomic_uint  *id_to_small_mults [[buffer(6)]],
    constant uint       &n_rows            [[buffer(7)]],
    uint                 gid               [[thread_position_in_grid]]
) {
    if (gid >= n_rows) return;

    uint input_pc = inputs[gid * 3u + 0u];
    uint input_ap = inputs[gid * 3u + 1u];
    uint input_fp = inputs[gid * 3u + 2u];

    // Decode instruction
    uint instr_id = lookup_addr_to_id(address_to_id, input_pc);
    uint instr_limbs[28];
    lookup_id_to_big(big_values, small_values, instr_id, instr_limbs);

    DecodedInstruction di = decode_instruction(instr_limbs);

    uint off0_signed = m31_sub(di.offset0, 32768u);
    uint off1_signed = m31_sub(di.offset1, 32768u);

    uint mem_dst_base = m31_add(
        m31_mul(di.dst_base_fp, input_fp),
        m31_mul(m31_sub(1u, di.dst_base_fp), input_ap)
    );
    uint mem0_base = m31_add(
        m31_mul(di.op0_base_fp, input_fp),
        m31_mul(m31_sub(1u, di.op0_base_fp), input_ap)
    );

    // op0: Read Positive Num Bits 29 from [mem0_base + off1_signed]
    uint mem1_addr = m31_add(mem0_base, off1_signed);
    uint mem1_id = lookup_and_inc_addr(address_to_id, addr_to_id_mults, mem1_addr);
    inc_id_mults(id_to_big_mults, id_to_small_mults, mem1_id);

    // dst: look up [mem_dst_base + off0_signed]
    uint dst_addr = m31_add(mem_dst_base, off0_signed);
    uint dst_id = lookup_and_inc_addr(address_to_id, addr_to_id_mults, dst_addr);
    // id_to_big NOT incremented for dst (SubComponentInputs has only 1 id_to_big)

    // Double-deref: reconstruct op0 value as address
    uint op0_limbs[28];
    lookup_id_to_big(big_values, small_values, mem1_id, op0_limbs);
    uint op0_val = m31_add(op0_limbs[0], m31_mul(op0_limbs[1], 512u));
    op0_val = m31_add(op0_val, m31_mul(op0_limbs[2], 262144u));
    op0_val = m31_add(op0_val, m31_mul(op0_limbs[3], 134217728u));

    uint off2_signed = m31_sub(di.offset2, 32768u);
    uint deref_addr = m31_add(op0_val, off2_signed);
    lookup_and_inc_addr(address_to_id, addr_to_id_mults, deref_addr);
    // No id_to_big increment for the deref result
}


// ===========================================================================
// KERNEL: jnz_opcode_taken multiplicity accumulation
//
// SubComponentInputs: 2 addr_to_id, 2 id_to_big
// ===========================================================================

kernel void mults_jnz_opcode_taken(
    device const uint   *inputs            [[buffer(0)]],
    device const uint   *address_to_id     [[buffer(1)]],
    device const uint   *big_values        [[buffer(2)]],
    device const uint   *small_values      [[buffer(3)]],
    device atomic_uint  *addr_to_id_mults  [[buffer(4)]],
    device atomic_uint  *id_to_big_mults   [[buffer(5)]],
    device atomic_uint  *id_to_small_mults [[buffer(6)]],
    constant uint       &n_rows            [[buffer(7)]],
    uint                 gid               [[thread_position_in_grid]]
) {
    if (gid >= n_rows) return;

    uint input_pc = inputs[gid * 3u + 0u];
    uint input_ap = inputs[gid * 3u + 1u];
    uint input_fp = inputs[gid * 3u + 2u];

    // Decode instruction
    uint instr_id = lookup_addr_to_id(address_to_id, input_pc);
    uint instr_limbs[28];
    lookup_id_to_big(big_values, small_values, instr_id, instr_limbs);

    DecodedInstruction di = decode_instruction(instr_limbs);

    uint off0_signed = m31_sub(di.offset0, 32768u);

    uint mem_dst_base = m31_add(
        m31_mul(di.dst_base_fp, input_fp),
        m31_mul(m31_sub(1u, di.dst_base_fp), input_ap)
    );

    // dst
    uint dst_addr = m31_add(mem_dst_base, off0_signed);
    uint dst_id = lookup_and_inc_addr(address_to_id, addr_to_id_mults, dst_addr);
    inc_id_mults(id_to_big_mults, id_to_small_mults, dst_id);

    // next_pc from [pc+1]
    uint next_pc_addr = m31_add(input_pc, 1u);
    uint next_pc_id = lookup_and_inc_addr(address_to_id, addr_to_id_mults, next_pc_addr);
    inc_id_mults(id_to_big_mults, id_to_small_mults, next_pc_id);
}


// ===========================================================================
// KERNEL: jump_opcode_rel_imm multiplicity accumulation
//
// SubComponentInputs: 1 addr_to_id, 1 id_to_big
// ===========================================================================

kernel void mults_jump_opcode_rel_imm(
    device const uint   *inputs            [[buffer(0)]],
    device const uint   *address_to_id     [[buffer(1)]],
    device const uint   *big_values        [[buffer(2)]],
    device const uint   *small_values      [[buffer(3)]],
    device atomic_uint  *addr_to_id_mults  [[buffer(4)]],
    device atomic_uint  *id_to_big_mults   [[buffer(5)]],
    device atomic_uint  *id_to_small_mults [[buffer(6)]],
    constant uint       &n_rows            [[buffer(7)]],
    uint                 gid               [[thread_position_in_grid]]
) {
    if (gid >= n_rows) return;

    uint input_pc = inputs[gid * 3u + 0u];

    // next_pc from [pc+1]
    uint next_pc_addr = m31_add(input_pc, 1u);
    uint next_pc_id = lookup_and_inc_addr(address_to_id, addr_to_id_mults, next_pc_addr);
    inc_id_mults(id_to_big_mults, id_to_small_mults, next_pc_id);
}


// ===========================================================================
// KERNEL: call_opcode_rel_imm multiplicity accumulation
//
// SubComponentInputs: 3 addr_to_id, 3 id_to_big
// ===========================================================================

kernel void mults_call_opcode_rel_imm(
    device const uint   *inputs            [[buffer(0)]],
    device const uint   *address_to_id     [[buffer(1)]],
    device const uint   *big_values        [[buffer(2)]],
    device const uint   *small_values      [[buffer(3)]],
    device atomic_uint  *addr_to_id_mults  [[buffer(4)]],
    device atomic_uint  *id_to_big_mults   [[buffer(5)]],
    device atomic_uint  *id_to_small_mults [[buffer(6)]],
    constant uint       &n_rows            [[buffer(7)]],
    uint                 gid               [[thread_position_in_grid]]
) {
    if (gid >= n_rows) return;

    uint input_pc = inputs[gid * 3u + 0u];
    uint input_ap = inputs[gid * 3u + 1u];

    // stored_fp from [ap]
    uint sfp_id = lookup_and_inc_addr(address_to_id, addr_to_id_mults, input_ap);
    inc_id_mults(id_to_big_mults, id_to_small_mults, sfp_id);

    // stored_ret_pc from [ap+1]
    uint ret_pc_addr = m31_add(input_ap, 1u);
    uint ret_pc_id = lookup_and_inc_addr(address_to_id, addr_to_id_mults, ret_pc_addr);
    inc_id_mults(id_to_big_mults, id_to_small_mults, ret_pc_id);

    // distance_to_next_pc from [pc+1]
    uint dist_addr = m31_add(input_pc, 1u);
    uint dist_id = lookup_and_inc_addr(address_to_id, addr_to_id_mults, dist_addr);
    inc_id_mults(id_to_big_mults, id_to_small_mults, dist_id);
}


// ===========================================================================
// KERNEL: ret_opcode multiplicity accumulation
//
// SubComponentInputs: 2 addr_to_id, 2 id_to_big
// ===========================================================================

kernel void mults_ret_opcode(
    device const uint   *inputs            [[buffer(0)]],
    device const uint   *address_to_id     [[buffer(1)]],
    device const uint   *big_values        [[buffer(2)]],
    device const uint   *small_values      [[buffer(3)]],
    device atomic_uint  *addr_to_id_mults  [[buffer(4)]],
    device atomic_uint  *id_to_big_mults   [[buffer(5)]],
    device atomic_uint  *id_to_small_mults [[buffer(6)]],
    constant uint       &n_rows            [[buffer(7)]],
    uint                 gid               [[thread_position_in_grid]]
) {
    if (gid >= n_rows) return;

    uint input_fp = inputs[gid * 3u + 2u];

    // next_pc from [fp-1]
    uint next_pc_addr = m31_sub(input_fp, 1u);
    uint next_pc_id = lookup_and_inc_addr(address_to_id, addr_to_id_mults, next_pc_addr);
    inc_id_mults(id_to_big_mults, id_to_small_mults, next_pc_id);

    // next_fp from [fp-2]
    uint next_fp_addr = m31_sub(input_fp, 2u);
    uint next_fp_id = lookup_and_inc_addr(address_to_id, addr_to_id_mults, next_fp_addr);
    inc_id_mults(id_to_big_mults, id_to_small_mults, next_fp_id);
}
