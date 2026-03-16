# Metal vs SIMD Proving Benchmark: fib(n) Scaling Analysis

**Date:** 2026-03-14
**Hardware:** Apple Silicon (M-series), Metal GPU
**Benchmark:** Cairo fib(n) end-to-end prove, Blake2s Merkle channel

## Results

| fib(n)   | Cycles     | Metal (ms) | Metal kHz | SIMD (ms) | SIMD kHz | Speedup |
|----------|------------|------------|-----------|-----------|----------|---------|
| 32,768   | 196,630    | 3,035.7    | 64.8      | 3,724.4   | 52.8     | 1.23x   |
| 65,536   | 393,238    | 3,467.9    | 113.4     | 3,756.2   | 104.7    | 1.08x   |
| 131,072  | 786,454    | 3,877.9    | 202.8     | 4,015.0   | 195.9    | 1.04x   |
| 262,144  | 1,572,886  | 5,483.4    | 286.8     | 9,597.4   | 163.9    | 1.75x   |
| 524,288  | 3,145,750  | 9,938.0    | 316.5     | 5,810.3   | 541.4    | 0.58x   |
| 1,048,576| 6,291,478  | 17,307.3   | 363.5     | 8,532.3   | 737.4    | 0.49x   |

> **Speedup** = SIMD_ms / Metal_ms. Values >1 mean Metal is faster.

### Throughput Scaling

```
Metal kHz:  64.8 → 113.4 → 202.8 → 286.8 → 316.5 → 363.5   (plateauing ~350 kHz)
SIMD  kHz:  52.8 → 104.7 → 195.9 → 163.9 → 541.4 → 737.4   (accelerating, ~700+ kHz)
```

Metal wins at small traces (32K-262K cycles, up to 1.75x) but loses at large
traces (524K+). SIMD overtakes Metal around 500K cycles and pulls further ahead
at 1M cycles (2x faster). The SIMD backend scales sub-linearly in time with
trace size, while Metal scales roughly linearly.

---

## Why Metal Is Slower at Large Traces

### The Core Problem: Most of the Pipeline Isn't on GPU

The Metal pipeline has GPU acceleration for specific kernels, but the majority
of wall-clock time at large traces is spent on CPU-bound phases that don't
benefit from Metal at all. Here is the phase breakdown at two scales:

#### fib(32,768) — Metal Total: 2,711 ms

| Phase                        |    ms | % of total | Accelerated? |
|------------------------------|------:|------------|--------------|
| Metal twiddles               |   298 | 11.0%      | CPU          |
| Preprocessed tree (GPU)      | 1,426 | 52.6%      | Hybrid       |
| Base trace gen + upload      |    94 | 3.5%       | CPU gen, GPU upload |
| Base trace commit (Merkle)   |    58 | 2.1%       | GPU Blake2s  |
| Interaction PoW              |    26 | 1.0%       | GPU grind    |
| Interaction trace gen+upload |   100 | 3.7%       | CPU gen, GPU upload |
| Interaction trace commit     |    55 | 2.0%       | GPU Blake2s  |
| Lowering                     |     1 | 0.0%       | CPU          |
| Composition (V1 GPU)         |   259 | 9.6%       | **GPU** (JIT kernels) |
| Composition commit           |    52 | 1.9%       | GPU Blake2s  |
| prove_values                 |   286 | 10.5%      | Mostly CPU   |
| **Total**                    | 2,711 | 100%       |              |

#### fib(1,048,576) — Metal Total: 16,961 ms

| Phase                        |    ms | % of total | Accelerated? |
|------------------------------|------:|------------|--------------|
| Metal twiddles               |   318 | 1.9%       | CPU          |
| Preprocessed tree (GPU)      | 1,354 | 8.0%       | Hybrid       |
| Base trace gen + upload      | 2,463 | 14.5%      | CPU gen, GPU upload |
| Base trace commit (Merkle)   | 1,917 | 11.3%      | GPU Blake2s  |
| Interaction PoW              |    11 | 0.1%       | GPU grind    |
| Interaction trace gen+upload | 1,857 | 10.9%      | CPU gen, GPU upload |
| Interaction trace commit     |   495 | 2.9%       | GPU Blake2s  |
| Lowering                     |     1 | 0.0%       | CPU          |
| Composition (V1 GPU)         | 4,451 | 26.2%      | **GPU** (JIT kernels) |
| Composition commit           |   103 | 0.6%       | GPU Blake2s  |
| prove_values                 | 3,976 | 23.4%      | Mostly CPU   |
| **Total**                    |16,961 | 100%       |              |

