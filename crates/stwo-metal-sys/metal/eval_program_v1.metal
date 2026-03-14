#include "secure_field_support.h"

constant uint STWO_METAL_EVAL_PROGRAM_V1_MAX_BASE_REGS = 1024u;
constant uint STWO_METAL_EVAL_PROGRAM_V1_MAX_EXT_REGS = 256u;

// Variant B: larger register file for complex Cairo components.
constant uint STWO_METAL_EVAL_PROGRAM_V1_MAX_BASE_REGS_B = 512u;
constant uint STWO_METAL_EVAL_PROGRAM_V1_MAX_EXT_REGS_B = 512u;

// Variant C: even larger ext register file for components like add_opcode.
constant uint STWO_METAL_EVAL_PROGRAM_V1_MAX_BASE_REGS_C = 512u;
constant uint STWO_METAL_EVAL_PROGRAM_V1_MAX_EXT_REGS_C = 768u;

constant uint STWO_METAL_EVAL_PROGRAM_V1_BASE_INST_WORDS = 4u;
constant uint STWO_METAL_EVAL_PROGRAM_V1_EXT_INST_WORDS = 5u;

constant uint STWO_METAL_EVAL_PROGRAM_V1_BASE_TRACE_COL = 0u;
constant uint STWO_METAL_EVAL_PROGRAM_V1_BASE_PREPROCESSED_COL = 1u;
constant uint STWO_METAL_EVAL_PROGRAM_V1_BASE_PARAM = 2u;
constant uint STWO_METAL_EVAL_PROGRAM_V1_BASE_CONST = 3u;
constant uint STWO_METAL_EVAL_PROGRAM_V1_BASE_ADD = 4u;
constant uint STWO_METAL_EVAL_PROGRAM_V1_BASE_SUB = 5u;
constant uint STWO_METAL_EVAL_PROGRAM_V1_BASE_MUL = 6u;
constant uint STWO_METAL_EVAL_PROGRAM_V1_BASE_NEG = 7u;
constant uint STWO_METAL_EVAL_PROGRAM_V1_BASE_INV = 8u;

constant uint STWO_METAL_EVAL_PROGRAM_V1_EXT_SECURE_COL = 0u;
constant uint STWO_METAL_EVAL_PROGRAM_V1_EXT_PARAM = 1u;
constant uint STWO_METAL_EVAL_PROGRAM_V1_EXT_CONST = 2u;
constant uint STWO_METAL_EVAL_PROGRAM_V1_EXT_ADD = 3u;
constant uint STWO_METAL_EVAL_PROGRAM_V1_EXT_SUB = 4u;
constant uint STWO_METAL_EVAL_PROGRAM_V1_EXT_MUL = 5u;
constant uint STWO_METAL_EVAL_PROGRAM_V1_EXT_NEG = 6u;

/// Reverse the bottom `bits` bits of `index`.
static inline uint stwo_metal_bit_reverse(uint index, uint bits) {
    uint result = 0u;
    for (uint i = 0u; i < bits; ++i) {
        result = (result << 1u) | (index & 1u);
        index >>= 1u;
    }
    return result;
}

/// Map a bit-reversed row index to the row that is `offset` positions away
/// in the natural circle domain ordering, then return the bit-reversed index
/// of that row.  Matches stwo's `offset_bit_reversed_circle_domain_index`.
static inline uint stwo_metal_offset_bit_reversed_circle_domain_index(
    uint i,
    uint domain_log_size,
    uint eval_log_size,
    int offset
) {
    uint prev = stwo_metal_bit_reverse(i, eval_log_size);
    uint half_size = 1u << (eval_log_size - 1u);
    int step = offset * int(1u << (eval_log_size - domain_log_size - 1u));
    if (prev < half_size) {
        prev = uint((int(prev) + step) % int(half_size));
        if (int(prev) < 0) prev += half_size;
    } else {
        int p = int(prev) - step;
        p = p % int(half_size);
        if (p < 0) p += int(half_size);
        prev = uint(p) + half_size;
    }
    return stwo_metal_bit_reverse(prev, eval_log_size);
}

static inline uint stwo_metal_eval_program_trace_value(
    device const uint *trace_values,
    device const uint *interaction_offsets,
    uint row_count,
    uint interaction,
    uint column,
    uint row_index,
    int offset
) {
    uint target_row;
    if (offset == 0) {
        target_row = row_index;
    } else {
        // Compute eval_log_size from row_count (row_count must be a power of 2).
        uint eval_log_size = 0u;
        uint tmp = row_count;
        while (tmp > 1u) { tmp >>= 1u; eval_log_size++; }
        // domain_log_size = eval_log_size - 1 (blowup factor 1).
        uint domain_log_size = eval_log_size - 1u;
        target_row = stwo_metal_offset_bit_reversed_circle_domain_index(
            row_index, domain_log_size, eval_log_size, offset
        );
    }
    uint global_column = interaction_offsets[interaction] + column;
    return trace_values[global_column * row_count + target_row];
}

static inline uint stwo_metal_eval_program_preprocessed_value(
    device const uint *preprocessed_values,
    uint row_count,
    uint column,
    uint row_index
) {
    return preprocessed_values[column * row_count + row_index];
}

