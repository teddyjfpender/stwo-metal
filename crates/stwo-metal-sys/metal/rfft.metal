#include "poly_support.h"

kernel void rfft_circle_part_u32(
    device uint *values [[buffer(0)]],
    device const uint *twiddles [[buffer(1)]],
    constant uint &values_len [[buffer(2)]],
    uint index [[thread_position_in_grid]]
) {
    uint pair_count = values_len >> 1u;
    if (index >= pair_count) {
        return;
    }

    uint idx0 = index << 1u;
    uint idx1 = idx0 + 1u;
    uint val0 = values[idx0];
    uint val1 = values[idx1];
    uint twiddle = stwo_metal_get_circle_twiddle(twiddles, index);
    uint temp = stwo_metal_m31_mul(val1, twiddle);

    values[idx0] = stwo_metal_m31_add(val0, temp);
    values[idx1] = stwo_metal_m31_sub(val0, temp);
}

kernel void rfft_line_part_u32(
    device uint *values [[buffer(0)]],
    device const uint *twiddles [[buffer(1)]],
    constant uint &values_log_len [[buffer(2)]],
    constant uint &layer [[buffer(3)]],
    constant uint &layer_domain_offset [[buffer(4)]],
    uint index [[thread_position_in_grid]]
) {
    uint values_len = 1u << values_log_len;
    uint pair_count = values_len >> 1u;
    if (index >= pair_count) {
        return;
    }

    uint number_polynomials = 1u << layer;
    uint h = index / number_polynomials;
    uint l = index % number_polynomials;
    uint idx0 = (h << (layer + 1u)) + l;
    uint idx1 = idx0 + number_polynomials;

    uint val0 = values[idx0];
    uint val1 = values[idx1];
    uint twiddle = twiddles[layer_domain_offset + h];
    uint temp = stwo_metal_m31_mul(val1, twiddle);

    values[idx0] = stwo_metal_m31_add(val0, temp);
    values[idx1] = stwo_metal_m31_sub(val0, temp);
}
