#include "secure_field_support.h"

/// Computes per-element products: result[i] = eval_values[i] * weights[i].
/// eval_values is an M31 buffer (one uint per element).
/// weights is a QM31 buffer (four uints per element, interleaved).
/// Output is a QM31 buffer.
kernel void barycentric_eval_terms_u32(
    device const uint *eval_values [[buffer(0)]],
    device const uint *weights [[buffer(1)]],
    device uint *terms [[buffer(2)]],
    constant uint &n_elements [[buffer(3)]],
    uint index [[thread_position_in_grid]]
) {
    if (index >= n_elements) {
        return;
    }

    uint eval_val = eval_values[index];
    StwoMetalQm31 weight = stwo_metal_load_qm31(weights, index);
    StwoMetalQm31 product = stwo_metal_qm31_mul_base(weight, eval_val);
    stwo_metal_store_qm31(terms, index, product);
}
