#include <metal_stdlib>
using namespace metal;

/// Bulk gather kernel for Merkle tree decommitment.
///
/// Reads hash words from a Merkle tree layer buffer at specified indices and
/// writes them contiguously to an output buffer.  Each hash is 8 x uint32
/// words.  One thread handles one hash gather operation.
///
/// Buffer layout:
///   buffer(0): layer_data — the source Merkle layer (flat array of uint32,
///              8 words per hash node).
///   buffer(1): gather_indices — array of uint32 node indices to read from
///              layer_data.  Each index `i` means: copy words
///              [i*8 .. i*8+7] from layer_data to the output.
///   buffer(2): output — destination buffer, 8 words per gathered hash,
///              written contiguously.
///   buffer(3): params — [num_gathers] (uint32).
///
/// Grid: dispatch `num_gathers` threads total.
kernel void merkle_decommit_gather(
    device const uint  *layer_data      [[buffer(0)]],
    device const uint  *gather_indices  [[buffer(1)]],
    device       uint  *output          [[buffer(2)]],
    constant     uint  *params          [[buffer(3)]],
    uint tid [[thread_position_in_grid]]
) {
    uint num_gathers = params[0];
    if (tid >= num_gathers) return;

    uint node_index = gather_indices[tid];
    uint src_base = node_index * 8u;
    uint dst_base = tid * 8u;

    // Copy 8 words (one Blake2s hash = 32 bytes).
    for (uint w = 0u; w < 8u; ++w) {
        output[dst_base + w] = layer_data[src_base + w];
    }
}

/// Multi-layer bulk gather: gathers hash words from multiple Merkle tree
/// layers in a single dispatch.  The caller packs all gather operations
/// across all layers into one flat array of (layer_selector, node_index)
/// pairs.
///
/// Buffer layout:
///   buffer(0..N-1): layer buffers (up to MAX_MERKLE_LAYERS layers).
///                   Unused slots should point to a valid buffer.
///   buffer(N):      gather_ops — packed array of (layer_id: uint16,
///                   node_index: uint16) encoded as a single uint32
///                   per gather op: low 16 bits = layer_id,
///                   high 16 bits = node_index.  For trees deeper
///                   than 16 bits, use the 32-bit variant below.
///   buffer(N+1):    output — 8 words per gathered hash.
///   buffer(N+2):    params — [num_gathers].
///
/// Note: Metal limits argument buffer count, so for practical use the
/// single-layer gather kernel is dispatched once per layer from a single
/// command buffer, avoiding the need for many buffer bindings.