At fib(1M), only **~26% of pipeline time** (composition V1) is fully
GPU-accelerated. The remaining **~74%** is CPU-bound:

- **Base/interaction trace generation (25%)**: `create_cairo_claim_generator`
  runs entirely on CPU (stwo-cairo witness gen). Data is then uploaded to GPU
  memory for commitment.

- **prove_values (23%)**: FRI folding, tree decommit, and sampled values
  evaluation. FRI fold kernels exist on GPU but tree decommit and Merkle path
  opening run on CPU. The `commit_line_evaluation_via_cpu_bridge` function
  explicitly downloads GPU data to CPU for tree operations.

- **Preprocessed tree (8%)**: One-time cost (amortised if cached), includes
  RFFT (GPU) + Merkle build (GPU Blake2s), but the gen_trace step is CPU.

- **Trace commits (14%)**: Merkle tree hashing uses GPU Blake2s for large
  trees but the base/interaction trace polynomials are IFFT'd and committed
  through a hybrid path.

### The Composition Phase Breakdown

Even within the GPU-accelerated composition phase, significant time is spent on
CPU operations. For fib(1M):

```
Composition (4,451 ms total):
  extract columns:    1,708 ms  (38%) — CPU: RFFT to eval domain, GPU→CPU download
  gpu_kernel:         2,654 ms  (60%) — GPU: JIT constraint evaluation
  denom_inv:             61 ms  ( 1%) — CPU: vanishing polynomial inverse
  jit_compile:            1 ms  ( 0%) — CPU: shader compilation (cached)
```

The `extract` phase (38% of composition) evaluates committed polynomials on
each component's evaluation domain. While the RFFT itself runs on GPU, each
component's result is downloaded to CPU (`eval.values.to_cpu()`) and then
re-uploaded as flat buffers for the constraint kernel. This GPU→CPU→GPU
round-trip is pure overhead.

### Why SIMD Scales Better

The SIMD backend (`prove_cairo` on `SimdBackend`) is a single monolithic
`prove_ex` call that:

1. **Stays entirely in CPU memory** — no GPU↔CPU transfers
2. **Uses rayon parallelism** — all 10 P-cores + E-cores saturated
3. **Benefits from cache locality** — SIMD AVX-like operations on contiguous
   memory, no scattered GPU buffer reads
4. **Scales linearly** — doubling trace size ~doubles work, but rayon keeps
   all cores busy; no fixed GPU dispatch overhead

At small traces, Metal wins because the GPU can finish the constraint
evaluation kernel faster than all CPU cores combined. But as traces grow,
the CPU-bound phases dominate and Metal's advantage in constraint evaluation
is dwarfed by the overhead of data transfer and CPU-only phases.

---

## Current Metal Architecture

### What Is GPU-Accelerated

| Operation | GPU Kernel | Status |
|-----------|-----------|--------|
| Constraint evaluation (V1) | `eval_program_v1.metal` (JIT-compiled) | Full GPU, 31 components |
| Blake2s Merkle leaf hashing | `blake2s_build_leaves_lifted_fast` | GPU for >16 cols, >4K rows |
| Blake2s Merkle layer hashing | `blake2s_build_next_layer` | Full GPU |
| Fused leaf+layer Merkle | `stwo_metal_blake2s_build_merkle_tree_fast_u32` | Single command buffer |
| RFFT (evaluation) | `rfft_circle_part_u32`, `rfft_tail_fused_u32` | Full GPU |
| IFFT (interpolation) | `ifft_circle_part_u32`, `ifft_line_part_u32` | Full GPU |
| FRI circle→line fold | `fri_fold_circle_into_line_first_layer_u32x4` | Full GPU |
| FRI line fold | `fri_fold_line_step_u32x4` | Full GPU |
| Proof-of-work grind | `grind.metal` | Full GPU |
| Twiddle precomputation | `twiddles.metal` | Full GPU |
| Bit reversal | `bit_reverse.metal` | Full GPU |

