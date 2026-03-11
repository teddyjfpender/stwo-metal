# Roadmap

## Purpose

This file is the long-range map for `stwo-metal`.

Inputs:

- the current `stwo-metal` repository state
- the local vendored Stwo snapshot under `vendor/`
- the accepted architecture contract in
  [`dn-0002-generic-backend-and-codegen-contract.md`](./dn-0002-generic-backend-and-codegen-contract.md)
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
| G5 | Lower generated artifacts into Metal runtime execution plans | `in_progress` | generated proving components drive Metal trace, evaluation, lookup, quotient, FRI, and commitment scheduling through the stable planning boundary |
| G6 | Separate benchmark lanes and optimize against the right target | `planned` | generic and generated benchmark rows are measured separately and optimization work no longer conflates them |
| G7 | Retire temporary compatibility shims | `planned` | acceptance-local adapters and example-specific wrappers are removed or reduced to non-architectural fixtures |
| G8 | Harden the contract against `stark-v` workloads | `planned` | a real downstream Stwo consumer uses the same generic/generated contract successfully |

## Active work definition

The next active implementation work is not “more example multiplication” and
not “more benchmark-local seams.”

The next active work is:

- lower one generated registration into a stable planning input for runtime-
  facing execution-plan work
- consume that lowering-facing input in one runtime-planning helper before
  widening lowering further
- use that canonical runtime-planning input in the first broader lowering-
  oriented execution helper beyond prove-plan selection
- keep the benchmark declaration path attached to that same generated seam
  instead of composing route validation around it separately
- package the shared generated boundary inputs into one reusable execution-
  binding helper for later scheduling work
- derive the first scheduling-oriented execution seed from that reusable
  binding helper without widening runtime policy prematurely
- consume that scheduling seed in the first non-declarative execution helper
  so runtime scheduling starts from one canonical seed
- widen that same execution seed into the next quotient or evaluation staging
  helper instead of letting later runtime helpers rebuild route metadata
- derive the first prove-values staging helper from that same canonical
  generated seed or binding instead of recomposing PCS staging metadata
- lift the first prove-values staging helper into a shared PCS-facing lowering
  boundary instead of leaving it owned by benchmark fixtures
- re-anchor the shared prove-values staging bridge on the lower generated
  execution contract instead of the benchmark boundary
- move that shared prove-values staging bridge below the public workload
  surface so generated runtime authority remains private and linear
- replace public workload-boundary dependence with a minimal execution-law
  surface that carries only plan and stage ownership for live staging helpers
- consume that reduced execution-law surface in more than one live staging
  helper before deciding whether it remains public beyond G5
- lower the remaining workload-side helpers that only need execution law so
  `MetalWorkloadBoundary` stops being the default live staging authority
- pick the first live helper that can move below the transitional public
  authority and onto the next lower private generated contract
- once one live helper sits below the public authority, decide which
  support-bridge path moves next without violating the private support-crate
  boundary
- once the first support-bridge path is lowered, identify the next remaining
  direct consumer of `MetalExecutionAuthority` and continue shrinking that set
- once benchmark and acceptance support-bridge paths both stage from lower
  private contracts, re-evaluate whether `MetalExecutionAuthority` can shrink
  or disappear from some public paths entirely
- once fixture edges no longer pass `MetalExecutionAuthority` into those
  support bridges, remove any planning helpers that no longer have live
  callers instead of preserving dormant transition layers
- after that dead-surface cleanup, enumerate the remaining direct
  `MetalExecutionAuthority` consumers and pick the next public or workspace-
  private contract that can shrink safely
- once that consumer set is smaller, remove any redundant companion-surface
  export path that duplicates the workload-facing home of execution-law types
- once private support crates no longer depend on the authority type, make the
  next G5 decision explicitly about workload and benchmark public law rather
  than private bridge validation glue
- once the benchmark boundary no longer exposes a redundant authority
  pass-through, decide explicitly whether the workload boundary should retain
  `execution_authority()` or collapse onto its narrower workload-law methods
- once execution law has collapsed onto workload methods, decide whether the
  full `stage_assignments()` slice is still required publicly or whether
  `stage_ownership()` is the stable semantic unit
- once that public-law cleanup is complete, move G5 back to generated lowering
  and runtime planning rather than doing more surface-only shrink work
- when a workload and benchmark boundary still duplicate the same runtime law,
  move that shared law down onto the generated execution seed before widening
  outward again
- keep the bridge-law surface non-public and private while generated lowering
  grows above it
- keep examples only as validation and benchmark inputs

## Native runtime direction

The mirrored Metal runtime port remains in scope, but it now serves the
generic/generated backend contract rather than defining it.

Runtime rules:

- use `.metal` for hot kernels
- keep native ownership and ABI explicit
- prefer reusable proving-operation kernels over workload-specific kernels
- let the execution-plan boundary decide how kernels are composed for a given
  artifact

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
