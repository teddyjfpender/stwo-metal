# DN-0014: Metal GPU Composition Pipeline

## Status

`shipped` — verified proof produced, ~4.5% faster than SimdBackend on cairo_fib_1000

## Purpose

Accelerate the Cairo proving pipeline by replacing the STARK composition step
with Metal V1 GPU composition, while keeping the rest of the pipeline on
SimdBackend. The key insight is reusing the pre-lowering phase's
`CommitmentSchemeProver<SimdBackend>` (which already has preprocessed, base,
and interaction trees committed) and only swapping the composition step for
GPU execution.

## Problem

The original DN-0014 design proposed a full `CommitmentSchemeProver<MetalBackend>`
pipeline with a `ConvertingTreeBuilder` adapter. Implementation revealed a
**GPU IFFT kernel bug** (see Appendix A) that produces incorrect polynomial
coefficients for columns ≥ 2^17 elements. This blocked the full MetalBackend
pipeline approach.

## Revised Design — Hybrid Composition Pipeline

### Architecture (implemented)

```
prove_cairo with --metal:
  │
  ├── Pre-lowering phase (SimdBackend — unchanged)
  │     ├── precompute_twiddles
  │     ├── preprocessed tree commit (SimdBackend IFFT+RFFT+Merkle)
  │     ├── base trace: write_trace → SimdBackend commit
  │     ├── interaction PoW: MetalBackend::grind (GPU Blake2s)
  │     ├── interaction trace: write_interaction_trace → SimdBackend commit
  │     └── → yields CommitmentSchemeProver<SimdBackend> + channel
  │
  ├── run_metal_composition_and_prove (Metal GPU)
  │     ├── draw random_coeff from channel
  │     ├── compute_metal_composition_poly (GPU V1 interpreter)
  │     │     Extract polynomials from commitment scheme
  │     │     Run V1 GPU kernels for large components
  │     │     Fall back to SIMD/CPU for small components
  │     ├── commit composition polynomial (SimdBackend)
  │     ├── draw OODS point, compute sample points from SimdBackend components
  │     └── prove_values (SimdBackend — FRI, quotients, decommit)
  │
  └── Verify proof (optional, --verify flag)
```

### Key function: `run_metal_composition_and_prove`

```rust
fn run_metal_composition_and_prove(
    mut commitment_scheme: CommitmentSchemeProver<'_, SimdBackend, Blake2sMerkleChannel>,
    channel: &mut Blake2sChannel,
    results: &[LoweringResult],
    components_for_prove: &[&dyn ComponentProver<SimdBackend>],
    include_all_preprocessed_columns: bool,
) -> (ExtendedStarkProof<Blake2sMerkleHasher>, f64, f64)
```

This function:
1. Draws `random_coeff` from the channel
2. Runs `compute_metal_composition_poly` for GPU composition (~290-370ms)
3. Commits the composition polynomial via SimdBackend
4. Draws OODS point and computes sample points from SimdBackend component provers
5. Runs `prove_values` on SimdBackend (~350-665ms)

### Why this works

The commitment scheme is reused from the pre-lowering phase — it already
contains the preprocessed, base, and interaction trees. The Metal GPU only
replaces the composition polynomial evaluation step, which is the compute-
intensive constraint evaluation across all components. Everything else
(IFFT, RFFT, Merkle, FRI, quotients, decommit) stays on SimdBackend.

This avoids:
- GPU IFFT bug (columns ≥ 2^17)
- Converting ~1-2GB Merkle trees between backends
- Re-committing trees that are already committed

### Composition V1 GPU dispatch

`compute_metal_composition_poly` uses hybrid dispatch:
- Components with ≥ 4096 evaluation rows → GPU V1 interpreter kernel
- Small components → SimdBackend/CPU fallback
- At cairo_fib_1000 scale: 13 GPU + 18 CPU hybrid dispatches
- Kernel throughput: ~14 ns/constraint-evaluation (near GPU peak for M31)

## Measured performance (cairo_fib_1000)

### Head-to-head (warm runs, `--metal --verify`)

