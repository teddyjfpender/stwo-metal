#include "secure_field_support.h"

constant uint STWO_METAL_EVAL_PROGRAM_V1_MAX_BASE_REGS = 1024u;
constant uint STWO_METAL_EVAL_PROGRAM_V1_MAX_EXT_REGS = 256u;

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

static inline uint stwo_metal_eval_program_trace_value(
    device const uint *trace_values,
    device const uint *interaction_offsets,
    uint row_count,
    uint interaction,
    uint column,
    uint row_index
) {
    uint global_column = interaction_offsets[interaction] + column;
    return trace_values[global_column * row_count + row_index];
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

        uint value = 0u;
        switch (op) {
            case STWO_METAL_EVAL_PROGRAM_V1_BASE_TRACE_COL:
                value = stwo_metal_eval_program_trace_value(
                    trace_values,
                    interaction_offsets,
                    row_count,
                    interaction,
                    a,
                    row_index
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
