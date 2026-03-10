# Program Plan

## Purpose

This file records the intended route for the `stwo-metal` program at milestone
granularity.

The authoritative long-range map lives in [`roadmap.md`](./roadmap.md). This
file keeps the currently intended route aligned with that map.

## Program objective

Deliver a truthful Stwo GPU backend on Apple Silicon by separating three
concerns that had previously been blurred together:

- generic backend substitution for correctness and coverage
- generated fast-path support for production performance
- example workloads as acceptance fixtures only

The current implementation repository is `stwo-metal`, but the architecture
being frozen is intentionally generic enough to inform sibling GPU backends
such as `stwo-cuda`.

## Program invariants

- the smallest stable public API wins
- core proving logic stays deterministic
- the vendored Stwo snapshot remains the semantic authority
- host-safe development remains possible on machines without a working GPU path
- generated support must fail closed when compatibility or inventory is missing
- generated outputs must remain durable and hand-tunable
- examples are acceptance fixtures, not rewrite targets
- every temporary bridge or compatibility shim must be logged in
  [`tech-debt-register.md`](./tech-debt-register.md)

## Milestone queue

This queue supersedes the earlier benchmark-led sequence as the active program
plan.

| Order | Milestone | Status | Exit condition |
| --- | --- | --- | --- |
| G0 | Freeze the generic backend contract | `completed` | the architecture distinguishes generic substitution, generated fast path, and temporary wrappers |
| G1 | Freeze the codegen input schema and fail-closed contract | `completed` | the required producer artifact, consumer subset, and unsupported-component behavior are specified |
| G2 | Build the backend planning and registration surface | `in_progress` | `stwo-metal` has a stable internal artifact-registry and execution-plan boundary with deterministic schema checks |
| G3 | Move acceptance coverage onto the stable generic path | `planned` | acceptance coverage no longer depends on architecture-local example shims where shared backend boundaries should exist |
| G4 | Land generated fast-path registration and ABI inventory | `planned` | generated artifacts register component identity, ABI, build inventory, and specialization keys through a stable surface |
| G5 | Lower generated artifacts into Metal runtime execution plans | `planned` | generated components drive trace, evaluation, lookup, quotient, FRI, and commitment scheduling through backend planning surfaces |
| G6 | Separate benchmark lanes and optimize the right rows | `planned` | generic and generated benchmark rows are measured independently and optimization work targets the generated lane explicitly |
| G7 | Retire temporary compatibility shims | `planned` | acceptance-local adapters and example-specific wrappers are removed or clearly reduced to non-architectural fixtures |
| G8 | Harden the contract against `stark-v` workloads | `planned` | `stark-v` uses the same generic/generated backend contract successfully |

## Immediate sequencing rules

- do not let new benchmark-local seams redefine the backend contract
- do not widen support claims through example-specific wrappers
- do not claim generated support without a schema-compatible artifact and a
  registered inventory
- keep verifier semantics unchanged across generic and generated lanes
- benchmark generic and generated rows separately once both exist

## Current focus

The active tranche is:

`G2 first slice: define the stable internal artifact-registry surface,
execution-plan surface, and explicit fail-closed dispatch contract so examples
remain only the acceptance matrix and benchmarks stop acting as the API source
of truth`

The active formal basis is:

- [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)
- [`dn-0002-generic-backend-and-codegen-contract.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0002-generic-backend-and-codegen-contract.md)

## Completed foundations carried forward

The earlier work remains valuable and is now treated as enabling foundation
rather than as the active sequencing source:

- repository isolation and process reset
- Apple Silicon host and native Metal runtime boundary
- bounded native Metal proving primitives and mirrored native hot-path files
- unchanged upstream example acceptance coverage for
  `wide_fibonacci`, `state_machine`, `blake`, and `xor`
- explicit `poseidon` tracking as an upstream protocol blocker rather than a
  Metal-backend gap
- benchmark-faithful wide-fibonacci Metal proving row and its current measured
  production baseline

## Current implementation obligations under G2

- define the internal Rust representation of the producer artifact
- define schema/version compatibility checks
- define the generated inventory and registration manifest boundary
- define the execution-plan lowering boundary from artifact to backend work
- define unsupported-component behavior and test it
- keep acceptance-local adapters and bounded CPU fallbacks explicit until G3 and
  G7 retire them
