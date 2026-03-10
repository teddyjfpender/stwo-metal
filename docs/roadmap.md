# Roadmap

## Purpose

This file is the long-range map for `stwo-metal`.

Inputs:

- the current `stwo-metal` repository state
- the local vendored Stwo snapshot under `vendor/`
- the working assumption that the target is a Rust frontend plus a Metal
  backend capable of proving Stwo traces quickly and correctly on Apple Silicon
- the upstream Stwo skill registry and relevant skills under
  `starkware-libs/stwo/.claude/skills`

Outputs:

- the architecture direction we are planning toward
- the milestones we will use to get there
- the rules for what we will and will not optimize early

## Design intent

The target shape is:

- Rust host orchestration
- native Metal runtime ownership on the host
- `.metal` compute kernels for the hot path
- correctness first, then speed
- unchanged Stwo workload logic except for backend wiring at the proving seam

The repository should become a real Metal backend, not a renamed CUDA fork and
not an unbounded GPU-experiment sandbox.

The primary product definition is:

- take a Stwo trace or example workload produced the normal Stwo way
- prove it with `MetalBackend`
- verify the proof with the standard verifier

The architecture driver is backend completeness, not custom benchmark rows.

The current native-port driver after example-backed acceptance is:

- mirror the relevant `stwo-metal-sys/cuda` structure into
  `stwo-metal-sys/metal`
- port hot-path native files in a documented sequence
- keep Metal file names and logical boundaries aligned with the CUDA source so
  parity, benchmarking, and review stay legible
- mark each mirrored Metal file as `scaffolded`, `parity-tested`, or
  `benchmark-active` rather than implying support just by file presence

## Benchmark north star

The first explicit benchmark objective is:

- `wide_fibonacci_prove_verify_v1`
- `log_n_instances = 20`
- `n_columns = 100`
- project-supplied reference goal: approach `90 ms`
- reference row source for that goal: RTX 4090 CUDA benchmark history

This benchmark target is a planning objective, not a correctness gate, not the
architecture source of truth, and not yet a support claim for `stwo-metal`.

## Example-backed acceptance focus

The primary acceptance direction is to prove upstream Stwo examples with
`MetalBackend` unchanged except for backend wiring.

Target acceptance set:

- `blake`
- `poseidon`
- `state_machine`
- `wide_fibonacci`
- `xor`

Current local constraint:

- the current vendored snapshot under `vendor/` does not yet expose the
  upstream `crates/examples` tree directly, so this acceptance set requires an
  explicit vendoring or import step before the matrix can be executed fully

Acceptance matrix:

| Example | Role | Current state | Exit signal |
| --- | --- | --- | --- |
| `blake` | acceptance workload | `complete` | proves and verifies through `MetalBackend` with unchanged workload logic |
| `poseidon` | acceptance workload | `blocked_upstream_protocol` | proves and verifies through `MetalBackend` with unchanged workload logic once the vendored lifted protocol supports the example’s AIR degree shape |
| `state_machine` | acceptance workload | `complete` | proves and verifies through `MetalBackend` with unchanged workload logic |
| `wide_fibonacci` | acceptance workload and perf reference | `complete` | proves and verifies through `MetalBackend`; benchmark remains secondary evidence |
| `xor` | acceptance workload | `complete` | proves and verifies through `MetalBackend` with unchanged workload logic |

## Planning assumptions

These are planning assumptions, not yet final implementation commitments:

1. The primary architecture hypothesis is Rust host orchestration plus native
   Metal and `.metal` kernels.
2. The host binding layer should stay close to native Metal semantics rather
   than hide them behind a large cross-platform runtime too early.
3. The local vendored Stwo snapshot is the semantic source of truth for proof
   behavior while the backend changes.
4. Deterministic unit tests against the local vendored Stwo CPU execution are
   the default correctness oracle for bounded Metal work.
5. Upstream examples are acceptance workloads, not workload-specific rewrite
   targets.

## Lessons applied from `stwo-cuda`

