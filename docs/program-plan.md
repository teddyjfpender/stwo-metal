# Program Plan

## Purpose

This file records the intended route for the `stwo-metal` program at milestone
granularity.

The authoritative long-range map lives in [`roadmap.md`](./roadmap.md). This
file keeps the currently intended route aligned with that map.

## Program objective

Deliver a truthful Apple Silicon and Metal backend path for Stwo without
smuggling CUDA-era assumptions into the long-term interface or process.

## Program invariants

- the smallest stable public API wins
- core proving logic stays deterministic
- host-safe development must remain possible on machines without a working GPU
  backend
- native runtime ownership, ABI, and memory rules must be explicit before broad
  implementation work
- deterministic validation against the local vendored Stwo CPU execution is the
  default correctness oracle for Metal work
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
| T5 | Prove one bounded Stwo trace path through Metal | `in_progress` | one declared trace or proving sub-path runs correctly on the Metal path |
| T6 | Restore one truthful end-to-end supported workload | `planned` | one declared workload runs end to end on the Metal path with matching semantics |

## Immediate sequencing rules

- do not start `T3` implementation work before `T1` and `T2` are approved
- do not claim a supported Metal row before `T6`
- benchmark work is secondary until one bounded Metal path is correct
- use the local vendored Stwo snapshot as the reference semantic authority
- require deterministic unit tests against vendored CPU execution for bounded
  Metal cuts

## Current focus

The active tranche is `T5 bounded proving-surface bring-up`, as tracked in
[`controller.md`](./controller.md) and sequenced by [`roadmap.md`](./roadmap.md).

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
- explicit CPU bridge retained as a bounded validation path for the same
  boundary

The first declared T5 proving sub-path candidate is:

- FRI first-layer fold from a bit-reversed secure circle evaluation into the
  first line layer

The next required T5 boundary is:

- one earlier witness-owned workload handoff for `fibonacci_example` before
  the current CPU quotient evaluation boundary
