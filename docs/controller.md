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

- Date opened: `2026-03-12`
- Status: `in_progress`
- Active tranche:
  `G11 hardening: AIR-driven Metal proving and GPU dispatch`
- Objective:
  close the three gaps between the current V1 runtime and production-grade
  AIR-driven proving: generic FrameworkEval lowering, multi-component
  composition, and Metal GPU dispatch for the evaluation-program hot path
- Active design note:
  [`dn-0012-air-driven-metal-proving-and-gpu-dispatch.md`](./dn-0012-air-driven-metal-proving-and-gpu-dispatch.md)
  (builds on DN-0008, DN-0010, DN-0011)
- Current owner area:
  `AIR-driven proving and GPU dispatch`

## Current blockers

- **CPU interpreter scaling:** the V1 evaluation program interpreter runs
  row-by-row on CPU; at log22 with 100 columns prove-core takes 5.9s on Metal
  vs 1.6s on SIMD — a 3.7x regression that erases the trace-generation
  advantage (Metal is 20x faster at trace gen but 3.7x slower at prove-core)
- **Hand-coded lowering:** only two lowering paths exist
  (`lower_wide_fibonacci_evaluation_program_v1`,
  `lower_virtual_snos_evaluation_program_v1`); a real `stwo-cairo` proof has
  O(20) AIR components, each requiring manual lowering today
- **Single-component composition:** `compute_composition_polynomial_v1` takes
  one program with one `log_n_rows`; real proofs have components at different
  row counts (memory at log22, range-check at log18, builtins at log14)
- Metal wins at log16–log20 (1.25x–1.75x over SIMD) but loses at log22
  (0.93x); the crossover must be pushed beyond log22 for production viability
- `VIRTUAL_SNOS` end-to-end prove/verify succeeds at small scale; the full
  V1 capability surface is exercised
- `poseidon` remains blocked by the vendored lifted protocol
- `stark-v` remains iced

## Next three deliverables

1. **GPU dispatch (D4–D5):** Metal compute kernel for V1 evaluation program
   execution; replace the CPU row-by-row interpreter with parallel GPU
   dispatch; target: prove-core faster than SIMD at all scales log16–log22.
2. **Generic FrameworkEval lowering (D1–D3):** recording evaluator that
   captures any `FrameworkEval` constraint DAG and emits V1 opcodes
   automatically; retire hand-coded lowering functions.
3. **Multi-component composition (D6–D8):** `execute_prove_core_multi_v1`
   that accepts components with different `log_n_rows` values; benchmark
   modeling 3+ components at different sizes.

Full specification: [`dn-0012-air-driven-metal-proving-and-gpu-dispatch.md`](./dn-0012-air-driven-metal-proving-and-gpu-dispatch.md)

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
