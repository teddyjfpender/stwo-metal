#pragma once

#include "fields_support.h"

struct StwoMetalQm31 {
    uint a;
    uint b;
    uint c;
    uint d;
};

static inline StwoMetalQm31 stwo_metal_qm31_add(StwoMetalQm31 lhs, StwoMetalQm31 rhs) {
    return StwoMetalQm31 {
        stwo_metal_m31_add(lhs.a, rhs.a),
        stwo_metal_m31_add(lhs.b, rhs.b),
        stwo_metal_m31_add(lhs.c, rhs.c),
        stwo_metal_m31_add(lhs.d, rhs.d),
    };
}

static inline StwoMetalQm31 stwo_metal_qm31_sub(StwoMetalQm31 lhs, StwoMetalQm31 rhs) {
    return StwoMetalQm31 {
        stwo_metal_m31_sub(lhs.a, rhs.a),
        stwo_metal_m31_sub(lhs.b, rhs.b),
        stwo_metal_m31_sub(lhs.c, rhs.c),
        stwo_metal_m31_sub(lhs.d, rhs.d),
    };
}

static inline StwoMetalQm31 stwo_metal_qm31_mul_base(StwoMetalQm31 value, uint scalar) {
    return StwoMetalQm31 {
        stwo_metal_m31_mul(value.a, scalar),
        stwo_metal_m31_mul(value.b, scalar),
        stwo_metal_m31_mul(value.c, scalar),
        stwo_metal_m31_mul(value.d, scalar),
    };
}

static inline StwoMetalQm31 stwo_metal_qm31_mul(StwoMetalQm31 lhs, StwoMetalQm31 rhs) {
    uint a0 = lhs.a;
    uint a1 = lhs.b;
    uint a2 = lhs.c;
    uint a3 = lhs.d;
    uint b0 = rhs.a;
    uint b1 = rhs.b;
    uint b2 = rhs.c;
    uint b3 = rhs.d;

    uint x0 = stwo_metal_m31_sub(stwo_metal_m31_mul(a0, b0), stwo_metal_m31_mul(a1, b1));
    uint x1 = stwo_metal_m31_add(stwo_metal_m31_mul(a0, b1), stwo_metal_m31_mul(a1, b0));
    uint y0 = stwo_metal_m31_sub(stwo_metal_m31_mul(a2, b2), stwo_metal_m31_mul(a3, b3));
    uint y1 = stwo_metal_m31_add(stwo_metal_m31_mul(a2, b3), stwo_metal_m31_mul(a3, b2));

    uint cross0 = stwo_metal_m31_sub(stwo_metal_m31_mul(a0, b2), stwo_metal_m31_mul(a1, b3));
    uint cross1 = stwo_metal_m31_add(stwo_metal_m31_mul(a0, b3), stwo_metal_m31_mul(a1, b2));
    uint cross2 = stwo_metal_m31_sub(stwo_metal_m31_mul(a2, b0), stwo_metal_m31_mul(a3, b1));
    uint cross3 = stwo_metal_m31_add(stwo_metal_m31_mul(a2, b1), stwo_metal_m31_mul(a3, b0));

    uint r_y0 = stwo_metal_m31_sub(stwo_metal_m31_mul(2u, y0), y1);
    uint r_y1 = stwo_metal_m31_add(y0, stwo_metal_m31_mul(2u, y1));

    return StwoMetalQm31 {
        stwo_metal_m31_add(x0, r_y0),
        stwo_metal_m31_add(x1, r_y1),
        stwo_metal_m31_add(cross0, cross2),
        stwo_metal_m31_add(cross1, cross3),
    };
}

static inline StwoMetalQm31 stwo_metal_load_qm31(device const uint *values, uint index) {
    uint base = index * 4u;
    return StwoMetalQm31 {
        values[base + 0u],
        values[base + 1u],
        values[base + 2u],
        values[base + 3u],
    };
}

static inline void stwo_metal_store_qm31(device uint *values, uint index, StwoMetalQm31 value) {
    uint base = index * 4u;
    values[base + 0u] = value.a;
    values[base + 1u] = value.b;
    values[base + 2u] = value.c;
    values[base + 3u] = value.d;
}

static inline StwoMetalQm31 stwo_metal_fri_fold_pair(
    StwoMetalQm31 f_p,
    StwoMetalQm31 f_neg_p,
    uint inverse_factor,
    StwoMetalQm31 alpha
) {
    StwoMetalQm31 f0 = stwo_metal_qm31_add(f_p, f_neg_p);
    StwoMetalQm31 f1 =
        stwo_metal_qm31_mul_base(stwo_metal_qm31_sub(f_p, f_neg_p), inverse_factor);
    return stwo_metal_qm31_add(f0, stwo_metal_qm31_mul(alpha, f1));
}