- Keep one active tranche at a time.
- Do not use benchmark runners as the primary diagnosis surface.
- Freeze interfaces before broad implementation.
- Keep native ownership, ABI, and memory rules explicit.
- Treat temporary bridges as debt with retirement points.
- Prefer the smallest correctness-preserving cut over wide speculative rewrites.

## Upstream skill alignment

`stwo-metal` adopts the upstream Stwo skill registry as a process input for
domain vocabulary, review discipline, and testing focus.

Minimum required alignment:

- use the upstream skill registry as the entry point for theory and review
  loading
- use the Rust codebase conventions skill when shaping Rust-side backend code
- use the testing strategy skill when defining test placement and coverage
- use the soundness review checklist for any soundness-critical change
- use the most specific Tier 1 mathematical skill before modifying
  theory-grounded components

For T2 and T3 specifically:

- host and runtime design should follow upstream Stwo terminology where it
  applies cleanly
- unit-test planning should stay aligned with the upstream testing strategy
- any future soundness-critical proving-path change must be reviewed against the
  upstream soundness checklist before approval

## Program invariants

- The public crate surface remains minimal and stable.
- Proof semantics do not change unless an approved design note says they do.
- Host-safe development must work without pretending the Metal backend already
  exists.
- We do not create a second semantic authority beside the local vendored Stwo
  snapshot.
- New Metal work must validate against the vendored Stwo CPU path before any
  performance claim matters.
- Performance work must follow a working correctness path.

## Architecture direction

### Primary path

Plan around:

- Rust frontend and orchestration
- native Metal device, queue, buffer, and pipeline management
- `.metal` kernels for hot proving operations

Why this is the default:

- it gives the clearest control over Apple GPU execution
- it keeps the runtime contract close to the actual platform
- it lets us write the real hot path directly instead of forcing a Rust-only
  GPU authoring model where that model is not buying us correctness or speed

Practical rule:

- if a hot path needs a Metal kernel, writing it in `.metal` is the default
  acceptable choice
- if a bounded Metal cut lacks a deterministic unit test against the vendored
  CPU path, it is not ready

## Native port roadmap

The native runtime work now returns to the copied CUDA subsystem directly.

Primary mirroring rule:

- for the active performance tranche, create and maintain matching Metal file
  names beside the CUDA source wherever a native Metal replacement is intended
- preserve conceptual module boundaries and naming even if the Metal
  implementation differs internally
- do not mark a mirrored file as complete until it has deterministic parity
  evidence against the vendored CPU path and, where relevant, benchmark
  evidence against the current CUDA-backed historical reference

Active mirrored hot-path set:

- `fields`
- `twiddles`
- `rfft`
- `ifft`
- `poly_utils`
- `quotients`
- `fold_circle_into_line`
- `fold_line`
- `prefix_sum`
- `mle`
- `gkr`

Port order for the next native tranche:

1. `fields`
2. `twiddles`
3. `rfft`
4. `ifft`
5. `poly_utils`
6. `quotients`
7. `fold_circle_into_line`
8. `fold_line`
9. `prefix_sum`
10. `mle`
11. `gkr`

Why this order:

- it follows the benchmark-critical proving path from field storage and domain
  material through FFT/poly machinery into quotient and fold operations, then
  into lookup-heavy proving support
- it keeps the earliest performance work tied to the widest reusable native
  surfaces rather than to workload-specific kernels
- it mirrors the CUDA subsystem in the same order a reviewer will use to check
  semantic equivalence

## Milestone map