kernel void eval_program_v1_reference_u32x4(
    device const uint *trace_values [[buffer(0)]],
    device const uint *interaction_offsets [[buffer(1)]],
    device const uint *preprocessed_values [[buffer(2)]],
    device const uint *base_params [[buffer(3)]],
    device const uint *ext_params [[buffer(4)]],
    device const uint *random_coeff_powers [[buffer(5)]],
    device const uint *base_insts [[buffer(6)]],
    device const uint *ext_insts [[buffer(7)]],
    device const uint *constraint_roots [[buffer(8)]],
    device uint *dst [[buffer(9)]],
    constant uint &row_count [[buffer(10)]],
    constant uint &n_base_insts [[buffer(11)]],
    constant uint &n_ext_insts [[buffer(12)]],
    constant uint &n_constraints [[buffer(13)]],
    uint row_index [[thread_position_in_grid]]
) {
    if (row_index >= row_count) {
        return;
    }

    thread uint base_regs[STWO_METAL_EVAL_PROGRAM_V1_MAX_BASE_REGS];
    thread StwoMetalQm31 ext_regs[STWO_METAL_EVAL_PROGRAM_V1_MAX_EXT_REGS];

    for (uint reg = 0u; reg < STWO_METAL_EVAL_PROGRAM_V1_MAX_BASE_REGS; ++reg) {
        base_regs[reg] = 0u;
    }
    for (uint reg = 0u; reg < STWO_METAL_EVAL_PROGRAM_V1_MAX_EXT_REGS; ++reg) {
        ext_regs[reg] = StwoMetalQm31 { 0u, 0u, 0u, 0u };
    }

    for (uint inst_index = 0u; inst_index < n_base_insts; ++inst_index) {
        uint word_base = inst_index * STWO_METAL_EVAL_PROGRAM_V1_BASE_INST_WORDS;
        uint word0 = base_insts[word_base + 0u];
        uint op = word0 & 0xFFu;
        uint interaction = (word0 >> 8u) & 0xFFu;
        uint dst_reg = (word0 >> 16u) & 0xFFFFu;
        uint a = base_insts[word_base + 1u];
        uint b = base_insts[word_base + 2u];
        int imm = as_type<int>(base_insts[word_base + 3u]);

        uint value = 0u;
        switch (op) {
            case STWO_METAL_EVAL_PROGRAM_V1_BASE_TRACE_COL:
                value = stwo_metal_eval_program_trace_value(
                    trace_values,
                    interaction_offsets,
                    row_count,
                    interaction,
                    a,
                    row_index,
                    imm
                );
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_BASE_PREPROCESSED_COL:
                value = stwo_metal_eval_program_preprocessed_value(
                    preprocessed_values,
                    row_count,
                    a,
                    row_index
                );
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_BASE_PARAM:
                value = base_params[a];
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_BASE_CONST:
                value = a;
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_BASE_ADD:
                value = stwo_metal_m31_add(base_regs[a], base_regs[b]);
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_BASE_SUB:
                value = stwo_metal_m31_sub(base_regs[a], base_regs[b]);
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_BASE_MUL:
                value = stwo_metal_m31_mul(base_regs[a], base_regs[b]);
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_BASE_NEG:
                value = stwo_metal_m31_neg(base_regs[a]);
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_BASE_INV:
                value = stwo_metal_m31_inv(base_regs[a]);
                break;
            default:
                value = 0u;
                break;
        }

        base_regs[dst_reg] = value;
    }

    for (uint inst_index = 0u; inst_index < n_ext_insts; ++inst_index) {
        uint word_base = inst_index * STWO_METAL_EVAL_PROGRAM_V1_EXT_INST_WORDS;
        uint word0 = ext_insts[word_base + 0u];
        uint op = word0 & 0xFFu;
        uint dst_reg = (word0 >> 16u) & 0xFFFFu;
        uint a = ext_insts[word_base + 1u];
        uint b = ext_insts[word_base + 2u];
        uint c = ext_insts[word_base + 3u];
        uint d = ext_insts[word_base + 4u];

        StwoMetalQm31 value = StwoMetalQm31 { 0u, 0u, 0u, 0u };
        switch (op) {
            case STWO_METAL_EVAL_PROGRAM_V1_EXT_SECURE_COL:
                value = StwoMetalQm31 {
                    base_regs[a],
                    base_regs[b],
                    base_regs[c],
                    base_regs[d],
                };
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_EXT_PARAM:
                value = stwo_metal_load_qm31(ext_params, a);
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_EXT_CONST:
                value = StwoMetalQm31 { a, b, c, d };
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_EXT_ADD:
                value = stwo_metal_qm31_add(ext_regs[a], ext_regs[b]);
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_EXT_SUB:
                value = stwo_metal_qm31_sub(ext_regs[a], ext_regs[b]);
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_EXT_MUL:
                value = stwo_metal_qm31_mul(ext_regs[a], ext_regs[b]);
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_EXT_NEG:
                value = stwo_metal_qm31_sub(StwoMetalQm31 { 0u, 0u, 0u, 0u }, ext_regs[a]);
                break;
            default:
                value = StwoMetalQm31 { 0u, 0u, 0u, 0u };
                break;
        }

        ext_regs[dst_reg] = value;
    }

    StwoMetalQm31 acc = StwoMetalQm31 { 0u, 0u, 0u, 0u };
    for (uint constraint_index = 0u; constraint_index < n_constraints; ++constraint_index) {
        uint root = constraint_roots[constraint_index];
        acc = stwo_metal_qm31_add(
            acc,
            stwo_metal_qm31_mul(ext_regs[root], stwo_metal_load_qm31(random_coeff_powers, constraint_index))
        );
    }

    stwo_metal_store_qm31(dst, row_index, acc);
}

// Threadgroup shared memory budget: 32,768 bytes max on Apple GPUs.
// 1536 base insts × 4 words × 4 bytes = 24,576 bytes
//  384 ext  insts × 5 words × 4 bytes =  7,680 bytes
// Total: 32,256 bytes (fits within 32KB limit).
// Programs exceeding these counts fall back to device memory reads.
constant uint STWO_METAL_EVAL_PROGRAM_V1_MAX_SHARED_BASE_INST_WORDS = 1536u * 4u;
constant uint STWO_METAL_EVAL_PROGRAM_V1_MAX_SHARED_EXT_INST_WORDS  = 384u * 5u;

