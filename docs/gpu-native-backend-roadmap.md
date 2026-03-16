# Fully GPU-Native stwo Backend Roadmap

## Goal: 5+ MHz proving speed at fib(524K)

Current: 1.69 MHz (1,865ms) → Target: 5+ MHz (~629ms)

## Phase 1: GPU Kernels (Parallel Implementation)

### K1: GPU IFFT Kernel ⬜
**Files to create/modify:**
- `crates/stwo-metal-sys/metal/ifft.metal` — Metal compute kernel
- `crates/stwo-metal-sys/metal/runtime.m` — ObjC dispatch
- `crates/stwo-metal-sys/src/metal.rs` — FFI + Rust wrapper
- `crates/stwo-metal/src/backend/metal/poly.rs` — Wire into `PolyOps::interpolate()`

**What it does:** Inverse FFT on circle domain polynomials. Currently every
`interpolate()` call downloads GPU eval to CPU, runs CPU IFFT, re-uploads.
This happens hundreds of times per proof (once per committed column).

**Expected savings:** ~400-600ms (eliminates the most frequent CPU↔GPU conversion)

**Validation:** `interpolate(evaluate(poly)) == poly` round-trip test

### K2: GPU Trace Generation ⬜
**Files to create/modify:**
- Already have kernels for all fib components (memory, opcodes, range checks, verify_instruction)
- Need: wire ALL GPU kernels as the PRIMARY path (not fallback)
- `vendor/stwo-cairo/.../cairo_claim_generator.rs` — make GPU path default
- `fixtures/standalone-benchmarks/src/bin/cairo_prove.rs` — remove CPU fallback for GPU-ready components

**What it does:** GPU generates trace columns directly. CPU only does
multiplicity accumulation (mults-only path, ~200ms).

**Expected savings:** ~200-300ms (trace column gen moves to GPU, overlapped)

**Validation:** GPU trace columns match CPU trace columns exactly (tests exist: 10/10 pass)

### K3: GPU Tree Decommit ⬜
**Files to create/modify:**
- `crates/stwo-metal-sys/metal/merkle_decommit.metal` — Metal kernel for Merkle path queries
- `crates/stwo-metal-sys/metal/runtime.m` — ObjC dispatch
- `crates/stwo-metal-sys/src/metal.rs` — FFI
- `crates/stwo-metal/src/backend/metal/blake2s.rs` — Wire into MerkleOps

**What it does:** Given query positions, extract Merkle authentication paths
from GPU-resident tree layers. Currently downloads all layers to CPU.

**Expected savings:** ~50-80ms

**Validation:** Decommit paths match CPU-generated paths

### K4: GPU Barycentric Evaluation ⬜
**Files to create/modify:**
- `crates/stwo-metal-sys/metal/barycentric.metal` — Metal kernel
- `crates/stwo-metal/src/backend/metal/poly.rs` — Wire into `PolyOps::barycentric_eval_at_point()`

**What it does:** Barycentric interpolation at arbitrary points. Currently
falls through to CPU bridge.

**Expected savings:** ~20-30ms

**Validation:** Result matches CPU barycentric evaluation

### K5: GPU FRI Decompose ⬜
**Files to create/modify:**
- `crates/stwo-metal-sys/metal/fri_decompose.metal` — Metal kernel
- `crates/stwo-metal/src/backend/metal/fri.rs` — Wire into `FriOps::decompose()`

**What it does:** Decomposes a polynomial into even/odd parts for FRI.
Currently uses CPU bridge via `materialize_line_evaluation_via_cpu_bridge()`.

**Expected savings:** ~30-50ms

**Validation:** Decomposed parts match CPU decomposition

---

## Phase 2: Integration & Wiring

### W1: Eliminate all `to_cpu()` from critical path ⬜
After K1-K5 are implemented, audit and remove every `to_cpu()`, `to_vec()`,
`from_slice()` call in the proving hot path:
- `crates/stwo-metal/src/backend/metal/poly.rs` — interpolate, evaluate paths
- `crates/stwo-metal/src/backend/metal/fri.rs` — fold, decompose paths
- `crates/stwo-metal/src/backend/metal/quotient.rs` — quotient eval paths
- `crates/stwo-metal/src/backend/metal/prove_runtime_v1.rs` — prove_values

### W2: GPU-resident polynomial storage ⬜
Ensure `CommitmentSchemeProver<MetalBackend>` stores polynomial coefficients
on GPU and never downloads them for RFFT/IFFT:
- `store_polynomials_coefficients = true` path keeps GPU buffers
- Composition polynomial split (left/right halves) stays on GPU
- FRI quotient polynomials stay on GPU through fold layers

### W3: End-to-end proof verification ⬜
Run full proving + verification with all GPU kernels active:
```
GPU_WITNESS=1 target/release/cairo_prove --metal --verify test_data/cairo_fib_32768/prover_input.json
```
Must produce a valid proof. Any bit-level mismatch = soundness bug.

---

## Performance Targets

| Milestone | Expected Time | MHz | Gate |
|-----------|-------------|-----|------|
| Current baseline | 1,865ms | 1.69 | — |
| After K1 (GPU IFFT) | ~1,200ms | 2.6 | K1 validated |
| After K1+K2 (GPU traces wired) | ~900ms | 3.5 | K2 validated |
| After K1-K5 + W1-W2 | ~600-700ms | 4.5-5.2 | All validated |
| After W3 (verified) | ~600-700ms | 4.5-5.2 | Proof valid |

## Implementation Notes

- K1 is the single highest-impact kernel — implement first
- K2 builds on existing GPU witness infrastructure (kernels exist)
- K3-K5 are smaller but needed to eliminate remaining CPU bridges
- W1-W2 are integration work after kernels are ready
- W3 is the correctness gate — nothing ships without proof verification