### What Is NOT GPU-Accelerated (CPU-Only)

| Operation | Time at fib(1M) | Why it matters |
|-----------|----------------|----------------|
| Cairo witness generation | 2,463 ms (base) + 1,857 ms (interaction) | 25% of pipeline — `create_cairo_claim_generator` is stwo-cairo CPU code |
| Column extraction for composition | 1,708 ms | GPU→CPU→GPU round-trip per component, RFFT then download |
| prove_values / FRI decommit | 3,976 ms | 23% — tree decommit runs on CPU, line commitment uses CPU bridge |
| Vanishing polynomial inverse | 61 ms | CPU scalar loop |
| Twiddle precomputation | 318 ms | CPU (despite twiddles.metal existing) |
| Quotient accumulation (non-WF) | included in composition | CPU bridge for general quotient ops |
| Channel operations | trivial | Inherently sequential |

### Key Architectural Bottlenecks

1. **GPU→CPU→GPU data round-trips in composition**: Each component's trace
   columns are evaluated on GPU (RFFT), downloaded to CPU, then re-uploaded as
   flat `U32Buffer` arrays for the constraint kernel. At fib(1M) this costs
   1.7 seconds — more than the actual GPU kernel.

2. **CPU-only witness generation**: The entire Cairo trace (base + interaction)
   is generated on CPU by stwo-cairo. At fib(1M) this is 4.3 seconds. The GPU
   sits idle during this phase.

3. **CPU bridge for FRI decommit**: `commit_line_evaluation_via_cpu_bridge`
   downloads FRI fold results from GPU to CPU for Merkle tree operations.
   The tree decommit at query positions is CPU-only.

4. **No pipeline overlap**: Phases execute sequentially. GPU work in one phase
   doesn't overlap with CPU work in the next. The GPU is idle during witness
   gen, and the CPU is idle during GPU kernels.

---

## Path to 10x SIMD Performance

The CUDA implementation achieves up to 192x improvement over SIMD. To reach
even 10x on Metal, fundamental architectural changes are needed:

### Phase 1: Eliminate GPU↔CPU Round-Trips (est. 2-3x improvement)

**Problem**: Column extraction downloads RFFT results from GPU to CPU, then
re-uploads for the constraint kernel.

**Fix**: Keep polynomial evaluations on GPU. The `extract_column_on_domain_metal`
function already does GPU RFFT — stop downloading the result. Instead, pass
GPU buffer handles directly to the constraint kernel. This requires the V1
eval program to read from `MTLBuffer` column arrays instead of flat packed
buffers.

**Impact**: Eliminates 1.7s at fib(1M) (38% of composition phase).

### Phase 2: GPU Witness Generation (est. 2-3x improvement)

**Problem**: `create_cairo_claim_generator` runs entirely on CPU. At fib(1M),
base+interaction trace generation is 4.3 seconds (25% of pipeline).

**Fix**: Port Cairo trace generation to Metal compute kernels. The trace is a
deterministic function of the execution trace — each row can be computed
independently. This is embarrassingly parallel and ideal for GPU.

The CUDA backend already does this. The Metal backend has a proof-of-concept
for wide fibonacci witness (`generate_wide_fibonacci_trace_u32`) but not for
the general Cairo AIR.

**Impact**: Could reduce 4.3s → 200-500ms (matching CUDA patterns).

### Phase 3: GPU-Native FRI and Decommit (est. 2-3x improvement)

**Problem**: prove_values takes 4.0s at fib(1M). FRI fold kernels run on GPU
but tree decommit, query position handling, and Merkle path opening run on
CPU after a GPU→CPU download.

**Fix**:
- Keep FRI fold results on GPU
- Build Merkle commitment trees on GPU (Blake2s kernels already exist)
- Implement GPU-side tree decommit / query-response generation
- Only download final proof bytes, not intermediate polynomial evaluations

**Impact**: Could reduce prove_values from 4.0s → 500ms-1s.

### Phase 4: Full Pipeline Overlap (est. 1.5-2x improvement)

**Problem**: Phases execute sequentially. GPU idles during CPU work.

