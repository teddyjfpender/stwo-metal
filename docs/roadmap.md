# Roadmap

## Purpose

This file is the long-range map for `stwo-metal`.

Inputs:

- the current `stwo-metal` repository state
- the local vendored Stwo snapshot under `vendor/`
- the accepted architecture contract in
  [`dn-0002-generic-backend-and-codegen-contract.md`](./dn-0002-generic-backend-and-codegen-contract.md)
- the V1 Metal execution contract in
  [`dn-0008-metal-evaluation-program-v1.md`](./dn-0008-metal-evaluation-program-v1.md)
- the engineering requirement that examples act as acceptance workloads rather
  than the implementation strategy

Outputs:

- the architecture direction for the backend
- the milestone sequence for generic and generated support
- the rules separating correctness, production, and benchmark work

## Product definition

`stwo-metal` is the Apple Silicon and Metal implementation of a generic Stwo
GPU backend architecture.

The product goal is:

- Stwo or a Stwo-owned framework/codegen layer produces a proving workload
- `stwo-metal` proves that workload on GPU through a generic or generated lane
- the stock verifier verifies the proof unchanged

This repository remains Metal-specific in implementation, but the contract is
intentionally backend-family generic so the same producer/consumer model can
inform sibling backends such as `stwo-cuda`.

## Architecture direction

The accepted architecture has three layers:

1. `Generic backend substitution`
   - correctness lane
   - thin backend wiring through public Stwo proving surfaces
2. `Generated fast path`
   - production lane
   - Stwo/framework/codegen emits a machine-readable proving artifact consumed
     by the backend
3. `Example-specific wrappers`
   - temporary compatibility shims only
   - tracked as debt and never treated as the architecture

The architectural source of truth is the proving-component contract, not any
single example or benchmark harness.

## Planning rules

- Examples are the test matrix, not the implementation strategy.
- Benchmarks measure the generic lane and generated lane separately.
- Generated support must fail closed when the artifact is absent or
  incompatible.
- Generated outputs must remain durable and hand-tunable.
- The vendored Stwo snapshot remains the semantic authority.
- Deterministic CPU-oracle validation remains the default correctness gate for
  bounded Metal work.

## Acceptance matrix

The examples remain valuable, but only as acceptance workloads.

| Workload | Role | Current state | Long-term role |
| --- | --- | --- | --- |
| `blake` | acceptance | `complete` | validates lookup-heavy generic/backend support |
| `poseidon` | acceptance | `blocked_upstream_protocol` | validates lifted-protocol compatibility once upstream permits it |
| `state_machine` | acceptance | `complete` | validates multi-tree and multi-component support |
| `wide_fibonacci` | acceptance and perf reference | `complete` | validates generic path and supports benchmark tracking |
| `xor` | acceptance | `complete` | validates mixed-component and MLE/GKR support |
| `stark-v` | future hardening workload | `planned` | validates the stable contract against a real downstream consumer |

## Benchmark lane

The benchmark north star remains:

- `wide_fibonacci_prove_verify_v1`
- `log_n_instances = 20`
- `n_columns = 100`
- reference goal: approach the project-supplied `90 ms` RTX 4090 row

Benchmark rules:

- benchmark the generic passing subset separately from the generated fast path
- do not use benchmark-only wrappers to define the backend contract
- treat performance work as subordinate to the frozen contract

## Producer/consumer contract

The backend target is no longer “arbitrary Rust workload inference.”

The target contract is:

- generic over Stwo-defined and codegen-defined proving components
- driven by a machine-readable producer artifact
- consumed through a backend registration and execution-plan boundary

The required fields and laws live in:

- [`dn-0002-generic-backend-and-codegen-contract.md`](./dn-0002-generic-backend-and-codegen-contract.md)
- [`dn-0008-metal-evaluation-program-v1.md`](./dn-0008-metal-evaluation-program-v1.md)

## Milestone map

This roadmap is the active planning source.

The superseded `T0` through `T8` sequence now lives in:

- [`milestone-archive.md`](./milestone-archive.md)

