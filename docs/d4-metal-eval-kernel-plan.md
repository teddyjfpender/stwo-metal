# D4: Metal Eval Kernel Implementation Plan

- Status: `draft`
- Date: `2026-03-12`
- Related:
  - [`dn-0008-metal-evaluation-program-v1.md`](./dn-0008-metal-evaluation-program-v1.md)
  - [`eval_program_v1.rs`](../crates/stwo-metal/src/backend/metal/eval_program_v1.rs)
  - [`eval_program_v1.metal`](../crates/stwo-metal-sys/metal/eval_program_v1.metal)
  - [`prove_runtime_v1.rs`](../crates/stwo-metal/src/backend/metal/prove_runtime_v1.rs)

## 1. Current State

The V1 evaluation program kernel **already exists** as
`eval_program_v1_reference_u32x4` in
`crates/stwo-metal-sys/metal/eval_program_v1.metal`. It is a fully functional
generic bytecode interpreter that:

- Runs one GPU thread per trace row (`thread_position_in_grid`)
- Allocates thread-local register files (1024 base, 256 ext)
- Executes all 9 base opcodes and all 7 ext opcodes
- Performs the random linear combination accumulation
- Writes one `StwoMetalQm31` per row to the output buffer

The host-side dispatch also exists in
`interpret_metal_evaluation_program_v1_on_metal` (Rust) and
`stwo_metal_eval_program_v1_reference_u32x4` (Objective-C runtime).

This plan therefore focuses on **optimizing** the existing kernel rather than
building it from scratch.

## 2. Existing Kernel Analysis

### 2.1 Kernel Function Signature (Current)

```metal
kernel void eval_program_v1_reference_u32x4(
    device const uint *trace_values          [[buffer(0)]],
    device const uint *interaction_offsets    [[buffer(1)]],
    device const uint *preprocessed_values   [[buffer(2)]],
    device const uint *base_params           [[buffer(3)]],
    device const uint *ext_params            [[buffer(4)]],
    device const uint *random_coeff_powers   [[buffer(5)]],
    device const uint *base_insts            [[buffer(6)]],
    device const uint *ext_insts             [[buffer(7)]],
    device const uint *constraint_roots      [[buffer(8)]],
    device uint *dst                         [[buffer(9)]],
    constant uint &row_count                 [[buffer(10)]],
    constant uint &n_base_insts              [[buffer(11)]],
    constant uint &n_ext_insts               [[buffer(12)]],
    constant uint &n_constraints             [[buffer(13)]],
    uint row_index [[thread_position_in_grid]]
);
```

### 2.2 Buffer Layout (Current)

| Buffer Index | Name | Layout | Size (uint32 elements) |
|---|---|---|---|
| 0 | trace_values | Column-major: `global_col * row_count + row_index` | total_columns * row_count |
| 1 | interaction_offsets | Cumulative column offset per interaction + sentinel | n_interactions + 1 |
| 2 | preprocessed_values | Column-major: `col * row_count + row_index` | n_preprocessed_cols * row_count |
| 3 | base_params | Flat array of M31 values | n_base_params |
| 4 | ext_params | Flat array of QM31 values (4 uint32 each) | n_ext_params * 4 |
| 5 | random_coeff_powers | Flat array of QM31 values (4 uint32 each) | n_constraints * 4 |
| 6 | base_insts | Packed instructions (4 uint32 per inst) | n_base_insts * 4 |
| 7 | ext_insts | Packed instructions (5 uint32 per inst) | n_ext_insts * 5 |
| 8 | constraint_roots | Array of ext register indices | n_constraints |
| 9 | dst | Output QM31 per row (4 uint32 each) | row_count * 4 |

### 2.3 Instruction Encoding

**Base instruction** (4 words):
```
word0: [op:8][interaction:8][dst:16]
word1: a (uint32)
word2: b (uint32)
word3: imm (int32, unused in current kernel)
```

**Ext instruction** (5 words):
```
word0: [op:8][reserved:8][dst:16]
word1: a (uint32)
word2: b (uint32)
word3: c (uint32)
word4: d (uint32)
```

### 2.4 Base Opcodes (9)

