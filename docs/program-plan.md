# Program Plan

## Purpose

This file records the intended route for the `stwo-metal` program at milestone
granularity.

The authoritative long-range map lives in [`roadmap.md`](./roadmap.md). This
file keeps the currently intended route aligned with that map.

## Program objective

Deliver a truthful Apple Silicon and Metal backend path for Stwo without
smuggling CUDA-era assumptions into the long-term interface or process.

The primary deliverable is to prove upstream Stwo examples with
`MetalBackend` and verify them unchanged except for backend wiring.

The first benchmark north star for that path remains the wide-fibonacci proving
row at `log_n_instances = 20` and `n_columns = 100`, aiming to approach the
project-supplied `90 ms` RTX 4090 reference result once the Metal workload is
truthful enough to measure, but that benchmark is a supporting objective rather
than the architectural source of truth.

## Program invariants

- the smallest stable public API wins
- core proving logic stays deterministic
- host-safe development must remain possible on machines without a working GPU
  backend
- native runtime ownership, ABI, and memory rules must be explicit before broad
  implementation work
- deterministic validation against the local vendored Stwo CPU execution is the
  default correctness oracle for Metal work
- upstream example workloads are acceptance fixtures, not rewrite targets
- every temporary bridge must be logged in
  [`tech-debt-register.md`](./tech-debt-register.md)

## Milestone queue

| Order | Milestone | Status | Exit condition |
| --- | --- | --- | --- |
| T0 | Reset repository identity and process docs | `completed` | `stwo-metal` is isolated and the docs set is clean |
| T1 | Freeze backend-neutral Rust boundary and architecture direction | `completed` | roadmap is approved, the default stack direction is explicit, and the public boundary to preserve is named |
| T2 | Define Apple Silicon host contract | `completed` | host modes, toolchain assumptions, and fail-safe behavior are documented |
| T3 | Design `stwo-metal-sys` runtime replacement | `completed` | native build, ABI, queue, and memory ownership are approved |
| T4 | Land first Metal-backed primitive path | `completed` | at least one bounded Metal execution path exists with deterministic validation |
| T5 | Prove one bounded Stwo trace path through Metal | `completed` | one declared trace or proving sub-path runs correctly on the Metal path |
| T5a | Rebaseline around generic backend completion and unchanged upstream examples | `completed` | planning documents and done criteria are corrected to the backend-first goal |
| T6 | Restore one truthful end-to-end supported workload | `planned` | one declared workload runs end to end on the Metal path with matching semantics |
| T7 | Prove upstream Stwo examples with `MetalBackend` unchanged except for backend wiring | `in_progress` | the accepted upstream example set proves and verifies through `MetalBackend` |

## Immediate sequencing rules

- do not start `T3` implementation work before `T1` and `T2` are approved
- do not claim a supported Metal row before `T6`
- benchmark work is secondary until backend-completion acceptance criteria are
  explicit
- use the local vendored Stwo snapshot as the reference semantic authority
- require deterministic unit tests against vendored CPU execution for bounded
  Metal cuts
- do not widen benchmark-specific proving rows while the example-backed
  backend-completion milestone is still being defined

## Current focus

The active tranche is `T7 tenth implementation slice: close lookup-heavy and
mixed-component upstream example rows`, as tracked in
[`controller.md`](./controller.md) and sequenced by
[`roadmap.md`](./roadmap.md).

The active formal basis for T2 and T3 is:

- [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)

The first completed T4 slice is:

- native Metal `BaseField` bit reversal with deterministic CPU-oracle parity
  tests on Apple Silicon

Current completed T5 supporting slices are:

- native Metal `SecureField` column round-trip and bit reversal
- native Metal `BaseField` coset-order to circle-domain bit-reversed
  permutation
- native Metal FRI first-layer fold from secure circle evaluation into the
  first line layer
- native Metal FRI line fold with repeated host-orchestrated fold steps and
  deterministic vendored CPU parity
- native `MetalLineEvaluation` and first inner-layer commitment root parity
- native first inner-layer query and decommit parity
- bounded native first inner-layer proof row
- bounded native inner-layer FRI sequence
- bounded FRI commitment slice with explicit last-layer degree-bound truncation
- bounded proof-facing inner FRI proof slice
- bounded first-layer circle commitment and decommit boundary
- bounded full FRI proof candidate
- bounded transcript-owned FRI prover
- declared bounded Blake2s FRI proving sub-path
- manifest-driven workload planner for exemplar Stwo workloads
- declared hybrid workload boundary with explicit witness, quotient, PCS, and
  Metal FRI ownership
- executable CPU-owned FRI-ready evaluation handoff into the declared hybrid
  workload boundary
- executable CPU-owned quotient evaluation handoff into the declared hybrid
  workload boundary
