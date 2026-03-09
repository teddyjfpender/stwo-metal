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
- Active tranche: `planning correction for example-driven backend completion`
- Objective:
  rebaseline `stwo-metal` around generic Stwo proving so the primary
  deliverable is proving upstream Stwo example traces with `MetalBackend` and
  verifying them unchanged except for backend wiring
- Active design note:
  [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)
- Current owner area:
  `backend-completion planning and example-backed acceptance`

## Current blockers

- the project goal had drifted toward benchmark-specific proving rows instead
  of a generic Stwo proving backend
- the local vendored snapshot does not currently contain the upstream
  `crates/examples` acceptance workloads named for the target set, so the
  example-backed acceptance matrix needs an explicit vendoring or import step
- internal Rust vocabulary is still CUDA-first in many places
- the bounded FRI commitment slice now exists, but its last-layer
  interpolation still crosses an explicit CPU bridge rather than a native
  `stwo-metal` interpolation boundary
- the active workload boundary still centers on benchmark-specific seams rather
  than a clean backend-wiring path for unchanged upstream examples
- the declared `wide_fibonacci` benchmark target remains useful for
  performance, but it is not the architectural source of truth and must stop
  driving milestone sequencing
- the native commitment and decommit boundary is still host-owned and
  readback-based rather than a GPU-side hash pipeline
- interpolation, evaluation, and trace-support primitives needed for a generic
  backend are still incomplete outside the currently bounded Metal slices

## Next three deliverables

1. Rebaseline the roadmap, program plan, and done criteria around generic
   Stwo proving with `MetalBackend`, not benchmark-first proving rows.
2. Open a formal milestone for proving upstream Stwo examples unchanged except
   for backend wiring, with an explicit acceptance matrix for `blake`,
   `poseidon`, `state_machine`, `wide_fibonacci`, and `xor`.
3. Freeze further bespoke benchmark-path expansion until backend-completion
   sequencing and acceptance criteria are written down.

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
