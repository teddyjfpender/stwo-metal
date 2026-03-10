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
  `G3 third slice: consolidate the remaining acceptance-local bridges behind
  one registered bridge catalog while keeping the framework CPU-domain bridge
  explicit`
- Objective:
  re-center `stwo-metal` on the correct long-term architecture: examples as the
  acceptance matrix, generic backend substitution as the correctness lane, and
  generated proving artifacts as the production lane
- Active design note:
  [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)
  and
  [`dn-0002-generic-backend-and-codegen-contract.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0002-generic-backend-and-codegen-contract.md)
- Current owner area:
  `acceptance integration against the stable planning seam`

## Current blockers

- internal Rust vocabulary is still CUDA-first in many places
- unchanged upstream `wide_fibonacci`, `state_machine`, `blake`, and `xor`
  example rows now prove and verify through `MetalBackend` in the acceptance
  harness, but those paths still rely on acceptance-local adapters
- the remaining framework-component bridge is still CPU-domain based and still
  lives only in the acceptance harness rather than a stable shared boundary
- the acceptance harness now has one checked bridge-catalog surface, but the
  durable home for that bridge law is still unresolved between a non-public
  shared boundary and an upstream-facing path
- full acceptance-matrix verification is currently blocked by the vendored
  Stwo snapshot requiring an older nightly feature surface than the installed
  local toolchains provide
- there is not yet a declared policy for how generated output becomes durable
  and hand-tunable within this repository
- `poseidon` is currently blocked by the vendored lifted protocol's AIR-degree
  limitation, so it remains an upstream protocol blocker rather than the next
  backend row
- benchmark work remains important, but it must now follow the generic and
  generated contract instead of driving it
- superseded milestone history has to stay out of the active control surface so
  the new sequence remains legible
- the current acceptance rows now all consume registered workload lanes and one
  checked bridge catalog, but the adapters themselves still live only in the
  acceptance harness
- the shared planning seam is now stable enough for G3 work, but the next
  decision is about bridge ownership and laws rather than more lane
  registration

## Next three deliverables

1. Define the minimal non-public laws for a reusable acceptance bridge surface
   so the current catalog does not drift into ad hoc test glue again.
2. Decide whether the framework-backed bridge should move into an upstream-
   facing boundary or a non-public shared internal boundary once the vendored
   workspace conflict is resolved.
3. Pin or refresh the vendored Stwo toolchain contract so acceptance-matrix
   verification becomes deterministic again.

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
