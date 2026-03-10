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

This queue is the active program plan.

The superseded `T0` through `T8` sequence now lives in:

- [`milestone-archive.md`](./milestone-archive.md)

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

`G2 second slice: widen the new private artifact-registry and execution-plan
seam beyond exemplar prove planning so more of the backend routes through one
shared fail-closed boundary`

The active formal basis is:

- [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)
- [`dn-0002-generic-backend-and-codegen-contract.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0002-generic-backend-and-codegen-contract.md)

## Current implementation obligations under G2

- widen the internal Rust representation of the producer artifact beyond the
  current planner-manifest subset
- extend schema/version compatibility checks across the broader registration
  path
- widen the generated inventory and registration manifest boundary so it can
  carry more than current exemplar prove metadata
- extend the execution-plan lowering boundary from prove planning toward the
  broader backend work graph
- keep unsupported-component behavior and tests explicit as the seam expands
- keep acceptance-local adapters and bounded CPU fallbacks explicit until G3 and
  G7 retire them

## G2 progress snapshot

The first G2 slice is now landed:

- a private `ArtifactRegistry` boundary exists in code
- a private `ExecutionPlan` lowering seam exists in code
- schema mismatch and unknown-component behavior are deterministic and tested
- the public planner and workload declarations now route through that seam
- benchmark declarations now also validate against registry-backed workload
  family and operation metadata
- workload-stage ownership metadata now lives in shared contract types and the
  generated registration manifest rather than in a second workload-local table
- per-component generated inventory now records registration key, ABI family,
  build inventory, and witness hooks through the same registry seam
- generated-route compatibility is now explicit and fail-closed for registered
  prove, workload-boundary, and declared benchmark routes
- workload and benchmark declarations now consume generated-route compatibility
  through the shared execution-plan seam rather than reaching into the registry
  directly
- one explicit unsupported-generated-component policy path now exists in code
  and is tested privately beneath the execution-plan layer

## Foundation already available

The new path starts from a real base rather than a blank implementation slate:

- Apple Silicon host/runtime foundation already exists
- mirrored Metal native hot-path structure already exists
- current non-blocked upstream acceptance rows already pass through
  `MetalBackend`
- `poseidon` remains tracked as an upstream protocol blocker rather than a
  Metal-backend gap
