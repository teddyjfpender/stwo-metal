# stwo-metal Phase 2 Engineering Handover Notes

**Date:** 2026-03-22
**Starting Performance:** 1.44 MHz at fib(524K) (from Phase 1 team)
**Final Performance:** 1.69 MHz at fib(524K), 1838ms best warm run
**Improvement:** +17.4% throughput, all proofs cryptographically verified

---

## 1. What Was Done (9 commits this session)

### Successful Optimizations

| Commit | Change | Impact |
|--------|--------|--------|
| `5b78141` | Async FRI fold steps (submit all, wait on last) | **prove_values: 405ms → 220ms (-46%)** |
| `5b78141` | Non-blocking GPU blit copies (remove waitUntilCompleted) | **-30ms** eliminating 600+ sync points |
| `5b78141` | Drop RFFT wait handles in evaluate_polynomials | **Merkle commits: 380ms → 267ms (-30%)** |
| `5b78141` | Skip coefficient promotion to private storage | **-20ms** eliminating 600+ blit CBs |
| `5b78141` | Lower Merkle GPU threshold (4096 → 4) | **-19ms** keeping small layers on GPU |
| `0ad6c5b` | Shared GPU buffers for extend_evals column uploads | **-100ms** (interaction trace extend_evals: 154ms → 99ms) |
| `e6029fc` | GPU range_check multiplicity bypass | Infrastructure for GPU-native traces |
| `739c574` | Pipelined claim gen (overlap with previous GPU phases) | **-20ms** from CPU/GPU overlap |
| `0278582` | Arc\<BaseColumnPool\> in vendored stwo | Enables cross-thread proof state |

### Attempted but Reverted/Abandoned

| Approach | Result | Root Cause |
|----------|--------|------------|
| GPU prefix sum for interaction trace | **Reverted** | Sequential GPU scan slower than CPU SIMD for large arrays (~7ms per column GPU vs ~2ms CPU SIMD) |
| Batched column staging (single Metal buffer + clone_range) | **Reverted** | clone_range creates new buffer objects anyway — no allocation savings over per-column from_packed_m31_slice |
| Poseidon-M31 Merkle hash | **Abandoned** | Analysis shows Poseidon permutation is ~3x slower than Blake2s per hash. Same 32-byte output size means no tree size advantage. Blake2s hashing is only ~38ms of the 138ms "Merkle commit" — rest is RFFT |
| Multi-proof pipelining (2 proofs in flight) | **Reverted** | **58% regression** — GPU/CPU/memory contention on unified Apple Silicon. Two proofs competing for same GPU queue + rayon thread pool + cache = worse than sequential |
| Batched IFFT (single CB for all columns) | **Reverted** | **292ms regression** — one massive CB with ~4400 encoders has high Metal processing overhead + lost rayon parallelism for CPU preparation |
| Composition GPU-resident (skip SimdBackend conversion) | **Blocked** | SimdBackend stores coefficients in transposed format for log_size > 16. Cannot directly copy to MetalBackend — must evaluate on domain first |

---

## 2. Performance Profile (Best Warm Run, 1838ms)

```
Base trace gen + upload:    732ms  (40%)  was 814ms
  ├── GPU opcode dispatch:  ~300ms (GPU, 6 Metal kernels, hardware-bound)
  ├── CPU write_trace:      ~200ms (rayon SIMD, range_checks/builtins/verify_instruction)
  ├── extend_evals upload:  ~100ms (shared buffer memcpy, was ~200ms before)
  └── overhead:             ~132ms (rayon scheduling, DashMap, GPU mults join)
Base Merkle commit:         138ms  (7%)   was 220ms
Interaction PoW:             13ms  (1%)
Interaction trace gen:      257ms  (14%)  was 300ms
  ├── GPU fused logup:       ~90ms (6 opcodes + memory, overlapped)
  ├── CPU logup:             ~67ms (range_checks, builtins, verify_instruction)
  └── extend_evals:          ~99ms (was 154ms before shared buffers)
Interaction Merkle commit:  103ms  (6%)   was 160ms
Composition GPU:            250ms  (14%)  thermal-dependent (250ms-4000ms)
Composition commit:          61ms  (3%)
prove_values:               220ms  (12%)  was 405ms
  ├── OODS sampling:        ~100ms
  ├── FRI folds:             ~50ms (async, wait on last only)
  └── Decommit + PoW:        ~70ms
Other:                       64ms  (3%)
```

---

## 3. Key Architectural Insights

### What Works on Apple Silicon

1. **Non-blocking GPU dispatches with same-queue ordering.** Metal command buffers on a single queue execute in FIFO order. Dropping handles without waiting is safe as long as subsequent GPU work reads the same buffers. This is the single most impactful pattern — it eliminates hundreds of CPU-GPU synchronization points.