| Opcode | Value | Behavior |
|---|---|---|
| TraceCol | 0 | `trace_values[(interaction_offsets[interaction] + a) * row_count + row_index]` |
| PreprocessedCol | 1 | `preprocessed_values[a * row_count + row_index]` |
| Param | 2 | `base_params[a]` |
| Const | 3 | `a` (immediate) |
| Add | 4 | `m31_add(base_regs[a], base_regs[b])` |
| Sub | 5 | `m31_sub(base_regs[a], base_regs[b])` |
| Mul | 6 | `m31_mul(base_regs[a], base_regs[b])` |
| Neg | 7 | `m31_neg(base_regs[a])` |
| Inv | 8 | `m31_inv(base_regs[a])` |

### 2.5 Ext Opcodes (7)

| Opcode | Value | Behavior |
|---|---|---|
| SecureCol | 0 | `{base_regs[a], base_regs[b], base_regs[c], base_regs[d]}` |
| Param | 1 | `load_qm31(ext_params, a)` |
| Const | 2 | `{a, b, c, d}` (4 immediates) |
| Add | 3 | `qm31_add(ext_regs[a], ext_regs[b])` |
| Sub | 4 | `qm31_sub(ext_regs[a], ext_regs[b])` |
| Mul | 5 | `qm31_mul(ext_regs[a], ext_regs[b])` |
| Neg | 6 | `qm31_sub({0,0,0,0}, ext_regs[a])` |

### 2.6 M31 and QM31 Arithmetic (from fields_support.h and secure_field_support.h)

All field arithmetic helpers are defined in the existing headers:

- `fields_support.h`: `stwo_metal_m31_add`, `stwo_metal_m31_sub`,
  `stwo_metal_m31_neg`, `stwo_metal_m31_mul`, `stwo_metal_m31_inv`,
  `stwo_metal_m31_square`, `stwo_metal_m31_pow_to_power_of_two`
- `secure_field_support.h`: `StwoMetalQm31`, `stwo_metal_qm31_add`,
  `stwo_metal_qm31_sub`, `stwo_metal_qm31_mul`, `stwo_metal_qm31_mul_base`,
  `stwo_metal_load_qm31`, `stwo_metal_store_qm31`

The M31 prime is `2^31 - 1 = 0x7FFFFFFF`. The multiplication uses a
two-step Barrett-like reduction:
```metal
static inline uint stwo_metal_m31_mul(uint lhs, uint rhs) {
    ulong product = (ulong)lhs * (ulong)rhs;
    ulong reduced =
        (((((product >> 31u) + product + 1u) >> 31u) + product) & (ulong)STWO_METAL_M31_P);
    return (uint)reduced;
}
```

The inversion uses Fermat's little theorem with an addition chain for
`P-2 = 2^31 - 3`:
```
t0 = val^3,  t1 = val^15,  t2 = val^255 * val^15,
t3 = ..., t4 = ..., t5 = ..., result = t5^(2^7) * t2
```

QM31 multiplication uses the standard CM31 complex multiplication with the
relation `j^2 = 1 + 2i`:
```metal
x = a0*b0, y = a2*b2   (CM31 multiplications of "low" and "high" halves)
cross = a0*b2 + a2*b0   (CM31 cross terms)
r_y0 = 2*y0 - y1, r_y1 = y0 + 2*y1   (apply j^2 = 1+2i)
result = {x0+r_y0, x1+r_y1, cross0+cross2, cross1+cross3}
```

## 3. Thread Dispatch Strategy (Current)

The existing dispatch in `runtime.m` is:

```objc
MTLSize grid_size = MTLSizeMake(row_count, 1, 1);
MTLSize threadgroup_size = MTLSizeMake(
    stwo_metal_threads_per_group(pipeline), 1, 1
);
[encoder dispatchThreads:grid_size threadsPerThreadgroup:threadgroup_size];
```

Where `stwo_metal_threads_per_group` returns `min(256, max(threadExecutionWidth, maxTotalThreadsPerThreadgroup))`.

On Apple M-series GPUs:
- `threadExecutionWidth` is 32 (SIMD group width)
- `maxTotalThreadsPerThreadgroup` depends on register usage; for the reference
  kernel with 1024 base + 256 ext registers, this will be severely constrained

## 4. Register Budget Constraints

### 4.1 Current Budget

The device budget is hardcoded at:
```rust
const METAL_EVAL_PROGRAM_V1_DEVICE_BUDGET: MetalEvaluationProgramBudgetV1 =
    MetalEvaluationProgramBudgetV1::new(1024, 256);
```

The Metal kernel mirrors this:
```metal
constant uint STWO_METAL_EVAL_PROGRAM_V1_MAX_BASE_REGS = 1024u;
constant uint STWO_METAL_EVAL_PROGRAM_V1_MAX_EXT_REGS = 256u;
```

