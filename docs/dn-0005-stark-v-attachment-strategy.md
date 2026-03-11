# DN-0005: `stark-v` Attachment Strategy

- Date: `2026-03-11`
- Status: `accepted`
- Owners: `project team`

## Purpose

Freeze the first executable hardening answer for `G8` by deciding how the
current pinned `stark-v` surface may attach to `stwo-metal` without widening
the backend API or making false generic-support claims.

## Input

- pinned downstream contract in
  [`dn-0004-stark-v-hardening-input-and-contract.md`](./dn-0004-stark-v-hardening-input-and-contract.md)
- current `stark-v` proving and preprocessing sources under:
  - `crates/prover/src/lib.rs`
  - `crates/prover/src/prover.rs`
  - `crates/prover/src/verifier.rs`
  - `crates/bench-cli/src/main.rs`
  - `crates/stwo-macros/src/components.rs`
  - workspace `Cargo.toml`

## Observed constraints

The pinned downstream surface is not backend-parametric today.

Evidence:

- `prove_rv32im` hardcodes `SimdBackend::precompute_twiddles` and
  `SimdBackend::grind`
- `preprocess` reconstructs cached preprocessing into
  `Poly<SimdBackend>` and `MerkleProverLifted<SimdBackend, ...>`
- generated macro output and component surfaces are typed around
  `ComponentProver<SimdBackend>` and
  `CircleEvaluation<SimdBackend, BaseField, BitReversedOrder>`
- the workspace depends on its own vendored `external/stwo`

These constraints mean the current downstream row is not a truthful
`MetalBackend` drop-in substitution candidate.

## Decision

For the pinned `stark-v` snapshot:

1. generic-lane substitution is `unsupported`
2. generated-lane attachment is the only plausible production path
3. the hardening answer must remain `fail_closed` until a generated artifact
   or equivalent backend-parametric proving surface exists

`stwo-metal` therefore does **not** claim that the current pinned `stark-v`
snapshot can prove through a simple backend swap.

## Output contract

Inputs:

- one local checkout of the pinned or compatible `stark-v` workspace
- the checked downstream contract from `DN-0004`

Outputs:

- one deterministic readiness checker that classifies the current attachment
  mode
- one explicit fail-closed result for the generic lane
- one explicit statement that generated mapping is the next valid path

Invariants:

- no silent fallback from generic substitution to hidden generated glue
- no claim that a SIMD-shaped downstream API is already backend-generic
- no widening of the public `stwo-metal` API solely to fit the current
  downstream snapshot

Failure modes:

- downstream proving surfaces stop exposing the SIMD-shaped evidence this note
  relies on
- downstream proves to be more backend-parametric than the current contract
  check can observe
- downstream changes vendored Stwo ownership in a way that requires a new
  compatibility assessment

## Resulting G8 policy

The first executable G8 row is:

- deterministic contract check
- deterministic attachment classification
- explicit fail-closed unsupported status for generic substitution

The next executable `supported` row requires at least one of:

- a generated artifact emitted from `stark-v` or its codegen layer that
  satisfies the `stwo-metal` generated registration contract
- a backend-parametric downstream proving surface that removes the current
  `SimdBackend`-typed constraints

## Alternatives rejected

- claim that the current downstream snapshot is generic-lane compatible
- fork or patch `stark-v` locally just to force an early green row
- add `stark-v`-specific public APIs to `stwo-metal`
- leave the attachment mode implicit and undocumented
