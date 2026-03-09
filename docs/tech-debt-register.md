# Technical Debt Register

Use this file for intentional temporary compromises that are currently accepted.

This is not a bug backlog. Every entry must state why the compromise exists,
how it is contained, and what retires it.

## Entry template

```md
### TD-XXXX: Title

- Status: active | planned | retired | superseded
- Category:
- Introduced:
- Owner area:

Why it exists now:

Current containment:

Risk if left in place:

Exit condition:

Target retirement point:
```

## Active debt

### TD-0001: Internal backend vocabulary is still CUDA-first

- Status: `active`
- Category: `boundary naming`
- Introduced: `2026-03-09`
- Owner area: `Rust backend boundary`

Why it exists now:

The repo identity has been corrected to `stwo-metal`, but internal modules and
types still use names such as `CudaBackend`, `stwo_cuda`, and
`stwo_cuda_link`. Renaming them before the backend-neutral interface is designed
would create churn without locking the right abstraction.

Current containment:

- `crates/stwo-metal/src/backend/cuda`
- `crates/stwo-metal/src/stwo_cuda`
- cfg gates and tests that still reference CUDA naming

Risk if left in place:

The public direction of the project stays clear, but internal design work can
accidentally inherit CUDA-specific concepts into the long-term Metal boundary.

Exit condition:

An approved boundary-neutral naming design lands and the internal Rust surface
is migrated to match it.

Target retirement point:

- `T1`

### TD-0002: Native runtime ownership is still the inherited CUDA build

- Status: `active`
- Category: `native runtime bridge`
- Introduced: `2026-03-09`
- Owner area: `stwo-metal-sys`

Why it exists now:

`stwo-metal-sys` still contains the copied CUDA native build, CMake files, and
the `stwo_cuda` static library target. This is acceptable only as a temporary
placeholder while the Metal runtime boundary is designed.

Current containment:

- `crates/stwo-metal-sys/cuda`
- `crates/stwo-metal-sys/build.rs`

Risk if left in place:

The project could drift into a renamed CUDA fork rather than a bounded Metal
port.

Exit condition:

The Metal runtime design is approved and the inherited CUDA native build is
either removed or isolated behind an explicit transition plan.

Target retirement point:

- `T3`

### TD-0003: CI and helper tooling have not yet been curated for `stwo-metal`

- Status: `active`
- Category: `tooling hygiene`
- Introduced: `2026-03-09`
- Owner area: `project operations`

Why it exists now:

The docs surface was reset first. Copied scripts and CI definitions still
contain CUDA-era assumptions and naming.

Current containment:

- `.github/`
- `scripts/`

Risk if left in place:

Contributors may infer unsupported process or runtime guarantees from stale
tooling.

Exit condition:

The active CI and helper scripts are audited against the `stwo-metal` host and
backend plan.

Target retirement point:

- `T2`

### TD-0004: The Metal runtime surface is still primitive-specific rather than backend-neutral

- Status: `active`
- Category: `runtime boundary shape`
- Introduced: `2026-03-09`
- Owner area: `stwo-metal-sys`

Why it exists now:

The native Metal runtime currently exposes bounded helpers for `u32`,
`u32x4`, and one explicit poly-order permutation. This is deliberate for the
first proving-surface slices, but it is not yet the stable backend-neutral ABI
we ultimately want.

Current containment:

- `crates/stwo-metal-sys/src/metal.rs`
- `crates/stwo-metal-sys/metal`
- `crates/stwo-metal/src/stwo_metal`

Risk if left in place:

Future ports could accumulate one-off runtime entrypoints instead of converging
on a coherent Metal ABI for proving operations.

Exit condition:

An approved runtime-boundary refactor groups the bounded helpers into a smaller
and more durable Metal ABI surface without losing deterministic parity tests.

Target retirement point:

- `T5`

### TD-0005: FRI domain factor generation is still host-derived rather than owned by the Metal runtime

- Status: `active`
- Category: `host orchestration bridge`
- Introduced: `2026-03-09`
- Owner area: `T5 bounded FRI bring-up`

