All 3 research agents complete. Here's the synthesis:

## Where the 1,936ms Goes (Best Warm Run)

```
Base trace gen:     508ms  ← 300ms is LookupData allocation (needed for CPU interaction traces)
Interaction trace:  364ms  ← 186ms is extend_evals commitment overhead
Composition:        375ms  ← 319ms GPU kernel (near-optimal when not throttled)
prove_values:       354ms  ← 230ms OODS sampling dominates
Merkle commits:     353ms  ← 4 trees, all sequential
```

## The Path to 629ms (5 MHz) — What's Physically Possible

| Phase | Current | Theoretical Floor | Achievable | Key Change |
|-------|---------|-------------------|-----------|-----------|
| Base trace | 508ms | **~100ms** | **~150ms** | Skip LookupData (GPU interaction traces make it unnecessary) → mults-only becomes just atomic increments |
| Interaction trace | 364ms | **~80ms** | **~120ms** | GPU interaction traces already work; fix extend_evals overhead |
| Composition | 375ms | **~50ms** | **~200ms** | Already near-optimal; thermal throttling is main issue |
| prove_values | 354ms | **~60ms** | **~150ms** | FRI fold+commit fusion, GPU-resident quotients |
| Merkle commits | 353ms | **~120ms** | **~200ms** | Partially irreducible (Fiat-Shamir dependencies) |
| **Total** | **1,936ms** | **~410ms** | **~820ms** | **3.8 MHz** |

## The Three Biggest Levers

**1. Eliminate LookupData from mults-only path (saves ~200-300ms from base trace)**
R2 found that `write_trace_simd_mults_only` allocates ~900MB of LookupData (Vec<[PackedM31; 30]> etc.) only needed for CPU interaction traces. Since GPU interaction traces are now wired, we can add a `write_trace_mults_only_no_lookupdata()` that skips all LookupData allocation — just atomic scatter-adds.

**2. Fix extend_evals overhead in interaction trace (saves ~100ms)**
186ms of the 364ms interaction phase is `extend_evals` — committing columns to the tree. This involves SimdBackend→MetalBackend conversion. Using `extend_opaque_evals` with GPU interaction traces (already wired) should eliminate this.

**3. FRI fold+commit fusion in prove_values (saves ~50-100ms)**
Currently separate GPU dispatches per FRI layer. Fusing fold+Merkle per layer reduces command buffer overhead.

**Hard limits we can't optimize past:**
- Fiat-Shamir sequential channel dependencies (phases MUST be serial)
- FRI layer-to-layer dependencies (each fold needs previous layer's commitment)
- Apple Silicon GPU thermal throttling (13x swings observed — irreducible)

**Realistic target: ~820ms = 3.8 MHz** with these three changes. To reach 5 MHz (629ms) would require GPU thermal management and further Merkle optimization, which pushes into hardware limits.

Want me to launch agents to implement the three biggest levers?