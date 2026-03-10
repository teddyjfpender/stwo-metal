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
| T6 | Restore one truthful end-to-end supported workload | `completed` | one declared workload runs end to end on the Metal path with matching semantics |
| T7 | Prove upstream Stwo examples with `MetalBackend` unchanged except for backend wiring | `completed` | the accepted upstream example set proves and verifies through `MetalBackend` |
| T8 | Mirror and port the native CUDA hot path into Metal for benchmark-grade performance work | `in_progress` | the selected native hot-path files exist under `metal/`, are status-tracked, and are being ported in the declared order |

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

The active tranche is `T8 tenth implementation slice: the wide-fibonacci
benchmark boundary is now closed through MetalBackend, so the next work is
measured optimization of the dominant prove stages and retirement of the next
explicit CPU bridge`, as tracked in
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
  `MetalBackend` now implements `MleOps` natively and `GkrOps` through native
  multilinear plus bounded lookup-oracle kernels, with only bounded host-side
  polynomial reconstruction left after the native oracle sums
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

The first active T8 supporting slices are:

- the native port now returns to the copied `stwo-metal-sys/cuda` subsystem as
  the primary performance source of truth for native boundary shape
- the active mirrored hot-path set is explicitly declared:
  `fields`, `twiddles`, `rfft`, `ifft`, `poly_utils`, `quotients`,
  `fold_circle_into_line`, `fold_line`, `prefix_sum`, `mle`, and `gkr`
- the declared port order now follows the benchmark-critical proving path from
  field storage and twiddles through FFT/poly, quotient, fold, and lookup/GKR
  support
- the first T8 implementation slice is to land the structural mirror under
  `crates/stwo-metal-sys/metal` with explicit implementation-status marking
  before claiming new native support
- `fields.metal` now contains the first compile-active reusable M31 kernel for
  native inversion
- `twiddles.metal` now contains the first compile-active native twiddle level
  generator
- `MetalBackend::precompute_twiddles` now retires the host twiddle bridge and
  validates native twiddle and inverse-twiddle output directly against the
  vendored CPU oracle
- `poly_utils.metal` now carries the first compile-active shared FFT helper
  kernel for native rescaling
- `rfft.metal` now carries the first compile-active native circle-evaluation
  core for `MetalBackend::evaluate_into`
- `ifft.metal` now carries the first compile-active native interpolation core
  for `MetalBackend::interpolate`
- `quotients.metal` now carries the compile-active mirrored quotient kernel
  previously isolated in the non-mirrored Metal proving lane
- `fold_circle_into_line.metal` now carries the compile-active mirrored first
  FRI fold kernel previously isolated in the non-mirrored Metal proving lane
- `fold_line.metal` now carries the compile-active mirrored line-fold kernel
  previously isolated in the non-mirrored Metal proving lane
- `mle.metal` now carries compile-active native first-variable fixing for both
  base-field and secure-field multilinear evaluations
- the explicit `MleOps` CPU bridge is now retired, with deterministic parity
  over both edge-size and normal-size multilinear evaluations
- `gkr.metal` now carries compile-active native eq-eval generation, next-layer
  construction, and bounded oracle-sum evaluation for `GrandProduct`,
  `LogUpGeneric`, `LogUpMultiplicities`, and `LogUpSingles`
- the previous `GkrOps` oracle CPU bridge is retired; the remaining host work
  is bounded polynomial reconstruction from native `eval_at_0` and
  `eval_at_2`
- `PolyOps::extend` and `split_at_mid` now avoid the vendored CPU backend and
  stay inside Metal-owned base-field storage
- `PolyOps` point evaluation and barycentric helpers now avoid the vendored
  CPU backend too, leaving only the bounded small-domain
  evaluate/interpolate fallback as the remaining explicit `PolyOps` CPU path
- `FriOps::decompose` now avoids the vendored CPU backend and stays inside
  Metal-owned secure-column storage
- the wider `FriOps` secure-column repacking plus host-side fold accumulation
  path now also avoids the vendored CPU backend, so the legacy explicit
  `FriOps` bridge is retired
- the first Apple Silicon benchmark activation run now exists for the native
  trace row: `wide_fibonacci_trace_generation_v1` completed in `66.61 ms` at
  `log_n_instances = 20`, `n_columns = 100`, `STWO_METAL_MODE=metal-dev`,
  `warmups = 0`, and `samples = 1`
- `prefix_sum.metal` now carries compile-active native inclusive prefix-sum
  support for bit-reversed circle-domain base-field columns
- `PORTING_STATUS.md` no longer has any scaffold-only mirrored hot-path files
- the lifted Blake2s Merkle and proof-of-work boundaries now avoid the
  vendored CPU backend and run as direct host-side Blake2s work over
  Metal-owned proving surfaces
- the standalone `wide_fibonacci_prove` benchmark row now executes end to end
  through `MetalBackend` and verifies successfully
- the first Apple Silicon end-to-end benchmark result is now recorded:
  `wide_fibonacci_prove_verify_v1 = 213731.915833 ms`, with
  `prove_ms = 213726.729125` and `verify_ms = 5.186708`
- the dominant prove-stage costs in that row are now measured explicitly:
  `prove_core_prove_values_ms = 100717.446`,
  `trace_commit_merkle_ms = 73443.648458`, and
  `prove_core_composition_commit_ms = 20958.021458`

The next required T8 boundary is:

- keep the completed mirrored hot-path set explicit and parity-tested while it
  carries benchmark-active work
- reduce the dominant measured prove stages in the end-to-end
  `wide_fibonacci_prove_verify_v1` row
- decide whether the next benchmark-facing structural win is a native lifted
  commitment/hash pipeline or retirement of the explicit `AccumulationOps` /
  `QuotientOps` CPU bridges
- keep the adapter-retirement debt explicit while the project focuses on
  native performance work