kernel void eval_program_v1_optimized_u32x4(
    device const uint *trace_values [[buffer(0)]],
    device const uint *interaction_offsets [[buffer(1)]],
    device const uint *preprocessed_values [[buffer(2)]],
    device const uint *base_params [[buffer(3)]],
    device const uint *ext_params [[buffer(4)]],
    device const uint *random_coeff_powers [[buffer(5)]],
    device const uint *base_insts [[buffer(6)]],
    device const uint *ext_insts [[buffer(7)]],
    device const uint *constraint_roots [[buffer(8)]],
    device uint *dst [[buffer(9)]],
    constant uint &row_count [[buffer(10)]],
    constant uint &n_base_insts [[buffer(11)]],
    constant uint &n_ext_insts [[buffer(12)]],
    constant uint &n_constraints [[buffer(13)]],
    constant uint &max_base_regs [[buffer(14)]],
    constant uint &max_ext_regs [[buffer(15)]],
    uint row_index [[thread_position_in_grid]],
    uint tid_in_group [[thread_position_in_threadgroup]],
    uint threads_in_group [[threads_per_threadgroup]]
) {
    if (row_index >= row_count) {
        return;
    }

    // --- Phase 0: Cooperative instruction fetch into threadgroup shared memory ---
    threadgroup uint shared_base_insts[STWO_METAL_EVAL_PROGRAM_V1_MAX_SHARED_BASE_INST_WORDS];
    threadgroup uint shared_ext_insts[STWO_METAL_EVAL_PROGRAM_V1_MAX_SHARED_EXT_INST_WORDS];

    uint base_inst_words = n_base_insts * STWO_METAL_EVAL_PROGRAM_V1_BASE_INST_WORDS;
    uint ext_inst_words  = n_ext_insts * STWO_METAL_EVAL_PROGRAM_V1_EXT_INST_WORDS;

    bool use_shared_base = base_inst_words <= STWO_METAL_EVAL_PROGRAM_V1_MAX_SHARED_BASE_INST_WORDS;
    bool use_shared_ext  = ext_inst_words <= STWO_METAL_EVAL_PROGRAM_V1_MAX_SHARED_EXT_INST_WORDS;

    if (use_shared_base) {
        for (uint i = tid_in_group; i < base_inst_words; i += threads_in_group) {
            shared_base_insts[i] = base_insts[i];
        }
    }
    if (use_shared_ext) {
        for (uint i = tid_in_group; i < ext_inst_words; i += threads_in_group) {
            shared_ext_insts[i] = ext_insts[i];
        }
    }
    threadgroup_barrier(mem_flags::mem_threadgroup);

    // --- Phase 1: Initialize register files (only used registers) ---
    thread uint base_regs[STWO_METAL_EVAL_PROGRAM_V1_MAX_BASE_REGS];
    thread StwoMetalQm31 ext_regs[STWO_METAL_EVAL_PROGRAM_V1_MAX_EXT_REGS];

    for (uint reg = 0u; reg < max_base_regs; ++reg) {
        base_regs[reg] = 0u;
    }
    for (uint reg = 0u; reg < max_ext_regs; ++reg) {
        ext_regs[reg] = StwoMetalQm31 { 0u, 0u, 0u, 0u };
    }

    // --- Phase 2: Execute base instructions ---
    for (uint inst_index = 0u; inst_index < n_base_insts; ++inst_index) {
        uint word_base = inst_index * STWO_METAL_EVAL_PROGRAM_V1_BASE_INST_WORDS;
        uint word0, a, b;
        int imm;
        if (use_shared_base) {
            word0 = shared_base_insts[word_base + 0u];
            a = shared_base_insts[word_base + 1u];
            b = shared_base_insts[word_base + 2u];
            imm = as_type<int>(shared_base_insts[word_base + 3u]);
        } else {
            word0 = base_insts[word_base + 0u];
            a = base_insts[word_base + 1u];
            b = base_insts[word_base + 2u];
            imm = as_type<int>(base_insts[word_base + 3u]);
        }
        uint op = word0 & 0xFFu;
        uint interaction = (word0 >> 8u) & 0xFFu;
        uint dst_reg = (word0 >> 16u) & 0xFFFFu;

        uint value = 0u;
        switch (op) {
            case STWO_METAL_EVAL_PROGRAM_V1_BASE_TRACE_COL:
                value = stwo_metal_eval_program_trace_value(
                    trace_values,
                    interaction_offsets,
                    row_count,
                    interaction,
                    a,
                    row_index,
                    imm
                );
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_BASE_PREPROCESSED_COL:
                value = stwo_metal_eval_program_preprocessed_value(
                    preprocessed_values,
                    row_count,
                    a,
                    row_index
                );
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_BASE_PARAM:
                value = base_params[a];
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_BASE_CONST:
                value = a;
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_BASE_ADD:
                value = stwo_metal_m31_add(base_regs[a], base_regs[b]);
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_BASE_SUB:
                value = stwo_metal_m31_sub(base_regs[a], base_regs[b]);
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_BASE_MUL:
                value = stwo_metal_m31_mul(base_regs[a], base_regs[b]);
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_BASE_NEG:
                value = stwo_metal_m31_neg(base_regs[a]);
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_BASE_INV:
                value = stwo_metal_m31_inv(base_regs[a]);
                break;
            default:
                value = 0u;
                break;
        }

        base_regs[dst_reg] = value;
    }

    // --- Phase 3: Execute ext instructions ---
    for (uint inst_index = 0u; inst_index < n_ext_insts; ++inst_index) {
        uint word_base = inst_index * STWO_METAL_EVAL_PROGRAM_V1_EXT_INST_WORDS;
        uint word0, a, b, c, d;
        if (use_shared_ext) {
            word0 = shared_ext_insts[word_base + 0u];
            a = shared_ext_insts[word_base + 1u];
            b = shared_ext_insts[word_base + 2u];
            c = shared_ext_insts[word_base + 3u];
            d = shared_ext_insts[word_base + 4u];
        } else {
            word0 = ext_insts[word_base + 0u];
            a = ext_insts[word_base + 1u];
            b = ext_insts[word_base + 2u];
            c = ext_insts[word_base + 3u];
            d = ext_insts[word_base + 4u];
        }
        uint op = word0 & 0xFFu;
        uint dst_reg = (word0 >> 16u) & 0xFFFFu;

        StwoMetalQm31 value = StwoMetalQm31 { 0u, 0u, 0u, 0u };
        switch (op) {
            case STWO_METAL_EVAL_PROGRAM_V1_EXT_SECURE_COL:
                value = StwoMetalQm31 {
                    base_regs[a],
                    base_regs[b],
                    base_regs[c],
                    base_regs[d],
                };
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_EXT_PARAM:
                value = stwo_metal_load_qm31(ext_params, a);
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_EXT_CONST:
                value = StwoMetalQm31 { a, b, c, d };
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_EXT_ADD:
                value = stwo_metal_qm31_add(ext_regs[a], ext_regs[b]);
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_EXT_SUB:
                value = stwo_metal_qm31_sub(ext_regs[a], ext_regs[b]);
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_EXT_MUL:
                value = stwo_metal_qm31_mul(ext_regs[a], ext_regs[b]);
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_EXT_NEG:
                value = stwo_metal_qm31_sub(StwoMetalQm31 { 0u, 0u, 0u, 0u }, ext_regs[a]);
                break;
            default:
                value = StwoMetalQm31 { 0u, 0u, 0u, 0u };
                break;
        }

        ext_regs[dst_reg] = value;
    }

    // --- Phase 4: Accumulate constraint values ---
    StwoMetalQm31 acc = StwoMetalQm31 { 0u, 0u, 0u, 0u };
    for (uint constraint_index = 0u; constraint_index < n_constraints; ++constraint_index) {
        uint root = constraint_roots[constraint_index];
        acc = stwo_metal_qm31_add(
            acc,
            stwo_metal_qm31_mul(ext_regs[root], stwo_metal_load_qm31(random_coeff_powers, constraint_index))
        );
    }

    // --- Phase 5: Store result ---
    stwo_metal_store_qm31(dst, row_index, acc);
}

// ── Variant B kernels: 512 base / 512 ext register files ──────────────────

