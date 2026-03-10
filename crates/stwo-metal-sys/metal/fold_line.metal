#include "secure_field_support.h"

kernel void fri_fold_line_step_u32x4(
    device const uint *src [[buffer(0)]],
    device uint *dst [[buffer(1)]],
    device const uint *inverse_x [[buffer(2)]],
    constant StwoMetalQm31 &alpha [[buffer(3)]],
    uint index [[thread_position_in_grid]]
) {
    uint pair_index = index;
    StwoMetalQm31 f_x = stwo_metal_load_qm31(src, pair_index * 2u);
    StwoMetalQm31 f_neg_x = stwo_metal_load_qm31(src, pair_index * 2u + 1u);
    uint inverse_factor = inverse_x[pair_index];

    stwo_metal_store_qm31(
        dst,
        pair_index,
        stwo_metal_fri_fold_pair(f_x, f_neg_x, inverse_factor, alpha)
    );
}
