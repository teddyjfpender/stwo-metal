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

- the shared V1 prove runtime (`prove_runtime_v1.rs`) owns the full prove
  pipeline for the generated row: trace commit, composition via V1 evaluation
  program, composition commit, sampled values via V1 dispatch, FRI, decommit,
  and proof assembly — all through `execute_prove_core_v1`
- full prove-core profiling is now available through
  `MetalCompositionDetailBreakdown` and `MetalProveValuesDetailBreakdown`,
  identifying three dominant bottlenecks at log23 (89.7% of prove-core):
  `eval_program` (38%, 7.96x scaling), `fri_and_decommit` (31%, 7.09x
  scaling), and `prepare` (20%, 11.99x scaling); the fix for the FRI timing
  gap (`prepare_post_composition_finish_runtime` was previously untracked) is
  deployed
- ABI reflection checks (`validate_eval_program_abi_layout_v1`,
  `validate_sampled_values_abi_layout_v1`) now run fail-closed at lowering
  time, verifying size, alignment, and field offsets of all 8 `#[repr(C)]`
  host/device boundary records
- `VIRTUAL_SNOS` is now lowered through the V1 evaluation program contract via
  `lower_virtual_snos_evaluation_program_v1`, exercising multi-interaction
  traces, Param opcode, and constraint aggregation; it is no longer a
  fail-closed stub
- the current generated-lane benchmark sweep still shows strong low-log and
  weak high-log scaling; profiling has identified the three dominant sub-phases
  but optimization work has not yet started
- internal Rust vocabulary is still CUDA-first in many places
- `poseidon` is currently blocked by the vendored lifted protocol's AIR-degree
  limitation
- `stark-v` is intentionally iced while the real downstream target is
  `stwo-cairo`/`VIRTUAL_SNOS`

## Next three deliverables

1. Optimize the three dominant prove-core bottlenecks identified by profiling:
   `eval_program` GPU dispatch (38% at log23), `fri_and_decommit` (31%),
   and `prepare` allocation pressure (20%).
2. Extend `virtual_snos` lowering to exercise additional V1 capabilities
   (ExtMul, Inv, PreprocessedCol) and validate end-to-end against the
   interpreter.
3. Produce the first `virtual_snos` end-to-end prove/verify cycle through the
   registered planner entry and V1 runtime.

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