kernel void eval_program_v1_reference_b_u32x4(
    device const uint *trace_values [[buffer(0)]],
    device const uint *interaction_offsets [[buffer(1)]],
    device const uint *preprocessed_values [[buffer(2)]],
    device const uint *base_params [[buffer(3)]],
    device const uint *ext_params [[buffer(4)]],
    device const uint *random_coeff_powers [[buffer(5)]],
    device const uint *base_insts [[buffer(6)]],
    device const uint *ext_insts [[buffer(7)]],
    device const uint *constraint_roots [[buffer(8)]],
    device uint *dst [[buffer(9)]],
    constant uint &row_count [[buffer(10)]],
    constant uint &n_base_insts [[buffer(11)]],
    constant uint &n_ext_insts [[buffer(12)]],
    constant uint &n_constraints [[buffer(13)]],
    uint row_index [[thread_position_in_grid]]
) {
    if (row_index >= row_count) {
        return;
    }

    thread uint base_regs[STWO_METAL_EVAL_PROGRAM_V1_MAX_BASE_REGS_B];
    thread StwoMetalQm31 ext_regs[STWO_METAL_EVAL_PROGRAM_V1_MAX_EXT_REGS_B];

    for (uint reg = 0u; reg < STWO_METAL_EVAL_PROGRAM_V1_MAX_BASE_REGS_B; ++reg) {
        base_regs[reg] = 0u;
    }
    for (uint reg = 0u; reg < STWO_METAL_EVAL_PROGRAM_V1_MAX_EXT_REGS_B; ++reg) {
        ext_regs[reg] = StwoMetalQm31 { 0u, 0u, 0u, 0u };
    }

    for (uint inst_index = 0u; inst_index < n_base_insts; ++inst_index) {
        uint word_base = inst_index * STWO_METAL_EVAL_PROGRAM_V1_BASE_INST_WORDS;
        uint word0 = base_insts[word_base + 0u];
        uint op = word0 & 0xFFu;
        uint interaction = (word0 >> 8u) & 0xFFu;
        uint dst_reg = (word0 >> 16u) & 0xFFFFu;
        uint a = base_insts[word_base + 1u];
        uint b = base_insts[word_base + 2u];
        int imm = as_type<int>(base_insts[word_base + 3u]);

        uint value = 0u;
        switch (op) {
            case STWO_METAL_EVAL_PROGRAM_V1_BASE_TRACE_COL:
                value = stwo_metal_eval_program_trace_value(
                    trace_values,
                    interaction_offsets,
                    row_count,
                    interaction,
                    a,
                    row_index,
                    imm
                );
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_BASE_PREPROCESSED_COL:
                value = stwo_metal_eval_program_preprocessed_value(
                    preprocessed_values,
                    row_count,
                    a,
                    row_index
                );
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_BASE_PARAM:
                value = base_params[a];
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_BASE_CONST:
                value = a;
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_BASE_ADD:
                value = stwo_metal_m31_add(base_regs[a], base_regs[b]);
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_BASE_SUB:
                value = stwo_metal_m31_sub(base_regs[a], base_regs[b]);
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_BASE_MUL:
                value = stwo_metal_m31_mul(base_regs[a], base_regs[b]);
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_BASE_NEG:
                value = stwo_metal_m31_neg(base_regs[a]);
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_BASE_INV:
                value = stwo_metal_m31_inv(base_regs[a]);
                break;
            default:
                value = 0u;
                break;
        }

        base_regs[dst_reg] = value;
    }

    for (uint inst_index = 0u; inst_index < n_ext_insts; ++inst_index) {
        uint word_base = inst_index * STWO_METAL_EVAL_PROGRAM_V1_EXT_INST_WORDS;
        uint word0 = ext_insts[word_base + 0u];
        uint op = word0 & 0xFFu;
        uint dst_reg = (word0 >> 16u) & 0xFFFFu;
        uint a = ext_insts[word_base + 1u];
        uint b = ext_insts[word_base + 2u];
        uint c = ext_insts[word_base + 3u];
        uint d = ext_insts[word_base + 4u];

        StwoMetalQm31 value = StwoMetalQm31 { 0u, 0u, 0u, 0u };
        switch (op) {
            case STWO_METAL_EVAL_PROGRAM_V1_EXT_SECURE_COL:
                value = StwoMetalQm31 {
                    base_regs[a],
                    base_regs[b],
                    base_regs[c],
                    base_regs[d],
                };
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_EXT_PARAM:
                value = stwo_metal_load_qm31(ext_params, a);
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_EXT_CONST:
                value = StwoMetalQm31 { a, b, c, d };
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_EXT_ADD:
                value = stwo_metal_qm31_add(ext_regs[a], ext_regs[b]);
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_EXT_SUB:
                value = stwo_metal_qm31_sub(ext_regs[a], ext_regs[b]);
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_EXT_MUL:
                value = stwo_metal_qm31_mul(ext_regs[a], ext_regs[b]);
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_EXT_NEG:
                value = stwo_metal_qm31_sub(StwoMetalQm31 { 0u, 0u, 0u, 0u }, ext_regs[a]);
                break;
            default:
                value = StwoMetalQm31 { 0u, 0u, 0u, 0u };
                break;
        }

        ext_regs[dst_reg] = value;
    }

    StwoMetalQm31 acc = StwoMetalQm31 { 0u, 0u, 0u, 0u };
    for (uint constraint_index = 0u; constraint_index < n_constraints; ++constraint_index) {
        uint root = constraint_roots[constraint_index];
        acc = stwo_metal_qm31_add(
            acc,
            stwo_metal_qm31_mul(ext_regs[root], stwo_metal_load_qm31(random_coeff_powers, constraint_index))
        );
    }

    stwo_metal_store_qm31(dst, row_index, acc);
}