**Fix**: Pipeline the proving phases:
- While GPU commits base trace Merkle → CPU generates interaction trace
- While GPU runs composition kernel → CPU prepares FRI factors
- While GPU computes FRI folds → CPU handles channel operations

This is an architecture change from sequential to pipeline-parallel.

**Impact**: Hides CPU latency behind GPU compute.

### Phase 5: Optimise Remaining GPU Kernels (est. 1.5-2x improvement)

The constraint evaluation kernel (V1) can be further optimised:
- **Threadgroup memory**: Cache frequently-accessed trace columns in
  threadgroup shared memory instead of device memory
- **SIMD group operations**: Use `simd_shuffle` for cross-lane reductions
  instead of register-file operations
- **Fused quotient**: Merge constraint evaluation + vanishing polynomial
  inverse + accumulation into a single kernel, avoiding intermediate buffers
- **Register tiling**: Current V1 interpreter uses a flat register file;
  specialised kernels could use register blocking for better occupancy

### Compound Effect

| Phase | Current fib(1M) | After optimisation | Reduction |
|-------|-----------------|-------------------|-----------|
| Witness gen | 4,320 ms | 400 ms | 10.8x |
| Preprocessed tree | 1,354 ms | 800 ms | 1.7x |
| Trace commits | 2,412 ms | 400 ms | 6.0x |
| Composition | 4,451 ms | 500 ms | 8.9x |
| prove_values | 3,976 ms | 600 ms | 6.6x |
| Overhead (twiddles, PoW, etc) | 448 ms | 200 ms | 2.2x |
| **Total** | **16,961 ms** | **~2,900 ms** | **~5.8x** |

With pipeline overlap hiding 30-40% of remaining CPU cost:
**Target: ~1,700-2,000 ms → 3,100-3,700 kHz → 4.2-5.0x vs SIMD**

Reaching 10x SIMD (7,370 kHz) requires the full optimisation stack plus
additional kernel-level work:
- Fused witness→commit (no intermediate trace materialisation)
- GPU-side channel/Fiat-Shamir (eliminate all CPU round-trips)
- Multi-command-buffer pipelining across FRI layers
- Preprocessed tree elimination (precomputed commitments)

---

## What the CUDA Implementation Does Differently

The CUDA backend achieves 192x improvement because:

1. **End-to-end GPU residence**: Data stays on GPU from witness generation
   through proof output. No CPU round-trips for intermediate results.

2. **GPU witness generation**: Trace rows computed in parallel on GPU.

3. **GPU-native FRI**: All FRI layers, Merkle commitments, and decommitments
   run on GPU. Only the final proof bytes are downloaded.

4. **Kernel fusion**: Multiple prove phases fused into fewer kernel launches,
   reducing dispatch overhead and intermediate buffer allocation.

5. **Higher GPU utilisation**: NVIDIA GPUs have higher memory bandwidth and
   more compute units than Apple Silicon's shared-memory GPU. The M-series
   GPU shares memory bandwidth with CPU cores.

The Metal implementation can close the gap significantly but will likely not
match CUDA's absolute throughput due to Apple Silicon's unified memory
architecture (shared bandwidth) vs discrete GPU (dedicated VRAM bandwidth).
The realistic target for Metal is **5-10x SIMD** rather than 192x.

---

## Reproducing These Results

```bash
cd fixtures/standalone-benchmarks

# Generate test data (requires cairo-compile in PATH)
./generate_fib_inputs.sh

# Run full benchmark suite
./run_fib_bench.sh

# Run specific sizes
./run_fib_bench.sh --sizes "32768 131072 524288" --warmup 1 --iters 3

# Metal-only or SIMD-only
./run_fib_bench.sh --metal-only --sizes "32768 65536"
./run_fib_bench.sh --simd-only --sizes "32768 65536"

# Single run with timing breakdown
STWO_METAL_MODE=metal-prod cargo run --features metal-runtime,cairo-prove \
    --bin cairo_prove --release -- --metal test_data/cairo_fib_32768/prover_input.json

# Bench mode (JSON output, multiple iterations)
STWO_METAL_MODE=metal-prod cargo run --features metal-runtime,cairo-prove \
    --bin cairo_prove --release -- --bench 4 --metal --verify \
    test_data/cairo_fib_32768/prover_input.json
```
