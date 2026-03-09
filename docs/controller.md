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
- Active tranche: `T7 fifth implementation slice: retire the remaining backend trait gaps after FriOps`
- Objective:
  close the remaining shared backend trait gaps after `FriOps` so the current
  explicit CPU prove bridge shrinks from a generic backend gap into the
  smaller set of lookup and channel-backed requirements that still block direct
  `MetalBackend` substitution
- Active design note:
  [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)
- Current owner area:
  `backend-completion planning and example-backed acceptance`

## Current blockers

- the project goal had drifted toward benchmark-specific proving rows instead
  of a generic Stwo proving backend
- `MetalBackend` now implements `ColumnOps`, `PolyOps`, `AccumulationOps`,
  `QuotientOps`, and `FriOps`, but the backend contract still remains open at
  the lookup layer because `GkrOps` is not implemented yet
- internal Rust vocabulary is still CUDA-first in many places
- the bounded FRI commitment slice now exists, but its last-layer
  interpolation still crosses an explicit CPU bridge rather than a native
  `stwo-metal` interpolation boundary
- only the first upstream example prove/verify slice exists so far:
  `wide_fibonacci` now proves and verifies through an acceptance fixture, but
  the proving path still crosses an explicit CPU bridge because direct
  `MetalBackend` substitution still lacks `GkrOps` and `BackendForChannel`
  support
- the example-backed acceptance harness is now reusable for single-trace
  Blake2s-backed CPU bridge flows, but only `wide_fibonacci` uses it so far
- the declared `wide_fibonacci` benchmark target remains useful for
  performance, but it is not the architectural source of truth and must stop
  driving milestone sequencing
- the native commitment and decommit boundary is still host-owned and
  readback-based rather than a GPU-side hash pipeline
- the remaining shared trait gap after the new bridge tranche starts at
  `GkrOps`, not another workload-specific acceptance harness

## Next three deliverables

1. Land the next shared backend-completion slice at the lookup boundary,
   starting with `MleOps` and `GkrOps` for `MetalBackend`.
2. Follow the lookup tranche with the next honest channel-backed gap rather
   than adding another example row too early.
3. Add the next upstream example through the reusable acceptance harness only
   after a shared backend slice meaningfully widens direct proving support.

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