kernel void eval_program_v1_optimized_b_u32x4(
    device const uint *trace_values [[buffer(0)]],
    device const uint *interaction_offsets [[buffer(1)]],
    device const uint *preprocessed_values [[buffer(2)]],
    device const uint *base_params [[buffer(3)]],
    device const uint *ext_params [[buffer(4)]],
    device const uint *random_coeff_powers [[buffer(5)]],
    device const uint *base_insts [[buffer(6)]],
    device const uint *ext_insts [[buffer(7)]],
    device const uint *constraint_roots [[buffer(8)]],
    device uint *dst [[buffer(9)]],
    constant uint &row_count [[buffer(10)]],
    constant uint &n_base_insts [[buffer(11)]],
    constant uint &n_ext_insts [[buffer(12)]],
    constant uint &n_constraints [[buffer(13)]],
    constant uint &max_base_regs [[buffer(14)]],
    constant uint &max_ext_regs [[buffer(15)]],
    uint row_index [[thread_position_in_grid]],
    uint tid_in_group [[thread_position_in_threadgroup]],
    uint threads_in_group [[threads_per_threadgroup]]
) {
    if (row_index >= row_count) {
        return;
    }

    // --- Phase 0: Cooperative instruction fetch into threadgroup shared memory ---
    threadgroup uint shared_base_insts[STWO_METAL_EVAL_PROGRAM_V1_MAX_SHARED_BASE_INST_WORDS];
    threadgroup uint shared_ext_insts[STWO_METAL_EVAL_PROGRAM_V1_MAX_SHARED_EXT_INST_WORDS];

    uint base_inst_words = n_base_insts * STWO_METAL_EVAL_PROGRAM_V1_BASE_INST_WORDS;
    uint ext_inst_words  = n_ext_insts * STWO_METAL_EVAL_PROGRAM_V1_EXT_INST_WORDS;

    bool use_shared_base = base_inst_words <= STWO_METAL_EVAL_PROGRAM_V1_MAX_SHARED_BASE_INST_WORDS;
    bool use_shared_ext  = ext_inst_words <= STWO_METAL_EVAL_PROGRAM_V1_MAX_SHARED_EXT_INST_WORDS;

    if (use_shared_base) {
        for (uint i = tid_in_group; i < base_inst_words; i += threads_in_group) {
            shared_base_insts[i] = base_insts[i];
        }
    }
    if (use_shared_ext) {
        for (uint i = tid_in_group; i < ext_inst_words; i += threads_in_group) {
            shared_ext_insts[i] = ext_insts[i];
        }
    }
    threadgroup_barrier(mem_flags::mem_threadgroup);

    // --- Phase 1: Initialize register files (only used registers) ---
    thread uint base_regs[STWO_METAL_EVAL_PROGRAM_V1_MAX_BASE_REGS_B];
    thread StwoMetalQm31 ext_regs[STWO_METAL_EVAL_PROGRAM_V1_MAX_EXT_REGS_B];

    for (uint reg = 0u; reg < max_base_regs; ++reg) {
        base_regs[reg] = 0u;
    }
    for (uint reg = 0u; reg < max_ext_regs; ++reg) {
        ext_regs[reg] = StwoMetalQm31 { 0u, 0u, 0u, 0u };
    }

    // --- Phase 2: Execute base instructions ---
    for (uint inst_index = 0u; inst_index < n_base_insts; ++inst_index) {
        uint word_base = inst_index * STWO_METAL_EVAL_PROGRAM_V1_BASE_INST_WORDS;
        uint word0, a, b;
        int imm;
        if (use_shared_base) {
            word0 = shared_base_insts[word_base + 0u];
            a = shared_base_insts[word_base + 1u];
            b = shared_base_insts[word_base + 2u];
            imm = as_type<int>(shared_base_insts[word_base + 3u]);
        } else {
            word0 = base_insts[word_base + 0u];
            a = base_insts[word_base + 1u];
            b = base_insts[word_base + 2u];
            imm = as_type<int>(base_insts[word_base + 3u]);
        }
        uint op = word0 & 0xFFu;
        uint interaction = (word0 >> 8u) & 0xFFu;
        uint dst_reg = (word0 >> 16u) & 0xFFFFu;

        uint value = 0u;
        switch (op) {
            case STWO_METAL_EVAL_PROGRAM_V1_BASE_TRACE_COL:
                value = stwo_metal_eval_program_trace_value(
                    trace_values,
                    interaction_offsets,
                    row_count,
                    interaction,
                    a,
                    row_index,
                    imm
                );
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_BASE_PREPROCESSED_COL:
                value = stwo_metal_eval_program_preprocessed_value(
                    preprocessed_values,
                    row_count,
                    a,
                    row_index
                );
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_BASE_PARAM:
                value = base_params[a];
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_BASE_CONST:
                value = a;
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_BASE_ADD:
                value = stwo_metal_m31_add(base_regs[a], base_regs[b]);
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_BASE_SUB:
                value = stwo_metal_m31_sub(base_regs[a], base_regs[b]);
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_BASE_MUL:
                value = stwo_metal_m31_mul(base_regs[a], base_regs[b]);
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_BASE_NEG:
                value = stwo_metal_m31_neg(base_regs[a]);
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_BASE_INV:
                value = stwo_metal_m31_inv(base_regs[a]);
                break;
            default:
                value = 0u;
                break;
        }

        base_regs[dst_reg] = value;
    }

    // --- Phase 3: Execute ext instructions ---
    for (uint inst_index = 0u; inst_index < n_ext_insts; ++inst_index) {
        uint word_base = inst_index * STWO_METAL_EVAL_PROGRAM_V1_EXT_INST_WORDS;
        uint word0, a, b, c, d;
        if (use_shared_ext) {
            word0 = shared_ext_insts[word_base + 0u];
            a = shared_ext_insts[word_base + 1u];
            b = shared_ext_insts[word_base + 2u];
            c = shared_ext_insts[word_base + 3u];
            d = shared_ext_insts[word_base + 4u];
        } else {
            word0 = ext_insts[word_base + 0u];
            a = ext_insts[word_base + 1u];
            b = ext_insts[word_base + 2u];
            c = ext_insts[word_base + 3u];
            d = ext_insts[word_base + 4u];
        }
        uint op = word0 & 0xFFu;
        uint dst_reg = (word0 >> 16u) & 0xFFFFu;

        StwoMetalQm31 value = StwoMetalQm31 { 0u, 0u, 0u, 0u };
        switch (op) {
            case STWO_METAL_EVAL_PROGRAM_V1_EXT_SECURE_COL:
                value = StwoMetalQm31 {
                    base_regs[a],
                    base_regs[b],
                    base_regs[c],
                    base_regs[d],
                };
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_EXT_PARAM:
                value = stwo_metal_load_qm31(ext_params, a);
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_EXT_CONST:
                value = StwoMetalQm31 { a, b, c, d };
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_EXT_ADD:
                value = stwo_metal_qm31_add(ext_regs[a], ext_regs[b]);
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_EXT_SUB:
                value = stwo_metal_qm31_sub(ext_regs[a], ext_regs[b]);
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_EXT_MUL:
                value = stwo_metal_qm31_mul(ext_regs[a], ext_regs[b]);
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_EXT_NEG:
                value = stwo_metal_qm31_sub(StwoMetalQm31 { 0u, 0u, 0u, 0u }, ext_regs[a]);
                break;
            default:
                value = StwoMetalQm31 { 0u, 0u, 0u, 0u };
                break;
        }

        ext_regs[dst_reg] = value;
    }

    // --- Phase 4: Accumulate constraint values ---
    StwoMetalQm31 acc = StwoMetalQm31 { 0u, 0u, 0u, 0u };
    for (uint constraint_index = 0u; constraint_index < n_constraints; ++constraint_index) {
        uint root = constraint_roots[constraint_index];
        acc = stwo_metal_qm31_add(
            acc,
            stwo_metal_qm31_mul(ext_regs[root], stwo_metal_load_qm31(random_coeff_powers, constraint_index))
        );
    }

    // --- Phase 5: Store result ---
    stwo_metal_store_qm31(dst, row_index, acc);
}

