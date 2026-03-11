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
  `G5 eighteenth slice: identify the remaining direct public-authority
  consumers and decide whether the transitional surface can now shrink`
- Objective:
  re-center `stwo-metal` on the correct long-term architecture: examples as the
  acceptance matrix, generic backend substitution as the correctness lane, and
  generated proving artifacts as the production lane
- Active design note:
  [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)
  and
  [`dn-0002-generic-backend-and-codegen-contract.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0002-generic-backend-and-codegen-contract.md)
  and
  [`dn-0003-acceptance-bridge-law-and-ownership.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0003-acceptance-bridge-law-and-ownership.md)
- Current owner area:
  `acceptance integration against the stable planning seam`

## Current blockers

- internal Rust vocabulary is still CUDA-first in many places
- unchanged upstream `wide_fibonacci`, `state_machine`, `blake`, and `xor`
  example rows now prove and verify through `MetalBackend` in the acceptance
  harness, but those paths still rely on a non-public shared bridge with
  explicit CPU-domain fallback debt
- the remaining framework-component bridge is still CPU-domain based even
  though its ownership has moved out of the acceptance harness and into a
  private shared support boundary
- there is not yet a declared policy for how generated output becomes durable
  and hand-tunable within this repository
- generated inventory now names ABI symbols and specialization keys, is
  consumed by workload and benchmark declarations, and now feeds a reusable
  execution-binding helper plus one scheduling seed and shared witness,
  evaluation, quotient, and prove-values staging helpers, but the shared
  prove-values bridge and the acceptance and benchmark witness lanes now anchor
  themselves on a transitional public `MetalExecutionAuthority` surface instead
  of directly on a lower private generated execution contract; the first
  workload-side live helpers have now moved below that public surface onto the
  private execution seed, and both the benchmark prove-values bridge and the
  upstream acceptance lane now depend on workspace-private validated lane
  contracts instead of consuming `MetalExecutionAuthority` directly
- `poseidon` is currently blocked by the vendored lifted protocol's AIR-degree
  limitation, so it remains an upstream protocol blocker rather than the next
  backend row
- benchmark work remains important, but it must now follow the generic and
  generated contract instead of driving it
- superseded milestone history has to stay out of the active control surface so
  the new sequence remains legible
- the current acceptance rows now all consume registered workload lanes and one
  checked bridge catalog backed by a private shared support crate, which closes
  the immediate G3 ownership question but still leaves adapter retirement for
  later milestones
- the shared planning seam is now stable enough for G3 work, and deterministic
  cargo verification is restored on the pinned nightly after modernizing the
  stale vendored chunking surface
- the next decision is about how much ABI, specialization, and generated build
  inventory belongs in the stable internal artifact registry before lowering
  starts in earnest

## Next three deliverables

1. Enumerate the remaining direct `MetalExecutionAuthority` consumers and pick
   the next one that can either move lower or disappear behind an existing
   private seam.
2. Keep the restored pinned-nightly verification path explicit and narrow while
   G5 continues lowering generated registrations.
3. Preserve the non-public bridge-law boundary and private support-crate
   ownership while deciding whether the transitional public execution-law
   surface can now shrink before G7.

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