- declared `wide_fibonacci` benchmark target for the log-size-20 proving row
- bounded native `.metal` wide-fibonacci trace generation with deterministic
  CPU-recurrence parity
- standalone `wide_fibonacci_trace` benchmark retargeted to the native Metal
  trace path
- `wide_fibonacci_prove` trace generation retargeted to the native Metal trace
  path with an explicit bridge back into the inherited CUDA proving lane
- workload-level CPU-owned wide-fibonacci witness handoff feeding the native
  Metal trace boundary before quotient accumulation
- bounded native `.metal` wide-fibonacci quotient accumulation with
  deterministic CPU-oracle parity
- `wide_fibonacci_prove` quotient accumulation retargeted to the native Metal
  path with an explicit quotient-output bridge back into the inherited CUDA
  proving lane
- explicit CPU bridge retained as a bounded validation path for the same
  boundary

The new planning correction outputs are:

- the backend-first project definition is explicit in the control docs
- a formal milestone exists for proving unchanged upstream examples with
  `MetalBackend`
- an acceptance matrix exists for the target upstream example set:
  `blake`, `poseidon`, `state_machine`, `wide_fibonacci`, and `xor`
- benchmark rows remain in the plan as secondary validation and performance
  surfaces

The first completed T7 supporting slices are:

- vendored upstream `stwo-examples` source pinned locally with recorded source
  provenance
- isolated acceptance fixture crate for upstream-example backend wiring
- first unchanged upstream `wide_fibonacci` example wired into the native Metal
  trace boundary with deterministic parity against the vendored example trace
- first unchanged upstream `wide_fibonacci` prove/verify boundary established
  through native Metal trace generation plus an explicit CPU prover/verifier
  bridge
- reusable single-trace Blake2s acceptance helper extracted so future example
  rows do not need bespoke test-local CPU-bridge proving code
- first direct backend-completion bridge tranche landed:
  `MetalBackend` now implements `PolyOps`, `AccumulationOps`, and
  `QuotientOps` through explicit CPU bridges over Metal-owned columns,
  evaluations, and secure-column storage
- deterministic parity tests now lock those `PolyOps` and PCS bridge surfaces
  against the vendored CPU backend
- second direct backend-completion bridge tranche landed:
  `MetalBackend` now implements `FriOps` through an explicit CPU bridge that
  repacks Metal-owned secure columns into the bounded Metal fold kernels and
  keeps `decompose` on the vendored CPU backend
- deterministic parity tests now lock the `FriOps` trait surface itself, not
  just the earlier bounded free-function FRI helpers
- lookup bridge tranche landed:
  `MetalBackend` now implements `MleOps` and `GkrOps` through explicit CPU
  bridges over Metal-owned multilinear and lookup-layer storage
- `MetalBackend` now explicitly implements the Stwo `Backend` trait
- Blake2s channel tranche landed:
  `MetalBackend` now implements the Blake2s `BackendForChannel` surface through
  explicit CPU-bridge `ColumnOps<Blake2sHash>`, lifted Merkle, and proof-of-work
  boundaries
- compile assertions and parity tests now cover both `Backend` and Blake2s
  `BackendForChannel` support
- the first unchanged upstream `wide_fibonacci` example now proves and verifies
  through `MetalBackend` with the stock prover and verifier
- the explicit outer CPU prove bridge has been retired from that acceptance
  row
- the remaining framework-component bridge is now localized to an
  acceptance-only adapter that keeps workload logic unchanged and names the
  CPU-domain bridge explicitly
- the unchanged upstream `state_machine` example now also proves and verifies
  through `MetalBackend`, covering a multi-tree and multi-component row
- the unchanged upstream `blake` example now also proves and verifies through
  `MetalBackend`, covering a lookup-heavy row with vendored setup replay for
  statement mixing and interaction-element transcript flow
- the unchanged upstream `xor` MLE-eval row now also proves and verifies
  through `MetalBackend`, covering a mixed-component path with one
  framework-backed component and one non-framework prover component
- the acceptance harness now contains both a framework-component adapter and a
  generic SIMD-component adapter, each kept explicit and local to the
  acceptance layer

The next required T7 boundary is:

- decide whether the acceptance-local adapters should remain local, move into a
  shared non-public helper boundary, or be superseded by an upstream-facing
  refactor
- record T7 honestly as complete for all named non-blocked rows in the current
  vendored snapshot
- keep `poseidon` explicitly classified as an upstream protocol blocker in the
  current vendored snapshot rather than a pending Metal-backend slice
- keep the next blocker explicit:
  vendored upstream `FrameworkComponent` still only implements
  `ComponentProver` for `CpuBackend` and `SimdBackend`, so the current
  adapter remains a named CPU-domain bridge
- keep any vendored protocol limitations, such as unsupported AIR degree
  shapes, separated from true Metal-backend gaps