### 4.2 Memory Footprint Per Thread

- Base registers: 1024 * 4 bytes = 4096 bytes
- Ext registers: 256 * 16 bytes = 4096 bytes
- Total per-thread register file: **8192 bytes**

### 4.3 Apple GPU Register File

Apple M-series GPUs have ~32 KB of registers per execution unit. With 8 KB of
thread-local storage per thread, the GPU can only schedule approximately 4
threads per EU simultaneously, which severely limits occupancy and
latency-hiding.

For typical real workloads:
- `wide_fibonacci` with 100 columns uses `max_base_regs = 701`,
  `max_ext_regs = 99`
- `virtual_snos` uses fewer registers (roughly `n_columns * 4` base,
  `n_columns - 1` ext)

The kernel initializes ALL 1024 + 256 registers to zero regardless of how many
the program actually uses. This is the single largest performance issue.

## 5. Performance Problems and Optimization Plan

### 5.1 Problem: Full Register Initialization

The kernel zeroes all 1280 registers on every thread launch:
```metal
for (uint reg = 0u; reg < STWO_METAL_EVAL_PROGRAM_V1_MAX_BASE_REGS; ++reg) {
    base_regs[reg] = 0u;
}
for (uint reg = 0u; reg < STWO_METAL_EVAL_PROGRAM_V1_MAX_EXT_REGS; ++reg) {
    ext_regs[reg] = StwoMetalQm31 { 0u, 0u, 0u, 0u };
}
```

**Fix**: Pass `max_base_regs` and `max_ext_regs` as constant parameters and
only initialize the registers the program actually uses:

```metal
constant uint &max_base_regs [[buffer(14)]],
constant uint &max_ext_regs  [[buffer(15)]],
```

Then:
```metal
for (uint reg = 0u; reg < max_base_regs; ++reg) {
    base_regs[reg] = 0u;
}
for (uint reg = 0u; reg < max_ext_regs; ++reg) {
    ext_regs[reg] = StwoMetalQm31 { 0u, 0u, 0u, 0u };
}
```

This will also improve the Metal compiler's ability to reason about array
bounds and potentially optimize register allocation.

### 5.2 Problem: Column-Major Trace Access Pattern

Trace access is `trace_values[global_column * row_count + row_index]`. When
multiple threads in a SIMD group execute the same TraceCol instruction, they
access consecutive row indices in the same column, which IS coalesced. This
access pattern is already good.

However, across different instructions within the same thread, the kernel jumps
between columns separated by `row_count` elements. This is unavoidable in the
interpreter model.

**Assessment**: The column-major layout is correct for GPU coalesced access. No
change needed.

### 5.3 Problem: Instruction Fetch from Device Memory

All threads read the same instructions from device memory. Every thread in the
grid reads the exact same `base_insts` and `ext_insts` arrays, but through
device pointers.

**Fix**: Use a threadgroup-cooperative load to bring instructions into
threadgroup shared memory:

```metal
threadgroup uint shared_base_insts[MAX_SHARED_INSTS * 4];
threadgroup uint shared_ext_insts[MAX_SHARED_INSTS * 5];

// Cooperative load: each thread loads a few instructions
uint tid_in_group = thread_position_in_threadgroup;
uint threads_in_group = threads_per_threadgroup;
for (uint i = tid_in_group; i < n_base_insts * 4u; i += threads_in_group) {
    shared_base_insts[i] = base_insts[i];
}
for (uint i = tid_in_group; i < n_ext_insts * 5u; i += threads_in_group) {
    shared_ext_insts[i] = ext_insts[i];
}
threadgroup_barrier(mem_flags::mem_threadgroup);
```

This amortizes instruction fetch across the threadgroup and keeps them in fast
threadgroup memory for the duration of execution.

**Caveat**: This requires knowing the maximum instruction count at compile time
to size the shared arrays. A reasonable cap is 2048 base instructions and 512
ext instructions, covering most workloads. Programs exceeding this fall back to
the device-memory path.

### 5.4 Problem: Switch Statement per Instruction

The core instruction dispatch is a `switch` on the opcode. Modern GPU compilers
handle this reasonably well, but divergence across threads in a SIMD group can
cause serialization.

**Assessment**: Since all threads execute the same program (same instruction
sequence), there is NO divergence in the switch. All threads in a SIMD group
take the same branch at each instruction. This is a non-issue.