Why it exists now:

The bounded Metal FRI slices currently derive inverse-`y` and inverse-`x`
factor buffers on the Rust host from vendored Stwo domain semantics and pass
them into native Metal kernels. This keeps the first proving-path cuts narrow
and explicit while the proving-facing runtime boundary is still being shaped.

Current containment:

- `crates/stwo-metal/src/backend/metal/fri.rs`

Risk if left in place:

The arithmetic remains correct, but the supported Metal story could stall at a
host-prepared twiddle bridge instead of converging on a more durable runtime
boundary for domain-derived factors.

Exit condition:

The declared proving path owns its domain-factor generation or equivalent
twiddle materialization at a stable runtime boundary without changing bounded
CPU-oracle parity guarantees.

Target retirement point:

- `T5`

### TD-0006: The first inner FRI-layer commitment still leaves Metal through an explicit CPU bridge

- Status: `active`
- Category: `proving-facing bridge`
- Introduced: `2026-03-09`
- Owner area: `T5 bounded FRI bring-up`

Why it exists now:

The bounded Metal lane originally needed an explicit CPU bridge to keep the
first inner-layer commitment honest before a native `stwo-metal` commitment
surface existed. The bridge remains available as a validation path even though
the native commitment boundary now exists.

Current containment:

- `crates/stwo-metal/src/backend/metal/handoff.rs`

Risk if left in place:

The bridge is now contained and explicit, but keeping it around too long risks
leaving two proving-facing paths alive when only one should remain primary.

Exit condition:

The native first inner-layer query and decommit path is stable enough that the
CPU bridge is no longer needed even as a validation surface.

Target retirement point:

- `T5`

### TD-0007: Native first inner-layer commitment is still host-owned and root-focused

- Status: `active`
- Category: `native commitment boundary`
- Introduced: `2026-03-09`
- Owner area: `T5 bounded FRI bring-up`

Why it exists now:

The native `stwo-metal` commitment boundary currently reads folded line values
back to the host and builds the lifted Merkle tree there. It now supports
native decommit semantics on top of that host-owned tree, but the hashing path
itself is still not GPU-side.

Current containment:

- `crates/stwo-metal/src/backend/metal/line.rs`

Risk if left in place:

The proving path can now commit and decommit honestly for the first inner
layer, but host-owned hashing may become an accidental long-term ceiling if the
team never explicitly decides whether that is acceptable for T5.

Exit condition:

The team has explicitly decided whether host-owned hashing is an acceptable T5
endpoint or has replaced it with a GPU-side hash path.

Target retirement point:

- `T5`

### TD-0008: Last-layer interpolation in the bounded FRI commitment slice is still CPU-bridged

- Status: `active`
- Category: `proving-facing bridge`
- Introduced: `2026-03-09`
- Owner area: `T5 bounded FRI bring-up`

Why it exists now:

The bounded `MetalFriCommitmentSlice` now packages the native inner-layer FRI
sequence with last-layer interpolation, but that interpolation still
materializes a `LineEvaluation<CpuBackend>` through the explicit handoff bridge
before deriving the bounded `LinePoly`.

Current containment:

- `crates/stwo-metal/src/backend/metal/commitment_slice.rs`
- `crates/stwo-metal/src/backend/metal/handoff.rs`

Risk if left in place:

The proof-facing slice is now truthful about its degree-bound output, but the
last-layer polynomial contract could remain partially CPU-owned instead of
converging on a clearer native `stwo-metal` interpolation boundary.

Exit condition:

The bounded Metal FRI path derives the last-layer polynomial through a native
`stwo-metal` interpolation boundary, or the team explicitly accepts the
CPU-bridged interpolation as the T5 endpoint.

Target retirement point:

- `T5`

### TD-0009: The bounded full FRI proof candidate still depends on caller-supplied folding alphas

- Status: `retired`
- Category: `transcript ownership`
- Introduced: `2026-03-09`
- Owner area: `T5 bounded FRI bring-up`

Why it existed:

