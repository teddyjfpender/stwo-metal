#include "fields_support.h"

kernel void prefix_sum_circle_domain_order_to_coset_order_u32(
    device const uint *src [[buffer(0)]],
    device uint *dst [[buffer(1)]],
    constant uint &len [[buffer(2)]],
    uint index [[thread_position_in_grid]]
) {
    uint half_len = len >> 1u;
    if (index >= half_len) {
        return;
    }

    dst[index * 2u] = src[index];
    dst[index * 2u + 1u] = src[len - 1u - index];
}

kernel void prefix_sum_coset_order_to_circle_domain_order_u32(
    device const uint *src [[buffer(0)]],
    device uint *dst [[buffer(1)]],
    constant uint &len [[buffer(2)]],
    uint index [[thread_position_in_grid]]
) {
    if (index >= len) {
        return;
    }

    uint half_len = len >> 1u;
    if (index < half_len) {
        dst[index] = src[index << 1u];
    } else {
        uint i = index - half_len;
        dst[index] = src[len - 1u - (i << 1u)];
    }
}

kernel void prefix_sum_inclusive_step_u32(
    device const uint *src [[buffer(0)]],
    device uint *dst [[buffer(1)]],
    constant uint &len [[buffer(2)]],
    constant uint &stride [[buffer(3)]],
    uint index [[thread_position_in_grid]]
) {
    if (index >= len) {
        return;
    }

    uint value = src[index];
    if (index >= stride) {
        value = stwo_metal_m31_add(value, src[index - stride]);
    }
    dst[index] = value;
}