### 5.5 Problem: m31_inv is Expensive

The `stwo_metal_m31_inv` function requires 30 multiplications (via the addition
chain for `P-2`). If a program has many `Inv` instructions, this dominates.

**Assessment**: This is inherent to the field arithmetic. No optimization
possible without algorithmic changes. The wide_fibonacci workload does not use
Inv, so this is primarily a concern for virtual_snos and stwo-cairo workloads.

### 5.6 Proposed Optimized Kernel

```metal
kernel void eval_program_v1_optimized_u32x4(
    device const uint *trace_values          [[buffer(0)]],
    device const uint *interaction_offsets    [[buffer(1)]],
    device const uint *preprocessed_values   [[buffer(2)]],
    device const uint *base_params           [[buffer(3)]],
    device const uint *ext_params            [[buffer(4)]],
    device const uint *random_coeff_powers   [[buffer(5)]],
    device const uint *base_insts            [[buffer(6)]],
    device const uint *ext_insts             [[buffer(7)]],
    device const uint *constraint_roots      [[buffer(8)]],
    device uint *dst                         [[buffer(9)]],
    constant uint &row_count                 [[buffer(10)]],
    constant uint &n_base_insts              [[buffer(11)]],
    constant uint &n_ext_insts               [[buffer(12)]],
    constant uint &n_constraints             [[buffer(13)]],
    constant uint &max_base_regs             [[buffer(14)]],
    constant uint &max_ext_regs              [[buffer(15)]],
    uint row_index                           [[thread_position_in_grid]],
    uint tid_in_group                        [[thread_position_in_threadgroup]],
    uint threads_in_group                    [[threads_per_threadgroup]]
);
```

Key changes from reference kernel:
1. Accepts `max_base_regs` and `max_ext_regs` to minimize register init
2. Uses threadgroup shared memory for instruction fetch
3. Otherwise identical execution semantics

### 5.7 Detailed Optimized Kernel Pseudocode

