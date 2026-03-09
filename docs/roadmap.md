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

The repository should become a real Metal backend, not a renamed CUDA fork and
not an unbounded GPU-experiment sandbox.

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

## Milestone map

| Order | Milestone | Status | Exit condition |
| --- | --- | --- | --- |
| T0 | Reset repository identity and process docs | `completed` | `stwo-metal` is isolated and the docs set is clean |
| T1 | Freeze the backend-neutral Rust boundary and architecture direction | `completed` | roadmap is approved, default stack is explicit, and the public boundary to preserve is named |
| T2 | Define the Apple Silicon host contract | `completed` | supported host modes, toolchain assumptions, and fail-safe behavior are approved |
| T3 | Design the native `stwo-metal-sys` Metal runtime | `completed` | device, queue, memory, ABI, and build ownership are approved in a design note |
| T4 | Land the first bounded Metal primitive path | `completed` | one reusable GPU-backed primitive exists with deterministic CPU-oracle validation |
| T5 | Prove one bounded Stwo trace path through Metal | `in_progress` | one declared trace or proving sub-path runs correctly on the Metal backend |
| T6 | Restore one truthful end-to-end supported workload | `planned` | one declared workload proves end to end on Metal with matching semantics and declared measurement |

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
- the explicit CPU bridge remains available only as a bounded validation and
  comparison surface
- the next T5 work is to freeze one executable handoff from a CPU-owned
  workload artifact into that declared hybrid workload boundary, starting with
  a FRI-ready evaluation input rather than pretending trace or quotient
  ownership already moved

### T6: Restore one truthful end-to-end supported workload

Required outputs:

- one named workload is support-honest
- correctness is demonstrated before throughput claims
- performance reporting uses a declared workload and environment

## Sequencing rules

- Do not widen the public API before T1 is settled.
- Do not implement the native runtime before T2 and T3 are approved.
- Do not optimize for speed before T5 correctness exists.
- Do not let benchmark harnesses become the primary debugging surface.
- Do not accept hidden CPU fallbacks in the supported Metal path.
- Do not land new Metal execution paths without deterministic unit-test parity
  against the vendored CPU reference.

## Current next three planning deliverables

1. Freeze one executable workload handoff from a CPU-owned artifact into the
   declared hybrid workload boundary.
2. Decide whether quotient accumulation or PCS commitment is the next native
   proving-stage replacement after that handoff lands.
3. Decide whether T5 can exit with host-owned commitment hashing still in
   place or requires a GPU-side hash path before any bounded proving row is
   called truthful.
