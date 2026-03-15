// witness_range_check.metal
//
// Generic Metal compute kernel for range-check witness trace generation.
//
// All range-check components (range_check_9_9, range_check_7_2_5, range_check_4_3,
// range_check_4_4, range_check_6, range_check_8, range_check_11, range_check_12,
// range_check_18, range_check_20, range_check_3_6_6_3, range_check_3_3_3_3_3,
// range_check_4_4_4_4) share the same trace pattern: each trace column is a
// direct copy of one multiplicity column.
//
// The kernel is generic over n_columns (number of multiplicity / trace columns).
//
// Input layout:
//   buffer(0) -- mults_in:      row-major [n_columns][column_length] u32 multiplicities
//                                (n_columns contiguous columns, each of column_length u32)
//   buffer(1) -- trace_out:     column-major [n_columns][column_length] u32 output trace
//   buffer(2) -- n_columns:     scalar, number of multiplicity columns (= N_TRACE_COLUMNS)
//   buffer(3) -- column_length: scalar, number of rows per column (power-of-two)
//
// Thread mapping: one thread per row index (0..column_length).
// Each thread copies all n_columns values for its row.

#include <metal_stdlib>
using namespace metal;

kernel void witness_range_check_trace(
    device const uint  *mults_in       [[buffer(0)]],
    device       uint  *trace_out      [[buffer(1)]],
    constant     uint  &n_columns      [[buffer(2)]],
    constant     uint  &column_length  [[buffer(3)]],
    uint               tid             [[thread_position_in_grid]]
) {
    if (tid >= column_length) {
        return;
    }

    // Copy each multiplicity column into the corresponding trace column.
    // Input:  mults_in[col * column_length + row]
    // Output: trace_out[col * column_length + row]
    // Since both are column-major with the same layout, this is a straight copy.
    for (uint col = 0; col < n_columns; col++) {
        uint offset = col * column_length + tid;
        trace_out[offset] = mults_in[offset];
    }
}