```metal
#include "secure_field_support.h"

constant uint STWO_METAL_EVAL_PROGRAM_V1_MAX_BASE_REGS = 1024u;
constant uint STWO_METAL_EVAL_PROGRAM_V1_MAX_EXT_REGS = 256u;
constant uint STWO_METAL_EVAL_PROGRAM_V1_BASE_INST_WORDS = 4u;
constant uint STWO_METAL_EVAL_PROGRAM_V1_EXT_INST_WORDS = 5u;

// Shared memory limits for instruction caching
constant uint MAX_SHARED_BASE_INST_WORDS = 2048u * 4u;  // 2048 instructions
constant uint MAX_SHARED_EXT_INST_WORDS  = 512u * 5u;   // 512 instructions

// Opcode constants (same as existing)
constant uint BASE_TRACE_COL       = 0u;
constant uint BASE_PREPROCESSED    = 1u;
constant uint BASE_PARAM           = 2u;
constant uint BASE_CONST           = 3u;
constant uint BASE_ADD             = 4u;
constant uint BASE_SUB             = 5u;
constant uint BASE_MUL             = 6u;
constant uint BASE_NEG             = 7u;
constant uint BASE_INV             = 8u;

constant uint EXT_SECURE_COL = 0u;
constant uint EXT_PARAM      = 1u;
constant uint EXT_CONST      = 2u;
constant uint EXT_ADD         = 3u;
constant uint EXT_SUB         = 4u;
constant uint EXT_MUL         = 5u;
constant uint EXT_NEG         = 6u;

kernel void eval_program_v1_optimized_u32x4(
    device const uint *trace_values          [[buffer(0)]],
    device const uint *interaction_offsets    [[buffer(1)]],
    device const uint *preprocessed_values   [[buffer(2)]],
    device const uint *base_params           [[buffer(3)]],
    device const uint *ext_params            [[buffer(4)]],
    device const uint *random_coeff_powers   [[buffer(5)]],
    device const uint *base_insts            [[buffer(6)]],
    device const uint *ext_insts             [[buffer(7)]],
    device const uint *constraint_roots      [[buffer(8)]],
    device uint *dst                         [[buffer(9)]],
    constant uint &row_count                 [[buffer(10)]],
    constant uint &n_base_insts              [[buffer(11)]],
    constant uint &n_ext_insts               [[buffer(12)]],
    constant uint &n_constraints             [[buffer(13)]],
    constant uint &max_base_regs             [[buffer(14)]],
    constant uint &max_ext_regs              [[buffer(15)]],
    uint row_index       [[thread_position_in_grid]],
    uint tid_in_group    [[thread_position_in_threadgroup]],
    uint threads_in_group [[threads_per_threadgroup]]
) {
    if (row_index >= row_count) {
        return;
    }

    // --- Phase 0: Cooperative instruction fetch into threadgroup memory ---
    threadgroup uint shared_base_insts[MAX_SHARED_BASE_INST_WORDS];
    threadgroup uint shared_ext_insts[MAX_SHARED_EXT_INST_WORDS];

    uint base_inst_words = n_base_insts * BASE_INST_WORDS;
    uint ext_inst_words  = n_ext_insts * EXT_INST_WORDS;

    bool use_shared_base = base_inst_words <= MAX_SHARED_BASE_INST_WORDS;
    bool use_shared_ext  = ext_inst_words <= MAX_SHARED_EXT_INST_WORDS;

    if (use_shared_base) {
        for (uint i = tid_in_group; i < base_inst_words; i += threads_in_group) {
            shared_base_insts[i] = base_insts[i];
        }
    }
    if (use_shared_ext) {
        for (uint i = tid_in_group; i < ext_inst_words; i += threads_in_group) {
            shared_ext_insts[i] = ext_insts[i];
        }
    }
    threadgroup_barrier(mem_flags::mem_threadgroup);

    // Select instruction source
    // (threadgroup if fits, otherwise device memory)
    threadgroup const uint *bi = use_shared_base ? shared_base_insts : nullptr;
    threadgroup const uint *ei = use_shared_ext  ? shared_ext_insts  : nullptr;

    // --- Phase 1: Initialize register files (only used registers) ---
    thread uint base_regs[STWO_METAL_EVAL_PROGRAM_V1_MAX_BASE_REGS];
    thread StwoMetalQm31 ext_regs[STWO_METAL_EVAL_PROGRAM_V1_MAX_EXT_REGS];

    for (uint reg = 0u; reg < max_base_regs; ++reg) {
        base_regs[reg] = 0u;
    }
    for (uint reg = 0u; reg < max_ext_regs; ++reg) {
        ext_regs[reg] = StwoMetalQm31 { 0u, 0u, 0u, 0u };
    }

    // --- Phase 2: Execute base instructions ---
    for (uint inst_index = 0u; inst_index < n_base_insts; ++inst_index) {
        uint word_base_off = inst_index * BASE_INST_WORDS;
        uint word0, a_val, b_val;

        if (use_shared_base) {
            word0 = bi[word_base_off + 0u];
            a_val = bi[word_base_off + 1u];
            b_val = bi[word_base_off + 2u];
        } else {
            word0 = base_insts[word_base_off + 0u];
            a_val = base_insts[word_base_off + 1u];
            b_val = base_insts[word_base_off + 2u];
        }

        uint op       = word0 & 0xFFu;
        uint interact = (word0 >> 8u) & 0xFFu;
        uint dst_reg  = (word0 >> 16u) & 0xFFFFu;

        uint value = 0u;
        switch (op) {
            case BASE_TRACE_COL: {
                uint global_column = interaction_offsets[interact] + a_val;
                value = trace_values[global_column * row_count + row_index];
                break;
            }
            case BASE_PREPROCESSED:
                value = preprocessed_values[a_val * row_count + row_index];
                break;
            case BASE_PARAM:
                value = base_params[a_val];
                break;
            case BASE_CONST:
                value = a_val;
                break;
            case BASE_ADD:
                value = stwo_metal_m31_add(base_regs[a_val], base_regs[b_val]);
                break;
            case BASE_SUB:
                value = stwo_metal_m31_sub(base_regs[a_val], base_regs[b_val]);
                break;
            case BASE_MUL:
                value = stwo_metal_m31_mul(base_regs[a_val], base_regs[b_val]);
                break;
            case BASE_NEG:
                value = stwo_metal_m31_neg(base_regs[a_val]);
                break;
            case BASE_INV:
                value = stwo_metal_m31_inv(base_regs[a_val]);
                break;
            default:
                value = 0u;
                break;
        }
        base_regs[dst_reg] = value;
    }

    // --- Phase 3: Execute ext instructions ---
    for (uint inst_index = 0u; inst_index < n_ext_insts; ++inst_index) {
        uint word_base_off = inst_index * EXT_INST_WORDS;
        uint word0, a_val, b_val, c_val, d_val;

        if (use_shared_ext) {
            word0 = ei[word_base_off + 0u];
            a_val = ei[word_base_off + 1u];
            b_val = ei[word_base_off + 2u];
            c_val = ei[word_base_off + 3u];
            d_val = ei[word_base_off + 4u];
        } else {
            word0 = ext_insts[word_base_off + 0u];
            a_val = ext_insts[word_base_off + 1u];
            b_val = ext_insts[word_base_off + 2u];
            c_val = ext_insts[word_base_off + 3u];
            d_val = ext_insts[word_base_off + 4u];
        }

        uint op      = word0 & 0xFFu;
        uint dst_reg = (word0 >> 16u) & 0xFFFFu;

        StwoMetalQm31 value = StwoMetalQm31 { 0u, 0u, 0u, 0u };
        switch (op) {
            case EXT_SECURE_COL:
                value = StwoMetalQm31 {
                    base_regs[a_val],
                    base_regs[b_val],
                    base_regs[c_val],
                    base_regs[d_val],
                };
                break;
            case EXT_PARAM:
                value = stwo_metal_load_qm31(ext_params, a_val);
                break;
            case EXT_CONST:
                value = StwoMetalQm31 { a_val, b_val, c_val, d_val };
                break;
            case EXT_ADD:
                value = stwo_metal_qm31_add(ext_regs[a_val], ext_regs[b_val]);
                break;
            case EXT_SUB:
                value = stwo_metal_qm31_sub(ext_regs[a_val], ext_regs[b_val]);
                break;
            case EXT_MUL:
                value = stwo_metal_qm31_mul(ext_regs[a_val], ext_regs[b_val]);
                break;
            case EXT_NEG:
                value = stwo_metal_qm31_sub(
                    StwoMetalQm31 { 0u, 0u, 0u, 0u }, ext_regs[a_val]
                );
                break;
            default:
                value = StwoMetalQm31 { 0u, 0u, 0u, 0u };
                break;
        }
        ext_regs[dst_reg] = value;
    }

    // --- Phase 4: Accumulate constraint values ---
    StwoMetalQm31 acc = StwoMetalQm31 { 0u, 0u, 0u, 0u };
    for (uint ci = 0u; ci < n_constraints; ++ci) {
        uint root = constraint_roots[ci];
        acc = stwo_metal_qm31_add(
            acc,
            stwo_metal_qm31_mul(
                ext_regs[root],
                stwo_metal_load_qm31(random_coeff_powers, ci)
            )
        );
    }

    // --- Phase 5: Store result ---
    stwo_metal_store_qm31(dst, row_index, acc);
}
```

