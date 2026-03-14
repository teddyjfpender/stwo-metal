# Handover Notes — 2026-03-13 Session 2

## Purpose

Capture the state after implementing the Metal GPU composition pipeline (DN-0014 revised),
discovering the GPU IFFT kernel bug, and shipping a verified-proof hybrid pipeline.

## What changed this session

### 1. Implemented `run_metal_composition_and_prove` (cairo_prove.rs)

Replaced the original `run_metal_full_pipeline` approach (which attempted a full
`CommitmentSchemeProver<MetalBackend>` pipeline) with a hybrid approach that reuses
the pre-lowering phase's `CommitmentSchemeProver<SimdBackend>`.

The new function:
1. Takes the existing commitment scheme (preprocessed + base + interaction trees committed)
2. Draws `random_coeff` from the channel
3. Runs `compute_metal_composition_poly` for GPU composition
4. Commits composition polynomial via SimdBackend
5. Computes sample points from SimdBackend component provers
6. Runs `prove_values` on SimdBackend

This sidesteps the GPU IFFT bug entirely since no new IFFT calls are needed.

### 2. Discovered GPU IFFT kernel bug

**Symptom**: `MetalBackend::interpolate_columns` (via `ifft_interpolate_in_place`)
produces incorrect polynomial coefficients for columns with ≥ 2^17 elements.

**Key findings**:
- First mismatch always at index 16 in output coefficients
- 77/161 preprocessed columns affected (log_size ≥ 17)
- All 84 columns with log_size < 17 produce correct results
- Bug persists with CPU-computed twiddles (`CPU_TWIDDLES=1`) — not a twiddle issue
- SimdBackend IFFT (`SIMD_IFFT=1`) produces correct results for same columns
- Bug is in `ifft_line_part_u32` kernel in `ifft.metal`
- Structural difference from working RFFT: IFFT uses `index >> layer` for twiddle
  indexing, RFFT uses explicit `layer_domain_offset` parameter

**Impact**: Blocks the full MetalBackend pipeline (ConvertingTreeBuilder approach).
If fixed, would enable ~1200ms savings from GPU preprocessed tree IFFT.

### 3. Removed orphaned code from prior session

Deleted ~166 lines of orphaned code (old lines 1256-1422) left over from a partially
removed `compute_metal_composition_poly_from_metal` function. Also removed unused
imports (`TreeSubspan`, `M31`) and the unused `input_for_metal` variable.

### 4. Updated DN-0014 design doc

Revised DN-0014 from "Full MetalBackend Pipeline via ConvertingTreeBuilder" to
"Metal GPU Composition Pipeline" reflecting the actual shipped architecture.
Added Appendix A documenting the GPU IFFT bug.

## Benchmark results

### Warm run (`--metal --verify`)

| Metric | Metal | SIMD | Delta |
|--------|-------|------|-------|
| Total | ~4880 ms | ~5110 ms | **-4.5%** |
| Composition | ~370 ms | ~540 ms | -31% |
| prove_values | ~665 ms | ~665 ms | — |
| Interaction PoW | ~42 ms | ~136 ms | -69% |

### Metal prove post-lowering

| Phase | Time (ms) |
|-------|-----------|
| Composition (GPU V1) | ~290-370 |
| prove_values (SimdBackend) | ~350-665 |
| **Total prove** | **~1080** |

## Remaining optimization targets

1. **Fix GPU IFFT kernel bug** — enables full MetalBackend preprocessed tree (~1200ms savings)
2. **Preprocessed tree caching** (~4s one-time) — serialize to disk
3. **GPU FRI PoW grind** (~50ms) — requires MetalBackend prove_values
4. **GPU FRI commit** (~82ms) — ~22ms net gain

## Key files changed

| File | Change |
|------|--------|
| `fixtures/standalone-benchmarks/src/bin/cairo_prove.rs` | New `run_metal_composition_and_prove`, removed orphaned code |
| `docs/dn-0014-full-metal-backend-pipeline-via-converting-tree-builder.md` | Revised to reflect shipped architecture |