| Order | Milestone | Status | Exit condition |
| --- | --- | --- | --- |
| T0 | Reset repository identity and process docs | `completed` | `stwo-metal` is isolated and the docs set is clean |
| T1 | Freeze the backend-neutral Rust boundary and architecture direction | `completed` | roadmap is approved, default stack is explicit, and the public boundary to preserve is named |
| T2 | Define the Apple Silicon host contract | `completed` | supported host modes, toolchain assumptions, and fail-safe behavior are approved |
| T3 | Design the native `stwo-metal-sys` Metal runtime | `completed` | device, queue, memory, ABI, and build ownership are approved in a design note |
| T4 | Land the first bounded Metal primitive path | `completed` | one reusable GPU-backed primitive exists with deterministic CPU-oracle validation |
| T5 | Prove one bounded Stwo trace path through Metal | `completed` | one declared trace or proving sub-path runs correctly on the Metal backend |
| T5a | Rebaseline around generic backend completion and unchanged upstream examples | `completed` | roadmap, controller, plan, and done criteria treat upstream example proving as the primary deliverable |
| T6 | Restore one truthful end-to-end supported workload | `planned` | one declared workload proves end to end on Metal with matching semantics and declared measurement |
| T7 | Prove upstream Stwo examples with `MetalBackend` unchanged except for backend wiring | `in_progress` | the accepted upstream example set proves and verifies through `MetalBackend` without workload-specific rewrites |
| T8 | Mirror and port the native CUDA hot path into Metal for benchmark-grade performance work | `in_progress` | the selected native `cuda/` hot-path files exist under `metal/` with tracked status, deterministic parity retirement criteria, and implementation work advancing in the declared order |

## Milestone detail

### T1: Freeze the boundary and architecture direction

Required outputs:

- approved roadmap
- decision on the default Metal stack direction
- named public API and boundary that should survive backend replacement
- initial debt register for copied CUDA residue
- validation rule that new Metal work must compare against the vendored CPU
  reference

Explicitly not part of T1:

- kernel translation
- benchmark chasing
- dependency scouting for alternative GPU stacks

### T2: Define the Apple Silicon host contract

Required outputs:

- supported host modes for development, compile-only, and Metal-enabled runs
- truthful behavior on machines with no Metal-capable execution path
- toolchain assumptions and environment variables
- first-success definition for the Metal lane
- test-oracle rules for comparing Metal results with vendored CPU execution

### T3: Design the native Metal runtime

Required outputs:

- host binding choice
- runtime ownership model for devices, queues, buffers, and pipeline state
- ABI surface between Rust and kernels
- memory-layout and synchronization rules
- failure behavior and validation plan

### T4: Land the first bounded Metal primitive path

Selection rule:

- choose the narrowest primitive that is reusable and has a clear CPU oracle
- prefer a primitive that exercises the real Metal runtime boundary without
  forcing the whole proving path to exist first

Candidate classes:

- buffer and column transfer primitives
- simple field-vector operations
- one small polynomial or trace-support primitive

Current completion evidence:

- native Metal build and runtime scaffolding exists in `stwo-metal-sys`
- `.metal` compilation and embedded `.metallib` loading are live on Apple
  Silicon
- `BaseField` bit reversal is implemented through a native Metal kernel
- deterministic parity tests compare the Metal result against the vendored CPU
  oracle

### T5: Prove one bounded Stwo trace path through Metal

Required outputs:

- one declared trace or proving sub-path uses the Metal backend
- deterministic parity checks against the local vendored CPU reference
- explicit unsupported behavior outside the bounded path

Current next slices inside T5:

- `SecureField` column operations now exist beside the `BaseField` Metal lane
- one bounded poly support primitive now exists:
  coset-order to circle-domain bit-reversed `BaseField` permutation
- the first declared T5 proving sub-path candidate is:
  FRI first-layer fold from a bit-reversed secure circle evaluation into the
  first line layer
- the bounded `fold_circle_into_line` first-layer primitive now exists with
  deterministic vendored CPU parity
- the bounded `fold_line` primitive now exists with deterministic vendored CPU
  parity for repeated host-orchestrated folds
- the first inner FRI-layer native line-evaluation and Merkle commitment
  boundary now exists with deterministic vendored CPU parity
- the first inner FRI-layer native query and decommit boundary now exists with
  deterministic vendored CPU parity
- the first inner FRI layer is now packaged as a bounded native proof-facing
  row with stable root and decommit semantics
- a bounded native inner-layer FRI sequence now exists with deterministic
  vendored CPU parity
- a bounded FRI commitment slice now exists with deterministic vendored CPU
  parity for the last-layer polynomial under the configured degree bound
- a bounded proof-facing inner FRI proof slice now exists on top of the
  commitment slice without implying first-layer support
- a bounded first-layer circle commitment and decommit boundary now exists with
  deterministic vendored CPU parity