| Order | Milestone | Status | Exit condition |
| --- | --- | --- | --- |
| G0 | Freeze the generic backend contract | `completed` | the architecture distinguishes generic substitution, generated fast path, and temporary wrappers |
| G1 | Freeze the codegen input schema and fail-closed contract | `completed` | the required producer artifact, consumer subset, and unsupported-component behavior are specified |
| G2 | Build the backend planning and registration surface | `completed` | `stwo-metal` owns a stable internal artifact-registry and execution-plan boundary with explicit schema compatibility checks |
| G3 | Move acceptance coverage onto the stable generic path | `completed` | example-backed support no longer depends on architecture-local example shims where shared backend boundaries should exist |
| G4 | Land the generated fast-path registration and ABI inventory | `completed` | generated artifacts can register component identity, ABI, build inventory, and specialization keys through a stable surface |
| G5 | Lower generated artifacts into Metal runtime execution plans | `completed` | generated proving components drive Metal trace, evaluation, lookup, quotient, FRI, and commitment scheduling through the stable planning boundary |
| G6 | Separate benchmark lanes and optimize against the right target | `completed` | generic and generated benchmark rows are measured separately and optimization work no longer conflates them |
| G7 | Retire temporary compatibility shims | `completed` | acceptance-local adapters and example-specific wrappers are removed or reduced to non-architectural fixtures |
| G8 | Harden the contract against `stark-v` workloads | `iced` | a real downstream Stwo consumer uses the same generic/generated contract successfully |
| G9 | Freeze and implement `MetalEvaluationProgramV1` | `in_progress` | generated and generic Metal execution both consume the same validated lowered program contract |
| G10 | Migrate benchmark-specialized rows onto the V1 program contract | `in_progress` | the active generated benchmark path is driven by the V1 artifact and overlay contract rather than bespoke benchmark-only lowering |
| G11 | Re-open downstream hardening on the V1 contract | `planned` | downstream consumers such as `stark-v` are evaluated against the V1 program contract instead of a pre-V1 bridge surface |

## Active work definition

The next active implementation work is not “more example multiplication,” not
“more benchmark-local seams,” and not more `stark-v` hardening while the V1
program contract is still unimplemented.

The next active work is:

- move one live generated prove-phase boundary in `wide_fibonacci_prove` from
  benchmark-specialized staging onto the already validated
  `MetalEvaluationProgramV1` runtime contract
- keep the remaining fixture shims as non-authoritative wrappers once the same
  benchmark law exists inside `stwo-metal`
- expand the live generated benchmark row from “selected V1 side execution plus
  backend-owned prove core and backend-owned generated sample orchestration”
  toward end-to-end selected V1 runtime ownership; prove core is now
  selected-runtime-owned for composition generation rather than merely
  selected-runtime-gated, the binary is no longer the main generated-sample or
  generated-iteration authority, and the generated lane's warmup/timed run
  orchestration now also enters through the backend boundary
- keep widening G10 from the now-landed post-composition sampled-values ABI:
  the repository has a correctness-first reference lane for the live generated
  post-composition shape, and the next step is the Metal runtime lane plus
  migration of the remaining prove-values/decommit flow onto that contract
- keep the migrated selected-runtime authority measurable and optimize it once
  it becomes part of the live prove path, instead of treating V1 as a free side
  check
- widen the first generated overlay registration into a durable overlay law
  instead of leaving it as one benchmark-shape registration
- keep examples as the acceptance matrix for the generic and generated lanes
- keep `stark-v` iced until the V1 contract is available to test downstream
  honestly

## Native runtime direction

The mirrored Metal runtime port remains in scope, but it now serves the
generic/generated backend contract rather than defining it.

Runtime rules:

- use `.metal` for hot kernels
- keep native ownership and ABI explicit
- prefer reusable proving-operation kernels over workload-specific kernels
- let the execution-plan boundary decide how kernels are composed for a given
  artifact
- keep host/device ABI records `#[repr(C)]`, fixed-width, and reflection-checked

## Upstream skill alignment

`stwo-metal` adopts the upstream Stwo skill registry as a process input for
domain vocabulary, testing strategy, and soundness review.

Minimum required alignment:

- use the upstream skill registry as the entry point for theory and review
  loading
- use the testing strategy and soundness guidance for any contract or
  soundness-sensitive backend change
- keep design-note vocabulary aligned with Stwo’s proving terminology

## Program invariants

- Public backend interfaces remain minimal and stable.
- Proof semantics do not change unless an approved design note says they do.
- Examples validate the backend; they do not define it.
- Generated paths are optional accelerators layered on a truthful generic lane.
- Unsupported generated components fail closed.
- Performance work must follow a working correctness path and must identify
  whether it measures the generic lane or generated lane.
- The long-term generated lane is defined by `MetalEvaluationProgramV1`, not by
  benchmark-local specialization.
