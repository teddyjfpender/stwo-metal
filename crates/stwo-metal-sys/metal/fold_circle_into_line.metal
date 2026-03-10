#include "secure_field_support.h"

kernel void fri_fold_circle_into_line_first_layer_u32x4(
    device const uint *src [[buffer(0)]],
    device uint *dst [[buffer(1)]],
    device const uint *inverse_y [[buffer(2)]],
    constant StwoMetalQm31 &alpha [[buffer(3)]],
    uint index [[thread_position_in_grid]]
) {
    uint pair_index = index;
    StwoMetalQm31 f_p = stwo_metal_load_qm31(src, pair_index * 2u);
    StwoMetalQm31 f_neg_p = stwo_metal_load_qm31(src, pair_index * 2u + 1u);
    uint inverse_factor = inverse_y[pair_index];

    stwo_metal_store_qm31(
        dst,
        pair_index,
        stwo_metal_fri_fold_pair(f_p, f_neg_p, inverse_factor, alpha)
    );
}