- a bounded full FRI proof candidate now exists by composing the native
  first-layer proof boundary with the inner proof slice
- a bounded transcript-owned Metal FRI prover now exists with vendored channel
  ordering and deterministic CPU parity
- one declared bounded Blake2s FRI proving sub-path now exists on top of that
  transcript-owned prover
- a manifest-driven workload planner now classifies exemplar Stwo workloads as
  `CpuOnly`, `MetalFriHybrid`, or `MetalFull`
- one declared hybrid workload boundary now exists for the exemplar workload
  set, with explicit ownership for witness, quotient, PCS, and Metal FRI
  stages
- one executable handoff now exists from a CPU-owned FRI-ready evaluation into
  the declared hybrid workload boundary, with deterministic CPU-oracle parity
- one executable handoff now exists from a CPU-owned quotient evaluation into
  the declared hybrid workload boundary, with deterministic CPU-oracle parity
- the `wide_fibonacci` benchmark target is now declared formally at
  `log_n_instances = 20` and `n_columns = 100`, with a project-supplied
  `90 ms` RTX 4090 reference goal
- a bounded native `.metal` wide-fibonacci trace-generation path now exists
  with deterministic CPU-recurrence parity
- the standalone `wide_fibonacci_trace` benchmark fixture now enters through
  the native Metal trace path rather than the inherited CUDA witness path
- the `wide_fibonacci_prove` benchmark now uses that native Metal trace path
  for its trace-generation phase before an explicit bridge back into the
  inherited CUDA proving lane
- one explicit CPU-owned wide-fibonacci witness handoff now exists for
  `fibonacci_example`, feeding the native Metal trace boundary before quotient
  accumulation
- one bounded native `.metal` wide-fibonacci quotient-accumulation primitive
  now exists with deterministic CPU-oracle parity
- the `wide_fibonacci_prove` benchmark now uses that native Metal quotient
  path before an explicit bridge back into the inherited CUDA proving lane
- the explicit CPU bridge remains available only as a bounded validation and
  comparison surface

T5 is now treated as completed bounded proving-surface groundwork. The next
work is no longer to keep extending benchmark-specific rows by default; it is
to re-anchor the program around generic backend completion and unchanged
upstream example proving.

### T5a: Rebaseline around generic backend completion and unchanged upstream examples

Required outputs:

- controller, roadmap, program plan, and done criteria all name generic Stwo
  proving with `MetalBackend` as the primary deliverable
- benchmark rows are explicitly demoted to supporting validation and
  performance surfaces
- one formal milestone exists for proving upstream Stwo examples unchanged
  except for backend wiring
- an explicit acceptance matrix exists for the target upstream example set
- further bespoke benchmark-path expansion is frozen until this correction is
  written down

### T6: Restore one truthful end-to-end supported workload

Required outputs:

- one named workload is support-honest
- correctness is demonstrated before throughput claims
- performance reporting uses a declared workload and environment

### T7: Prove upstream Stwo examples with `MetalBackend` unchanged except for backend wiring

Required outputs:

- the accepted upstream example set is available in the repo or otherwise
  pinned as an auditable input
- each example can be proved with `MetalBackend` and verified with the stock
  verifier
- workload logic remains upstream-owned; backend wiring is the only intended
  delta
- failures are tracked per example as backend-completion gaps rather than
  patched through workload-specific rewrites

Current first implementation slice:

- the upstream `stwo-examples` source is now pinned locally under the vendored
  snapshot with recorded source provenance
- one isolated acceptance fixture now consumes the vendored upstream
  `wide_fibonacci` example unchanged except for backend wiring
- that first acceptance fixture proves the example can feed the current native
  Metal trace boundary without passing through a bespoke benchmark harness
- a second acceptance fixture now proves and verifies the unchanged vendored
  `wide_fibonacci` component by bridging the native Metal trace into the stock
  CPU prover and verifier
  this is explicit bridge-backed execution, not yet direct `MetalBackend`
  substitution
- the single-trace Blake2s acceptance harness is now factored so future
  example-backed CPU-bridge prove/verify rows do not require bespoke test-local
  proving code
