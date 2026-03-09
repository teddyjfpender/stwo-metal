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

### TD-0011: The executable hybrid workload still begins after quotient accumulation

- Status: `active`
- Category: `workload handoff`
- Introduced: `2026-03-09`
- Owner area: `T5 workload handoff`

Why it exists now:

`stwo-metal` now has a declared hybrid workload boundary with explicit witness,
quotient, PCS, and FRI ownership, plus an explicit CPU-owned
wide-fibonacci witness handoff feeding the native Metal trace boundary. A
bounded native wide-fibonacci quotient primitive now exists for the benchmark
row, but the declared executable hybrid workload still begins only once a
CPU-owned quotient evaluation already exists.

Current containment:

- `crates/stwo-metal/src/backend/metal/workload.rs`
- `crates/stwo-metal/src/backend/metal/subpath.rs`
- `fixtures/standalone-benchmarks/src/bin/wide_fibonacci_prove.rs`

Risk if left in place:

The project could sound more workload-complete than it really is, even though
the declared executable hybrid path still begins after quotient accumulation.

Exit condition:

One declared workload owns a native quotient-accumulation boundary on top of
the witness-owned handoff, so the executable Metal workload no longer begins
after a precomputed CPU-owned quotient evaluation.

Target retirement point:

- `T5`

### TD-0012: The wide-fibonacci prove benchmark still bridges native Metal quotient output into the inherited CUDA-era proving lane

- Status: `active`
- Category: `benchmark execution boundary`
- Introduced: `2026-03-09`
- Owner area: `T5 benchmark-target alignment`

Why it exists now:

`stwo-metal` now declares the log-size-20 wide-fibonacci benchmark target with
an explicit `90 ms` RTX 4090 reference goal, and the standalone
`wide_fibonacci_trace` benchmark now enters through a native `.metal`
trace-generation path. `wide_fibonacci_prove` now generates its trace and
accumulates its quotient through native Metal paths, but it still bridges the
quotient output back into the inherited CUDA proving lane for pre-FRI PCS
commitment and the rest of the proving flow.

Current containment:

- `crates/stwo-metal/src/backend/metal/benchmark.rs`
- `fixtures/standalone-benchmarks/src/bin/wide_fibonacci_prove.rs`

Risk if left in place:

The project could overstate benchmark progress if the executable prove row
starts in Metal but still quietly depends on the inherited CUDA proving lane
after quotient accumulation.

Exit condition:

The executable `wide_fibonacci_prove` row no longer needs the Metal-to-CUDA
quotient-output bridge, and any remaining non-Metal stages are explicitly
named rather than implicitly inherited from CUDA.

Target retirement point:

- `T6`

### TD-0013: The target upstream example acceptance set is not yet vendored in the local snapshot

- Status: `retired`
- Category: `acceptance-input gap`
- Introduced: `2026-03-09`
- Owner area: `T5a planning correction`

Why it existed:

The project was rebaselined around proving upstream Stwo examples with
`MetalBackend` unchanged except for backend wiring, but the current local
vendored snapshot under `vendor/` did not expose the upstream
`crates/examples` tree directly. That meant the target acceptance set existed
as named scope, not yet as a fully local executable matrix.

Current containment:

- `docs/roadmap.md`
- `docs/program-plan.md`
- `docs/controller.md`

Resolution:

The upstream `stwo-examples` source is now pinned locally under
`vendor/stwo-upstream-dev-62b228e/crates/examples` with recorded source
provenance, so `T7` now has an auditable local input.

### TD-0014: The vendored upstream example crate is pinned from a separately recorded upstream commit and a locally adapted manifest

- Status: `active`
- Category: `acceptance-input pin`
- Introduced: `2026-03-09`
- Owner area: `T7 acceptance harness`

Why it exists now:

The vendored upstream example crate is now present locally, but it was copied
from a separately recorded upstream commit and given a local `Cargo.toml` so it
can build against the vendored `stwo` and `stwo-constraint-framework` crates in
this repository. The example workload logic is intended to remain upstream-owned,
but the pin still relies on that explicit local adaptation.

Current containment:

- `vendor/stwo-upstream-dev-62b228e/crates/examples/Cargo.toml`
- `vendor/stwo-upstream-dev-62b228e/crates/examples/STWO_UPSTREAM_SOURCE.md`

Risk if left in place:

The acceptance surface could drift subtly from the exact upstream example crate
shape if the local manifest adaptation or source pin is not maintained
carefully.

Exit condition:

The example crate is pinned in a way that no longer requires a separate local
manifest adaptation, or the project deliberately adopts and documents that
adapted vendoring model as stable process.

Target retirement point:

- `T7`

### TD-0015: The first upstream-example prove/verify boundary still depends on an explicit CPU prover bridge

- Status: `active`
- Category: `backend-completion gap`
- Introduced: `2026-03-09`
- Owner area: `T7 example proving`

Why it exists now:

The unchanged vendored upstream `wide_fibonacci` example now proves and
verifies through a real acceptance fixture, but the proving path still crosses
an explicit CPU bridge after native Metal trace generation. `MetalBackend` now
implements `PolyOps`, `AccumulationOps`, and `QuotientOps` through explicit CPU
bridges, and it now implements `FriOps` through an explicit bridge-backed FRI
trait slice. Direct backend substitution still lacks `GkrOps` and the
`BackendForChannel` surfaces. The current bridge therefore remains broader than
the eventual supported boundary even though most of the generic backend gap is
now retired.

Current containment:

- `crates/stwo-metal/src/backend/metal/witness.rs`
- `crates/stwo-metal/src/backend/metal/poly.rs`
- `crates/stwo-metal/src/backend/metal/accumulation.rs`
- `crates/stwo-metal/src/backend/metal/quotient.rs`
- `crates/stwo-metal/src/backend/metal/fri.rs`
- `fixtures/upstream-example-acceptance/src/lib.rs`
- `fixtures/upstream-example-acceptance/tests/wide_fibonacci_prove_verify.rs`

Risk if left in place:

The project could overclaim example-backed proving support while the critical
prove path still depends on CPU backend substitution. It also limits how much
performance signal the acceptance harness can provide for the eventual
log-size-20 wide-fibonacci target.

Exit condition:

At least one accepted upstream example proves and verifies through a direct
`MetalBackend` path, with no explicit CPU prover bridge required after native
Metal trace generation and with the remaining shared backend traits satisfied
directly enough to make backend substitution truthful.

Target retirement point:

- `T7`
