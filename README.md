# stwo-metal

A Metal GPU-accelerated backend for the [stwo](https://github.com/starkware-libs/stwo) STARK prover, targeting Apple Silicon. Proves Cairo programs on-device using Metal compute shaders for IFFT/RFFT, Blake2s Merkle hashing, constraint evaluation, FRI folding, and more.

## Benchmark Results

**Hardware:** Apple Silicon (M-series)
**Workload:** Cairo `fib(n)` end-to-end prove + verify, Blake2s Merkle channel
**Best run:** 1,838 ms at fib(524K) = **1.71 MHz**

### Metal vs CPU (SIMD) Throughput

| fib(n) | Cycles | Metal (ms) | Metal (MHz) | SIMD (ms) | SIMD (MHz) | Speedup |
|---------|-----------|------------|-------------|-----------|------------|---------|
| 32,768 | 196,630 | 902 | 0.22 | 3,385 | 0.06 | 3.75x |
| 131,072 | 786,454 | 1,107 | 0.71 | 3,938 | 0.20 | 3.56x |
| 262,144 | 1,572,886 | 1,357 | 1.16 | 4,529 | 0.35 | 3.34x |
| 524,288 | 3,145,750 | 1,838 | 1.71 | 5,575 | 0.56 | 3.03x |

> **Speedup** = SIMD time / Metal time. Metal is 3-3.75x faster across all sizes.

### Throughput Scaling

```
Metal MHz:  0.22 ──> 0.71 ──> 1.16 ──> 1.71   (scales with trace size)
SIMD  MHz:  0.06 ──> 0.20 ──> 0.35 ──> 0.56   (CPU-bound plateau)
```

### What's GPU-Accelerated

| Operation | Status |
|-----------|--------|
| IFFT / RFFT (polynomial transforms) | Metal compute |
| Blake2s Merkle tree (leaves + layers) | Metal compute |
| Constraint evaluation (31 components) | Metal compute (JIT) |
| FRI fold (circle-to-line + line steps) | Metal compute |
| Proof-of-work grind | Metal compute |
| Opcode witness generation (6 kernels) | Metal compute |
| Twiddle / bit-reversal precomputation | Metal compute |

### Phase Breakdown (fib(524K), 1,838 ms best warm run)

```
Base trace gen + upload       732ms  (40%)
Base Merkle commit            138ms   (7%)
Interaction trace gen         257ms  (14%)
Interaction Merkle commit     103ms   (6%)
Composition (GPU)             250ms  (14%)
Composition commit             61ms   (3%)
prove_values (FRI + decommit) 220ms  (12%)
Other                          77ms   (4%)
```

## Building and Running

```bash
cd fixtures/standalone-benchmarks

# Build
STWO_METAL_MODE=metal-prod cargo build \
  --features metal-runtime,cairo-prove \
  --bin cairo_prove --release

# Verify proof correctness
GPU_WITNESS=1 GPU_OPCODE_INTER=1 \
  target/release/cairo_prove --metal --verify \
  test_data/cairo_fib_32768/prover_input.json

# Benchmark (12 iterations, first 2-3 are warmup)
GPU_WITNESS=1 GPU_OPCODE_INTER=1 \
  target/release/cairo_prove --metal --bench 12 \
  test_data/cairo_fib_524288/prover_input.json
```

## Repository Structure

- **`crates/stwo-metal`** -- Metal backend implementation (poly, FRI, Blake2s, witness, quotient)
- **`crates/stwo-metal-sys`** -- Metal shader compilation and FFI bindings (`runtime.m`, `.metal` shaders)
- **`vendor/stwo-upstream-dev-62b228e`** -- Vendored stwo fork with `Arc<BaseColumnPool>` for cross-thread proof state
- **`vendor/stwo-cairo`** -- Vendored stwo-cairo with GPU witness hooks
- **`fixtures/standalone-benchmarks`** -- Benchmark harness and test data

## License

See individual crate licenses.
