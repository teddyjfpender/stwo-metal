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
- Active tranche: `T7 first implementation slice: vendored examples and wide-fibonacci trace wiring`
- Objective:
  execute the first example-backed backend-completion slice by pinning the
  upstream example set locally and wiring one unchanged example into the
  native Metal path without another bespoke benchmark seam
- Active design note:
  [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)
- Current owner area:
  `backend-completion planning and example-backed acceptance`

## Current blockers

- the project goal had drifted toward benchmark-specific proving rows instead
  of a generic Stwo proving backend
- `MetalBackend` still does not implement Stwo's full `Backend` contract, so
  the first T7 slice can wire unchanged examples into current Metal surfaces
  but cannot yet prove them end to end through backend substitution alone
- internal Rust vocabulary is still CUDA-first in many places
- the bounded FRI commitment slice now exists, but its last-layer
  interpolation still crosses an explicit CPU bridge rather than a native
  `stwo-metal` interpolation boundary
- only the first upstream example wiring slice exists so far:
  `wide_fibonacci` now feeds the native Metal trace boundary through an
  acceptance fixture, but the prove/verify path is not yet backend-complete
- the declared `wide_fibonacci` benchmark target remains useful for
  performance, but it is not the architectural source of truth and must stop
  driving milestone sequencing
- the native commitment and decommit boundary is still host-owned and
  readback-based rather than a GPU-side hash pipeline
- interpolation, evaluation, and trace-support primitives needed for a generic
  backend are still incomplete outside the currently bounded Metal slices

## Next three deliverables

1. Define the first honest prove/verify boundary for one vendored upstream
   example, starting from the unchanged example component rather than a custom
   benchmark harness.
2. Generalize the acceptance harness pattern so the next upstream examples can
   be added by backend wiring instead of workload rewrites.
3. Keep benchmark-specific proving rows secondary while generic backend gaps
   are closed tranche by tranche.

## Explicitly not doing now

- pretending the copied CUDA implementation is already a complete Metal backend
- widening the claimed Metal support faster than deterministic parity can cover
- renaming every inherited CUDA symbol before the replacement slices stabilize
- treating benchmark numbers as the first proof of correctness instead of
  deterministic parity against the vendored CPU path
- re-implementing upstream example workloads when backend wiring should be the
  only change needed to prove and verify them

## Update rule

Update this file whenever the active tranche, blockers, or next three
deliverables change.
