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
  `G2 first slice: freeze the internal artifact-registry surface,
  execution-plan surface, and fail-closed dispatch contract so the backend is
  defined by generic and generated Stwo proving artifacts rather than by
  examples or benchmark-local seams`
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
  Stwo/codegen-produced proving artifacts
- there is not yet a stable execution-plan boundary that lowers one proving
  artifact into backend-owned work
- generic and generated lanes are now specified, but not yet encoded as stable
  backend planning surfaces
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

## Next three deliverables

1. Define the internal artifact-registry boundary, including schema versioning,
   compatibility checks, and generated inventory ownership.
2. Define the execution-plan boundary that lowers one proving artifact into
   trace, evaluation, lookup, quotient, FRI, and commitment work without
   exposing runtime policy to callers.
3. Keep acceptance-local adapters, bounded CPU fallbacks, and unsupported
   generated-component behavior explicit until the new shared boundaries exist.

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
