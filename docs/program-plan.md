# Program Plan

## Purpose

This file records the intended route for the `stwo-metal` program at milestone
granularity.

The authoritative long-range map lives in [`roadmap.md`](./roadmap.md). This
file keeps the currently intended route aligned with that map and avoids
chronological implementation storytelling.

## Program objective

Deliver a truthful Stwo GPU backend on Apple Silicon by separating three
concerns cleanly:

- generic backend substitution for correctness and coverage
- generated fast-path support for production performance
- example workloads as acceptance fixtures only

The implementation repository is `stwo-metal`, but the contract being frozen is
intentionally backend-family generic so it can inform sibling backends such as
`stwo-cuda`.

## Program invariants

- the smallest stable public API wins
- core proving logic stays deterministic
- the vendored Stwo snapshot remains the semantic authority
- generated support must fail closed when compatibility or inventory is missing
- generated outputs must remain durable and hand-tunable
- examples are acceptance fixtures, not rewrite targets
- downstream targets do not redefine the backend contract
- active control docs describe current state only; chronology belongs in
  [`decision-log.md`](./decision-log.md) or
  [`milestone-archive.md`](./milestone-archive.md)

## Milestone queue

The superseded `T0` through `T8` sequence now lives in:

- [`milestone-archive.md`](./milestone-archive.md)

| Order | Milestone | Status | Exit condition |
| --- | --- | --- | --- |
| G0 | Freeze the generic backend contract | `completed` | the architecture distinguishes generic substitution, generated fast path, and temporary wrappers |
| G1 | Freeze the codegen input schema and fail-closed contract | `completed` | the required producer artifact, consumer subset, and unsupported-component behavior are specified |
| G2 | Build the backend planning and registration surface | `completed` | `stwo-metal` has a stable internal artifact-registry and execution-plan boundary with deterministic schema checks |
| G3 | Move acceptance coverage onto the stable generic path | `completed` | acceptance coverage no longer depends on architecture-local example shims where shared backend boundaries should exist |
| G4 | Land generated fast-path registration and ABI inventory | `completed` | generated artifacts register component identity, ABI, build inventory, and specialization keys through a stable surface |
| G5 | Lower generated artifacts into Metal runtime execution plans | `completed` | generated components drive trace, evaluation, lookup, quotient, FRI, and commitment scheduling through backend planning surfaces |
| G6 | Separate benchmark lanes and optimize the right rows | `completed` | generic and generated benchmark rows are measured independently and optimization work targets the generated lane explicitly |
| G7 | Retire temporary compatibility shims | `completed` | acceptance-local adapters and example-specific wrappers are removed or clearly reduced to non-architectural fixtures |
| G8 | Harden the contract against `stark-v` workloads | `iced` | `stark-v` uses the same generic/generated backend contract successfully |
| G9 | Freeze and implement `MetalEvaluationProgramV1` | `in_progress` | generic and generated Metal execution consume one validated lowered program contract |
| G10 | Migrate the active generated lane onto the V1 program contract | `in_progress` | the benchmark-specialized generated row is driven by the V1 program and overlay law rather than bespoke lowering |
| G11 | Harden the converged V1 contract against `stwo-cairo` workloads | `planned` | the V1 program contract is exercised against `stwo-cairo`, with `VIRTUAL_SNOS` as the first named downstream target |

## Active tranche

The active tranche is:

`G10 convergence: reduce benchmark-specific generated-row ownership and move the live prove path onto shared V1 runtime contracts`

The active formal basis is:

- [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](./dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)
- [`dn-0002-generic-backend-and-codegen-contract.md`](./dn-0002-generic-backend-and-codegen-contract.md)
- [`dn-0008-metal-evaluation-program-v1.md`](./dn-0008-metal-evaluation-program-v1.md)
- [`dn-0009-v1-post-composition-sampled-values-abi.md`](./dn-0009-v1-post-composition-sampled-values-abi.md)
- [`dn-0010-generated-row-convergence-and-runtime-optimization.md`](./dn-0010-generated-row-convergence-and-runtime-optimization.md)
- [`dn-0011-stwo-cairo-and-virtual-snos-target.md`](./dn-0011-stwo-cairo-and-virtual-snos-target.md)

## Current state summary

### G9

`MetalEvaluationProgramV1` now has:

- fixed-width `#[repr(C)]` ABI records
- deterministic semantic hashing and fail-closed validation
- first real component lowering
- a deterministic Rust reference interpreter
- a first Metal device lane
- explicit dispatch selection and first overlay registration

What remains:

- broaden the shared V1 runtime family so more of the live generated proof flow
  is owned by the same contract, not by benchmark-specialized orchestration
- add the reflection-checked ABI verification promised by the V1 contract

### G10

The generated `wide_fibonacci` row now has:

- backend-owned sample execution
- backend-owned timed iteration and benchmark-run orchestration
- backend-owned composition generation from selected V1 runtime output
- backend-owned sampled-values ABI and runtime
- backend-owned prepared-finish and prepared tree-decommit contracts
- backend-owned proof/result reporting metadata

What remains:

- collapse the remaining generated-row-specialized orchestration onto shared V1
  runtime paths
- make the backend-owned runtime family the clear primary authority, not a
  migrated layer wrapped around older prove flow
- improve high-log scaling after that convergence is in place

### G11

The downstream hardening target is now:

- `stwo-cairo` as the proving-system integration target
- `VIRTUAL_SNOS` as the first named downstream proving row
- `starknet-privacy` as the concrete downstream consumer to keep in mind

This is intentionally downstream of `G10`. We should not claim `stwo-cairo` or
`VIRTUAL_SNOS` support on pre-V1 bridge surfaces.

## Current implementation focus

The active implementation focus is:

- keep collapsing the generated row onto shared V1 runtime paths
- remove specialized benchmark-only orchestration that still remains
- optimize the backend-owned runtime family from that cleaner baseline
- keep examples as the acceptance matrix rather than the producer surface
- keep `stark-v` iced while the actual downstream target shifts to
  `stwo-cairo` / `VIRTUAL_SNOS`

## Current benchmark evidence

The current generated-lane `wide_fibonacci` sweep motivates this focus:

- generated Metal is ahead of SIMD at `log16..20`
- generated Metal falls behind at `log21..23`
- so the next work is primarily a runtime-convergence and scaling problem, not
  another isolated low-log micro-optimization problem

## Immediate sequencing rules

- do not widen benchmark-specialized seams while they are being retired
- do not let the current generated `wide_fibonacci` row become the de facto
  long-term ABI
- do not widen support claims through example-specific wrappers
- do not claim downstream `stwo-cairo` or `VIRTUAL_SNOS` readiness before the
  V1 runtime family is the honest proving authority
- keep generic and generated lanes semantically aligned even when their runtime
  shapes differ

## Next three deliverables

1. Collapse the remaining generated-row proof flow onto shared V1/runtime
   contracts so the backend-owned runtime family becomes the clear primary
   prove-path authority.
2. Reduce the specialized benchmark-only orchestration that still remains so
   the benchmark binaries become reporting-only edges for the generated lane.
3. Freeze the first `stwo-cairo` / `VIRTUAL_SNOS` hardening inputs and
   integration checks for `G11` without letting them redefine the backend
   contract.
