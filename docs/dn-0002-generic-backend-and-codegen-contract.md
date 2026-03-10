# DN-0002: Generic Backend and Codegen Contract

- Status: `accepted`
- Date: `2026-03-10`
- Owners: `project team`
- Related roadmap items:
  - [`roadmap.md`](./roadmap.md)
  - [`program-plan.md`](./program-plan.md)

## Summary

`stwo-metal` must stop treating example workloads as the architecture surface.
The long-term product is a generic Stwo GPU backend with two execution lanes:

- a correctness-oriented generic backend-substitution lane
- a performance-oriented generated fast path fed by Stwo/framework/codegen
  artifacts

Examples remain important, but only as an acceptance matrix.

This design note freezes the producer/consumer contract that the backend should
target:

- Stwo or Stwo codegen produces a machine-readable proving artifact
- the GPU backend consumes that artifact through a stable registration and
  execution-plan boundary
- generated outputs remain durable and hand-tunable
- missing generated support fails closed instead of silently widening support
  claims

Although this repository implements `stwo-metal`, the contract is intentionally
backend-family generic so it can inform sibling GPU backends such as
`stwo-cuda`.

## Problem

The repository has learned a useful but incomplete lesson from the first Metal
bring-up:

- upstream examples are excellent acceptance workloads
- they are not the right long-term integration surface

If the backend keeps growing through example-specific wrappers or bespoke
benchmark seams, it will accumulate local wins without establishing a durable
GPU proving interface for Stwo-defined proving components.

The missing architectural freeze is:

- what exact artifact the Stwo producer emits
- what exact subset of that artifact the GPU backend consumes
- which operations remain generic
- which operations are generated
- what happens when generated support does not exist

## Scope

This note specifies:

- the architectural layers for Stwo GPU backends
- the required producer artifact shape
- the stable consumer boundary expected by `stwo-metal`
- fail-closed behavior
- the distinction between acceptance, generic correctness, and generated
  performance work
- the role of hand tuning after generation

This note does not specify:

- one final Rust type layout for every artifact struct
- one final on-disk serialization format
- one final Metal kernel ABI for every operation
- one final `stark-v` integration plan

## Goals

- Make the backend generic over Stwo-defined and codegen-defined proving
  components, not over named examples.
- Preserve a thin correctness lane where backend substitution works with the
  smallest stable public Stwo proving surfaces.
- Introduce a generated fast path that scales to framework/codegen-produced
  AIRs.
- Keep generated outputs inspectable, durable, and hand-tunable.
- Separate acceptance fixtures from backend architecture.
- Keep verifier behavior unchanged.

## Non-goals

- promise efficient support for arbitrary Rust code with no Stwo metadata
- generate one bespoke GPU subsystem per example
- silently fall back to CPU inside a claimed GPU proving row
- let benchmark harnesses define the long-term backend API

## Architecture layers

### 1. Generic backend substitution

This is the correctness lane.

The backend should slot into the public generic Stwo proving interfaces when a
workload already fits them cleanly.

Properties:

- widest semantic coverage
- smallest amount of producer metadata
- thinnest possible backend wiring
- not assumed to be the fastest lane

Responsibilities:

- prove correct Stwo-defined workloads through public backend traits
- keep failure behavior explicit when a component exceeds the supported generic
  path
- provide the baseline acceptance path for examples and future workloads

### 2. Generated fast path

This is the production lane.

For framework/codegen-defined AIRs, the producer must emit a machine-readable
artifact that describes the proving component sufficiently for the GPU backend
to register and drive optimized execution.

Properties:

- narrower but faster than the generic path
- driven by durable metadata rather than example-specific glue
- allowed to emit generated Rust registration, ABI declarations, build
  manifests, and kernel stubs or adapters
- must remain inspectable and hand-tunable

### 3. Example-specific wrappers

These are temporary compatibility shims and acceptance fixtures.

Properties:

- minimized
- not part of the stable architecture
- explicitly tracked as debt when needed

Use cases:

- upstream public seams are incomplete
- framework adapters are needed temporarily to validate backend behavior
- acceptance coverage must move before the stable shared boundary is ready

## Required producer artifact

The producer is Stwo itself, or a Stwo-owned framework/codegen layer.

The backend must not infer this information from ad hoc workload-specific Rust
types when a stable artifact could provide it directly.

The required conceptual artifact is `GpuProvingArtifact`.

### Artifact fields

Each produced component artifact must provide at least:

- `schema_version`
  - producer/consumer compatibility gate
- `producer_identity`
  - Stwo version, codegen version, and source provenance
- `backend_family`
  - the intended generated target family, if narrowed
- `component_identity`
  - stable component name, hash, and semantic version
- `trace_layout`
  - column groups, widths, domains, offsets, packing, and row counts
- `mask_layout`
  - mask families, offsets, arity, and access pattern metadata
- `evaluation_abi`
  - evaluation entrypoint identity, argument layout, challenge layout, result
    layout, and field kinds
- `lookup_layout`
  - lookup families, multiplicity behavior, compression, and table ownership
- `degree_and_lifting_profile`
  - degree bounds, lifting mode, and protocol constraints required for proving
- `domain_material`
  - twiddle requirements, coset/domain shape, and factor-generation needs