## 6. Host-Side Dispatch Code Design

### 6.1 Rust Dispatch Function

The existing `interpret_metal_evaluation_program_v1_on_metal` function at line
1605 of `eval_program_v1.rs` already handles:

1. Runtime availability check
2. Budget validation
3. Trace flattening via `flatten_trace_interactions`
4. Packing base/ext instructions via `pack_base_insts`/`pack_ext_insts`
5. Creating `U32Buffer` for each input
6. Calling the FFI function
7. Unpacking the output `QM31` values

For the optimized kernel, the changes needed are:

```rust
// In the Rust dispatch function, add max_base_regs/max_ext_regs params:
let dst = U32Buffer::eval_program_v1_optimized_u32x4(
    &trace_values,
    &interaction_offsets,
    &preprocessed_values,
    &base_params,
    &ext_params,
    &random_coeff_powers,
    &base_insts,
    &ext_insts,
    &constraint_roots,
    n_rows,
    runtime.trace.trace_interactions.len() as u32,
    runtime.trace.preprocessed_columns.len() as u32,
    runtime.base_params.len() as u32,
    runtime.ext_params.len() as u32,
    program.base_insts().len() as u32,
    program.ext_insts().len() as u32,
    program.constraint_roots().len() as u32,
    program.header().max_base_regs,   // NEW
    program.header().max_ext_regs,    // NEW
);
```

### 6.2 Objective-C Runtime Function

Add a new function in `runtime.m` following the exact same pattern as
`stwo_metal_eval_program_v1_reference_u32x4`, but:

1. Named `stwo_metal_eval_program_v1_optimized_u32x4`
2. Accepts two additional `uint32_t` parameters: `max_base_regs`,
   `max_ext_regs`