| Metric | Metal pipeline | SimdBackend | Delta |
|--------|---------------|-------------|-------|
| Total | ~4880 ms | ~5110 ms | **-4.5%** |
| Composition | ~370 ms (GPU) | ~540 ms (CPU) | -31% |
| prove_values | ~665 ms | ~665 ms | — |
| Interaction PoW | ~42 ms (GPU) | ~136 ms (CPU) | -69% |

### Metal prove post-lowering breakdown

| Phase | Time (ms) |
|-------|-----------|
| Composition (GPU V1) | ~290-370 |
| prove_values | ~350-665 |
| **Total prove** | **~1080** |

### Pre-lowering phase (shared, dominated by preprocessed tree)

| Sub-phase | Time (ms) |
|-----------|-----------|
| preprocessed tree commit | ~3900-4900 |
| base trace commit | ~55-60 |
| interaction PoW (GPU) | ~42 |
| interaction trace commit | ~68-94 |

## Delivery sequence

| # | Deliverable | Status |
|---|-------------|--------|
| ~~D1~~ | ~~ConvertingTreeBuilder~~ | Superseded — not needed |
| ~~D2~~ | ~~Full MetalBackend pipeline~~ | Superseded — blocked by GPU IFFT bug |
| D3 | `run_metal_composition_and_prove` | **DONE** — hybrid approach |
| D4 | Verification | **DONE** — `--metal --verify` passes |
| D5 | Benchmark | **DONE** — ~4.5% faster warm runs |

## Future work: Full MetalBackend pipeline (blocked)

The original full-pipeline design remains attractive (~25% speedup from GPU
preprocessed tree IFFT) but is blocked by the GPU IFFT kernel bug. Once
fixed, the `ConvertingTreeBuilder` approach from the original design can be
revisited. See Appendix A.

## Appendix A: GPU IFFT Kernel Bug

**Symptom**: `MetalBackend::interpolate_columns` produces incorrect polynomial
coefficients for columns with ≥ 2^17 (131072) elements. First mismatch is
always at index 16.

**Scope**: 77 of 161 preprocessed columns at cairo_fib_1000 scale are affected
(those with log_size ≥ 17).

**Investigation results**:
- Verified with `CPU_TWIDDLES=1`: bug persists with CPU-computed twiddles,
  ruling out twiddle computation as the cause
- Verified with `SIMD_IFFT=1`: SimdBackend IFFT produces correct results,
  confirming the kernel is the issue
- The `ifft_line_part_u32` kernel in `ifft.metal` uses
  `twiddle_index = index >> layer` for twiddle lookup. The RFFT kernel
  (`rfft_line_part_u32`) uses a `layer_domain_offset` parameter instead.
  This structural difference may be related to the bug.
- Columns < 2^17 produce correct results (all 84 small columns match)

**Root cause**: Not yet identified. The bug is in the Metal IFFT kernel
dispatch or twiddle indexing logic, not in host-side code.

**Impact if fixed**: GPU IFFT for preprocessed tree would save ~1200-1300ms
(IFFT alone: 450ms GPU vs 1735ms SIMD, plus eliminating the download phase).

## Key files

| File | Role |
|------|------|
| `fixtures/standalone-benchmarks/src/bin/cairo_prove.rs` | Main integration — `run_metal_composition_and_prove` |
| `crates/stwo-metal/src/backend/metal/poly.rs` | GPU IFFT/RFFT (IFFT has bug for ≥2^17) |
| `crates/stwo-metal/src/backend/metal/blake2s.rs` | GPU Merkle + PoW grind |
| `crates/stwo-metal/src/backend/metal/eval_program_v1.rs` | V1 GPU composition dispatch |
| `crates/stwo-metal/src/backend/metal/prove_runtime_v1.rs` | MetalBackend prove pipeline |
| `crates/stwo-metal-sys/metal/ifft.metal` | GPU IFFT kernel (buggy for large columns) |
| `vendor/stwo-cairo/.../prover.rs` | Reference prove_cairo flow |
