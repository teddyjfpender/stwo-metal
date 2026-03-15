// witness_memory_addr_to_id.metal
//
// Metal compute kernel for generating the memory_address_to_id trace.
//
// The trace table has N_TRACE_COLUMNS = SPLIT * 2 columns, organized as
// (id0, mult0, id1, mult1, ..., id15, mult15) in column-major layout.
//
// The input is a contiguous array of address->id mappings (u32 values),
// split into SPLIT chunks of size `chunk_size` (= column_length), where
// chunk_size is a power-of-two >= ceil(n_ids / SPLIT).
//
// Input layout (all buffers are dense arrays of uint32_t):
//   buffer(0) — ids:          [n_ids] u32 raw memory IDs (addr offset by 1)
//   buffer(1) — mults:        [n_ids] u32 multiplicities
//   buffer(2) — trace_out:    column-major [n_trace_columns][column_length] u32 output
//   buffer(3) — n_ids:        scalar, total number of ID entries
//   buffer(4) — column_length: scalar, rows per column (power-of-two, per chunk)
//   buffer(5) — split:        scalar, number of chunks (MEMORY_ADDRESS_TO_ID_SPLIT)
//
// Thread mapping: one thread per (chunk_index, row_index), linearized as
//   thread_id = chunk_index * column_length + row_index
// Total grid size = split * column_length.

#include <metal_stdlib>
using namespace metal;

kernel void witness_memory_addr_to_id_trace(
    device const uint  *ids          [[buffer(0)]],
    device const uint  *mults        [[buffer(1)]],
    device       uint  *trace_out    [[buffer(2)]],
    constant     uint  &n_ids        [[buffer(3)]],
    constant     uint  &column_length [[buffer(4)]],
    constant     uint  &split        [[buffer(5)]],
    uint               tid           [[thread_position_in_grid]]
) {
    uint total_threads = split * column_length;
    if (tid >= total_threads) {
        return;
    }

    uint chunk_idx = tid / column_length;
    uint row = tid % column_length;

    // Global index into the flat ids/mults array.
    uint global_row = chunk_idx * column_length + row;

    // Each chunk has 2 columns: (id, multiplicity).
    // Column-major layout: trace_out[(chunk*2 + col) * column_length + row]
    uint id_col = chunk_idx * 2u;
    uint mult_col = chunk_idx * 2u + 1u;

    uint id_val = 0u;
    uint mult_val = 0u;

    if (global_row < n_ids) {
        id_val = ids[global_row];
        mult_val = mults[global_row];
    }

    trace_out[id_col * column_length + row] = id_val;
    trace_out[mult_col * column_length + row] = mult_val;
}