3. Sets them as constant buffers at indices 14 and 15
4. Points to the `eval_program_v1_optimized_u32x4` kernel function

```objc
[encoder setBytes:&max_base_regs length:sizeof(max_base_regs) atIndex:14];
[encoder setBytes:&max_ext_regs  length:sizeof(max_ext_regs)  atIndex:15];
```

### 6.3 FFI Binding

Add to `metal.rs`:
- An `extern "C"` declaration for `stwo_metal_eval_program_v1_optimized_u32x4`
- A safe wrapper in the `ffi` module
- A method on `U32Buffer` that calls the wrapper

### 6.4 Build System Integration

The `eval_program_v1.metal` file is **already** in `METAL_SOURCES` and
`rerun-if-changed` in `build.rs`. Since the new kernel goes in the same
`.metal` file, no build system changes are needed.

## 7. Integration with execute_evaluation_program_v1_on_trace_interactions

### 7.1 Current Flow

```
execute_evaluation_program_v1_on_trace_interactions
  -> execute_selected_metal_evaluation_program_v1_on_metal
    -> select_metal_evaluation_program_dispatch_v1
      -> checks for overlay match (wide_fibonacci etc.)
      -> falls back to GenericMetalInterpreter
    -> match dispatch:
      GenericMetalInterpreter -> interpret_metal_evaluation_program_v1_on_metal
                                  -> U32Buffer::eval_program_v1_reference_u32x4
      GeneratedOverlay(wide_fib) -> execute_wide_fibonacci_overlay_v1_on_metal
```

### 7.2 Integration Strategy

**Option A (Recommended): Replace the reference kernel**

Modify `interpret_metal_evaluation_program_v1_on_metal` to call the optimized
kernel instead of the reference kernel. The optimized kernel is a strict
superset of the reference kernel's capabilities (identical semantics, two
additional parameters). The reference kernel can be kept for correctness
testing.

```rust
// In interpret_metal_evaluation_program_v1_on_metal:
let dst = U32Buffer::eval_program_v1_optimized_u32x4(
    // ... same as before ...
    program.header().max_base_regs,
    program.header().max_ext_regs,
)?;
```

This requires no changes to the dispatch logic, overlay system, or the
`execute_evaluation_program_v1_on_trace_interactions` caller.

**Option B: Add as new dispatch kind**

Add `OptimizedMetalInterpreter` to `MetalEvaluationProgramDispatchKindV1`. This
is more complex and less useful since the optimized kernel should always be
preferred over the reference kernel.

### 7.3 CPU Fallback

The CPU interpreter `interpret_metal_evaluation_program_v1` remains unchanged
as the correctness oracle. It is used:
1. In tests for verification
2. In `prove_runtime_v1.rs` for sanity checks (when
   `skip_reference_sanity_check` is false)

## 8. Validation Strategy

### 8.1 Bit-Exact Comparison Against CPU Interpreter

The existing test infrastructure already validates GPU output against the CPU
interpreter. Specifically:

```rust
// In tests:
let interpreted = interpret_metal_evaluation_program_v1(&program, runtime)?;
let metal_result = interpret_metal_evaluation_program_v1_on_metal(&program, runtime)?;
assert_eq!(interpreted, metal_result);
```

This exact pattern should be replicated for the optimized kernel:

```rust
#[test]
fn optimized_kernel_matches_reference_interpreter_wide_fibonacci() {
    // Lower wide_fibonacci program
    // Run CPU interpreter
    // Run optimized GPU kernel
    // Assert bit-exact match for all rows
}

#[test]
fn optimized_kernel_matches_reference_interpreter_virtual_snos() {
    // Lower virtual_snos program
    // Run CPU interpreter
    // Run optimized GPU kernel
    // Assert bit-exact match for all rows
}
```

### 8.2 Cross-Kernel Comparison

Additionally, compare the optimized kernel against the reference kernel to
verify they produce identical results:

```rust
#[test]
fn optimized_kernel_matches_reference_kernel() {
    let ref_result = U32Buffer::eval_program_v1_reference_u32x4(...)?;
    let opt_result = U32Buffer::eval_program_v1_optimized_u32x4(...)?;
    assert_eq!(ref_result.to_vec()?, opt_result.to_vec()?);
}
```

### 8.3 Edge Cases to Test

