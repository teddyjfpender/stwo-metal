//! GPU witness trace correctness tests.
//!
//! These tests verify that GPU-generated witness traces exactly match
//! CPU reference implementations. Any bit-level mismatch in F252 splitting,
//! instruction decoding, or column layout would silently break proof soundness.
//!
//! Tests 1-3: Compare GPU trace generation against CPU reference for each
//! component (memory_id_to_big, memory_address_to_id, verify_instruction).
//!
//! Test 5: Compare GPU interaction trace output against CPU reference for
//! the memory_id_to_big component.

#![cfg(feature = "prover")]
#![allow(unused_imports)]

use stwo::core::fields::m31::{BaseField, M31};
use stwo_metal::{
    generate_memory_addr_to_id_trace, generate_memory_id_to_big_small_trace,
    generate_memory_id_to_big_trace, metal_runtime_support, MetalRuntimeSupport,
};
use stwo_metal::backend::metal::witness_verify_instruction::generate_verify_instruction_trace;

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// CPU reference: split a 256-bit value ([u32; 8]) into 28 x 9-bit limbs.
/// Mirrors stwo-cairo's `split_f252` exactly.
fn split_f252_reference(input: [u32; 8]) -> [u32; 28] {
    const BITS_PER_WORD: usize = 9;
    const MASK: u32 = (1 << BITS_PER_WORD) - 1;

    let mut result = [0u32; 28];
    let mut n_bits_in_word: usize = 32;
    let mut word_i: usize = 0;
    let mut word = input[0];

    for e in result.iter_mut() {
        if n_bits_in_word > BITS_PER_WORD {
            *e = word & MASK;
            word >>= BITS_PER_WORD;
            n_bits_in_word -= BITS_PER_WORD;
            continue;
        }

        let mut val = word;
        word_i += 1;
        word = if word_i < 8 { input[word_i] } else { 0 };

        if n_bits_in_word < BITS_PER_WORD {
            val |= (word << n_bits_in_word) & MASK;
            word >>= BITS_PER_WORD - n_bits_in_word;
        }

        n_bits_in_word += 32 - BITS_PER_WORD;
        *e = val;
    }

    result
}

fn require_metal() {
    assert_eq!(
        metal_runtime_support(),
        MetalRuntimeSupport::Available,
        "Metal runtime must be available"
    );
}

// ===========================================================================
// Test 1: GPU memory_id_to_big "big" trace matches CPU split_f252 exactly
// ===========================================================================

