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
| G2 | Build the backend planning and registration surface | `completed` | `stwo-metal` has a stable internal artifact-registry and execution-plan boundary with deterministic schema checks |
| G3 | Move acceptance coverage onto the stable generic path | `completed` | acceptance coverage no longer depends on architecture-local example shims where shared backend boundaries should exist |
| G4 | Land generated fast-path registration and ABI inventory | `completed` | generated artifacts register component identity, ABI, build inventory, and specialization keys through a stable surface |
| G5 | Lower generated artifacts into Metal runtime execution plans | `in_progress` | generated components drive trace, evaluation, lookup, quotient, FRI, and commitment scheduling through backend planning surfaces |
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

`G5 fifteenth slice: finish moving workload-side execution-law checks onto the
reduced authority contract, then decide the next lower private seam`

The active formal basis is:

- [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)
- [`dn-0002-generic-backend-and-codegen-contract.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0002-generic-backend-and-codegen-contract.md)
- [`dn-0003-acceptance-bridge-law-and-ownership.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0003-acceptance-bridge-law-and-ownership.md)

## Current implementation obligations under G3

- preserve the non-public bridge laws from
  [`dn-0003-acceptance-bridge-law-and-ownership.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0003-acceptance-bridge-law-and-ownership.md)
- keep the private support-crate ownership boundary explicit until G7 retires
  or upstreams the current compatibility bridges
- keep bounded CPU fallbacks explicit while the shared bridge remains non-public

## Current implementation obligations under G4

- widen generated registration to include explicit ABI inventory and
  specialization metadata beyond route eligibility
- keep generated support fail-closed when ABI inventory or specialization
  metadata is missing
- route new generated registration queries through the existing stable
  artifact-registry and execution-plan seam instead of adding side tables

## G4 progress snapshot

The first G4 slice is now landed:

- the generated artifact registry now exposes explicit ABI symbols in addition
  to ABI family, build modules, and witness hooks
- the generated artifact registry now records explicit specialization keys for
  registered components
- that richer inventory is queryable through the same stable internal
  registry boundary instead of requiring route lookup first

The second G4 slice is now landed:

- workload boundaries now retain the generated inventory they were declared
  from instead of dropping back to route-only metadata
- the wide-fibonacci benchmark boundary now validates itself against generated
  specialization inventory rather than treating route support as sufficient
- the richer generated inventory is now active declaration input, not passive
  manifest data

The third G4 slice is now landed:

- one lowering-facing generated registration object now exists in code
- workload and benchmark declarations can consume checked generated inventory
  through that shared registration object instead of reassembling raw manifest
  fields
- G4 is now complete and the next honest work moves to G5 lowering

## G5 progress snapshot

The first G5 slice is now landed:

- one checked lowering-facing generated registration input now derives from the
  shared registry seam
- the lowering path no longer needs to reassemble workload family, ABI family,
  ABI symbols, build modules, witness hooks, and specialization keys from raw
  manifest fragments

The second G5 slice is now landed:

- one runtime-planning helper now consumes the canonical lowering-facing
  registration input directly
- prove-plan selection no longer bypasses the lowering input by jumping
  straight from registrations to raw component plan inputs

The third G5 slice is now landed:

- workload-boundary declaration now consumes one shared workload-boundary
  lowering input derived from the canonical lowering and runtime-planning
  inputs
- the first broader lowering-oriented execution helper now uses the same
  generated seam instead of rebuilding route state locally

The fourth G5 slice is now landed:

- benchmark declaration now consumes one shared benchmark-boundary input
  derived from the same generated seam as workload-boundary declaration
- benchmark routing no longer mixes direct registration lookup with
  independently composed workload-boundary state

The fifth G5 slice is now landed:

- workload and benchmark declaration now share one reusable generated
  execution-binding helper
- the next honest G5 step is runtime scheduling on top of that shared binding,
  not more boundary-shape cleanup

The sixth G5 slice is now landed:

- one scheduling-oriented execution seed now derives from the reusable binding
  helper
- the benchmark declaration path consumes that seed, so the next honest G5
  work is a non-declarative execution helper rather than more declaration
  normalization

The seventh G5 slice is now landed:

- both workload and benchmark witness generation now consume the same checked
  execution seed in live non-declarative code
- the first runtime trace helper now lives behind that seed instead of letting
  workload and benchmark paths construct raw witness requests independently
- the next honest G5 work is to widen that same seed into quotient and
  evaluation staging rather than reintroducing a second runtime metadata seam

The eighth G5 slice is now landed:

- CPU FRI-ready evaluation handoff and CPU quotient-evaluation handoff now
  consume the same checked execution seed instead of rebuilding plan and stage
  ownership checks locally
- workload boundaries now treat the execution seed as the single source of
  truth for plan and stage ownership rather than storing a parallel copy of
  that runtime metadata
- the next honest G5 work is the first prove-values staging helper above those
  handoffs

The ninth G5 slice is now landed:

- the wide-fibonacci prove benchmark now declares one generated benchmark
  boundary up front and uses it for both trace generation and prove-values
  staging
- the first live prove-values staging helper now exists above the workload
  handoff layer instead of recomposing its runtime contract entirely inside the
  prove path
- the next honest G5 work is lifting that helper into a shared PCS-facing
  lowering boundary so fixture code stops owning it

The tenth G5 slice is now landed:

- the first prove-values staging helper now lives in a private shared support
  crate instead of inside the benchmark fixture itself
- the wide-fibonacci benchmark row now consumes that shared bridge for
  prove-values staging while keeping the main `stwo-metal` API surface stable
- the next honest G5 work is re-anchoring that shared bridge on the lower
  generated execution contract instead of the benchmark boundary

The eleventh G5 slice is now landed:

- the shared prove-values staging bridge now consumes `MetalWorkloadBoundary`
  directly instead of depending on `MetalWideFibonacciBenchmarkBoundary`
- the benchmark lane is now only a caller of that bridge, not the authority
  for the prove-values staging contract
- the next honest G5 work is moving that bridge below the public workload
  surface and onto the lower generated execution contract itself

The twelfth G5 slice is now landed:

- `stwo-metal` now exposes a minimal `MetalExecutionAuthority` that carries
  only plan and stage-law truth for runtime staging
- the shared prove-values staging bridge now consumes that lower execution-law
  surface instead of depending on `MetalWorkloadBoundary`
- the next honest G5 work is consuming that same reduced execution-law surface
  in another live staging helper so G5 keeps moving downward without widening
  benchmark or workload contracts again

The thirteenth G5 slice is now landed:

- the private upstream-acceptance bridge now validates and carries the reduced
  `MetalExecutionAuthority` instead of treating `MetalWorkloadBoundary` as its
  live lane law
- the benchmark witness boundary now consumes that same reduced
  execution-authority surface, and the wide-fibonacci benchmark row now calls
  it directly
- the next honest G5 work is lowering the remaining workload-side helper
  checks that only need plan and stage law off `MetalWorkloadBoundary`

The fourteenth G5 slice is now landed:

- workload-side hybrid-FRI declaration checks now consume authority-shaped
  helpers instead of treating `MetalWorkloadBoundary` as the live execution-law
  owner
- the wide-fibonacci witness handoff now checks CPU witness ownership through
  the same reduced execution-authority contract
- the next honest G5 work is deciding which live helper moves below the
  transitional public authority and onto the next lower private generated seam

## G2 progress snapshot

G2 is now complete:

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

## G3 progress snapshot

The first G3 slice is now landed:

- the `wide_fibonacci` acceptance row now requires a registered Metal workload
  boundary before constructing its framework-component bridge
- the acceptance harness has one explicit `AcceptanceMetalLane` abstraction
  instead of allowing the first registered workload bridge to float entirely
  outside the shared backend contract

The second G3 slice is now landed:

- `wide_fibonacci`, `state_machine`, `blake`, and `xor` now all declare
  registered Metal workload lanes before constructing their local acceptance
  bridges
- the shared planner manifest now names those acceptance workloads explicitly
  enough to keep lane construction off ad hoc local constants

The third G3 slice is now landed:

- the acceptance harness now exposes one checked `AcceptanceMetalBridgeCatalog`
  instead of a growing set of free bridge constructors
- framework-backed and SIMD-backed acceptance rows now consume that same
  registered bridge catalog
- the attempted crate-owned `stwo-metal` bridge path was rejected as unsafe for
  now because the vendored `constraint-framework` dependency introduces a
  nested-workspace conflict in the current repository layout
- the remaining honest gap is bridge ownership and law definition, not more
  acceptance-lane registration

The verification follow-up to the third G3 slice is now landed:

- the vendored Stwo snapshot now compiles again on the repository-pinned
  nightly after replacing the stale `array_chunks` usage that had drifted out
  of compatibility
- the private `artifact` and `execution_plan` tests are green again
- the acceptance harness unit tests and the current non-blocked example
  prove/verify matrix are green again on `nightly-2025-07-14`

The fourth G3 slice is now landed:

- the registered acceptance bridge catalog no longer lives in the acceptance
  harness crate itself
- the current framework-backed and SIMD-backed bridge implementations now live
  in a private shared support crate outside the main workspace, which avoids
  the vendored nested-workspace conflict while keeping the bridge non-public
- the acceptance harness now re-exports that private bridge boundary and keeps
  examples in the role of acceptance fixtures instead of ownership anchors
- the remaining honest gap moves from bridge ownership to generated ABI and
  specialization inventory for G4

## Foundation already available

The new path starts from a real base rather than a blank implementation slate:

- Apple Silicon host/runtime foundation already exists
- mirrored Metal native hot-path structure already exists
- current non-blocked upstream acceptance rows already pass through
  `MetalBackend`
- `poseidon` remains tracked as an upstream protocol blocker rather than a
  Metal-backend gap