2. **Shared (not private) buffers for CPU→GPU uploads.** On unified memory, `newBufferWithBytes` (shared) does allocation + memcpy in one step. Private staging (shared → GPU blit → private) adds overhead without meaningful GPU cache benefit for the prover workload.

3. **Per-column rayon-parallelized IFFT submission.** Multiple threads submitting IFFTs to the same Metal queue keeps it saturated. The CPU preparation (buffer clone, twiddle lookup) parallelizes across cores while GPU processes the queue serially. This is BETTER than batching all IFFTs into one CB.

4. **Fused Merkle tree building (leaves + all layers in one dispatch).** Already implemented by Phase 1 team for >16 columns. We extended the GPU threshold to work for any column count ≥ 4.

### What Does NOT Work on Apple Silicon

1. **Multi-proof pipelining.** The unified GPU cannot run two workloads in parallel. Two proofs competing for the same Metal queue + rayon thread pool + L2 cache creates contention that outweighs any CPU/GPU overlap. Sequential proof execution is optimal.

2. **Single massive command buffers.** Encoding 4000+ compute encoders into one CB causes Metal runtime overhead that exceeds the savings from eliminating per-CB creation. Apple Silicon's command processor prefers many small CBs over few giant ones.

3. **GPU sequential prefix sum.** A single GPU thread doing O(n) sequential work for n=2M elements takes ~7ms per column. CPU SIMD (256-bit vectorized) does the same in ~2ms. The GPU's advantage is parallelism, not sequential throughput.

4. **Field-native hashes (Poseidon-M31).** The M31 multiply is fast (~2 cycles) but the Poseidon permutation requires ~960 multiplications per hash vs ~50 cycles for Blake2s. With identical 32-byte output, there's no tree size advantage. Blake2s is simply a faster hash function on this hardware.

### Why 2 MHz Is Unreachable (Current Architecture)

The **732ms base trace generation** is 40% of total time and is the hard floor:

- **300ms GPU opcode dispatch**: 6 Metal kernels generating trace columns from CasmState inputs. Already GPU-accelerated with optimized kernels. Hardware-bound.

- **432ms CPU write_trace**: Rayon-parallelized SIMD trace generation for ~25 non-GPU components (range_checks, builtins, verify_instruction, aggregators). Each component reads multiplicities (atomic columns) and generates trace columns. Already parallelized across CPU cores.

- **Row-major Merkle leaves**: `leaf[i] = Hash(col0[i], col1[i], ..., colN[i])` — each leaf requires ALL columns. Prevents column-level streaming through the IFFT → RFFT → Hash pipeline. You cannot start hashing until the last column is ready.

- **Unified GPU**: Cannot overlap GPU work from different proofs or different pipeline phases. The Metal command queue is a single FIFO.

### Path to 2 MHz

The only viable path is **rewriting the remaining 25 CPU-bound component trace generators as Metal GPU kernels** (extending the pattern of the 6 existing GPU opcode witness kernels). Each kernel would:
1. Read multiplicity data from atomic columns
2. Generate trace columns on GPU
3. Feed directly into RFFT → Merkle pipeline (no CPU round-trip)

This eliminates the 432ms CPU write_trace entirely. Estimated result: ~1406ms / 2.16 MHz.

This is a multi-month project — each component has unique trace generation logic (offset encoding for verify_instruction, F252 decomposition for memory_id_to_big, Poseidon rounds for poseidon_aggregator, etc.). The 6 opcode kernels took the Phase 1 team ~40 commits.

---

## 4. Build and Benchmark Commands

```bash
# Build the benchmark binary (MUST specify metal-prod for benchmarks)
cd fixtures/standalone-benchmarks
STWO_METAL_MODE=metal-prod cargo build \
  --features metal-runtime,cairo-prove \
  --bin cairo_prove --release

# Verify proof correctness (ALWAYS run after ANY change)
GPU_WITNESS=1 GPU_OPCODE_INTER=1 \
  target/release/cairo_prove --metal --verify \
  test_data/cairo_fib_32768/prover_input.json

# Benchmark (12 iterations, first 2-3 are warmup)
GPU_WITNESS=1 GPU_OPCODE_INTER=1 \
  target/release/cairo_prove --metal --bench 12 \
  test_data/cairo_fib_524288/prover_input.json

# Available test data sizes
ls test_data/
# cairo_fib_32768  cairo_fib_65536  cairo_fib_131072
# cairo_fib_262144 cairo_fib_524288 cairo_fib_1048576
# cairo_fib_2097152 cairo_fib_4194304

# Environment variables
# GPU_WITNESS=1        Enable GPU witness bypass (required for Metal path)
# GPU_OPCODE_INTER=1   Enable GPU fused interaction traces for opcodes
# NO_GPU_MULTS=1       Disable GPU multiplicity accumulation (debug)
# NO_GPU_OPCODE=1      Disable GPU opcode traces (debug)
# GPU_COOL=1           Insert 8ms cooling pauses between GPU phases
# GPU_COMP_BATCHES=N   Split composition into N sub-batches
```