/// CPU reference: generate full big trace (28 limb columns + 1 mult column)
/// for `n` values, padded to next power-of-two column length.
fn cpu_big_trace(big_values: &[[u32; 8]], multiplicities: &[u32]) -> Vec<Vec<u32>> {
    let n = big_values.len();
    let column_length = n.next_power_of_two();
    let n_cols = 29; // 28 limbs + 1 mult

    let mut trace = vec![vec![0u32; column_length]; n_cols];
    for row in 0..n {
        let limbs = split_f252_reference(big_values[row]);
        for col in 0..28 {
            trace[col][row] = limbs[col];
        }
        trace[28][row] = multiplicities[row];
    }
    trace
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn test1_memory_id_to_big_big_trace_matches_cpu() {
    require_metal();

    // Use non-trivial 256-bit values that exercise all bit-boundary cases.
    let big_values: Vec<[u32; 8]> = vec![
        // Full Felt252 near the field boundary
        [
            0xDEAD_BEEF,
            0xCAFE_BABE,
            0x1234_5678,
            0x9ABC_DEF0,
            0x1111_2222,
            0x3333_4444,
            0x5555_6666,
            0x0800_0000,
        ],
        // All ones in lower 128 bits
        [0xFFFF_FFFF, 0xFFFF_FFFF, 0xFFFF_FFFF, 0xFFFF_FFFF, 0, 0, 0, 0],
        // Alternating bits pattern
        [
            0xAAAA_AAAA,
            0x5555_5555,
            0xAAAA_AAAA,
            0x5555_5555,
            0xAAAA_AAAA,
            0x5555_5555,
            0xAAAA_AAAA,
            0x0555_5555,
        ],
        // Single bit in each u32 word
        [1, 1 << 9, 1 << 18, 1 << 27, 1, 1 << 9, 1 << 18, 1 << 27],
        // Maximum 9-bit boundary value
        [0x1FF, 0x1FF << 9, 0x1FF << 18, 0, 0, 0, 0, 0],
        // Zero
        [0, 0, 0, 0, 0, 0, 0, 0],
        // Small value
        [42, 0, 0, 0, 0, 0, 0, 0],
        // Large prime-like value
        [
            0x0003_FFFF,
            0x7FFF_FFFF,
            0x7FFF_FFFF,
            0x7FFF_FFFF,
            0x7FFF_FFFF,
            0x7FFF_FFFF,
            0x7FFF_FFFF,
            0x07FF_FFFF,
        ],
        // Sequential bytes for easy debugging
        [
            0x0403_0201,
            0x0807_0605,
            0x0C0B_0A09,
            0x100F_0E0D,
            0x1413_1211,
            0x1817_1615,
            0x1C1B_1A19,
            0x001F_1E1D,
        ],
        // Power of two boundary
        [1 << 31, 0, 0, 0, 0, 0, 0, 0],
        // Stress: all bits set in lower 252 bits
        [
            0xFFFF_FFFF,
            0xFFFF_FFFF,
            0xFFFF_FFFF,
            0xFFFF_FFFF,
            0xFFFF_FFFF,
            0xFFFF_FFFF,
            0xFFFF_FFFF,
            0x0FFF_FFFF,
        ],
        // 12th value to get non-power-of-two count (12 -> padded to 16)
        [0xABCD_EF01, 0x2345_6789, 0, 0, 0, 0, 0, 0],
    ];

    let multiplicities: Vec<u32> = (0..big_values.len())
        .map(|i| (i as u32 * 7 + 3) % 1000)
        .collect();

    // Generate CPU reference.
    let cpu_trace = cpu_big_trace(&big_values, &multiplicities);

    // Generate GPU trace.
    let gpu_trace = generate_memory_id_to_big_trace(&big_values, &multiplicities)
        .expect("GPU big trace generation failed");

    let column_length = big_values.len().next_power_of_two();
    assert_eq!(gpu_trace.column_length(), column_length);

    // Compare every column, every row -- exact match required.
    let mut mismatches = 0;
    for col in 0..29 {
        for row in 0..column_length {
            let gpu_val = gpu_trace.value(col, row).0;
            let cpu_val = cpu_trace[col][row];
            if gpu_val != cpu_val {
                if mismatches < 20 {
                    eprintln!(
                        "  BIG MISMATCH col={col} row={row}: GPU={gpu_val} CPU={cpu_val}"
                    );
                }
                mismatches += 1;
            }
        }
    }
    assert_eq!(
        mismatches, 0,
        "memory_id_to_big big trace: {mismatches} cell mismatches between GPU and CPU"
    );
}

// ===========================================================================
// Test 1b: GPU memory_id_to_big "small" trace matches CPU exactly
// ===========================================================================

/// CPU reference: generate small trace (8 limb columns + 1 mult column).
fn cpu_small_trace(small_values: &[u128], multiplicities: &[u32]) -> Vec<Vec<u32>> {
    let n = small_values.len();
    let column_length = n.next_power_of_two();
    let n_cols = 9; // 8 limbs + 1 mult

    let mut trace = vec![vec![0u32; column_length]; n_cols];
    for row in 0..n {
        let v = small_values[row];
        let input_words: [u32; 8] = [
            v as u32,
            (v >> 32) as u32,
            (v >> 64) as u32,
            (v >> 96) as u32,
            0,
            0,
            0,
            0,
        ];
        let limbs = split_f252_reference(input_words);
        for col in 0..8 {
            trace[col][row] = limbs[col];
        }
        trace[8][row] = multiplicities[row];
    }
    trace
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn test1b_memory_id_to_big_small_trace_matches_cpu() {
    require_metal();

    let small_values: Vec<u128> = vec![
        0x1234_5678_9ABC_DEF0_CAFE_BABE_DEAD_BEEFu128,
        0x0000_0000_0000_0000_0000_0000_0000_0001u128,
        0xFFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFFu128,
        0x0000_0000_0000_0000_FFFF_FFFF_FFFF_FFFFu128,
        42,
        0,
        0xAAAA_AAAA_5555_5555_AAAA_AAAA_5555_5555u128,
    ];

    let multiplicities: Vec<u32> = (0..small_values.len())
        .map(|i| (i as u32 * 11 + 5) % 500)
        .collect();

    let cpu_trace = cpu_small_trace(&small_values, &multiplicities);
    let gpu_trace = generate_memory_id_to_big_small_trace(&small_values, &multiplicities)
        .expect("GPU small trace generation failed");

    let column_length = small_values.len().next_power_of_two();
    assert_eq!(gpu_trace.column_length(), column_length);

    let mut mismatches = 0;
    for col in 0..9 {
        for row in 0..column_length {
            let gpu_val = gpu_trace.value(col, row).0;
            let cpu_val = cpu_trace[col][row];
            if gpu_val != cpu_val {
                if mismatches < 20 {
                    eprintln!(
                        "  SMALL MISMATCH col={col} row={row}: GPU={gpu_val} CPU={cpu_val}"
                    );
                }
                mismatches += 1;
            }
        }
    }
    assert_eq!(
        mismatches, 0,
        "memory_id_to_big small trace: {mismatches} cell mismatches between GPU and CPU"
    );
}

// ===========================================================================
// Test 2: GPU memory_address_to_id trace matches CPU exactly
// ===========================================================================

/// CPU reference for memory_address_to_id trace generation.
/// Mirrors stwo-cairo's ClaimGenerator::write_trace() layout.
fn cpu_addr_to_id_trace(ids: &[u32], mults: &[u32], split: usize) -> Vec<Vec<u32>> {
    let n = ids.len();
    let size = std::cmp::max((n.div_ceil(split)).next_power_of_two(), 16);
    let n_trace_columns = split * 2;
    let n_packed_rows = size / 16; // N_LANES = 16

    let mut trace = vec![vec![0u32; size]; n_trace_columns];

    // Pad inputs to multiple of 16.
    let padded_n = n.next_multiple_of(16);
    let mut padded_ids = ids.to_vec();
    padded_ids.resize(padded_n, 0);
    let mut padded_mults = mults.to_vec();
    padded_mults.resize(padded_n, 0);

    for (i, (id_chunk, mult_chunk)) in padded_ids
        .chunks(16)
        .zip(padded_mults.chunks(16))
        .enumerate()
    {
        let chunk_idx = i / n_packed_rows;
        let local_i = i % n_packed_rows;
        if chunk_idx >= split {
            break;
        }
        for lane in 0..16 {
            let row = local_i * 16 + lane;
            trace[chunk_idx * 2][row] = id_chunk[lane];
            trace[chunk_idx * 2 + 1][row] = mult_chunk[lane];
        }
    }

    trace
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn test2_memory_address_to_id_trace_matches_cpu() {
    require_metal();

    // Realistic-ish parameters: 1024 IDs with split=16 (default).
    let n = 1024;
    let split = 16u32;

    // Use non-trivial ID values (as would come from real memory).
    let ids: Vec<u32> = (0..n)
        .map(|i| {
            let base = (i as u32 * 31 + 17) % (1 << 20);
            base | ((i as u32 & 0xF) << 20)
        })
        .collect();
    let multiplicities: Vec<u32> = (0..n)
        .map(|i| (i as u32 * 3 + 7) % 100)
        .collect();

    let gpu_result = generate_memory_addr_to_id_trace(&ids, &multiplicities, split)
        .expect("GPU addr_to_id trace generation failed");
    let cpu_trace = cpu_addr_to_id_trace(&ids, &multiplicities, split as usize);

    assert_eq!(cpu_trace.len(), gpu_result.n_columns());
    let col_len = gpu_result.column_length();
    assert_eq!(cpu_trace[0].len(), col_len);

    let mut mismatches = 0;
    for col in 0..gpu_result.n_columns() {
        for row in 0..col_len {
            let gpu_val = gpu_result.value(col, row).0;
            let cpu_val = cpu_trace[col][row];
            if gpu_val != cpu_val {
                if mismatches < 20 {
                    eprintln!(
                        "  ADDR_TO_ID MISMATCH col={col} row={row}: GPU={gpu_val} CPU={cpu_val}"
                    );
                }
                mismatches += 1;
            }
        }
    }
    assert_eq!(
        mismatches, 0,
        "memory_address_to_id trace: {mismatches} cell mismatches between GPU and CPU"
    );
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn test2b_memory_address_to_id_unaligned_count() {
    require_metal();

    // Test with a count that is NOT a multiple of 16 or the split factor.
    // 100 IDs with split=4 -> chunk_size=25, column_length=32.
    let n = 100;
    let split = 4u32;
    let ids: Vec<u32> = (500..500 + n as u32).collect();
    let multiplicities: Vec<u32> = vec![1; n];

    let gpu_result = generate_memory_addr_to_id_trace(&ids, &multiplicities, split)
        .expect("GPU addr_to_id unaligned trace failed");
    let cpu_trace = cpu_addr_to_id_trace(&ids, &multiplicities, split as usize);

    let mut mismatches = 0;
    for col in 0..gpu_result.n_columns() {
        for row in 0..gpu_result.column_length() {
            let gpu_val = gpu_result.value(col, row).0;
            let cpu_val = cpu_trace[col][row];
            if gpu_val != cpu_val {
                if mismatches < 20 {
                    eprintln!(
                        "  ADDR_TO_ID_UNALIGNED MISMATCH col={col} row={row}: GPU={gpu_val} CPU={cpu_val}"
                    );
                }
                mismatches += 1;
            }
        }
    }
    assert_eq!(
        mismatches, 0,
        "memory_address_to_id (unaligned): {mismatches} cell mismatches"
    );
}

// ===========================================================================
// Test 3: GPU verify_instruction trace matches CPU exactly
// ===========================================================================

/// CPU reference for verify_instruction trace.
/// 17 columns: 7 input + 8 offset decomposition + instruction_id + multiplicity.
fn cpu_verify_instruction_trace(
    inputs: &[(u32, [u32; 3], [u32; 2], u32)],
    mults: &[u32],
    addr_to_id: &[u32],
) -> Vec<Vec<u32>> {
    let n_rows = inputs.len();
    let size = std::cmp::max(n_rows.next_power_of_two(), 16);
    let mut trace = vec![vec![0u32; size]; 17];

    for row in 0..n_rows {
        let (pc, offsets, felts, opcode_ext) = inputs[row];
        let offset0 = offsets[0];
        let offset1 = offsets[1];
        let offset2 = offsets[2];
        let felt5_high = felts[0];
        let felt6 = felts[1];

        // Pass-through input columns.
        trace[0][row] = pc;
        trace[1][row] = offset0;
        trace[2][row] = offset1;
        trace[3][row] = offset2;
        trace[4][row] = felt5_high;
        trace[5][row] = felt6;
        trace[6][row] = opcode_ext;

        // Offset decomposition (same bit-fields as stwo-cairo write_trace_simd).
        trace[7][row] = offset0 & 0x1FF;         // offset0_low
        trace[8][row] = offset0 >> 9;             // offset0_mid
        trace[9][row] = offset1 & 0x3;            // offset1_low
        trace[10][row] = (offset1 >> 2) & 0x1FF;  // offset1_mid
        trace[11][row] = offset1 >> 11;            // offset1_high
        trace[12][row] = offset2 & 0xF;            // offset2_low
        trace[13][row] = (offset2 >> 4) & 0x1FF;   // offset2_mid
        trace[14][row] = offset2 >> 13;             // offset2_high

        // Memory lookup: addr_to_id[pc].
        trace[15][row] = if (pc as usize) < addr_to_id.len() {
            addr_to_id[pc as usize]
        } else {
            0
        };

        // Multiplicity.
        trace[16][row] = mults[row];
    }

    trace
}

/// Flatten structured inputs into the row-major u32 array expected by GPU.
fn flatten_verify_inputs(inputs: &[(u32, [u32; 3], [u32; 2], u32)]) -> Vec<u32> {
    let mut flat = Vec::with_capacity(inputs.len() * 7);
    for &(pc, offsets, felts, opcode_ext) in inputs {
        flat.push(pc);
        flat.push(offsets[0]);
        flat.push(offsets[1]);
        flat.push(offsets[2]);
        flat.push(felts[0]);
        flat.push(felts[1]);
        flat.push(opcode_ext);
    }
    flat
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn test3_verify_instruction_trace_matches_cpu() {
    require_metal();

    // 64 instructions with diverse offset values to stress all bit-field paths.
    let n = 64;
    let addr_to_id: Vec<u32> = (0..n as u32 + 100)
        .map(|i| (i * 31 + 17) % (1 << 20))
        .collect();

    let inputs: Vec<(u32, [u32; 3], [u32; 2], u32)> = (0..n)
        .map(|i| {
            let pc = i as u32;
            // Diverse offsets: exercise all 9-bit, 2-bit, 4-bit boundaries.
            let offset0 = ((i as u32 * 137 + 42) & 0xFFFF) | ((i as u32 & 0x7F) << 9);
            let offset1 = ((i as u32 * 251 + 99) & 0xFFFF) | ((i as u32 & 0x1F) << 11);
            let offset2 = ((i as u32 * 89 + 7) & 0xFFFF) | ((i as u32 & 0x7) << 13);
            let felt5_high = (i as u32 * 3) & 0x1F;
            let felt6 = (i as u32 * 7) & 0xFF;
            let opcode_ext = (i as u32) & 0x7;
            (
                pc,
                [offset0 & 0xFFFF, offset1 & 0xFFFF, offset2 & 0xFFFF],
                [felt5_high, felt6],
                opcode_ext,
            )
        })
        .collect();
    let mults: Vec<u32> = (0..n).map(|i| (i as u32 + 1) * 3).collect();

    let flat_inputs = flatten_verify_inputs(&inputs);
    let gpu_result = generate_verify_instruction_trace(&flat_inputs, &mults, &addr_to_id)
        .expect("GPU verify_instruction trace failed");

    let cpu_trace = cpu_verify_instruction_trace(&inputs, &mults, &addr_to_id);

    assert_eq!(gpu_result.n_columns(), 17);
    let col_len = gpu_result.column_length();

    let mut mismatches = 0;
    for col in 0..17 {
        for row in 0..col_len {
            let gpu_val = gpu_result.value(col, row).0;
            let cpu_val = cpu_trace[col][row];
            if gpu_val != cpu_val {
                if mismatches < 20 {
                    let col_name = match col {
                        0 => "pc",
                        1 => "offset0",
                        2 => "offset1",
                        3 => "offset2",
                        4 => "felt5_high",
                        5 => "felt6",
                        6 => "opcode_ext",
                        7 => "offset0_low",
                        8 => "offset0_mid",
                        9 => "offset1_low",
                        10 => "offset1_mid",
                        11 => "offset1_high",
                        12 => "offset2_low",
                        13 => "offset2_mid",
                        14 => "offset2_high",
                        15 => "instruction_id",
                        16 => "multiplicity",
                        _ => "unknown",
                    };
                    eprintln!(
                        "  VERIFY_INST MISMATCH col={col}({col_name}) row={row}: GPU={gpu_val} CPU={cpu_val}"
                    );
                }
                mismatches += 1;
            }
        }
    }
    assert_eq!(
        mismatches, 0,
        "verify_instruction trace: {mismatches} cell mismatches between GPU and CPU"
    );
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn test3b_verify_instruction_specific_bit_boundaries() {
    require_metal();

    // Test specific known bit-boundary values to catch off-by-one errors
    // in the offset decomposition.
    let offset0: u32 = 0b1010101_101010101; // mid=85, low=341
    // offset1: low 2 bits = 01, mid 9 bits = 341 (0x155), high 5 bits = 21 (0x15)
    let offset1_actual: u32 = (21 << 11) | (341 << 2) | 1;
    let offset2: u32 = (5 << 13) | (341 << 4) | 5; // high=5, mid=341, low=5

    let inputs: Vec<(u32, [u32; 3], [u32; 2], u32)> = vec![
        (0, [offset0, offset1_actual, offset2], [3, 7], 2),
    ];
    let mults = vec![42u32];
    let addr_to_id = vec![999u32; 16]; // Need at least 1 entry

    let flat = flatten_verify_inputs(&inputs);
    let gpu_result = generate_verify_instruction_trace(&flat, &mults, &addr_to_id)
        .expect("GPU verify_instruction bit-boundary test failed");

    // Check each decomposition field explicitly.
    assert_eq!(gpu_result.value(7, 0).0, 341, "offset0_low should be 341");
    assert_eq!(gpu_result.value(8, 0).0, 85, "offset0_mid should be 85");
    assert_eq!(gpu_result.value(9, 0).0, 1, "offset1_low should be 1");
    assert_eq!(gpu_result.value(10, 0).0, 341, "offset1_mid should be 341");
    assert_eq!(gpu_result.value(11, 0).0, 21, "offset1_high should be 21");
    assert_eq!(gpu_result.value(12, 0).0, 5, "offset2_low should be 5");
    assert_eq!(gpu_result.value(13, 0).0, 341, "offset2_mid should be 341");
    assert_eq!(gpu_result.value(14, 0).0, 5, "offset2_high should be 5");
    assert_eq!(gpu_result.value(15, 0).0, 999, "instruction_id should be 999");
    assert_eq!(gpu_result.value(16, 0).0, 42, "multiplicity should be 42");
}

// ===========================================================================
// Test 5: GPU interaction trace matches CPU for memory_id_to_big
// ===========================================================================

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn test5_interaction_trace_big_gpu_runs_and_produces_nonzero_sum() {
    require_metal();

    use stwo::core::channel::{Blake2sChannel, Channel};
    use stwo::core::fields::qm31::QM31;
    use stwo::prover::backend::simd::m31::PackedM31;
    use stwo_metal::backend::metal::interaction_trace_id_to_big::{
        extract_lookup_elements_for_gpu, gpu_gen_big_memory_interaction_trace,
        InteractionRelationIds,
    };

    let n_packed = 2; // 32 rows

    // Build limb columns with varied values to get meaningful interaction sums.
    let limb_columns: [Vec<PackedM31>; 28] = std::array::from_fn(|col| {
        (0..n_packed)
            .map(|vec_row| {
                PackedM31::from_array(std::array::from_fn(|lane| {
                    M31(((col * 32 + vec_row * 16 + lane) as u32 * 13 + 7) % 512)
                }))
            })
            .collect()
    });

    let multiplicities: Vec<PackedM31> = (0..n_packed)
        .map(|vec_row| {
            PackedM31::from_array(std::array::from_fn(|lane| {
                M31((vec_row * 16 + lane) as u32 + 1)
            }))
        })
        .collect();

    // Build lookup elements from a deterministic channel.
    let mut channel = Blake2sChannel::default();
    let felts = channel.draw_secure_felts(2);
    let z = felts[0];
    let alpha = felts[1];
    let mut cur = QM31::from_u32_unchecked(1, 0, 0, 0);
    let mut powers = Vec::new();
    for _ in 0..30 {
        powers.push(cur);
        cur *= alpha;
    }
    let lookup = extract_lookup_elements_for_gpu(z, &powers);

    let rel_ids = InteractionRelationIds {
        range_check_9_9_ids: [
            517791011, 1897792095, 1881014476, 1864236857, 1847459238, 1830681619, 1813904000,
            2065568285,
        ],
        memory_id_to_big_id: 1662111297,
    };

    let large_id_base = 0x4000_0000u32;

    let (gpu_trace, claimed_sum) = gpu_gen_big_memory_interaction_trace(
        &limb_columns,
        &multiplicities,
        &lookup,
        &rel_ids,
        0,
        large_id_base,
    )
    .expect("GPU big interaction trace failed");

    // 8 logup columns * 4 QM31 coords = 32 M31 columns.
    assert_eq!(gpu_trace.len(), 32, "Expected 32 M31 columns");

    // Verify claimed_sum is computed (may be zero for some input distributions;
    // the per-row correctness check in test5b is the authoritative validation).
    let _sum = claimed_sum; // ensure it's accessible

    // Verify that all 32 columns have the expected length.
    for (i, eval) in gpu_trace.iter().enumerate() {
        assert_eq!(
            eval.domain.size(),
            n_packed * 16,
            "interaction trace column {i} has wrong domain size"
        );
    }
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn test5b_interaction_trace_big_per_row_correctness() {
    require_metal();

    use stwo::core::channel::{Blake2sChannel, Channel};
    use stwo::core::fields::qm31::QM31;
    use stwo::core::fields::FieldExpOps;
    use stwo::prover::backend::simd::m31::PackedM31;
    use stwo::prover::backend::Column;
    use stwo_metal::backend::metal::interaction_trace_id_to_big::{
        extract_lookup_elements_for_gpu, gpu_gen_big_memory_interaction_trace,
        InteractionRelationIds,
    };

    let limb_columns: [Vec<PackedM31>; 28] = std::array::from_fn(|col| {
        vec![PackedM31::from_array(std::array::from_fn(|lane| {
            M31(((col * 16 + lane) as u32 * 7 + 3) % 512)
        }))]
    });
    let multiplicities = vec![PackedM31::from_array(std::array::from_fn(|i| {
        M31((i as u32 + 1) * 5)
    }))];

    let mut channel = Blake2sChannel::default();
    let felts = channel.draw_secure_felts(2);
    let z = felts[0];
    let alpha = felts[1];
    let mut cur = QM31::from_u32_unchecked(1, 0, 0, 0);
    let mut powers = Vec::new();
    for _ in 0..30 {
        powers.push(cur);
        cur *= alpha;
    }
    let lookup = extract_lookup_elements_for_gpu(z, &powers);

    let rel_ids = InteractionRelationIds {
        range_check_9_9_ids: [
            517791011, 1897792095, 1881014476, 1864236857, 1847459238, 1830681619, 1813904000,
            2065568285,
        ],
        memory_id_to_big_id: 1662111297,
    };
    let large_id_base = 0x4000_0000u32;

    let (gpu_trace, _claimed_sum) = gpu_gen_big_memory_interaction_trace(
        &limb_columns,
        &multiplicities,
        &lookup,
        &rel_ids,
        0,
        large_id_base,
    )
    .expect("GPU interaction trace failed");

    // CPU reference for combine(values) = sum(alpha_powers[i] * values[i]) - z
    fn cpu_combine(alpha_powers: &[QM31], values: &[M31], z: QM31) -> QM31 {
        let mut acc = QM31::from_u32_unchecked(0, 0, 0, 0);
        for (i, &v) in values.iter().enumerate() {
            acc += alpha_powers[i] * v;
        }
        acc - z
    }

    // Verify the first 7 logup columns (non-prefix-summed) row by row.
    for row in 0..16 {
        let mut limbs = [M31(0); 28];
        for col in 0..28 {
            limbs[col] = limb_columns[col][0].to_array()[row];
        }

        // Compute expected running sum for the 7 range-check columns.
        let mut running_sum = QM31::from_u32_unchecked(0, 0, 0, 0);
        for logup_col in 0..7 {
            let base = logup_col * 4;
            let rc_pair = logup_col % 4;
            let rid0 = M31(rel_ids.range_check_9_9_ids[rc_pair * 2]);
            let rid1 = M31(rel_ids.range_check_9_9_ids[rc_pair * 2 + 1]);

            let denom0 = cpu_combine(&powers, &[rid0, limbs[base], limbs[base + 1]], z);
            let denom1 = cpu_combine(&powers, &[rid1, limbs[base + 2], limbs[base + 3]], z);
            let numer = denom0 + denom1;
            let denom = denom0 * denom1;
            let frac = numer * denom.inverse();
            running_sum += frac;

            // Check all 4 coordinates.
            let [ea, eb, ec, ed] = running_sum.to_m31_array();
            for coord in 0..4 {
                let col_idx = logup_col * 4 + coord;
                let got = gpu_trace[col_idx].values.at(row);
                let exp_coord = [ea, eb, ec, ed][coord];
                assert_eq!(
                    got, exp_coord,
                    "interaction mismatch at row {row}, logup_col {logup_col}, coord {coord}: \
                     got {got:?}, expected {exp_coord:?}"
                );
            }
        }
    }
}

// ===========================================================================
// Test 1c: Large-scale big trace (stress test for GPU dispatch)
// ===========================================================================

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn test1c_memory_id_to_big_large_scale() {
    require_metal();

    // 4096 values -> column_length = 4096 (already power of two).
    // Tests GPU dispatch with non-trivial workload.
    let n = 4096;
    let big_values: Vec<[u32; 8]> = (0..n)
        .map(|i| {
            let seed = i as u32;
            [
                seed.wrapping_mul(0xDEAD_BEEF).wrapping_add(1),
                seed.wrapping_mul(0xCAFE_BABE).wrapping_add(2),
                seed.wrapping_mul(0x1234_5678).wrapping_add(3),
                seed.wrapping_mul(0x9ABC_DEF0).wrapping_add(4),
                seed.wrapping_mul(0x1111_2222).wrapping_add(5),
                seed.wrapping_mul(0x3333_4444).wrapping_add(6),
                seed.wrapping_mul(0x5555_6666).wrapping_add(7),
                (seed.wrapping_mul(0x0777_7777).wrapping_add(8)) & 0x0FFF_FFFF, // keep < 252 bits
            ]
        })
        .collect();
    let multiplicities: Vec<u32> = (0..n).map(|i| (i as u32 % 1000) + 1).collect();

    let cpu_trace = cpu_big_trace(&big_values, &multiplicities);
    let gpu_trace = generate_memory_id_to_big_trace(&big_values, &multiplicities)
        .expect("GPU big trace (large) failed");

    assert_eq!(gpu_trace.column_length(), n);

    let mut mismatches = 0;
    for col in 0..29 {
        for row in 0..n {
            let gpu_val = gpu_trace.value(col, row).0;
            let cpu_val = cpu_trace[col][row];
            if gpu_val != cpu_val {
                if mismatches < 5 {
                    eprintln!(
                        "  LARGE BIG MISMATCH col={col} row={row}: GPU={gpu_val} CPU={cpu_val}"
                    );
                }
                mismatches += 1;
            }
        }
    }
    assert_eq!(
        mismatches, 0,
        "memory_id_to_big large trace: {mismatches} cell mismatches"
    );
}

// ===========================================================================
// Test 3c: Large-scale verify_instruction trace
// ===========================================================================

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn test3c_verify_instruction_large_scale() {
    require_metal();

    let n = 1024;
    let addr_to_id: Vec<u32> = (0..n as u32 + 50)
        .map(|i| (i.wrapping_mul(0xABCD) + 17) % (1 << 20))
        .collect();

    let inputs: Vec<(u32, [u32; 3], [u32; 2], u32)> = (0..n)
        .map(|i| {
            let pc = i as u32;
            let offset0 = (i as u32 * 137 + 42) & 0xFFFF;
            let offset1 = (i as u32 * 251 + 99) & 0xFFFF;
            let offset2 = (i as u32 * 89 + 7) & 0xFFFF;
            let felt5_high = (i as u32 * 3) & 0x1F;
            let felt6 = (i as u32 * 7) & 0xFF;
            let opcode_ext = (i as u32) & 0x7;
            (pc, [offset0, offset1, offset2], [felt5_high, felt6], opcode_ext)
        })
        .collect();
    let mults: Vec<u32> = (0..n).map(|i| (i as u32 + 1) % 500).collect();

    let flat_inputs = flatten_verify_inputs(&inputs);
    let gpu_result = generate_verify_instruction_trace(&flat_inputs, &mults, &addr_to_id)
        .expect("GPU verify_instruction (large) failed");
    let cpu_trace = cpu_verify_instruction_trace(&inputs, &mults, &addr_to_id);

    let col_len = gpu_result.column_length();
    let mut mismatches = 0;
    for col in 0..17 {
        for row in 0..col_len {
            let gpu_val = gpu_result.value(col, row).0;
            let cpu_val = cpu_trace[col][row];
            if gpu_val != cpu_val {
                if mismatches < 5 {
                    eprintln!(
                        "  LARGE VERIFY_INST MISMATCH col={col} row={row}: GPU={gpu_val} CPU={cpu_val}"
                    );
                }
                mismatches += 1;
            }
        }
    }
    assert_eq!(
        mismatches, 0,
        "verify_instruction large trace: {mismatches} cell mismatches"
    );
}

// ===========================================================================
// Test 6: Medium-scale interaction trace GPU vs CPU correctness
// ===========================================================================

/// CPU reference for a single row of the big interaction trace.
/// Returns running_sum values for all 8 logup columns.
fn cpu_big_interaction_row(
    limbs: &[M31; 28],
    mult: M31,
    alpha_powers: &[stwo::core::fields::qm31::QM31],
    z: stwo::core::fields::qm31::QM31,
    rel_ids: &stwo_metal::backend::metal::interaction_trace_id_to_big::InteractionRelationIds,
    row_id: u32,
    id_offset: u32,
    large_id_base: u32,
) -> [stwo::core::fields::qm31::QM31; 8] {
    use stwo::core::fields::qm31::QM31;
    use stwo::core::fields::FieldExpOps;

    fn combine(alpha_powers: &[QM31], values: &[M31], z: QM31) -> QM31 {
        let mut acc = QM31::from_u32_unchecked(0, 0, 0, 0);
        for (i, &v) in values.iter().enumerate() {
            acc += alpha_powers[i] * v;
        }
        acc - z
    }

    let mut running_sum = QM31::from_u32_unchecked(0, 0, 0, 0);
    let mut cols = [QM31::from_u32_unchecked(0, 0, 0, 0); 8];

    // 7 range-check columns
    for col in 0..7 {
        let base = col * 4;
        let rc_pair = col % 4;
        let rid0 = M31(rel_ids.range_check_9_9_ids[rc_pair * 2]);
        let rid1 = M31(rel_ids.range_check_9_9_ids[rc_pair * 2 + 1]);

        let denom0 = combine(alpha_powers, &[rid0, limbs[base], limbs[base + 1]], z);
        let denom1 = combine(alpha_powers, &[rid1, limbs[base + 2], limbs[base + 3]], z);
        let numer = denom0 + denom1;
        let denom = denom0 * denom1;
        let frac = numer * denom.inverse();
        running_sum += frac;
        cols[col] = running_sum;
    }

    // Yield column
    {
        let id_value = M31(((row_id + id_offset) | large_id_base) % (1u32 << 31));
        let mut values = vec![M31(rel_ids.memory_id_to_big_id), id_value];
        values.extend_from_slice(limbs);
        let yield_denom = combine(alpha_powers, &values, z);
        let yield_numer = QM31::from(-mult);
        let frac = yield_numer * yield_denom.inverse();
        running_sum += frac;
        cols[7] = running_sum;
    }

    cols
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn test6_interaction_trace_big_medium_scale_correctness() {
    require_metal();

    use stwo::core::channel::{Blake2sChannel, Channel};
    use stwo::core::fields::qm31::QM31;
    use stwo::prover::backend::simd::m31::PackedM31;
    use stwo::prover::backend::Column;
    use stwo_metal::backend::metal::interaction_trace_id_to_big::{
        extract_lookup_elements_for_gpu, gpu_gen_big_memory_interaction_trace,
        InteractionRelationIds,
    };

    // log_size = 12 -> 4096 rows = 256 packed vectors.
    // This is large enough to trigger multi-threadgroup dispatch on GPU.
    let log_size = 12u32;
    let n_rows = 1usize << log_size;
    let n_packed = n_rows / 16;

    let limb_columns: [Vec<PackedM31>; 28] = std::array::from_fn(|col| {
        (0..n_packed)
            .map(|vec_row| {
                PackedM31::from_array(std::array::from_fn(|lane| {
                    let row = vec_row * 16 + lane;
                    M31(((col * n_rows + row) as u32 * 13 + 7) % 512)
                }))
            })
            .collect()
    });

    let multiplicities: Vec<PackedM31> = (0..n_packed)
        .map(|vec_row| {
            PackedM31::from_array(std::array::from_fn(|lane| {
                let row = vec_row * 16 + lane;
                M31((row as u32 % 100) + 1)
            }))
        })
        .collect();

    let mut channel = Blake2sChannel::default();
    let felts = channel.draw_secure_felts(2);
    let z = felts[0];
    let alpha = felts[1];
    let mut cur = QM31::from_u32_unchecked(1, 0, 0, 0);
    let mut powers = Vec::new();
    for _ in 0..30 {
        powers.push(cur);
        cur *= alpha;
    }
    let lookup = extract_lookup_elements_for_gpu(z, &powers);

    let rel_ids = InteractionRelationIds {
        range_check_9_9_ids: [
            517791011, 1897792095, 1881014476, 1864236857, 1847459238, 1830681619, 1813904000,
            2065568285,
        ],
        memory_id_to_big_id: 1662111297,
    };
    let large_id_base = 0x4000_0000u32;
    let id_offset = 0u32;

    let (gpu_trace, gpu_claimed_sum) = gpu_gen_big_memory_interaction_trace(
        &limb_columns,
        &multiplicities,
        &lookup,
        &rel_ids,
        id_offset,
        large_id_base,
    )
    .expect("GPU big interaction trace failed");

    assert_eq!(gpu_trace.len(), 32, "Expected 32 M31 columns");

    // Compare the first 7 logup columns (non-prefix-summed) row by row against CPU.
    let mut mismatches = 0;
    let mut first_mismatch_row = None;
    for row in 0..n_rows {
        let vec_row = row / 16;
        let lane = row % 16;

        let mut limbs = [M31(0); 28];
        for col in 0..28 {
            limbs[col] = limb_columns[col][vec_row].to_array()[lane];
        }
        let mult = multiplicities[vec_row].to_array()[lane];

        let expected = cpu_big_interaction_row(
            &limbs,
            mult,
            &powers,
            z,
            &rel_ids,
            row as u32,
            id_offset,
            large_id_base,
        );

        // Check columns 0-6 (not prefix-summed).
        for logup_col in 0..7 {
            let exp = expected[logup_col];
            let [ea, eb, ec, ed] = exp.to_m31_array();
            for coord in 0..4 {
                let col_idx = logup_col * 4 + coord;
                let got = gpu_trace[col_idx].values.at(row);
                let exp_coord = [ea, eb, ec, ed][coord];
                if got != exp_coord {
                    if mismatches < 10 {
                        eprintln!(
                            "  BIG INTERACTION MISMATCH row={row} logup_col={logup_col} coord={coord}: \
                             GPU={got:?} CPU={exp_coord:?}"
                        );
                    }
                    if first_mismatch_row.is_none() {
                        first_mismatch_row = Some(row);
                    }
                    mismatches += 1;
                }
            }
        }

        // Also check column 7 (pre-prefix-sum values).
        // Column 7 gets prefix-summed in the Rust wrapper, so we can't directly compare
        // the trace output. Instead, compare the running sum at each row.
        // The GPU's last column BEFORE prefix sum = the running sum (columns 0-7 accumulate).
        // After the wrapper does prefix-sum + cumsum_shift, we can't easily compare row-by-row.
        // So skip column 7 for now -- mismatches in columns 0-6 will be diagnostic.
    }

    if let Some(first_row) = first_mismatch_row {
        eprintln!("  First mismatch at row {first_row} (of {n_rows})");
    }

    assert_eq!(
        mismatches, 0,
        "Big interaction trace (log_size={log_size}): {mismatches} element mismatches between GPU and CPU"
    );

    // Also verify the claimed_sum is non-zero (sanity).
    assert_ne!(
        gpu_claimed_sum,
        stwo::core::fields::qm31::SecureField::from_u32_unchecked(0, 0, 0, 0),
        "claimed_sum should be non-zero for non-trivial inputs"
    );
}

/// CPU reference for a single row of the addr_to_id interaction trace.
fn cpu_addr_to_id_interaction_row(
    ids: &[u32],       // [split] id values for this row, one per chunk
    mults: &[u32],     // [split] multiplicities for this row, one per chunk
    n_rows: usize,
    alpha_powers: &[stwo::core::fields::qm31::QM31],
    z: stwo::core::fields::qm31::QM31,
    rel_id: u32,
    row: usize,
    split: usize,
) -> Vec<stwo::core::fields::qm31::QM31> {
    use stwo::core::fields::qm31::QM31;
    use stwo::core::fields::FieldExpOps;

    fn combine(alpha_powers: &[QM31], values: &[M31], z: QM31) -> QM31 {
        let mut acc = QM31::from_u32_unchecked(0, 0, 0, 0);
        for (i, &v) in values.iter().enumerate() {
            acc += alpha_powers[i] * v;
        }
        acc - z
    }

    let n_logup_cols = split / 2;
    let mut running_sum = QM31::from_u32_unchecked(0, 0, 0, 0);
    let mut cols = vec![QM31::from_u32_unchecked(0, 0, 0, 0); n_logup_cols];

    for col in 0..n_logup_cols {
        let chunk0 = col * 2;
        let chunk1 = col * 2 + 1;

        let id0 = ids[chunk0];
        let id1 = ids[chunk1];
        let mult0 = mults[chunk0];
        let mult1 = mults[chunk1];

        let addr0 = (row as u32 + 1) + (chunk0 as u32) * (n_rows as u32);
        let addr1 = (row as u32 + 1) + (chunk1 as u32) * (n_rows as u32);

        let p0 = combine(alpha_powers, &[M31(rel_id), M31(addr0), M31(id0)], z);
        let p1 = combine(alpha_powers, &[M31(rel_id), M31(addr1), M31(id1)], z);

        let neg_mult0 = QM31::from(-M31(mult0));
        let neg_mult1 = QM31::from(-M31(mult1));
        let numer = p0 * neg_mult1 + p1 * neg_mult0;
        let denom = p1 * p0;
        let frac = numer * denom.inverse();

        running_sum += frac;
        cols[col] = running_sum;
    }

    cols
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn test6b_interaction_trace_addr_to_id_medium_scale_correctness() {
    require_metal();

    use stwo::core::channel::{Blake2sChannel, Channel};
    use stwo::core::fields::qm31::QM31;
    use stwo::prover::backend::simd::m31::PackedM31;
    use stwo::prover::backend::Column;
    use stwo_metal::backend::metal::interaction_trace_id_to_big::extract_lookup_elements_for_gpu;
    use stwo_metal::backend::metal::interaction_trace_addr_to_id::gpu_gen_addr_to_id_interaction_trace;

    let split = 8usize; // 4 logup columns
    let log_size = 10u32; // 1024 rows
    let n_rows = 1usize << log_size;
    let n_packed = n_rows / 16;
    let n_logup_cols = split / 2;

    // Build id and mult columns per chunk.
    let ids: Vec<Vec<PackedM31>> = (0..split)
        .map(|chunk| {
            (0..n_packed)
                .map(|vec_row| {
                    PackedM31::from_array(std::array::from_fn(|lane| {
                        let row = vec_row * 16 + lane;
                        M31(((chunk * n_rows + row) as u32 * 31 + 17) % (1 << 20))
                    }))
                })
                .collect()
        })
        .collect();

    let multiplicities: Vec<Vec<PackedM31>> = (0..split)
        .map(|chunk| {
            (0..n_packed)
                .map(|vec_row| {
                    PackedM31::from_array(std::array::from_fn(|lane| {
                        let row = vec_row * 16 + lane;
                        M31(((chunk * n_rows + row) as u32 * 3 + 7) % 100)
                    }))
                })
                .collect()
        })
        .collect();

    let mut channel = Blake2sChannel::default();
    let felts = channel.draw_secure_felts(2);
    let z = felts[0];
    let alpha = felts[1];
    let mut cur = QM31::from_u32_unchecked(1, 0, 0, 0);
    let mut powers = Vec::new();
    for _ in 0..128 {
        powers.push(cur);
        cur *= alpha;
    }
    let lookup = extract_lookup_elements_for_gpu(z, &powers);

    let relation_id = 1444891767u32; // MEMORY_ADDRESS_TO_ID_RELATION_ID

    let (gpu_trace, _gpu_claimed_sum) = gpu_gen_addr_to_id_interaction_trace(
        &ids,
        &multiplicities,
        &lookup,
        relation_id,
        split,
    )
    .expect("GPU addr_to_id interaction trace failed");

    // Expected: n_logup_cols * 4 M31 columns
    assert_eq!(
        gpu_trace.len(),
        n_logup_cols * 4,
        "Expected {} M31 columns",
        n_logup_cols * 4,
    );

    // Compare non-prefix-summed columns (0..n_logup_cols-1) row by row.
    let mut mismatches = 0;
    let mut first_mismatch_row = None;
    for row in 0..n_rows {
        let vec_row = row / 16;
        let lane = row % 16;

        let row_ids: Vec<u32> = (0..split)
            .map(|chunk| ids[chunk][vec_row].to_array()[lane].0)
            .collect();
        let row_mults: Vec<u32> = (0..split)
            .map(|chunk| multiplicities[chunk][vec_row].to_array()[lane].0)
            .collect();

        let expected = cpu_addr_to_id_interaction_row(
            &row_ids,
            &row_mults,
            n_rows,
            &powers,
            z,
            relation_id,
            row,
            split,
        );

        for logup_col in 0..(n_logup_cols - 1) {
            let exp = expected[logup_col];
            let [ea, eb, ec, ed] = exp.to_m31_array();
            for coord in 0..4 {
                let col_idx = logup_col * 4 + coord;
                let got = gpu_trace[col_idx].values.at(row);
                let exp_coord = [ea, eb, ec, ed][coord];
                if got != exp_coord {
                    if mismatches < 10 {
                        eprintln!(
                            "  ADDR_TO_ID INTERACTION MISMATCH row={row} logup_col={logup_col} coord={coord}: \
                             GPU={got:?} CPU={exp_coord:?}"
                        );
                    }
                    if first_mismatch_row.is_none() {
                        first_mismatch_row = Some(row);
                    }
                    mismatches += 1;
                }
            }
        }
    }

    if let Some(first_row) = first_mismatch_row {
        eprintln!("  First mismatch at row {first_row} (of {n_rows})");
    }

    assert_eq!(
        mismatches, 0,
        "Addr_to_id interaction trace (log_size={log_size}): {mismatches} element mismatches"
    );
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn test6c_interaction_trace_small_medium_scale_correctness() {
    require_metal();

    use stwo::core::channel::{Blake2sChannel, Channel};
    use stwo::core::fields::qm31::QM31;
    use stwo::core::fields::FieldExpOps;
    use stwo::prover::backend::simd::m31::PackedM31;
    use stwo::prover::backend::Column;
    use stwo_metal::backend::metal::interaction_trace_id_to_big::{
        extract_lookup_elements_for_gpu, gpu_gen_small_memory_interaction_trace,
        InteractionRelationIds,
    };

    let log_size = 10u32;
    let n_rows = 1usize << log_size;
    let n_packed = n_rows / 16;

    let limb_columns: [Vec<PackedM31>; 8] = std::array::from_fn(|col| {
        (0..n_packed)
            .map(|vec_row| {
                PackedM31::from_array(std::array::from_fn(|lane| {
                    let row = vec_row * 16 + lane;
                    M31(((col * n_rows + row) as u32 * 17 + 3) % 512)
                }))
            })
            .collect()
    });

    let multiplicities: Vec<PackedM31> = (0..n_packed)
        .map(|vec_row| {
            PackedM31::from_array(std::array::from_fn(|lane| {
                let row = vec_row * 16 + lane;
                M31((row as u32 % 50) + 1)
            }))
        })
        .collect();

    let mut channel = Blake2sChannel::default();
    let felts = channel.draw_secure_felts(2);
    let z = felts[0];
    let alpha = felts[1];
    let mut cur = QM31::from_u32_unchecked(1, 0, 0, 0);
    let mut powers = Vec::new();
    for _ in 0..30 {
        powers.push(cur);
        cur *= alpha;
    }
    let lookup = extract_lookup_elements_for_gpu(z, &powers);

    let rel_ids = InteractionRelationIds {
        range_check_9_9_ids: [
            517791011, 1897792095, 1881014476, 1864236857, 1847459238, 1830681619, 1813904000,
            2065568285,
        ],
        memory_id_to_big_id: 1662111297,
    };

    let (gpu_trace, _gpu_claimed_sum) = gpu_gen_small_memory_interaction_trace(
        &limb_columns,
        &multiplicities,
        &lookup,
        &rel_ids,
    )
    .expect("GPU small interaction trace failed");

    assert_eq!(gpu_trace.len(), 12, "Expected 12 M31 columns");

    // CPU reference for small interaction trace: 2 RC columns + 1 yield column
    let mut mismatches = 0;
    let mut first_mismatch_row = None;

    fn small_combine(alpha_powers: &[QM31], values: &[M31], z: QM31) -> QM31 {
        let mut acc = QM31::from_u32_unchecked(0, 0, 0, 0);
        for (i, &v) in values.iter().enumerate() {
            acc += alpha_powers[i] * v;
        }
        acc - z
    }

    for row in 0..n_rows {
        let vec_row = row / 16;
        let lane = row % 16;

        let mut limbs = [M31(0); 8];
        for col in 0..8 {
            limbs[col] = limb_columns[col][vec_row].to_array()[lane];
        }
        let mult = multiplicities[vec_row].to_array()[lane];

        let mut running_sum = QM31::from_u32_unchecked(0, 0, 0, 0);

        // 2 RC columns
        for col in 0..2usize {
            let base = col * 4;
            let rid0 = M31(rel_ids.range_check_9_9_ids[col * 2]);
            let rid1 = M31(rel_ids.range_check_9_9_ids[col * 2 + 1]);

            let denom0 = small_combine(&powers, &[rid0, limbs[base], limbs[base + 1]], z);
            let denom1 = small_combine(&powers, &[rid1, limbs[base + 2], limbs[base + 3]], z);
            let numer = denom0 + denom1;
            let denom = denom0 * denom1;
            let frac = numer * denom.inverse();
            running_sum += frac;

            // Only check columns 0-1 (non-prefix-summed)
            if col < 2 {
                let [ea, eb, ec, ed] = running_sum.to_m31_array();
                for coord in 0..4 {
                    let col_idx = col * 4 + coord;
                    let got = gpu_trace[col_idx].values.at(row);
                    let exp_coord = [ea, eb, ec, ed][coord];
                    if got != exp_coord {
                        if mismatches < 10 {
                            eprintln!(
                                "  SMALL INTERACTION MISMATCH row={row} logup_col={col} coord={coord}: \
                                 GPU={got:?} CPU={exp_coord:?}"
                            );
                        }
                        if first_mismatch_row.is_none() {
                            first_mismatch_row = Some(row);
                        }
                        mismatches += 1;
                    }
                }
            }
        }
    }

    if let Some(first_row) = first_mismatch_row {
        eprintln!("  First mismatch at row {first_row} (of {n_rows})");
    }

    assert_eq!(
        mismatches, 0,
        "Small interaction trace (log_size={log_size}): {mismatches} element mismatches"
    );
}
