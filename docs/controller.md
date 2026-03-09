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
- Active tranche: `T7 eighth implementation slice: direct MetalBackend prove/verify for a multi-tree upstream example`
- Objective:
  widen direct `MetalBackend` acceptance from the first single-trace example to
  a multi-tree upstream example while keeping the remaining framework-component
  bridge explicit and local
- Active design note:
  [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)
- Current owner area:
  `backend-completion planning and example-backed acceptance`

## Current blockers

- the project goal had drifted toward benchmark-specific proving rows instead
  of a generic Stwo proving backend
- `MetalBackend` now implements the full Stwo `Backend` trait
- `MetalBackend` now implements the Blake2s `BackendForChannel` surface through
  explicit CPU-bridge Merkle and proof-of-work boundaries
- internal Rust vocabulary is still CUDA-first in many places
- the bounded FRI commitment slice now exists, but its last-layer
  interpolation still crosses an explicit CPU bridge rather than a native
  `stwo-metal` interpolation boundary
- unchanged upstream `wide_fibonacci` and `state_machine` examples now prove
  and verify through `MetalBackend` in the acceptance harness
- the remaining framework-component bridge is still CPU-domain based and still
  lives only in the acceptance harness rather than a stable shared boundary
- the example-backed acceptance harness now covers both a single-trace row and
  a multi-tree row, but only the framework-backed example shape is proven so
  far
- the declared `wide_fibonacci` benchmark target remains useful for
  performance, but it is not the architectural source of truth and must stop
  driving milestone sequencing
- the native commitment and decommit boundary is still host-owned and
  readback-based rather than a GPU-side hash pipeline
- `poseidon` is currently blocked by the vendored lifted protocol's AIR-degree
  limitation, so it is not the immediate next backend row
- the next acceptance blocker is onboarding rows with lookup-heavy or
  non-framework prover surfaces, starting with `blake` and then `xor`,
  without hiding the remaining CPU-domain bridge

## Next three deliverables

1. Unblock the `blake` acceptance row with the smallest truthful vendored or
   acceptance-layer surface needed for direct `MetalBackend` substitution.
2. Decide whether the current acceptance-local adapter should remain local,
   move into a shared non-public helper boundary, or be superseded by an
   upstream-facing refactor.
3. Land the next non-framework or lookup-heavy acceptance row, with `xor` as
   the next likely candidate after `blake`, and keep upstream protocol limits
   separated from Metal-backend gaps.

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
