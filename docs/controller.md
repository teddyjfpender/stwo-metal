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
- examples remain acceptance workloads rather than the architecture surface

## Current operating state

- Date opened: `2026-03-10`
- Status: `in_progress`
- Active tranche:
  `G2 second slice: widen the new internal artifact-registry and
  execution-plan seam from exemplar prove planning into the broader backend
  registration path without widening the public API`
- Objective:
  re-center `stwo-metal` on the correct long-term architecture: examples as the
  acceptance matrix, generic backend substitution as the correctness lane, and
  generated proving artifacts as the production lane
- Active design note:
  [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)
  and
  [`dn-0002-generic-backend-and-codegen-contract.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0002-generic-backend-and-codegen-contract.md)
- Current owner area:
  `backend planning surface and generated-contract freeze`

## Current blockers

- there is not yet a stable internal artifact-registry boundary for
  Stwo/codegen-produced proving artifacts beyond the current prove, benchmark,
  workload-stage, and generated-route subset
- there is not yet a stable execution-plan boundary for operations beyond the
  current prove-planning path
- generic and generated lanes are now encoded as a private planning seam, but
  not yet adopted broadly enough to count as the finished G2 surface
- internal Rust vocabulary is still CUDA-first in many places
- unchanged upstream `wide_fibonacci`, `state_machine`, `blake`, and `xor`
  example rows now prove and verify through `MetalBackend` in the acceptance
  harness, but those paths still rely on acceptance-local adapters
- the remaining framework-component bridge is still CPU-domain based and still
  lives only in the acceptance harness rather than a stable shared boundary
- there is not yet a declared policy for how generated output becomes durable
  and hand-tunable within this repository
- `poseidon` is currently blocked by the vendored lifted protocol's AIR-degree
  limitation, so it remains an upstream protocol blocker rather than the next
  backend row
- benchmark work remains important, but it must now follow the generic and
  generated contract instead of driving it
- superseded milestone history has to stay out of the active control surface so
  the new sequence remains legible
- per-component generated inventory and generated-route compatibility now exist,
  and benchmark/workload declarations consume that seam, but the broader
  backend still has not fully adopted it

## Next three deliverables

1. Widen the shared execution-plan seam beyond planner, workload, and benchmark
   declarations so more backend routing consumes the same fail-closed contract.
2. Start pulling the next backend-facing registration and routing sites onto the
   same seam so unsupported generated support is handled consistently in one
   place.
3. Keep acceptance-local adapters, bounded CPU fallbacks, and unsupported
   generated-component behavior explicit until the new shared boundaries fully
   replace the old ad hoc paths.

## Explicitly not doing now

- using acceptance examples as the architecture surface
- adding new benchmark-local seams before the planning boundary is frozen
- promising generated support before the producer/consumer contract exists in
  code
- re-implementing upstream example workloads when backend wiring should remain
  the only change needed to prove and verify them
- treating benchmark numbers as the first proof of correctness instead of
  deterministic parity against the vendored CPU path

## Update rule

Update this file whenever the active tranche, blockers, or next three
deliverables change.
