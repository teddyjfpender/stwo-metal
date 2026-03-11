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
  `G10 first slice: execute MetalEvaluationProgramV1 on the live generated trace`
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

- the repository now has a minimal `MetalEvaluationProgramV1` ABI module,
  validator, first generated `fibonacci_example` lowering, a deterministic
  reference interpreter, and a fail-closed first Metal `.metal` interpreter
  lane for the currently lowered `fibonacci_example` subset
- the active generated `wide_fibonacci` row can now lower its benchmark target
  into a validated V1 program and execute that program on the live generated
  trace through both the reference and Metal device lanes, but the actual prove
  path is still only partially migrated: the prove-values staging and execution
  law now live on the benchmark boundary in `stwo-metal`, but the benchmark
  still does not prove through the V1 runtime contract end to end
- the V1 contract now has both a correctness-first reference interpreter and a
  first Metal `.metal` interpreter lane, but the active generated benchmark
  row still does not prove through that V1 runtime contract
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

1. Migrate the active generated `wide_fibonacci` benchmark row onto the
   validated V1 runtime contract for the next live prove-phase boundary beyond
   prove-values execution.
2. Define the overlay lookup law keyed by semantic hash and capability profile,
   then migrate the active generated benchmark row onto that same contract
   instead of benchmark-local lowering.
3. Add layout/reflection verification for the V1 host/device boundary records
   once the first live Metal runtime consumer exists, so the ABI surface is
   checked both statically and against compiled Metal metadata.

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
