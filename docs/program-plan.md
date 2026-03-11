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
| G5 | Lower generated artifacts into Metal runtime execution plans | `completed` | generated components drive trace, evaluation, lookup, quotient, FRI, and commitment scheduling through backend planning surfaces |
| G6 | Separate benchmark lanes and optimize the right rows | `completed` | generic and generated benchmark rows are measured independently and optimization work targets the generated lane explicitly |
| G7 | Retire temporary compatibility shims | `completed` | acceptance-local adapters and example-specific wrappers are removed or clearly reduced to non-architectural fixtures |
| G8 | Harden the contract against `stark-v` workloads | `in_progress` | `stark-v` uses the same generic/generated backend contract successfully |

## Immediate sequencing rules

- do not let new benchmark-local seams redefine the backend contract
- do not widen support claims through example-specific wrappers
- do not claim generated support without a schema-compatible artifact and a
  registered inventory
- keep verifier semantics unchanged across generic and generated lanes
- benchmark generic and generated rows separately once both exist

## Current focus

The active tranche is:

`G8 eighth slice: freeze the support-promotion gate for the vendored stark-v
row`

The active formal basis is:

- [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)
- [`dn-0002-generic-backend-and-codegen-contract.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0002-generic-backend-and-codegen-contract.md)
- [`dn-0003-acceptance-bridge-law-and-ownership.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0003-acceptance-bridge-law-and-ownership.md)
- [`dn-0004-stark-v-hardening-input-and-contract.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0004-stark-v-hardening-input-and-contract.md)
- [`dn-0005-stark-v-attachment-strategy.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0005-stark-v-attachment-strategy.md)
- [`dn-0006-stark-v-generated-minimum-contract.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0006-stark-v-generated-minimum-contract.md)

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

The fifteenth G5 slice is now landed:

- wide-fibonacci witness eligibility and hybrid-FRI lane support now consume
  the private registered execution seed directly instead of the transitional
  public `MetalExecutionAuthority`
- this is the first live helper path that now sits below the public reduced
  execution-law surface
- the next honest G5 work is choosing the first support-bridge path that can
  move below that public surface without breaking the private support-crate
  ownership model

## G6 progress snapshot

The first G6 slice is now landed and executed:

- benchmark JSON now names the benchmark lane explicitly
- the wide-fibonacci sweep/report path now runs deterministically over
  `log_n_instances = 16..23`
- the benchmark harness once again validates the current lane-aware outputs and
  no longer hard-fails on missing historical artifact JSON unless explicitly
  asked to do so
- the first real generated-metal comparison table now exists from live
  artifacts, with crossover ahead of SIMD at `log_size = 16` and `18`, near
  parity at `17`, and a scaling deficit from `19` onward

The next honest G6 work is:

The second G6 slice is now landed and measured:

- `wide_fibonacci_prove` now supports a real `generic-metal` lane in addition
  to `generated-metal`
- the generic lane uses the upstream example-backed backend path rather than a
  generated benchmark-specific fast path
- the first measured generic row at `log_size = 16` is dramatically slower
  than the generated lane, so the generic sweep is now intentionally bounded
  while G6 keeps the dual-lane comparison honest and executable

The third G6 slice is now landed:

- one dual-lane report now consumes the generated full-range table and the
  bounded generic table together without conflating them
- the optimization target is now explicit in the report itself: `generated-metal`
- G6 can now treat the generic lane as a correctness-and-coverage benchmark row
  while continuing to optimize the generated lane against the current scaling
  deficit from `log_size = 19` onward

The fourth G6 slice is now landed:

- one dual-lane runner now emits generated, generic, and combined wide-fibonacci
  benchmark artifacts in one deterministic command
- the benchmark contract is now complete enough to treat G6 as done: the lanes
  are separate in schema, separate in artifacts, and explicit in the report
- the generated lane remains the active optimization target, while the generic
  lane remains a bounded correctness-and-coverage benchmark row

The first G7 slice is now landed:

- the former private support bridges now live in one fixture-owned shim crate
  under `fixtures/stwo-metal-fixture-shims`
- the acceptance harness and standalone benchmark fixture both consume that
  fixture-owned shim crate
- the old `support/` bridge crates are retired from the live dependency graph

The next honest G8 work is:

The first G8 slice is now landed:

- one pinned downstream `stark-v` hardening note now records the current HEAD
  input and the minimum prove/verify/preprocess contract
- one deterministic checker now validates the minimum downstream contract shape
  against a local `stark-v` checkout

The next honest G8 work is:

- decide whether the first executable hardening row is generic substitution,
  generated mapping, or explicit fail-closed unsupported status
- keep the public backend API unchanged while the first downstream hardening
  slice lands

The sixteenth G5 slice is now landed:

- the benchmark prove-values bridge now validates one workspace-private
  `WideFibonacciProveValuesLane` contract and stages from that lower contract
  instead of consuming `MetalExecutionAuthority` directly
- the wide-fibonacci benchmark row now constructs that private lane contract at
  the edge and hands the live helper only the validated support-bridge seam it
  actually needs
- the next honest G5 work is deciding whether the upstream acceptance lane is
  the next public-authority consumer to lower

The seventeenth G5 slice is now landed:

- the upstream acceptance bridge now validates one workspace-private
  `AcceptanceMetalLane` contract, and the acceptance fixtures construct that
  lane at the edge instead of handing the bridge a richer workload boundary
- the acceptance matrix stayed green on that narrower lane contract for
  `wide_fibonacci`, `state_machine`, `blake`, and `xor`
- the next honest G5 work is identifying the remaining direct
  `MetalExecutionAuthority` consumers and deciding whether the transitional
  public surface can now shrink

The eighteenth G5 slice is now landed:

- fixture edges no longer construct bridge lanes from `MetalExecutionAuthority`
  directly; both the benchmark prove-values path and the upstream acceptance
  path now enter their private support crates through boundary-based
  constructors only
- the dead transitional planning helpers that lost all live callers after that
  lowering have been removed instead of being kept behind `allow(dead_code)`
- the next honest G5 work is enumerating the remaining direct
  `MetalExecutionAuthority` consumers and deciding whether any part of the
  transitional public compatibility surface can now shrink

The nineteenth G5 slice is now landed:

- the redundant root-level companion export for `MetalExecutionAuthority` is
  now removed; callers that truly need the type must take it from the
  workload-facing module where the boundary methods already live
- the private support crates now compile against that workload-scoped path, so
  the workload surface remains the single semantic home for execution-law
  visibility
- the next honest G5 work is enumerating the remaining direct
  `MetalExecutionAuthority` consumers after this root-surface shrink and
  deciding which one can move lower or disappear next

The twentieth G5 slice is now landed:

- the private benchmark and acceptance support crates no longer import or
  validate through `MetalExecutionAuthority`; they now read directly from the
  workload and benchmark boundaries they already receive
- that leaves `MetalExecutionAuthority` with only workload- and benchmark-API
  consumers plus the workload-scoped companion export
- the next honest G5 work is deciding whether one of those remaining public
  consumers can shrink without lying about the current workload boundary law

The twenty-first G5 slice is now landed:

- `MetalWideFibonacciBenchmarkBoundary` no longer exposes its own redundant
  `execution_authority()` pass-through; benchmark callers now go through the
  workload boundary if they truly need that transitional type
- that leaves `MetalWorkloadBoundary` as the only live public API owner of
  `execution_authority()`, apart from tests that still pin the current
  workload-law contract
- the next honest G5 work is deciding whether `MetalWorkloadBoundary` can
  retire `execution_authority()` entirely in favor of its narrower workload-law
  methods without obscuring stage ownership semantics

The twenty-second G5 slice is now landed:

- `MetalExecutionAuthority` is retired entirely; `MetalWorkloadBoundary` now
  owns execution law directly through `plan()`, `stage_assignments()`, and
  `stage_ownership()`
- workload, benchmark, and public-surface tests now pin that narrower law
  directly instead of going through an intermediate authority object
- the next honest G5 work is deciding whether `stage_assignments()` still
  belongs on the public workload surface, or whether callers only need
  per-stage ownership

The twenty-third G5 slice is now landed:

- `stage_assignments()` and the public `MetalWorkloadStageAssignment` companion
  export are gone; the workload-facing public law is now just `plan()` and
  `stage_ownership()`
- the richer stage-assignment slice remains internal to the generated manifest,
  artifact registry, and execution-plan layers where lowering still needs it
- the next honest G5 work is to move back up from surface cleanup and pick the
  next runtime helper that should depend on the lower generated contract rather
  than the boundary-shaped API

The twenty-fourth G5 slice is now landed:

- the generated execution seed now owns the shared wide-fibonacci witness-shape
  law, including equal-length, power-of-two, and minimum-column validation
- workload and benchmark boundaries now delegate that shared witness-shape law
  to the lower generated contract instead of duplicating it locally
- the next honest G5 work is to pick the next boundary-owned staging rule that
  still duplicates lower generated logic after this witness-shape move

The twenty-fifth G5 slice is now landed:

- the wide-fibonacci benchmark boundary now owns prove-values lane validation
  through its generated execution seed instead of leaving that law in the
  private benchmark support bridge
- the private benchmark prove-values bridge now consumes a validated benchmark
  lane and only maps the resulting narrow error surface, so it no longer
  re-checks benchmark plan or stage ownership on its own
- the next honest G5 work is to lower the remaining acceptance-lane and
  workload-side staging rules that still depend on boundary-local plan checks
  or duplicated error mapping

The twenty-sixth G5 slice is now landed:

- `MetalWorkloadBoundary` now owns acceptance-lane Metal-capability validation
  inside `stwo-metal` instead of leaving that rule inside the private upstream
  bridge crate
- the private upstream acceptance bridge now consumes the validated acceptance
  lane and only maps the narrow error surface it needs, so it no longer
  re-checks workload plan law on its own
- the next honest G5 work is to lower the next workload-side staging or
  error-mapping rule that still duplicates generated seed law above the
  boundary surface

The twenty-seventh G5 slice is now landed:

- the remaining workload-side handoff translation now runs through one shared
  generated-seed-to-boundary mapper instead of separate witness and
  evaluation-specific translation helpers
- workload FRI-ready, quotient-evaluation, and wide-fibonacci witness handoffs
  now all delegate their seed-law mapping through the same boundary path
- G5 is now complete: generated proving components drive runtime planning and
  live staging through the stable planning boundary instead of through local
  bridge-owned or boundary-duplicated law

## G6 progress snapshot

The first G6 slice is now active:

The first G6 slice is now landed:

- standalone benchmark JSON now carries an explicit `benchmark_lane` field
  instead of relying on `classification` and `dependency_row` alone
- a deterministic wide-fibonacci Metal sweep script now exists for
  `log_n_instances = 16..23`
- a dedicated comparison-table renderer now exists for the wide-fibonacci
  SIMD-vs-Metal generated lane report shape
- the next honest G6 work is to run and stabilize that sweep on real artifacts
  and then decide how the future generic lane plugs into the same table/report
  path

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
