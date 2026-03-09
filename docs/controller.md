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
- Active tranche: `T5 bounded proving-surface bring-up`
- Objective:
  grow the first truthful Metal-backed proving surface by porting bounded Stwo
  primitives with deterministic vendored CPU parity and explicit unsupported
  edges
- Active design note:
  [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)
- Current owner area:
  `bounded Metal runtime and proving-surface bring-up`

## Current blockers

- internal Rust vocabulary is still CUDA-first in many places
- the bounded FRI commitment slice now exists, but its last-layer
  interpolation still crosses an explicit CPU bridge rather than a native
  `stwo-metal` interpolation boundary
- one executable CPU-owned quotient evaluation handoff now exists, but the
  executable hybrid workload still starts after witness generation and quotient
  accumulation rather than owning an earlier workload artifact boundary
- the declared `wide_fibonacci` benchmark target is now pinned at
  `log_n_instances = 20`, `n_columns = 100`, with a project-supplied 4090
  reference goal of `90 ms`; the standalone trace benchmark and the prove
  benchmark trace-generation phase now enter through a native `.metal` path,
  but the prove benchmark still crosses an explicit Metal-to-CUDA trace bridge
  before the inherited proving lane
- the native commitment and decommit boundary is still host-owned and
  readback-based rather than a GPU-side hash pipeline
- interpolation, evaluation, and trace-support primitives beyond the bounded
  wide-fibonacci trace kernel are still outside the Metal lane

## Next three deliverables

1. Freeze one earlier witness-owned workload handoff for `fibonacci_example`
   that feeds the native Metal trace boundary before the CPU quotient
   evaluation boundary.
2. Replace the explicit Metal-to-CUDA trace-evaluation bridge inside
   `wide_fibonacci_prove` with an earlier proving-stage boundary.
3. Decide whether quotient accumulation or pre-FRI PCS commitment is the next
   native proving-stage replacement after that earlier handoff is selected.

## Explicitly not doing now

- pretending the copied CUDA implementation is already a complete Metal backend
- widening the claimed Metal support faster than deterministic parity can cover
- renaming every inherited CUDA symbol before the replacement slices stabilize
- treating benchmark numbers as the first proof of correctness instead of
  deterministic parity against the vendored CPU path

## Update rule

Update this file whenever the active tranche, blockers, or next three
deliverables change.
