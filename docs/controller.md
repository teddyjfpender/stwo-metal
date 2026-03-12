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
  `G10 migration: move post-composition proof flow onto the V1 sampled-values contract`
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
  and
  [`dn-0009-v1-post-composition-sampled-values-abi.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0009-v1-post-composition-sampled-values-abi.md)
  and
  [`dn-0010-generated-row-convergence-and-runtime-optimization.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0010-generated-row-convergence-and-runtime-optimization.md)
  and
  [`dn-0011-stwo-cairo-and-virtual-snos-target.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0011-stwo-cairo-and-virtual-snos-target.md)
- Current owner area:
  `V1 contract and runtime planning`

## Current blockers

- the shared V1 prove runtime (`prove_runtime_v1.rs`) now owns composition
  generation, post-composition sampled values, prove values, and prove core as
  a single contract consumed by the generated benchmark row; the benchmark
  module is reporting-only and no longer carries thin wrapper methods or alias
  types — it calls `execute_prove_core_v1` directly
- the generated `wide_fibonacci` prove path now runs end to end through the
  shared V1 runtime contract: trace commit → composition polynomial via
  selected V1 evaluation program → composition commit → sampled values via
  selected V1 dispatch → FRI → decommit → proof assembly, all owned by
  `prove_runtime_v1`
- composition generation now has per-sub-phase timing
  (`MetalCompositionDetailBreakdown`: twiddle, trace extension, eval program,
  quotient application, interpolation) for diagnosing scaling degradation at
  high log sizes; quotient application uses in-place mutation instead of
  map+collect; the reference sanity check can be skipped via
  `MetalProveRuntimeContextV1::with_skip_reference_sanity_check` for
  production/benchmark runs
- the current generated-lane benchmark sweep still shows strong low-log and
  weak high-log scaling (`log16..20` ahead, `log21..23` behind SIMD); the
  composition detail breakdown is now available to identify which sub-phase
  dominates at high logs
- `VIRTUAL_SNOS` is now registered in the planner manifest under the
  `stwo_cairo` workload family with FriOnly support tier, CPU-owned stages
  (except Metal-native FRI), and fail-closed lowering behavior; integration
  tests validate planner recognition, inventory exposure, and fail-closed
  behavior for unsupported routes
- host/device ABI reflection checks for shared Metal boundary records are not
  yet part of the implemented runtime contract
- internal Rust vocabulary is still CUDA-first in many places
- `poseidon` is currently blocked by the vendored lifted protocol's AIR-degree
  limitation, so it remains an upstream protocol blocker rather than the next
  backend row
- `stark-v` is intentionally iced while the real downstream target shifts to
  `stwo-cairo`, specifically the `VIRTUAL_SNOS` row expected by
  `starknet-privacy`

## Next three deliverables

1. Profile the composition detail breakdown at `log21..23` and reduce the
   dominant sub-phase to bring high-log scaling closer to SIMD parity.
2. Add ABI reflection checks to the V1 runtime contract so host/device
   boundary records are verified at lowering time.
3. Begin `G11` hardening: produce the first `stwo-cairo` input artifact and
   evaluate it against the converged V1 runtime through `virtual_snos`.

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
