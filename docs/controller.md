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
- Active tranche: `T7 tenth implementation slice: close lookup-heavy and mixed-component upstream example rows`
- Objective:
  close the named upstream-example acceptance matrix for all non-blocked rows
  while keeping the remaining acceptance-local bridges explicit, local, and
  auditable
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
- unchanged upstream `wide_fibonacci`, `state_machine`, `blake`, and `xor`
  example rows now prove and verify through `MetalBackend` in the acceptance
  harness
- the remaining framework-component bridge is still CPU-domain based and still
  lives only in the acceptance harness rather than a stable shared boundary
- the example-backed acceptance harness now covers single-trace, multi-tree,
  lookup-heavy, and mixed-component rows, but those bridges are still
  acceptance-local rather than stable shared boundaries
- the declared `wide_fibonacci` benchmark target remains useful for
  performance, but it is not the architectural source of truth and must stop
  driving milestone sequencing
- the native commitment and decommit boundary is still host-owned and
  readback-based rather than a GPU-side hash pipeline
- `poseidon` is currently blocked by the vendored lifted protocol's AIR-degree
  limitation, so it is not the immediate next backend row
- the only named upstream-example row still open in the current target set is
  `poseidon`, and that row is blocked by the vendored lifted protocol rather
  than by a known Metal-backend gap

## Next three deliverables

1. Decide whether the current acceptance-local framework and SIMD-component
   adapters should remain local, move into a shared non-public helper
   boundary, or be superseded by an upstream-facing refactor.
2. Record the current T7 truth explicitly: all named rows except `poseidon`
   now prove and verify through `MetalBackend`, and `poseidon` remains an
   upstream protocol blocker.
3. Re-enter benchmark and performance work from the backend-first posture,
   with `wide_fibonacci` as the primary performance row rather than the
   architectural source of truth.

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
