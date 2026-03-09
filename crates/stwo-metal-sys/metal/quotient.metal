#include <metal_stdlib>

using namespace metal;

constant uint M31_P = 2147483647u;

struct Qm31 {
    uint a;
    uint b;
    uint c;
    uint d;
};

static inline uint m31_add(uint x, uint y) {
    uint sum = x + y;
    return sum >= M31_P ? sum - M31_P : sum;
}

static inline uint m31_sub(uint x, uint y) {
    return x >= y ? x - y : x + M31_P - y;
}

static inline uint m31_mul(uint x, uint y) {
    ulong product = (ulong)x * (ulong)y;
    ulong reduced = (((((product >> 31u) + product + 1u) >> 31u) + product) & (ulong)M31_P);
    return (uint)reduced;
}

static inline uint m31_square(uint x) {
    return m31_mul(x, x);
}

static inline Qm31 qm31_add(Qm31 lhs, Qm31 rhs) {
    return Qm31 {
        m31_add(lhs.a, rhs.a),
        m31_add(lhs.b, rhs.b),
        m31_add(lhs.c, rhs.c),
        m31_add(lhs.d, rhs.d),
    };
}

static inline Qm31 qm31_mul_base(Qm31 value, uint scalar) {
    return Qm31 {
        m31_mul(value.a, scalar),
        m31_mul(value.b, scalar),
        m31_mul(value.c, scalar),
        m31_mul(value.d, scalar),
    };
}

static inline Qm31 load_qm31(device const uint *values, uint index) {
    uint base = index * 4u;
    return Qm31 {
        values[base + 0u],
        values[base + 1u],
        values[base + 2u],
        values[base + 3u],
    };
}

static inline void store_qm31(device uint *values, uint index, Qm31 value) {
    uint base = index * 4u;
    values[base + 0u] = value.a;
    values[base + 1u] = value.b;
    values[base + 2u] = value.c;
    values[base + 3u] = value.d;
}

static inline uint trace_value(
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

    uint a = trace_value(trace_evaluations, eval_domain_size, 0u, row_index);
    uint b = trace_value(trace_evaluations, eval_domain_size, 1u, row_index);
    Qm31 row_res = Qm31 { 0u, 0u, 0u, 0u };

    for (uint constraint_index = 0u; constraint_index < n_constraints; ++constraint_index) {
        uint column_index = constraint_index + 2u;
        uint c = trace_value(trace_evaluations, eval_domain_size, column_index, row_index);
        uint recurrence = m31_add(m31_square(a), m31_square(b));
        uint constraint = m31_sub(c, recurrence);
        row_res = qm31_add(row_res, qm31_mul_base(load_qm31(random_coeff_powers, constraint_index), constraint));
        a = b;
        b = c;
    }

    uint denominator_index = row_index >> domain_log_size;
    uint denominator_inverse = denominator_inverses[denominator_index];
    store_qm31(dst, row_index, qm31_mul_base(row_res, denominator_inverse));
}