// ── Variant C kernels: 512 base / 768 ext register files ──────────────────

kernel void eval_program_v1_reference_c_u32x4(
    device const uint *trace_values [[buffer(0)]],
    device const uint *interaction_offsets [[buffer(1)]],
    device const uint *preprocessed_values [[buffer(2)]],
    device const uint *base_params [[buffer(3)]],
    device const uint *ext_params [[buffer(4)]],
    device const uint *random_coeff_powers [[buffer(5)]],
    device const uint *base_insts [[buffer(6)]],
    device const uint *ext_insts [[buffer(7)]],
    device const uint *constraint_roots [[buffer(8)]],
    device uint *dst [[buffer(9)]],
    constant uint &row_count [[buffer(10)]],
    constant uint &n_base_insts [[buffer(11)]],
    constant uint &n_ext_insts [[buffer(12)]],
    constant uint &n_constraints [[buffer(13)]],
    uint row_index [[thread_position_in_grid]]
) {
    if (row_index >= row_count) {
        return;
    }

    thread uint base_regs[STWO_METAL_EVAL_PROGRAM_V1_MAX_BASE_REGS_C];
    thread StwoMetalQm31 ext_regs[STWO_METAL_EVAL_PROGRAM_V1_MAX_EXT_REGS_C];

    for (uint reg = 0u; reg < STWO_METAL_EVAL_PROGRAM_V1_MAX_BASE_REGS_C; ++reg) {
        base_regs[reg] = 0u;
    }
    for (uint reg = 0u; reg < STWO_METAL_EVAL_PROGRAM_V1_MAX_EXT_REGS_C; ++reg) {
        ext_regs[reg] = StwoMetalQm31 { 0u, 0u, 0u, 0u };
    }

    for (uint inst_index = 0u; inst_index < n_base_insts; ++inst_index) {
        uint word_base = inst_index * STWO_METAL_EVAL_PROGRAM_V1_BASE_INST_WORDS;
        uint word0 = base_insts[word_base + 0u];
        uint op = word0 & 0xFFu;
        uint interaction = (word0 >> 8u) & 0xFFu;
        uint dst_reg = (word0 >> 16u) & 0xFFFFu;
        uint a = base_insts[word_base + 1u];
        uint b = base_insts[word_base + 2u];
        int imm = as_type<int>(base_insts[word_base + 3u]);

        uint value = 0u;
        switch (op) {
            case STWO_METAL_EVAL_PROGRAM_V1_BASE_TRACE_COL:
                value = stwo_metal_eval_program_trace_value(
                    trace_values,
                    interaction_offsets,
                    row_count,
                    interaction,
                    a,
                    row_index,
                    imm
                );
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_BASE_PREPROCESSED_COL:
                value = stwo_metal_eval_program_preprocessed_value(
                    preprocessed_values,
                    row_count,
                    a,
                    row_index
                );
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_BASE_PARAM:
                value = base_params[a];
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_BASE_CONST:
                value = a;
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_BASE_ADD:
                value = stwo_metal_m31_add(base_regs[a], base_regs[b]);
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_BASE_SUB:
                value = stwo_metal_m31_sub(base_regs[a], base_regs[b]);
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_BASE_MUL:
                value = stwo_metal_m31_mul(base_regs[a], base_regs[b]);
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_BASE_NEG:
                value = stwo_metal_m31_neg(base_regs[a]);
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_BASE_INV:
                value = stwo_metal_m31_inv(base_regs[a]);
                break;
            default:
                value = 0u;
                break;
        }

        base_regs[dst_reg] = value;
    }

    for (uint inst_index = 0u; inst_index < n_ext_insts; ++inst_index) {
        uint word_base = inst_index * STWO_METAL_EVAL_PROGRAM_V1_EXT_INST_WORDS;
        uint word0 = ext_insts[word_base + 0u];
        uint op = word0 & 0xFFu;
        uint dst_reg = (word0 >> 16u) & 0xFFFFu;
        uint a = ext_insts[word_base + 1u];
        uint b = ext_insts[word_base + 2u];
        uint c = ext_insts[word_base + 3u];
        uint d = ext_insts[word_base + 4u];

        StwoMetalQm31 value = StwoMetalQm31 { 0u, 0u, 0u, 0u };
        switch (op) {
            case STWO_METAL_EVAL_PROGRAM_V1_EXT_SECURE_COL:
                value = StwoMetalQm31 {
                    base_regs[a],
                    base_regs[b],
                    base_regs[c],
                    base_regs[d],
                };
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_EXT_PARAM:
                value = stwo_metal_load_qm31(ext_params, a);
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_EXT_CONST:
                value = StwoMetalQm31 { a, b, c, d };
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_EXT_ADD:
                value = stwo_metal_qm31_add(ext_regs[a], ext_regs[b]);
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_EXT_SUB:
                value = stwo_metal_qm31_sub(ext_regs[a], ext_regs[b]);
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_EXT_MUL:
                value = stwo_metal_qm31_mul(ext_regs[a], ext_regs[b]);
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_EXT_NEG:
                value = stwo_metal_qm31_sub(StwoMetalQm31 { 0u, 0u, 0u, 0u }, ext_regs[a]);
                break;
            default:
                value = StwoMetalQm31 { 0u, 0u, 0u, 0u };
                break;
        }

        ext_regs[dst_reg] = value;
    }

    StwoMetalQm31 acc = StwoMetalQm31 { 0u, 0u, 0u, 0u };
    for (uint constraint_index = 0u; constraint_index < n_constraints; ++constraint_index) {
        uint root = constraint_roots[constraint_index];
        acc = stwo_metal_qm31_add(
            acc,
            stwo_metal_qm31_mul(ext_regs[root], stwo_metal_load_qm31(random_coeff_powers, constraint_index))
        );
    }

    stwo_metal_store_qm31(dst, row_index, acc);
}

