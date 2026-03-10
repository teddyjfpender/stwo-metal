#include "secure_field_support.h"

static inline uint stwo_metal_trace_value(
    device const uint *trace_evaluations,
    uint eval_domain_size,
    uint column_index,
    uint row_index
) {
    return trace_evaluations[column_index * eval_domain_size + row_index];
}

kernel void accumulate_wide_fibonacci_quotients_u32x4(
    device const uint *trace_evaluations [[buffer(0)]],
    device const uint *random_coeff_powers [[buffer(1)]],
    device const uint *denominator_inverses [[buffer(2)]],
    device uint *dst [[buffer(3)]],
    constant uint &eval_domain_size [[buffer(4)]],
    constant uint &n_constraints [[buffer(5)]],
    constant uint &domain_log_size [[buffer(6)]],
    uint row_index [[thread_position_in_grid]]
) {
    if (row_index >= eval_domain_size) {
        return;
    }

    uint a = stwo_metal_trace_value(trace_evaluations, eval_domain_size, 0u, row_index);
    uint b = stwo_metal_trace_value(trace_evaluations, eval_domain_size, 1u, row_index);
    StwoMetalQm31 row_res = StwoMetalQm31 { 0u, 0u, 0u, 0u };

    for (uint constraint_index = 0u; constraint_index < n_constraints; ++constraint_index) {
        uint column_index = constraint_index + 2u;
        uint c = stwo_metal_trace_value(trace_evaluations, eval_domain_size, column_index, row_index);
        uint recurrence = stwo_metal_m31_add(stwo_metal_m31_square(a), stwo_metal_m31_square(b));
        uint constraint = stwo_metal_m31_sub(c, recurrence);
        row_res = stwo_metal_qm31_add(
            row_res,
            stwo_metal_qm31_mul_base(
                stwo_metal_load_qm31(random_coeff_powers, constraint_index),
                constraint
            )
        );
        a = b;
        b = c;
    }

    uint denominator_index = row_index >> domain_log_size;
    uint denominator_inverse = denominator_inverses[denominator_index];
    stwo_metal_store_qm31(dst, row_index, stwo_metal_qm31_mul_base(row_res, denominator_inverse));
}
