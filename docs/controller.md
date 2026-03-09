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
- Active tranche: `T7 seventh implementation slice: direct MetalBackend prove/verify for the first unchanged upstream example`
- Objective:
  retire the outer CPU prove helper for the first unchanged upstream example
  and replace it with a direct `MetalBackend` prove/verify path that keeps the
  remaining framework-component bridge explicit and local
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
- the first unchanged upstream example now proves and verifies through
  `MetalBackend` in the acceptance harness:
  `wide_fibonacci` uses a local framework-component adapter instead of the
  earlier outer CPU prove helper
- the remaining framework-component bridge is still CPU-domain based and still
  lives only in the acceptance harness rather than a stable shared boundary
- the example-backed acceptance harness is now reusable for single-trace
  Blake2s-backed backend substitution flows, but only `wide_fibonacci` uses it
  so far
- the declared `wide_fibonacci` benchmark target remains useful for
  performance, but it is not the architectural source of truth and must stop
  driving milestone sequencing
- the native commitment and decommit boundary is still host-owned and
  readback-based rather than a GPU-side hash pipeline
- the next acceptance blocker is no longer the first prove/verify seam;
  it is generalizing beyond the single-trace framework-backed adapter without
  hiding the remaining CPU-domain bridge

## Next three deliverables

1. Generalize the acceptance-backed `ComponentProver<MetalBackend>` adapter
   beyond the first single-trace example without forking workload logic.
2. Decide whether the current acceptance-local adapter should remain local,
   move into a shared non-public helper boundary, or be superseded by an
   upstream-facing refactor.
3. Onboard the next upstream example row through direct `MetalBackend`
   prove/verify instead of reintroducing an outer CPU prove bridge.

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