kernel void eval_program_v1_optimized_c_u32x4(
    device const uint *trace_values [[buffer(0)]],
    device const uint *interaction_offsets [[buffer(1)]],
    device const uint *preprocessed_values [[buffer(2)]],
    device const uint *base_params [[buffer(3)]],
    device const uint *ext_params [[buffer(4)]],
    device const uint *random_coeff_powers [[buffer(5)]],
    device const uint *base_insts [[buffer(6)]],
    device const uint *ext_insts [[buffer(7)]],
    device const uint *constraint_roots [[buffer(8)]],
    device uint *dst [[buffer(9)]],
    constant uint &row_count [[buffer(10)]],
    constant uint &n_base_insts [[buffer(11)]],
    constant uint &n_ext_insts [[buffer(12)]],
    constant uint &n_constraints [[buffer(13)]],
    constant uint &max_base_regs [[buffer(14)]],
    constant uint &max_ext_regs [[buffer(15)]],
    uint row_index [[thread_position_in_grid]],
    uint tid_in_group [[thread_position_in_threadgroup]],
    uint threads_in_group [[threads_per_threadgroup]]
) {
    if (row_index >= row_count) {
        return;
    }

    // --- Phase 0: Cooperative instruction fetch into threadgroup shared memory ---
    threadgroup uint shared_base_insts[STWO_METAL_EVAL_PROGRAM_V1_MAX_SHARED_BASE_INST_WORDS];
    threadgroup uint shared_ext_insts[STWO_METAL_EVAL_PROGRAM_V1_MAX_SHARED_EXT_INST_WORDS];

    uint base_inst_words = n_base_insts * STWO_METAL_EVAL_PROGRAM_V1_BASE_INST_WORDS;
    uint ext_inst_words  = n_ext_insts * STWO_METAL_EVAL_PROGRAM_V1_EXT_INST_WORDS;

    bool use_shared_base = base_inst_words <= STWO_METAL_EVAL_PROGRAM_V1_MAX_SHARED_BASE_INST_WORDS;
    bool use_shared_ext  = ext_inst_words <= STWO_METAL_EVAL_PROGRAM_V1_MAX_SHARED_EXT_INST_WORDS;

    if (use_shared_base) {
        for (uint i = tid_in_group; i < base_inst_words; i += threads_in_group) {
            shared_base_insts[i] = base_insts[i];
        }
    }
    if (use_shared_ext) {
        for (uint i = tid_in_group; i < ext_inst_words; i += threads_in_group) {
            shared_ext_insts[i] = ext_insts[i];
        }
    }
    threadgroup_barrier(mem_flags::mem_threadgroup);

    // --- Phase 1: Initialize register files (only used registers) ---
    thread uint base_regs[STWO_METAL_EVAL_PROGRAM_V1_MAX_BASE_REGS_C];
    thread StwoMetalQm31 ext_regs[STWO_METAL_EVAL_PROGRAM_V1_MAX_EXT_REGS_C];

    for (uint reg = 0u; reg < max_base_regs; ++reg) {
        base_regs[reg] = 0u;
    }
    for (uint reg = 0u; reg < max_ext_regs; ++reg) {
        ext_regs[reg] = StwoMetalQm31 { 0u, 0u, 0u, 0u };
    }

    // --- Phase 2: Execute base instructions ---
    for (uint inst_index = 0u; inst_index < n_base_insts; ++inst_index) {
        uint word_base = inst_index * STWO_METAL_EVAL_PROGRAM_V1_BASE_INST_WORDS;
        uint word0, a, b;
        int imm;
        if (use_shared_base) {
            word0 = shared_base_insts[word_base + 0u];
            a = shared_base_insts[word_base + 1u];
            b = shared_base_insts[word_base + 2u];
            imm = as_type<int>(shared_base_insts[word_base + 3u]);
        } else {
            word0 = base_insts[word_base + 0u];
            a = base_insts[word_base + 1u];
            b = base_insts[word_base + 2u];
            imm = as_type<int>(base_insts[word_base + 3u]);
        }
        uint op = word0 & 0xFFu;
        uint interaction = (word0 >> 8u) & 0xFFu;
        uint dst_reg = (word0 >> 16u) & 0xFFFFu;

        uint value = 0u;
        switch (op) {
            case STWO_METAL_EVAL_PROGRAM_V1_BASE_TRACE_COL:
                value = stwo_metal_eval_program_trace_value(
                    trace_values,
                    interaction_offsets,
                    row_count,
                    interaction,
                    a,
                    row_index,
                    imm
                );
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_BASE_PREPROCESSED_COL:
                value = stwo_metal_eval_program_preprocessed_value(
                    preprocessed_values,
                    row_count,
                    a,
                    row_index
                );
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_BASE_PARAM:
                value = base_params[a];
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_BASE_CONST:
                value = a;
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_BASE_ADD:
                value = stwo_metal_m31_add(base_regs[a], base_regs[b]);
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_BASE_SUB:
                value = stwo_metal_m31_sub(base_regs[a], base_regs[b]);
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_BASE_MUL:
                value = stwo_metal_m31_mul(base_regs[a], base_regs[b]);
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_BASE_NEG:
                value = stwo_metal_m31_neg(base_regs[a]);
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_BASE_INV:
                value = stwo_metal_m31_inv(base_regs[a]);
                break;
            default:
                value = 0u;
                break;
        }

        base_regs[dst_reg] = value;
    }

    // --- Phase 3: Execute ext instructions ---
    for (uint inst_index = 0u; inst_index < n_ext_insts; ++inst_index) {
        uint word_base = inst_index * STWO_METAL_EVAL_PROGRAM_V1_EXT_INST_WORDS;
        uint word0, a, b, c, d;
        if (use_shared_ext) {
            word0 = shared_ext_insts[word_base + 0u];
            a = shared_ext_insts[word_base + 1u];
            b = shared_ext_insts[word_base + 2u];
            c = shared_ext_insts[word_base + 3u];
            d = shared_ext_insts[word_base + 4u];
        } else {
            word0 = ext_insts[word_base + 0u];
            a = ext_insts[word_base + 1u];
            b = ext_insts[word_base + 2u];
            c = ext_insts[word_base + 3u];
            d = ext_insts[word_base + 4u];
        }
        uint op = word0 & 0xFFu;
        uint dst_reg = (word0 >> 16u) & 0xFFFFu;

        StwoMetalQm31 value = StwoMetalQm31 { 0u, 0u, 0u, 0u };
        switch (op) {
            case STWO_METAL_EVAL_PROGRAM_V1_EXT_SECURE_COL:
                value = StwoMetalQm31 {
                    base_regs[a],
                    base_regs[b],
                    base_regs[c],
                    base_regs[d],
                };
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_EXT_PARAM:
                value = stwo_metal_load_qm31(ext_params, a);
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_EXT_CONST:
                value = StwoMetalQm31 { a, b, c, d };
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_EXT_ADD:
                value = stwo_metal_qm31_add(ext_regs[a], ext_regs[b]);
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_EXT_SUB:
                value = stwo_metal_qm31_sub(ext_regs[a], ext_regs[b]);
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_EXT_MUL:
                value = stwo_metal_qm31_mul(ext_regs[a], ext_regs[b]);
                break;
            case STWO_METAL_EVAL_PROGRAM_V1_EXT_NEG:
                value = stwo_metal_qm31_sub(StwoMetalQm31 { 0u, 0u, 0u, 0u }, ext_regs[a]);
                break;
            default:
                value = StwoMetalQm31 { 0u, 0u, 0u, 0u };
                break;
        }

        ext_regs[dst_reg] = value;
    }

    // --- Phase 4: Accumulate constraint values ---
    StwoMetalQm31 acc = StwoMetalQm31 { 0u, 0u, 0u, 0u };
    for (uint constraint_index = 0u; constraint_index < n_constraints; ++constraint_index) {
        uint root = constraint_roots[constraint_index];
        acc = stwo_metal_qm31_add(
            acc,
            stwo_metal_qm31_mul(ext_regs[root], stwo_metal_load_qm31(random_coeff_powers, constraint_index))
        );
    }

    // --- Phase 5: Store result ---
    stwo_metal_store_qm31(dst, row_index, acc);
}

