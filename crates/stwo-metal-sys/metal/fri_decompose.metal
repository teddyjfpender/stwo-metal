#include "secure_field_support.h"

/// Compute signed partial terms for the FRI decomposition coefficient (lambda).
///
/// For the first half of the evaluation (i < half_domain_size), the term is
/// +eval[i]; for the second half it is -eval[i].  The resulting per-element
/// QM31 terms are written to `dst` and then reduced via the existing
/// `gkr_sum_reduce_pairs_u32x4` kernel to obtain (a_sum - b_sum).
///
/// Coordinate layout: four separate M31 columns.
kernel void fri_decompose_lambda_terms_coords_u32x4(
    device const uint *src_0 [[buffer(0)]],
    device const uint *src_1 [[buffer(1)]],
    device const uint *src_2 [[buffer(2)]],
    device const uint *src_3 [[buffer(3)]],
    device uint *dst [[buffer(4)]],
    constant uint &half_domain_size [[buffer(5)]],
    uint index [[thread_position_in_grid]]
) {
    StwoMetalQm31 value = {
        src_0[index],
        src_1[index],
        src_2[index],
        src_3[index],
    };

    // First half contributes positively; second half is negated.
    if (index >= half_domain_size) {
        value = stwo_metal_qm31_neg(value);
    }

    stwo_metal_store_qm31(dst, index, value);
}

/// Apply the decomposition correction: subtract lambda from the first half of
/// the evaluation and add it to the second half.  Reads and writes four
/// separate M31 coordinate columns.
kernel void fri_decompose_apply_lambda_coords_u32x4(
    device const uint *src_0 [[buffer(0)]],
    device const uint *src_1 [[buffer(1)]],
    device const uint *src_2 [[buffer(2)]],
    device const uint *src_3 [[buffer(3)]],
    device uint *dst_0 [[buffer(4)]],
    device uint *dst_1 [[buffer(5)]],
    device uint *dst_2 [[buffer(6)]],
    device uint *dst_3 [[buffer(7)]],
    constant StwoMetalQm31 &lambda [[buffer(8)]],
    constant uint &half_domain_size [[buffer(9)]],
    uint index [[thread_position_in_grid]]
) {
    StwoMetalQm31 value = {
        src_0[index],
        src_1[index],
        src_2[index],
        src_3[index],
    };

    StwoMetalQm31 corrected;
    if (index < half_domain_size) {
        corrected = stwo_metal_qm31_sub(value, lambda);
    } else {
        corrected = stwo_metal_qm31_add(value, lambda);
    }

    dst_0[index] = corrected.a;
    dst_1[index] = corrected.b;
    dst_2[index] = corrected.c;
    dst_3[index] = corrected.d;
}