- a follow-on acceptance slice now proves and verifies the unchanged vendored
  `wide_fibonacci` component through direct `MetalBackend` substitution with
  the stock prover and verifier
- the remaining framework-component bridge is localized to an acceptance-only
  adapter rather than the earlier outer CPU prove helper
- a further acceptance slice now proves and verifies the unchanged vendored
  `state_machine` example through direct `MetalBackend` substitution, covering
  a multi-tree and multi-component proving row
- a further acceptance slice now proves and verifies the unchanged vendored
  `blake` example through direct `MetalBackend` substitution, covering a
  lookup-heavy row with vendored setup replay for statement mixing and
  interaction-element transcript flow
- a further acceptance slice now proves and verifies the unchanged vendored
  `xor` MLE-eval row through direct `MetalBackend` substitution, covering a
  mixed-component path with one framework-backed component and one
  non-framework prover component

Current next slice inside T7:

- the first backend-completion bridge tranche is now landed:
  `MetalBackend` implements `PolyOps`, `AccumulationOps`, and `QuotientOps`
  through explicit CPU bridges over Metal-owned columns and evaluations
- the next backend-completion bridge tranche is now landed too:
  `MetalBackend` implements `FriOps` through an explicit CPU bridge that
  repacks Metal-owned secure columns into the bounded Metal fold kernels and
  keeps `decompose` on the vendored CPU backend
- the lookup bridge tranche is now landed:
  `MetalBackend` implements `MleOps` and `GkrOps` through explicit CPU bridges
  over Metal-owned multilinear and lookup-layer storage
- the Blake2s channel bridge tranche is now landed:
  `MetalBackend` implements the Blake2s `BackendForChannel` surface through
  explicit CPU-bridge Merkle and proof-of-work boundaries
- those slices are accepted because they shrink the generic backend gap
  without pretending the remaining prover traits are native Metal yet
- `MetalBackend` now satisfies the generic Stwo `Backend` trait and the
  Blake2s `BackendForChannel` surface
- the next honest blocker is no longer adding another named example row; it is
  deciding what to do with the acceptance-local framework and SIMD-component
  adapters now that all named non-blocked rows are covered
- `poseidon` is not the immediate next row because the vendored upstream
  snapshot already marks its lifted proving path unsupported for the current
  AIR degree shape
- only after those bridge-retirement slices meaningfully widen shared proving
  support should T7 be marked complete-for-current-vendor or widened to a new
  accepted example set

### T8: Mirror and port the native CUDA hot path into Metal for benchmark-grade performance work

Required outputs:

- the active CUDA hot-path files have matching names under
  `crates/stwo-metal-sys/metal`
- each mirrored Metal file has an explicit implementation status
- the port order is declared and tracked rather than inferred from whichever
  file was touched most recently
- parity and benchmark retirement criteria are explicit before each native file
  graduates from scaffold to support claim

Current first implementation slice:

- add the structural mirror for the first hot-path set under
  `crates/stwo-metal-sys/metal`
- keep the current compile-active Metal set explicit
- begin file-by-file implementation in the declared order starting at
  `fields`, then `twiddles`, then FFT/poly support

Current next slice inside T8:

- keep the mirrored native subsystem in place and explicit
- treat `fields.metal` and `twiddles.metal` as compile-active,
  parity-tested replacements
- move directly into `rfft`, `ifft`, and `poly_utils` as the next FFT/poly
  tranche on top of the native twiddle boundary

## Sequencing rules

- Do not widen the public API before T1 is settled.
- Do not implement the native runtime before T2 and T3 are approved.
- Do not optimize for speed before T5 correctness exists.
- Do not let benchmark harnesses become the primary debugging surface.
- Do not accept hidden CPU fallbacks in the supported Metal path.
- Do not land new Metal execution paths without deterministic unit-test parity
  against the vendored CPU reference.

## Current next three planning deliverables

1. Land the native `rfft` slice on top of parity-tested Metal twiddles.
2. Land the native `ifft` slice with deterministic parity against the vendored
   CPU backend.
3. Move the FFT/poly tranche into `poly_utils` before widening quotient and
   fold performance work.