- Programs with 0 base instructions, 0 ext instructions
- Programs with max register usage (1024 base, 256 ext)
- Programs with 1 row
- Programs that use Inv opcode
- Programs that use all ext opcodes (SecureCol, Param, Const, Add, Sub, Mul, Neg)
- Programs where instruction count exceeds shared memory capacity (>2048 base
  or >512 ext instructions), triggering the device-memory fallback path

### 8.4 Prove-Level Validation

The `prove_runtime_v1.rs` already has a sanity check path that compares
selected dispatch output against the reference interpreter. This provides
end-to-end validation through the full prove flow.

## 9. Performance Expectations

### 9.1 Expected Improvements

| Optimization | Expected Impact | Rationale |
|---|---|---|
| Bounded register init | 2-5x for small programs | Wide_fibonacci uses ~700 of 1024 base regs; virtual_snos uses far fewer. Avoiding the unused-register init loop removes wasted work. |
| Shared memory instructions | 10-30% for many-instruction programs | Reduces device memory bandwidth for instruction fetch. Effect is proportional to instruction count relative to trace data volume. |
| Combined | Measurable at all log sizes | Primarily affects per-thread startup cost; benefits smaller row counts more. |

### 9.2 Scaling Analysis

At `log16` (65536 rows): Each thread's per-row work dominates setup cost.
Register init improvement matters less.

At `log20+` (1M+ rows): The kernel is likely memory-bound on trace data access.
Register init is amortized over the program execution. The shared memory
optimization for instructions helps more because fewer cache lines are consumed
by repeated instruction fetch.

### 9.3 What Will NOT Improve

The high-log scaling degradation seen in benchmarks (log21-23 being slower than
SIMD) is likely caused by:

1. Total memory bandwidth pressure from trace data access (each row reads
   multiple columns from different memory regions)
2. The memory footprint of the trace itself exceeding GPU cache
3. The sequential nature of the interpreter loop (instruction-by-instruction
   execution within each thread)

These are fundamental to the interpreter approach and can only be addressed by:
- Generated overlay kernels (which eliminate the interpreter loop entirely)
- Trace data layout optimizations (row-major tiling for locality)
- Multi-pass execution (process fewer columns at a time to fit in cache)

## 10. Implementation Sequence

### Step 1: Add the optimized kernel to eval_program_v1.metal

Add `eval_program_v1_optimized_u32x4` below the existing reference kernel in
`crates/stwo-metal-sys/metal/eval_program_v1.metal`. The function accepts
`max_base_regs` and `max_ext_regs` as additional `constant uint &` parameters
at buffer indices 14 and 15.

No build system changes needed since the file is already compiled.

### Step 2: Add runtime.m dispatch function

Add `stwo_metal_eval_program_v1_optimized_u32x4` to
`crates/stwo-metal-sys/metal/runtime.m`, following the same pattern as the
reference function but with two additional `uint32_t` parameters.

### Step 3: Add FFI bindings in metal.rs

Add the `extern "C"` declaration, safe wrapper, and `U32Buffer` method in
`crates/stwo-metal-sys/src/metal.rs`.

### Step 4: Wire into interpret_metal_evaluation_program_v1_on_metal

In `crates/stwo-metal/src/backend/metal/eval_program_v1.rs`, modify the
`interpret_metal_evaluation_program_v1_on_metal` function to call the optimized
kernel, passing `program.header().max_base_regs` and
`program.header().max_ext_regs`.

### Step 5: Add tests

Add test functions that compare the optimized kernel output against:
- The CPU reference interpreter
- The existing reference GPU kernel

### Step 6: Benchmark

Run the existing `wide_fibonacci` benchmark sweep at `log16..23` and compare
against the baseline to measure the improvement.

## 11. Future Optimizations (Out of Scope for D4)

These are noted for completeness but are not part of this implementation:

1. **Function constants / specialization**: Use Metal function constants to
   compile the program's `n_base_insts`, `n_ext_insts`, and opcode sequence
   at pipeline creation time, eliminating the interpreter loop entirely for
   known programs. This is the most impactful possible optimization.

2. **Instruction stream compression**: Pack the opcode into fewer bits and use
   a more compact encoding to reduce instruction memory footprint.

3. **Row-major trace tiling**: Reorganize trace data so that a tile of rows x
   columns fits in cache, reducing cache thrashing for wide traces.

4. **Multiple rows per thread**: Process 2-4 rows per thread to amortize
   instruction decode cost and improve instruction cache utilization.

5. **Constraint accumulation fusion**: Fuse the constraint accumulation loop
   into the ext instruction execution to avoid a second pass over ext_regs.
