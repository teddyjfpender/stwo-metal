# DN-0006: `stark-v` Generated Minimum Contract

- Date: `2026-03-11`
- Status: `accepted`
- Owners: `project team`

## Purpose

Define the smallest truthful generated-artifact subset that would let the
current pinned `stark-v` family attach to `stwo-metal` through the generated
lane.

## Input

- the producer/consumer laws from
  [`dn-0002-generic-backend-and-codegen-contract.md`](./dn-0002-generic-backend-and-codegen-contract.md)
- the pinned downstream contract from
  [`dn-0004-stark-v-hardening-input-and-contract.md`](./dn-0004-stark-v-hardening-input-and-contract.md)
- the current fail-closed classification from
  [`dn-0005-stark-v-attachment-strategy.md`](./dn-0005-stark-v-attachment-strategy.md)

## Problem

`DN-0005` concludes that the current pinned `stark-v` snapshot is not a
generic-lane substitution candidate. That still leaves one unresolved
question: what exact generated output would be sufficient for the first
supported downstream row?

Without that minimum contract, G8 risks becoming an open-ended request for
"some future generated support" instead of a bounded downstream hardening
target.

## Decision

The first supported `stark-v` row must provide one generated proving artifact
family with enough information for `stwo-metal` to register the component set
and lower it into the existing execution-plan and workload-stage contract.

The minimum required generated subset is:

1. `producer_identity`
   - Stwo version
   - `stark-v` codegen/macro version
   - provenance for the generated output
2. `component_identity`
   - stable component name
   - stable registration key
   - schema version
3. `evaluation_abi`
   - entrypoint names for generated evaluation hooks
   - field family
   - challenge/result layout identifiers
4. `trace_layout`
   - column groups
   - domain sizes
   - bit-reversed or other ordering metadata
   - packing shape required by the component family
5. `lookup_layout`
   - whether the component emits interaction/logup traces
   - lookup family names
   - multiplicity ownership
6. `degree_and_lifting_profile`
   - lifted vs non-lifted component profile
   - degree bounds or degree class identifiers
7. `domain_material`
   - twiddle-domain requirements
   - coset/log-size requirements
8. `commitment_layout`
   - which trees are committed
   - leaf packing family
   - hash family
9. `query_layout`
   - point-evaluation family
   - query grouping and decommit ordering
10. `witness_hooks`
   - named witness-generation entrypoints when the downstream component family
     cannot use pure generic trace staging
11. `specialization_keys`
   - stable dimensions that are legal to specialize without changing proving
     semantics
12. `generated_inventory`
   - manifest module
   - ABI symbols
   - build modules

## Minimal downstream promise

For the first supported row, `stark-v` does **not** need to emit one artifact
per individual opcode table.

The minimum truthful producer promise is one artifact family that covers:

- the main execution trace family
- the interaction/logup trace family
- the quotient and PCS commitment path
- the FRI and Blake2s path

This keeps the first downstream row bounded while still requiring enough
artifact data to avoid hidden SIMD assumptions.

## Output contract

Inputs:

- one downstream generated artifact or equivalent machine-readable manifest

Outputs:

- one schema-compatible generated registration row in `stwo-metal`
- one deterministic fail-closed error when any required subset is absent

Invariants:

- the first supported row must still respect the stable `ArtifactRegistry` and
  `ExecutionPlan` seam
- downstream support claims remain false until the generated subset is present
- the public `stwo-metal` API does not widen just to mirror downstream macros

Failure modes:

- downstream emits only Rust macro expansion with no machine-readable artifact
- downstream artifact lacks specialization keys or ABI inventory
- downstream artifact hides lookup or commitment ownership behind opaque hooks

## Explicitly not required for the first supported row

- a fully hand-tuned Metal kernel per opcode
- one artifact per tiny internal witness helper
- a new public `stwo-metal` API for `stark-v`
- generic-lane support for the current SIMD-shaped snapshot
