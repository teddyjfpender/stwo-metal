#include <metal_stdlib>
using namespace metal;

constant uint STWO_METAL_BLAKE2S_IV[8] = {
    0x6A09E667u, 0xBB67AE85u, 0x3C6EF372u, 0xA54FF53Au,
    0x510E527Fu, 0x9B05688Cu, 0x1F83D9ABu, 0x5BE0CD19u
};

constant uchar STWO_METAL_BLAKE2S_SIGMA[10][16] = {
    {  0,  1,  2,  3,  4,  5,  6,  7,  8,  9, 10, 11, 12, 13, 14, 15 },
    { 14, 10,  4,  8,  9, 15, 13,  6,  1, 12,  0,  2, 11,  7,  5,  3 },
    { 11,  8, 12,  0,  5,  2, 15, 13, 10, 14,  3,  6,  7,  1,  9,  4 },
    {  7,  9,  3,  1, 13, 12, 11, 14,  2,  6,  5, 10,  4,  0, 15,  8 },
    {  9,  0,  5,  7,  2,  4, 10, 15, 14,  1, 11, 12,  6,  8,  3, 13 },
    {  2, 12,  6, 10,  0, 11,  8,  3,  4, 13,  7,  5, 15, 14,  1,  9 },
    { 12,  5,  1, 15, 14, 13,  4, 10,  0,  7,  6,  3,  9,  2,  8, 11 },
    { 13, 11,  7, 14, 12,  1,  3,  9,  5,  0, 15,  4,  8,  6,  2, 10 },
    {  6, 15, 14,  9, 11,  3,  0,  8, 12,  2, 13,  7,  1,  4, 10,  5 },
    { 10,  2,  8,  4,  7,  6,  1,  5, 15, 11,  9, 14,  3, 12, 13,  0 }
};

struct StwoMetalBlake2sState {
    uint h[8];
    uint t;
    uchar buf[64];
    uint buflen;
};

static inline uint stwo_metal_rotr32(uint value, uint shift) {
    return (value >> shift) | (value << (32u - shift));
}

static inline void stwo_metal_blake2s_g(
    thread uint *v,
    thread const uint *m,
    uint round,
    uint index,
    thread uint &a,
    thread uint &b,
    thread uint &c,
    thread uint &d
) {
    a = a + b + m[STWO_METAL_BLAKE2S_SIGMA[round][2u * index + 0u]];
    d = stwo_metal_rotr32(d ^ a, 16u);
    c = c + d;
    b = stwo_metal_rotr32(b ^ c, 12u);
    a = a + b + m[STWO_METAL_BLAKE2S_SIGMA[round][2u * index + 1u]];
    d = stwo_metal_rotr32(d ^ a, 8u);
    c = c + d;
    b = stwo_metal_rotr32(b ^ c, 7u);
    (void)v;
}

static inline void stwo_metal_blake2s_compress(
    thread StwoMetalBlake2sState &state,
    thread const uchar *block,
    uint total_bytes,
    uint last_block
) {
    uint m[16];
    for (uint i = 0; i < 16u; ++i) {
        uint base = 4u * i;
        m[i] =
            ((uint)block[base + 0u]) |
            (((uint)block[base + 1u]) << 8u) |
            (((uint)block[base + 2u]) << 16u) |
            (((uint)block[base + 3u]) << 24u);
    }

    uint v[16];
    for (uint i = 0; i < 8u; ++i) {
        v[i] = state.h[i];
        v[i + 8u] = STWO_METAL_BLAKE2S_IV[i];
    }
    v[12] ^= total_bytes;
    v[14] ^= last_block;

    for (uint round = 0; round < 10u; ++round) {
        stwo_metal_blake2s_g(v, m, round, 0u, v[0], v[4], v[8], v[12]);
        stwo_metal_blake2s_g(v, m, round, 1u, v[1], v[5], v[9], v[13]);
        stwo_metal_blake2s_g(v, m, round, 2u, v[2], v[6], v[10], v[14]);
        stwo_metal_blake2s_g(v, m, round, 3u, v[3], v[7], v[11], v[15]);
        stwo_metal_blake2s_g(v, m, round, 4u, v[0], v[5], v[10], v[15]);
        stwo_metal_blake2s_g(v, m, round, 5u, v[1], v[6], v[11], v[12]);
        stwo_metal_blake2s_g(v, m, round, 6u, v[2], v[7], v[8], v[13]);
        stwo_metal_blake2s_g(v, m, round, 7u, v[3], v[4], v[9], v[14]);
    }

    for (uint i = 0; i < 8u; ++i) {
        state.h[i] ^= v[i] ^ v[i + 8u];
    }
}

