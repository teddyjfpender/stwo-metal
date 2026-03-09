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
- Active tranche: `T7 sixth implementation slice: expose the component-prover blocker after backend completion`
- Objective:
  move the remaining explicit CPU prove bridge from backend and channel
  infrastructure into one named upstream component-prover blocker so the next
  tranche can target the real acceptance seam instead of more backend
  speculation
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
- only the first upstream example prove/verify slice exists so far:
  `wide_fibonacci` now proves and verifies through an acceptance fixture, but
  the proving path still crosses an explicit CPU bridge because the vendored
  upstream `FrameworkComponent` only implements `ComponentProver` for
  `CpuBackend` and `SimdBackend`
- the example-backed acceptance harness is now reusable for single-trace
  Blake2s-backed CPU bridge flows, but only `wide_fibonacci` uses it so far
- the declared `wide_fibonacci` benchmark target remains useful for
  performance, but it is not the architectural source of truth and must stop
  driving milestone sequencing
- the native commitment and decommit boundary is still host-owned and
  readback-based rather than a GPU-side hash pipeline
- the remaining acceptance blocker is no longer a generic backend trait gap;
  it is the component-prover layer for framework-backed upstream examples

## Next three deliverables

1. Define the smallest honest `ComponentProver<MetalBackend>` path for
   framework-backed upstream examples without forking workload logic.
2. Decide whether that component-prover slice belongs in an upstream-facing
   vendored refactor or a local adapter boundary.
3. Use that component-prover slice to retire the explicit CPU prove bridge for
   the first unchanged upstream example before adding more example rows.

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
