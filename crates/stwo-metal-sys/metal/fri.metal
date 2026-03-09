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

static inline Qm31 qm31_add(Qm31 lhs, Qm31 rhs) {
    return Qm31 {
        m31_add(lhs.a, rhs.a),
        m31_add(lhs.b, rhs.b),
        m31_add(lhs.c, rhs.c),
        m31_add(lhs.d, rhs.d),
    };
}

static inline Qm31 qm31_sub(Qm31 lhs, Qm31 rhs) {
    return Qm31 {
        m31_sub(lhs.a, rhs.a),
        m31_sub(lhs.b, rhs.b),
        m31_sub(lhs.c, rhs.c),
        m31_sub(lhs.d, rhs.d),
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

static inline Qm31 qm31_mul(Qm31 lhs, Qm31 rhs) {
    uint a0 = lhs.a;
    uint a1 = lhs.b;
    uint a2 = lhs.c;
    uint a3 = lhs.d;
    uint b0 = rhs.a;
    uint b1 = rhs.b;
    uint b2 = rhs.c;
    uint b3 = rhs.d;

    uint x0 = m31_sub(m31_mul(a0, b0), m31_mul(a1, b1));
    uint x1 = m31_add(m31_mul(a0, b1), m31_mul(a1, b0));
    uint y0 = m31_sub(m31_mul(a2, b2), m31_mul(a3, b3));
    uint y1 = m31_add(m31_mul(a2, b3), m31_mul(a3, b2));

    uint cross0 = m31_sub(m31_mul(a0, b2), m31_mul(a1, b3));
    uint cross1 = m31_add(m31_mul(a0, b3), m31_mul(a1, b2));
    uint cross2 = m31_sub(m31_mul(a2, b0), m31_mul(a3, b1));
    uint cross3 = m31_add(m31_mul(a2, b1), m31_mul(a3, b0));

    uint r_y0 = m31_sub(m31_mul(2u, y0), y1);
    uint r_y1 = m31_add(y0, m31_mul(2u, y1));

    return Qm31 {
        m31_add(x0, r_y0),
        m31_add(x1, r_y1),
        m31_add(cross0, cross2),
        m31_add(cross1, cross3),
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

static inline Qm31 fri_fold_pair(Qm31 f_p, Qm31 f_neg_p, uint inverse_factor, Qm31 alpha) {
    Qm31 f0 = qm31_add(f_p, f_neg_p);
    Qm31 f1 = qm31_mul_base(qm31_sub(f_p, f_neg_p), inverse_factor);
    return qm31_add(f0, qm31_mul(alpha, f1));
}

kernel void fri_fold_circle_into_line_first_layer_u32x4(
    device const uint *src [[buffer(0)]],
    device uint *dst [[buffer(1)]],
    device const uint *inverse_y [[buffer(2)]],
    constant Qm31 &alpha [[buffer(3)]],
    uint index [[thread_position_in_grid]]
) {
    const uint pair_index = index;

    Qm31 f_p = load_qm31(src, pair_index * 2u);
    Qm31 f_neg_p = load_qm31(src, pair_index * 2u + 1u);
    uint itwid = inverse_y[pair_index];

    store_qm31(dst, pair_index, fri_fold_pair(f_p, f_neg_p, itwid, alpha));
}

kernel void fri_fold_line_step_u32x4(
    device const uint *src [[buffer(0)]],
    device uint *dst [[buffer(1)]],
    device const uint *inverse_x [[buffer(2)]],
    constant Qm31 &alpha [[buffer(3)]],
    uint index [[thread_position_in_grid]]
) {
    const uint pair_index = index;

    Qm31 f_x = load_qm31(src, pair_index * 2u);
    Qm31 f_neg_x = load_qm31(src, pair_index * 2u + 1u);
    uint inverse_factor = inverse_x[pair_index];

    store_qm31(dst, pair_index, fri_fold_pair(f_x, f_neg_x, inverse_factor, alpha));
}
