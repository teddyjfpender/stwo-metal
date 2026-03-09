#include <metal_stdlib>

using namespace metal;

static uint bit_reverse_index(uint index, uint log_len) {
    uint reversed = 0;
    for (uint bit = 0; bit < log_len; ++bit) {
        reversed = (reversed << 1) | ((index >> bit) & 1u);
    }
    return reversed;
}

kernel void bit_reverse_u32(
    device uint *values [[buffer(0)]],
    constant uint &log_len [[buffer(1)]],
    uint index [[thread_position_in_grid]]
) {
    const uint len = 1u << log_len;
    if (index >= len) {
        return;
    }

    const uint reversed = bit_reverse_index(index, log_len);
    if (reversed > index) {
        const uint tmp = values[index];
        values[index] = values[reversed];
        values[reversed] = tmp;
    }
}
