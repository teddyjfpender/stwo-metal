// witness_memory_id_to_big.metal
//
// Metal compute kernel for generating the memory_id_to_big "big" trace.
//
// The trace table has FELT252_N_WORDS (28) value columns followed by one
// multiplicity column.  Each value column holds one 9-bit limb of the Felt252
// representation of a memory value.
//
// Input layout (all buffers are dense arrays of uint32_t):
//   buffer(0) — big_values: row-major [n_rows][8] u32 words (256-bit values)
//   buffer(1) — mults:      [n_rows]         u32 multiplicities
//   buffer(2) — trace_out:  column-major [29][column_length] u32 output
//   buffer(3) — n_values:   scalar, number of real rows (may be < column_length)
//   buffer(4) — column_length: scalar, total rows per column (power-of-two,
//               padded with zeros beyond n_values)
//
// Thread mapping: one thread per row index in [0, column_length).

#include <metal_stdlib>
using namespace metal;

// Felt252 representation constants matching stwo-cairo.
constant uint FELT252_N_WORDS  = 28u;
constant uint BITS_PER_WORD    = 9u;
constant uint WORD_MASK        = (1u << BITS_PER_WORD) - 1u; // 0x1FF
constant uint N_INPUT_WORDS    = 8u;
constant uint N_TRACE_COLUMNS  = FELT252_N_WORDS + 1u; // 29

/// Split 8 x 32-bit words into 28 x 9-bit limbs.
///
/// This mirrors `split::<8, 28, u32>` from stwo-cairo's felt.rs.
/// The algorithm streams through the input words, chopping 9 bits at a time
/// and bridging across word boundaries when needed.
static void split_f252(
    thread const uint (&input)[8],
    thread uint (&output)[28]
) {
    uint n_bits_in_word = 32u;
    uint word_i = 0u;
    uint word = input[0];

    for (uint e = 0u; e < FELT252_N_WORDS; ++e) {
        if (n_bits_in_word > BITS_PER_WORD) {
            output[e] = word & WORD_MASK;
            word >>= BITS_PER_WORD;
            n_bits_in_word -= BITS_PER_WORD;
            continue;
        }

        // The remaining bits in `word` fill part (or all) of the limb.
        uint val = word;
        word_i += 1u;
        word = (word_i < N_INPUT_WORDS) ? input[word_i] : 0u;

        if (n_bits_in_word < BITS_PER_WORD) {
            val |= (word << n_bits_in_word) & WORD_MASK;
            word >>= (BITS_PER_WORD - n_bits_in_word);
        }

        n_bits_in_word += 32u - BITS_PER_WORD;
        output[e] = val;
    }
}

kernel void witness_memory_id_to_big_trace(
    device const uint  *big_values    [[buffer(0)]],
    device const uint  *mults         [[buffer(1)]],
    device       uint  *trace_out     [[buffer(2)]],
    constant     uint  &n_values      [[buffer(3)]],
    constant     uint  &column_length [[buffer(4)]],
    uint               row           [[thread_position_in_grid]]
) {
    if (row >= column_length) {
        return;
    }

    // ---------------------------------------------------------------
    // Load the 256-bit value (or zero-pad if beyond n_values).
    // ---------------------------------------------------------------
    uint input_words[8];
    if (row < n_values) {
        uint base = row * N_INPUT_WORDS;
        for (uint i = 0u; i < N_INPUT_WORDS; ++i) {
            input_words[i] = big_values[base + i];
        }
    } else {
        for (uint i = 0u; i < N_INPUT_WORDS; ++i) {
            input_words[i] = 0u;
        }
    }

    // ---------------------------------------------------------------
    // Split into 28 x 9-bit limbs.
    // ---------------------------------------------------------------
    uint limbs[28];
    split_f252(input_words, limbs);

    // ---------------------------------------------------------------
    // Write limb columns (column-major layout).
    // ---------------------------------------------------------------
    for (uint col = 0u; col < FELT252_N_WORDS; ++col) {
        trace_out[col * column_length + row] = limbs[col];
    }

    // ---------------------------------------------------------------
    // Write multiplicity column (last column).
    // ---------------------------------------------------------------
    uint mult = (row < n_values) ? mults[row] : 0u;
    trace_out[FELT252_N_WORDS * column_length + row] = mult;
}

// ---------------------------------------------------------------------------
// Small-value variant
//
// Input layout:
//   buffer(0) — small_values: row-major [n_values][4] u32 words (u128 as 4 limbs)
//   buffer(1) — mults:        [n_values] u32 multiplicities
//   buffer(2) — trace_out:    column-major [9][column_length] u32 output
//   buffer(3) — n_values:     scalar
//   buffer(4) — column_length: scalar (power-of-two, >= n_values)
//
// N_M31_IN_SMALL_FELT252 = 8 value columns + 1 multiplicity column = 9.
// ---------------------------------------------------------------------------
constant uint N_SMALL_WORDS   = 8u;  // N_M31_IN_SMALL_FELT252
constant uint N_SMALL_COLUMNS = N_SMALL_WORDS + 1u; // 9

kernel void witness_memory_id_to_big_small_trace(
    device const uint  *small_values   [[buffer(0)]],
    device const uint  *mults          [[buffer(1)]],
    device       uint  *trace_out      [[buffer(2)]],
    constant     uint  &n_values       [[buffer(3)]],
    constant     uint  &column_length  [[buffer(4)]],
    uint               row            [[thread_position_in_grid]]
) {
    if (row >= column_length) {
        return;
    }

    // Load 4 u32 limbs of the u128 value (or zero-pad).
    uint u128_limbs[4];
    if (row < n_values) {
        uint base = row * 4u;
        for (uint i = 0u; i < 4u; ++i) {
            u128_limbs[i] = small_values[base + i];
        }
    } else {
        for (uint i = 0u; i < 4u; ++i) {
            u128_limbs[i] = 0u;
        }
    }

    // Build the 8-word input for split_f252 (upper 4 words are zero).
    uint input_words[8];
    for (uint i = 0u; i < 4u; ++i) {
        input_words[i] = u128_limbs[i];
    }
    for (uint i = 4u; i < 8u; ++i) {
        input_words[i] = 0u;
    }

    // Split into 28 limbs, but we only use the first N_SMALL_WORDS = 8.
    uint limbs[28];
    split_f252(input_words, limbs);

    // Write the 8 value columns (column-major).
    for (uint col = 0u; col < N_SMALL_WORDS; ++col) {
        trace_out[col * column_length + row] = limbs[col];
    }

    // Write multiplicity column (last column).
    uint mult = (row < n_values) ? mults[row] : 0u;
    trace_out[N_SMALL_WORDS * column_length + row] = mult;
}
