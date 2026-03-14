#include "poly_support.h"

kernel void ifft_circle_part_u32(
    device uint *values [[buffer(0)]],
    device const uint *inverse_twiddles [[buffer(1)]],
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
    uint twiddle = stwo_metal_get_circle_twiddle(inverse_twiddles, index);

    values[idx0] = stwo_metal_m31_add(val0, val1);
    values[idx1] = stwo_metal_m31_mul(stwo_metal_m31_sub(val0, val1), twiddle);
}

kernel void ifft_line_part_u32(
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
    uint twiddle_index = index >> layer;
    uint l = index & (number_polynomials - 1u);
    uint idx0 = (twiddle_index << (layer + 1u)) + l;
    uint idx1 = idx0 + number_polynomials;

    uint val0 = values[idx0];
    uint val1 = values[idx1];
    uint twiddle = twiddles[layer_domain_offset + twiddle_index];

    values[idx0] = stwo_metal_m31_add(val0, val1);
    values[idx1] = stwo_metal_m31_mul(stwo_metal_m31_sub(val0, val1), twiddle);
}

kernel void ifft_line_stage_u32(
    device uint *values [[buffer(0)]],
    device const uint *twiddles [[buffer(1)]],
    constant uint &values_log_len [[buffer(2)]],
    constant uint &stage_domain_log_size [[buffer(3)]],
    uint index [[thread_position_in_grid]]
) {
    uint values_len = 1u << values_log_len;
    uint pair_count = values_len >> 1u;
    if (index >= pair_count) {
        return;
    }

    uint stage_domain_size = 1u << stage_domain_log_size;
    uint half_stage_size = stage_domain_size >> 1u;
    uint group = index / half_stage_size;
    uint inner = index % half_stage_size;
    uint base = group * stage_domain_size;
    uint idx0 = base + inner;
    uint idx1 = idx0 + half_stage_size;

    uint val0 = values[idx0];
    uint val1 = values[idx1];
    uint twiddle = twiddles[inner];

    values[idx0] = stwo_metal_m31_add(val0, val1);
    values[idx1] = stwo_metal_m31_mul(stwo_metal_m31_sub(val0, val1), twiddle);
}
