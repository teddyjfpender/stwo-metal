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
  `G8 eighth slice: freeze the support-promotion gate for the vendored stark-v
  row`
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
  and
  [`dn-0004-stark-v-hardening-input-and-contract.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0004-stark-v-hardening-input-and-contract.md)
  and
  [`dn-0005-stark-v-attachment-strategy.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0005-stark-v-attachment-strategy.md)
  and
  [`dn-0006-stark-v-generated-minimum-contract.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0006-stark-v-generated-minimum-contract.md)
  and
  [`dn-0007-stark-v-support-promotion-gate.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0007-stark-v-support-promotion-gate.md)
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
  contracts instead of consuming `MetalExecutionAuthority` directly; fixture
  edges now enter those bridges through boundary-based constructors only, and
  the dead transitional planning helpers that lost all live callers have been
  removed instead of left as dormant API; the redundant root-level companion
  export for `MetalExecutionAuthority` is now gone, so the type is available
  through the workload-facing module where its semantics actually live; the
  private support crates no longer import or validate through the authority
  type at all, so the remaining direct consumers are now the workload/benchmark
  API and the workload-scoped companion reexport; the benchmark boundary no
  longer exposes its own redundant `execution_authority()` pass-through, so the
  remaining direct callers now sit on `MetalWorkloadBoundary` and tests of that
  workload-law surface; `MetalExecutionAuthority` itself is now gone, leaving
  the workload law as `plan()` and `stage_ownership()`, while the richer stage-
  assignment slice remains internal to the planning and execution-plan layers;
  the generated execution seed now also owns the shared wide-fibonacci witness
  shape law that workload and benchmark boundaries previously duplicated
- `poseidon` is currently blocked by the vendored lifted protocol's AIR-degree
  limitation, so it remains an upstream protocol blocker rather than the next
  backend row
- benchmark work remains important, but it must now follow the generic and
  generated contract instead of driving it; G5 is now complete, G6 is now
  complete with a dual-lane report surface, and G7 is now complete because the
  remaining compatibility bridges are fixture-owned rather than
  architecture-adjacent support crates
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
- `stark-v` is now pinned as the first real G8 downstream input, and the
  current snapshot is now classified as `generic_lane = unsupported`,
  `generated_lane = required`, `status = fail_closed`; it is still not
  executed through `stwo-metal`; the pinned checkout is now vendored locally,
  the minimum generated subset for the first supported downstream row is now
  frozen, but no compatible artifact exists yet and the current checkout
  exposes no machine-readable generated artifact signal

## Next three deliverables

1. Keep one deterministic local fail-closed row for the vendored `stark-v`
   input.
2. Keep one frozen promotion gate that prevents support claims without an
   actual downstream support signal.
3. Wait for or integrate one real downstream support signal:
   backend-parametric proving or a generated artifact satisfying `DN-0006`.

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
