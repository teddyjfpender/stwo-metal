#include <metal_stdlib>

using namespace metal;

constant uint M31_P = 2147483647u;

static inline uint m31_add(uint x, uint y) {
    uint sum = x + y;
    return sum >= M31_P ? sum - M31_P : sum;
}

static inline uint m31_mul(uint x, uint y) {
    ulong product = (ulong)x * (ulong)y;
    ulong reduced = (((((product >> 31u) + product + 1u) >> 31u) + product) & (ulong)M31_P);
    return (uint)reduced;
}

static inline uint m31_square(uint x) {
    return m31_mul(x, x);
}

kernel void generate_wide_fibonacci_trace_u32(
    device const uint *input_a [[buffer(0)]],
    device const uint *input_b [[buffer(1)]],
    device uint *trace_values [[buffer(2)]],
    constant uint &input_len [[buffer(3)]],
    constant uint &n_columns [[buffer(4)]],
    uint row_index [[thread_position_in_grid]]
) {
    if (row_index >= input_len) {
        return;
    }

    trace_values[row_index] = input_a[row_index];
    if (n_columns == 1u) {
        return;
    }

    trace_values[input_len + row_index] = input_b[row_index];
    for (uint column = 2u; column < n_columns; ++column) {
        uint prev2 = trace_values[((column - 2u) * input_len) + row_index];
        uint prev1 = trace_values[((column - 1u) * input_len) + row_index];
        trace_values[(column * input_len) + row_index] =
            m31_add(m31_square(prev2), m31_square(prev1));
    }
}
