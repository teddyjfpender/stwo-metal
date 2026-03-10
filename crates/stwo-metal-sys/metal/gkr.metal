#include "secure_field_support.h"

kernel void gkr_gen_eq_evals_u32x4(
    device const uint *factors [[buffer(0)]],
    device uint *dst [[buffer(1)]],
    constant StwoMetalQm31 &v [[buffer(2)]],
    constant uint &y_size [[buffer(3)]],
    uint index [[thread_position_in_grid]]
) {
    uint eval_count = 1u << y_size;
    if (index >= eval_count) {
        return;
    }

    StwoMetalQm31 eval = v;
    uint shifted_index = index;
    for (int factor_pair = (int)y_size - 1; factor_pair >= 0; --factor_pair) {
        uint factor_index = (uint)factor_pair * 2u + (shifted_index & 1u);
        eval = stwo_metal_qm31_mul(eval, stwo_metal_load_qm31(factors, factor_index));
        shifted_index >>= 1u;
    }

    stwo_metal_store_qm31(dst, index, eval);
}

kernel void gkr_next_grand_product_layer_u32x4(
    device const uint *src [[buffer(0)]],
    device uint *dst [[buffer(1)]],
    constant uint &next_layer_size [[buffer(2)]],
    uint index [[thread_position_in_grid]]
) {
    if (index >= next_layer_size) {
        return;
    }

    StwoMetalQm31 lhs = stwo_metal_load_qm31(src, index * 2u);
    StwoMetalQm31 rhs = stwo_metal_load_qm31(src, index * 2u + 1u);
    stwo_metal_store_qm31(dst, index, stwo_metal_qm31_mul(lhs, rhs));
}

kernel void gkr_next_logup_generic_layer_u32x4(
    device const uint *numerators [[buffer(0)]],
    device const uint *denominators [[buffer(1)]],
    device uint *next_numerators [[buffer(2)]],
    device uint *next_denominators [[buffer(3)]],
    constant uint &next_layer_size [[buffer(4)]],
    uint index [[thread_position_in_grid]]
) {
    if (index >= next_layer_size) {
        return;
    }

    StwoMetalQm31 numerator0 = stwo_metal_load_qm31(numerators, index * 2u);
    StwoMetalQm31 numerator1 = stwo_metal_load_qm31(numerators, index * 2u + 1u);
    StwoMetalQm31 denominator0 = stwo_metal_load_qm31(denominators, index * 2u);
    StwoMetalQm31 denominator1 = stwo_metal_load_qm31(denominators, index * 2u + 1u);

    stwo_metal_store_qm31(
        next_numerators,
        index,
        stwo_metal_qm31_add(
            stwo_metal_qm31_mul(numerator0, denominator1),
            stwo_metal_qm31_mul(numerator1, denominator0)
        )
    );
    stwo_metal_store_qm31(
        next_denominators,
        index,
        stwo_metal_qm31_mul(denominator0, denominator1)
    );
}

kernel void gkr_next_logup_multiplicities_layer_u32(
    device const uint *numerators [[buffer(0)]],
    device const uint *denominators [[buffer(1)]],
    device uint *next_numerators [[buffer(2)]],
    device uint *next_denominators [[buffer(3)]],
    constant uint &next_layer_size [[buffer(4)]],
    uint index [[thread_position_in_grid]]
) {
    if (index >= next_layer_size) {
        return;
    }

    uint numerator0 = numerators[index * 2u];
    uint numerator1 = numerators[index * 2u + 1u];
    StwoMetalQm31 denominator0 = stwo_metal_load_qm31(denominators, index * 2u);
    StwoMetalQm31 denominator1 = stwo_metal_load_qm31(denominators, index * 2u + 1u);

    stwo_metal_store_qm31(
        next_numerators,
        index,
        stwo_metal_qm31_add(
            stwo_metal_qm31_mul_base(denominator1, numerator0),
            stwo_metal_qm31_mul_base(denominator0, numerator1)
        )
    );
    stwo_metal_store_qm31(
        next_denominators,
        index,
        stwo_metal_qm31_mul(denominator0, denominator1)
    );
}

kernel void gkr_next_logup_singles_layer_u32x4(
    device const uint *denominators [[buffer(0)]],
    device uint *next_numerators [[buffer(1)]],
    device uint *next_denominators [[buffer(2)]],
    constant uint &next_layer_size [[buffer(3)]],
    uint index [[thread_position_in_grid]]
) {
    if (index >= next_layer_size) {
        return;
    }

    StwoMetalQm31 denominator0 = stwo_metal_load_qm31(denominators, index * 2u);
    StwoMetalQm31 denominator1 = stwo_metal_load_qm31(denominators, index * 2u + 1u);

    stwo_metal_store_qm31(
        next_numerators,
        index,
        stwo_metal_qm31_add(denominator0, denominator1)
    );
    stwo_metal_store_qm31(
        next_denominators,
        index,
        stwo_metal_qm31_mul(denominator0, denominator1)
    );
}