---

## 5. File Map (Changes This Session)

### Modified Files

| File | What Changed |
|------|-------------|
| `crates/stwo-metal-sys/metal/runtime.m` | Non-blocking GPU copies, async FRI fold, batched IFFT infra, GPU prefix sum dispatch, reduce_sum_m31_4col |
| `crates/stwo-metal-sys/src/metal.rs` | FFI bindings for all new runtime functions |
| `crates/stwo-metal-sys/metal/prefix_sum.metal` | reduce_sum_m31 + prefix_sum_subtract_m31 kernels |
| `crates/stwo-metal/src/backend/metal/poly.rs` | Dropped RFFT waits, skipped coefficient promotion |
| `crates/stwo-metal/src/backend/metal/fri.rs` | Async FRI fold chain (submit all, wait on last) |
| `crates/stwo-metal/src/backend/metal/blake2s.rs` | Lowered Merkle GPU threshold (4096 → 4) |
| `crates/stwo-metal/src/backend/metal/interaction_trace_generic.rs` | (reverted GPU prefix sum — CPU SIMD is faster) |
| `crates/stwo-metal/src/stwo_metal/secure_field_vec.rs` | Async fold_line_step variant |
| `crates/stwo-metal/src/stwo_metal/base_field_vec.rs` | from_u32_slice_private, public from_buffer |
| `vendor/stwo-upstream-dev-62b228e/crates/stwo/src/prover/pcs/mod.rs` | Arc\<BaseColumnPool\> (was MaybeOwned borrow) |
| `vendor/stwo-cairo/.../cairo_claim_generator.rs` | gpu_range_check_mult_upload_fn callback |
| `vendor/stwo-cairo/.../prover.rs` | Arc\<BaseColumnPool\> call site |
| `fixtures/standalone-benchmarks/src/bin/cairo_prove.rs` | Shared buffer extend_evals, pipelined PreparedProofInput, composition shared commit, range_check GPU bypass |

### Vendored Library Changes (require re-application if upstream updates)

1. **stwo-upstream-dev** `pcs/mod.rs`: `CommitmentSchemeProver::base_column_pool` changed from `MaybeOwned<'a, BaseColumnPool<B>>` to `Arc<BaseColumnPool<B>>`. The `with_memory_pool()` constructor takes `Arc` instead of `&'a`. This enables cross-thread proof state ownership.

2. **stwo-cairo** `cairo_claim_generator.rs`: Added `gpu_range_check_mult_upload_fn` callback field for GPU-native range_check trace upload. Added `commit_range_check_or_gpu!` macro.

3. **stwo-cairo** `prover.rs`: Updated `prove_cairo_with_precompute` to take `Arc<BaseColumnPool>`.

---

## 6. Things to Try Next

If continuing this work, the highest-impact directions are:

1. **GPU witness kernels for remaining components** (the 2 MHz path). Start with `range_check_9_9` (LOG_SIZE 18, 262K rows, 8 multiplicity columns) as a template — it's the largest range_check and the simplest trace generation pattern.

2. **Fused IFFT+RFFT "coset extension" kernel**. Instead of IFFT → zero-pad → RFFT (3 steps), a single kernel that converts from trace domain to blowup domain. This eliminates the intermediate coefficient buffer and one full GPU pass. Saves maybe ~30ms.

3. **Column-major Merkle tree variant**. If you can change the tree layout so each column is hashed independently (not all columns per row), then column-level streaming becomes possible. This breaks verifier compatibility but could be worth it for a prover-specific commitment scheme.

4. **Multi-queue with explicit event synchronization**. Instead of running two full proofs (which causes contention), use a second Metal queue ONLY for Merkle hashing. Signal an MTLSharedEvent after each RFFT group completes, and have the Merkle queue wait on it. This gives true compute+memory overlap within a single proof.

---

## 7. Known Issues

1. **Thermal throttling on composition**: The GPU composition phase varies from 250ms (cool) to 4000ms+ (throttled). No software fix exists — Apple Silicon's thermal time constant is longer than any reasonable cooling pause. The best mitigation is to run fewer iterations with longer intervals.

2. **GPU prefix sum infrastructure exists but is unused**: The `reduce_sum_m31` and `prefix_sum_subtract_m31` Metal kernels are compiled and available but not called from the hot path. The sequential GPU scan is slower than CPU SIMD for M31 prefix sum. Would need a proper parallel Hillis-Steele or Blelloch scan to be competitive.

3. **Shared worktree contamination**: The `.claude/worktrees/` directory may contain stale agent worktree references. These are harmless but show up in `git status`. Clean with `rm -rf .claude/worktrees/`.
