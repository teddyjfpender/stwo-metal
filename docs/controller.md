# Controller

## Purpose

This is the active control document for `stwo-metal`.

Inputs:

- current repository state
- active blockers
- long-range sequencing from [`roadmap.md`](./roadmap.md)
- program milestones from [`program-plan.md`](./program-plan.md)

Outputs:

- one active objective
- one active tranche
- explicit blockers
- next three deliverables

Invariants:

- the public repository identity is `stwo-metal`
- backend changes must preserve proving semantics unless an approved design note
  says otherwise
- no hidden expansion of scope while the backend boundary is being cleaned up

## Current operating state

- Date opened: `2026-03-09`
- Status: `in_progress`
- Active tranche: `T5 bounded proving-surface bring-up`
- Objective:
  grow the first truthful Metal-backed proving surface by porting bounded Stwo
  primitives with deterministic vendored CPU parity and explicit unsupported
  edges
- Active design note:
  [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)
- Current owner area:
  `bounded Metal runtime and proving-surface bring-up`

## Current blockers

- internal Rust vocabulary is still CUDA-first in many places
- the bounded FRI commitment slice now exists, but its last-layer
  interpolation still crosses an explicit CPU bridge rather than a native
  `stwo-metal` interpolation boundary
- the bounded full FRI proof candidate now exists, but it still depends on
  caller-supplied folding alphas rather than transcript-owned challenge flow
- the bounded full FRI proof candidate is not yet integrated into one declared
  Stwo proving sub-path
- the native commitment and decommit boundary is still host-owned and
  readback-based rather than a GPU-side hash pipeline
- interpolation, evaluation, and trace-generation primitives are still outside
  the Metal lane

## Next three deliverables

1. Freeze one declared Stwo proving sub-path that consumes the bounded full FRI
   proof candidate and names its remaining unsupported edges.
2. Decide whether T5 can exit with host-owned commitment hashing still in place
   or requires a GPU-side hash path before any bounded proving row is called
   truthful.
3. Keep the unsupported matrix, explicit CPU bridges, transcript gaps, and
   CPU-oracle parity coverage explicit as the path grows.

## Explicitly not doing now

- pretending the copied CUDA implementation is already a complete Metal backend
- widening the claimed Metal support faster than deterministic parity can cover
- renaming every inherited CUDA symbol before the replacement slices stabilize
- treating benchmark numbers as the first proof of correctness instead of
  deterministic parity against the vendored CPU path

## Update rule

Update this file whenever the active tranche, blockers, or next three
deliverables change.