The bounded Metal FRI proof slice initially packaged the first-layer and
inner-layer proof surfaces into the vendored `ExtendedFriProof` shape, but it
still took the first-layer alpha and inner-layer alphas as explicit caller
inputs instead of deriving them from a transcript-owned channel boundary.

Current containment:

- `crates/stwo-metal/src/backend/metal/proof.rs`
- `crates/stwo-metal/src/backend/metal/proof_slice.rs`

Risk if left in place:

The bounded proof object shape would have remained truthful, but the supported
Metal story could have stalled at a proof-construction helper rather than a
declared proving sub-path with transcript ownership aligned to vendored Stwo
semantics.

Exit condition:

The bounded FRI lane owns its challenge flow at an explicit transcript boundary
without weakening deterministic CPU-oracle parity.

Target retirement point:

- `T5`

### TD-0010: The declared bounded FRI sub-path is still FRI-only and not yet workload-complete

- Status: `retired`
- Category: `workload integration`
- Introduced: `2026-03-09`
- Owner area: `T5 bounded proving-path bring-up`

Why it existed:

`stwo-metal` now has a declared bounded Blake2s FRI proving sub-path, but that
declared path still stops at the FRI boundary. It does not yet own quotient,
trace, or PCS integration for one declared Stwo workload.

Current containment:

- `crates/stwo-metal/src/backend/metal/subpath.rs`
- `crates/stwo-metal/src/backend/metal/prover.rs`

Risk if left in place:

The project could over-index on a truthful FRI lane while still lacking one
declared Stwo workload boundary that demonstrates where FRI plugs into the rest
of the prover.

Exit condition:

One declared Stwo workload boundary consumes the declared bounded FRI sub-path
with explicit quotient, trace, and PCS ownership and deterministic CPU-oracle
parity.

Target retirement point:

- `T5`

### TD-0011: The declared hybrid workload boundary still begins at a FRI-ready evaluation input

- Status: `active`
- Category: `workload handoff`
- Introduced: `2026-03-09`
- Owner area: `T5 workload handoff`

Why it exists now:

`stwo-metal` now has a declared hybrid workload boundary with explicit witness,
quotient, PCS, and FRI ownership, and it now has an executable handoff from a
CPU-owned quotient evaluation. The CPU-owned stages are named, but witness
artifacts and quotient accumulation do not yet feed the boundary through an
earlier stable handoff.

Current containment:

- `crates/stwo-metal/src/backend/metal/workload.rs`
- `crates/stwo-metal/src/backend/metal/subpath.rs`
- `crates/stwo-metal/src/backend/metal/benchmark.rs`

Risk if left in place:

The project could sound more workload-complete than it really is, even though
the executable hybrid path still begins after witness generation and quotient
accumulation.

Exit condition:

One declared workload owns a stable handoff from a CPU-owned witness artifact
into the executable Metal workload boundary with deterministic CPU-oracle
parity.

Target retirement point:

- `T5`

### TD-0012: The wide-fibonacci prove benchmark still enters through the inherited CUDA-era proving lane

- Status: `active`
- Category: `benchmark execution boundary`
- Introduced: `2026-03-09`
- Owner area: `T5 benchmark-target alignment`

Why it exists now:

`stwo-metal` now declares the log-size-20 wide-fibonacci benchmark target with
an explicit `90 ms` RTX 4090 reference goal, and the standalone
`wide_fibonacci_trace` benchmark now enters through a native `.metal`
trace-generation path. The remaining benchmark gap is that
`wide_fibonacci_prove` still enters through the inherited CUDA witness and
proving path after that trace boundary.

Current containment:

- `crates/stwo-metal/src/backend/metal/benchmark.rs`
- `fixtures/standalone-benchmarks/src/bin/wide_fibonacci_prove.rs`

Risk if left in place:

The project could optimize the Metal FRI/proof lane while still overstating
benchmark progress if the executable prove row keeps inheriting its early
stages from the CUDA path.

Exit condition:

The executable `wide_fibonacci_prove` row enters through the native Metal trace
boundary and any remaining CPU-owned stages are explicitly named rather than
implicitly inherited from CUDA.

Target retirement point:

- `T6`