kernel void eval_program_v1_wide_fibonacci_u32x4(
    device const uint *trace_values [[buffer(0)]],
    device const uint *interaction_offsets [[buffer(1)]],
    device const uint *random_coeff_powers [[buffer(2)]],
    device uint *dst [[buffer(3)]],
    constant uint &row_count [[buffer(4)]],
    constant uint &n_constraints [[buffer(5)]],
    uint row_index [[thread_position_in_grid]]
) {
    if (row_index >= row_count) {
        return;
    }

    uint main_trace_base = interaction_offsets[1];
    StwoMetalQm31 acc = StwoMetalQm31 { 0u, 0u, 0u, 0u };
    for (uint constraint_index = 0u; constraint_index < n_constraints; ++constraint_index) {
        uint a = trace_values[(main_trace_base + constraint_index) * row_count + row_index];
        uint b = trace_values[(main_trace_base + constraint_index + 1u) * row_count + row_index];
        uint c = trace_values[(main_trace_base + constraint_index + 2u) * row_count + row_index];
        uint residue = stwo_metal_m31_sub(
            c,
            stwo_metal_m31_add(
                stwo_metal_m31_mul(a, a),
                stwo_metal_m31_mul(b, b)
            )
        );
        StwoMetalQm31 lifted = StwoMetalQm31 { residue, 0u, 0u, 0u };
        acc = stwo_metal_qm31_add(
            acc,
            stwo_metal_qm31_mul(lifted, stwo_metal_load_qm31(random_coeff_powers, constraint_index))
        );
    }

    stwo_metal_store_qm31(dst, row_index, acc);
}

kernel void sampled_values_v1_wide_fibonacci_u32x4(
    device const uint *tree_descs [[buffer(0)]],
    device const uint *column_descs [[buffer(1)]],
    device const uint *values [[buffer(2)]],
    device const uint *point_x [[buffer(3)]],
    device uint *dst [[buffer(4)]],
    constant uint &n_trees [[buffer(5)]],
    uint thread_index [[thread_position_in_grid]]
) {
    if (thread_index != 0 || n_trees == 0u) {
        return;
    }

    uint last_tree_base = (n_trees - 1u) * 2u;
    uint first_column = tree_descs[last_tree_base + 0u];

    uint left_value_index_0 = column_descs[(first_column + 0u) * 2u + 0u];
    uint left_value_index_1 = column_descs[(first_column + 1u) * 2u + 0u];
    uint left_value_index_2 = column_descs[(first_column + 2u) * 2u + 0u];
    uint left_value_index_3 = column_descs[(first_column + 3u) * 2u + 0u];
    uint right_value_index_0 = column_descs[(first_column + 4u) * 2u + 0u];
    uint right_value_index_1 = column_descs[(first_column + 5u) * 2u + 0u];
    uint right_value_index_2 = column_descs[(first_column + 6u) * 2u + 0u];
    uint right_value_index_3 = column_descs[(first_column + 7u) * 2u + 0u];

    StwoMetalQm31 basis_i = StwoMetalQm31 { 0u, 1u, 0u, 0u };
    StwoMetalQm31 basis_u = StwoMetalQm31 { 0u, 0u, 1u, 0u };
    StwoMetalQm31 basis_iu = StwoMetalQm31 { 0u, 0u, 0u, 1u };

    StwoMetalQm31 left_eval_0 = stwo_metal_load_qm31(values, left_value_index_0);
    StwoMetalQm31 left_eval_1 = stwo_metal_load_qm31(values, left_value_index_1);
    StwoMetalQm31 left_eval_2 = stwo_metal_load_qm31(values, left_value_index_2);
    StwoMetalQm31 left_eval_3 = stwo_metal_load_qm31(values, left_value_index_3);
    StwoMetalQm31 right_eval_0 = stwo_metal_load_qm31(values, right_value_index_0);
    StwoMetalQm31 right_eval_1 = stwo_metal_load_qm31(values, right_value_index_1);
    StwoMetalQm31 right_eval_2 = stwo_metal_load_qm31(values, right_value_index_2);
    StwoMetalQm31 right_eval_3 = stwo_metal_load_qm31(values, right_value_index_3);

    StwoMetalQm31 left = stwo_metal_qm31_add(
        stwo_metal_qm31_add(
            left_eval_0,
            stwo_metal_qm31_mul(left_eval_1, basis_i)
        ),
        stwo_metal_qm31_add(
            stwo_metal_qm31_mul(left_eval_2, basis_u),
            stwo_metal_qm31_mul(left_eval_3, basis_iu)
        )
    );
    StwoMetalQm31 right = stwo_metal_qm31_add(
        stwo_metal_qm31_add(
            right_eval_0,
            stwo_metal_qm31_mul(right_eval_1, basis_i)
        ),
        stwo_metal_qm31_add(
            stwo_metal_qm31_mul(right_eval_2, basis_u),
            stwo_metal_qm31_mul(right_eval_3, basis_iu)
        )
    );
    StwoMetalQm31 point_x_qm31 = stwo_metal_load_qm31(point_x, 0u);
    StwoMetalQm31 acc =
        stwo_metal_qm31_add(left, stwo_metal_qm31_mul(right, point_x_qm31));
    stwo_metal_store_qm31(dst, 0u, acc);
}