static inline void stwo_metal_blake2s_init(thread StwoMetalBlake2sState &state) {
    state.h[0] = STWO_METAL_BLAKE2S_IV[0] ^ 0x01010020u;
    state.h[1] = STWO_METAL_BLAKE2S_IV[1];
    state.h[2] = STWO_METAL_BLAKE2S_IV[2];
    state.h[3] = STWO_METAL_BLAKE2S_IV[3];
    state.h[4] = STWO_METAL_BLAKE2S_IV[4];
    state.h[5] = STWO_METAL_BLAKE2S_IV[5];
    state.h[6] = STWO_METAL_BLAKE2S_IV[6];
    state.h[7] = STWO_METAL_BLAKE2S_IV[7];
    state.t = 0u;
    state.buflen = 0u;
}

static inline void stwo_metal_blake2s_update(
    thread StwoMetalBlake2sState &state,
    thread const uchar *input,
    uint input_len
) {
    uint left = state.buflen;
    uint fill = 64u - left;
    uint offset = 0u;

    if (input_len > fill) {
        for (uint i = 0; i < fill; ++i) {
            state.buf[left + i] = input[i];
        }
        state.t += 64u;
        stwo_metal_blake2s_compress(state, state.buf, state.t, 0u);
        offset += fill;
        input_len -= fill;
        while (input_len > 64u) {
            state.t += 64u;
            stwo_metal_blake2s_compress(state, input + offset, state.t, 0u);
            offset += 64u;
            input_len -= 64u;
        }
        left = 0u;
    }

    for (uint i = 0; i < input_len; ++i) {
        state.buf[left + i] = input[offset + i];
    }
    state.buflen = left + input_len;
}

static inline void stwo_metal_blake2s_finalize(
    thread StwoMetalBlake2sState &state,
    device uint *dst_words,
    uint dst_index
) {
    state.t += state.buflen;
    for (uint i = state.buflen; i < 64u; ++i) {
        state.buf[i] = 0u;
    }
    stwo_metal_blake2s_compress(state, state.buf, state.t, 0xFFFFFFFFu);
    for (uint i = 0; i < 8u; ++i) {
        dst_words[dst_index * 8u + i] = state.h[i];
    }
}

static inline uint stwo_metal_lifted_column_index(uint lifted_index, uint log_ratio) {
    if (log_ratio == 0u) {
        return lifted_index;
    }
    return ((lifted_index >> (log_ratio + 1u)) << 1u) + (lifted_index & 1u);
}

kernel void blake2s_build_leaves_lifted_u32(
    device const uint *flat_columns [[buffer(0)]],
    device const uint *column_offsets [[buffer(1)]],
    device const uint *column_log_sizes [[buffer(2)]],
    device uint *dst [[buffer(3)]],
    constant uint &n_columns [[buffer(4)]],
    constant uint &lifting_log_size [[buffer(5)]],
    uint row_index [[thread_position_in_grid]]
) {
    uint row_count = 1u << lifting_log_size;
    if (row_index >= row_count) {
        return;
    }

    StwoMetalBlake2sState state;
    stwo_metal_blake2s_init(state);
    uchar bytes[4];

    for (uint column_index = 0u; column_index < n_columns; ++column_index) {
        uint column_log_size = column_log_sizes[column_index];
        uint source_index =
            stwo_metal_lifted_column_index(row_index, lifting_log_size - column_log_size);
        uint value = flat_columns[column_offsets[column_index] + source_index];
        bytes[0] = (uchar)(value & 0xFFu);
        bytes[1] = (uchar)((value >> 8u) & 0xFFu);
        bytes[2] = (uchar)((value >> 16u) & 0xFFu);
        bytes[3] = (uchar)((value >> 24u) & 0xFFu);
        stwo_metal_blake2s_update(state, bytes, 4u);
    }

    stwo_metal_blake2s_finalize(state, dst, row_index);
}
