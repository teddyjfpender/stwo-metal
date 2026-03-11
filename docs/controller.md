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
  `G9 first slice: freeze and operationalize MetalEvaluationProgramV1`
- Objective:
  move `stwo-metal` from a benchmark-specialized generated path to one stable
  lowered-program contract that both the generic interpreter lane and the
  generated overlay lane can consume
- Active design note:
  [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)
  and
  [`dn-0002-generic-backend-and-codegen-contract.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0002-generic-backend-and-codegen-contract.md)
  and
  [`dn-0008-metal-evaluation-program-v1.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0008-metal-evaluation-program-v1.md)
- Current owner area:
  `V1 contract and runtime planning`

## Current blockers

- the repository still lacks an implemented `MetalEvaluationProgramV1` ABI
  crate/module and validator, so the frozen V1 contract exists in docs but not
  yet in the runtime surface
- the active generated `wide_fibonacci` row is still driven by pre-V1
  benchmark-specialized lowering rather than a stable lowered-program contract
- the generic interpreter lane for the V1 program does not exist yet, so the
  generated overlay lane has no program-level semantic baseline inside the new
  contract
- the current acceptance matrix still proves backend viability, but it does not
  yet validate one shared lowered-program contract across generic and generated
  execution modes
- host/device ABI reflection checks for shared Metal boundary records are not
  yet part of the implemented runtime contract
- internal Rust vocabulary is still CUDA-first in many places
- `poseidon` is currently blocked by the vendored lifted protocol's AIR-degree
  limitation, so it remains an upstream protocol blocker rather than the next
  backend row
- `stark-v` is intentionally iced while the V1 contract is still being brought
  into code; downstream hardening is no longer the active path

## Next three deliverables

1. Land the first `MetalEvaluationProgramV1` ABI surface in code: header,
   section descriptors, fixed-width sections, semantic hash, and a validator
   error surface.
2. Lower one existing generated component path onto `MetalEvaluationProgramV1`
   and prove it can drive a generic interpreter lane on Metal without changing
   proof semantics.
3. Define the overlay lookup law keyed by semantic hash and capability profile,
   then migrate the active generated benchmark row onto that same contract
   instead of benchmark-local lowering.

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