- `commitment_layout`
  - tree families, leaf packing rules, hash family, and decommit ordering
- `query_layout`
  - query grouping, point-evaluation needs, and decommit extraction rules
- `witness_hooks`
  - optional named witness-generation hooks and ownership boundaries
- `specialization_keys`
  - dimensions that allow generated fast-path specialization without changing
    semantics
- `generated_inventory`
  - generated module names, ABI symbols, manifest entries, and build ownership

### Artifact laws

- The artifact must describe proving semantics, not opaque executable policy.
- The artifact must be stable enough to check into source control or otherwise
  persist for review.
- The artifact must be sufficient for the backend to reject unsupported
  components without heuristic type inspection.
- The artifact must not require the backend to inspect arbitrary Rust AST or
  workload-specific implementation details.

## Consumer contract for `stwo-metal`

`stwo-metal` consumes the artifact through two narrow internal boundaries:

- `ArtifactRegistry`
  - stores known generated component registrations and compatibility metadata
- `ExecutionPlan`
  - lowers one artifact plus runtime dimensions into backend operations and
    kernel dispatch choices

The exact Rust type names may differ, but the contract must preserve the
following responsibilities.

### Artifact registry responsibilities

- schema compatibility checks
- component identity lookup
- generated inventory lookup
- generated-versus-generic capability checks
- fail-closed registration errors
- provenance retention for debugging and review

### Execution-plan responsibilities

- resolve generic versus generated execution for each proving component
- bind trace, mask, query, quotient, FRI, lookup, and commitment operations to
  backend-owned resources
- keep runtime policy private to the backend
- expose only stable plan inputs and outputs to callers

## Generic versus generated operations

The backend contract must state which operations are always generic and which
may be generated or specialized.

### Remain generic by default

- verifier-facing proof format and verification semantics
- backend trait contracts that already fit the public Stwo proving surface
- channel semantics and transcript ownership
- fail-closed capability reporting
- deterministic CPU-oracle parity validation
- benchmark harness contracts and measurement metadata

### Eligible for generated fast-path support

- component registration
- evaluation ABI shims
- mask and trace access mapping
- lookup packing and multiplicity layout
- quotient scheduling and accumulation layout
- FRI schedule specialization
- Merkle leaf packing and commitment layout
- decommit packing and witness extraction
- runtime build inventory and kernel registration
- workload-specific witness hooks when explicitly declared by the producer

## Fail-closed behavior

This project must not silently claim GPU proving support when the required
generated path does not exist.

Required behavior:

1. If a workload fits the generic backend-substitution lane, use that lane.
2. If the workload requires generated support and no compatible artifact is
   registered, return an explicit unsupported-component error.
3. If a generated artifact exists but is schema-incompatible, reject it.
4. If a generated artifact exists but the target backend family lacks the
   required kernels or registrations, reject it.
5. If a temporary compatibility shim is used, name it explicitly in docs and
   track it as debt.

Not allowed:

- silent CPU fallback inside a claimed GPU proving row
- implicit example-specific dispatch chosen by ad hoc type inspection
- widening public support claims because one local acceptance wrapper worked

## Generated output and hand tuning

The generated fast path must support later human optimization.

Requirements:

- generated output is durable and reviewable
- generated output has a stable manifest and provenance record
- generated and hand-tuned code can coexist without losing semantic identity
- hand tuning must preserve the producer artifact contract
- a hand-tuned replacement must still declare which generated artifact it
  satisfies

This means codegen should emit stable names, manifests, and ABI declarations
that can be patched deliberately rather than hidden inside ephemeral build
steps.

## Examples and benchmarks

Examples are the acceptance matrix, not the implementation strategy.

Current acceptance class:

- `blake`
- `poseidon`
- `state_machine`
- `wide_fibonacci`
- `xor`

Future hardening class:

- `stark-v` workloads, once the generic contract and generated fast path are
  stable enough to serve as a real consumer

Benchmark rules:

- benchmark the generic passing subset separately from the generated fast path
- do not present example-specific wrappers as production architecture
- treat `wide_fibonacci` as a performance reference, not as the API source of
  truth

## Invariants

- Public backend interfaces stay minimal and stable.
- The semantic authority remains the vendored Stwo snapshot.
- Examples validate the backend; they do not define it.
- Generated paths are optional accelerators on top of a truthful generic lane.
- Every unsupported generated component fails closed.
- Hand-tuned fast paths remain tied to the same artifact contract.

## Initial consequences for `stwo-metal`

- Acceptance-local adapters must be retired into a shared boundary or replaced
  by the generic/generated contract.
- Future milestone sequencing must separate:
  - acceptance examples
  - generic backend path
  - generated fast path
- Benchmark work remains valuable, but it must follow the contract rather than
  define it.
- `stark-v` is a future hardening workload, not the first architecture source
  of truth.

## Validation requirements

Before generated fast-path support is claimed for any component family:

- the artifact schema version is frozen and documented
- schema compatibility tests exist
- generic-versus-generated dispatch is deterministic
- unsupported-component behavior is tested
- generated inventory provenance is retained
- at least one acceptance workload proves through the generic lane and one
  proves through the generated lane without changing verifier semantics
