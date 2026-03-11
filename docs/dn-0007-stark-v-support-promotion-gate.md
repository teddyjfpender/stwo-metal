# DN-0007: `stark-v` Support Promotion Gate

- Date: `2026-03-11`
- Status: `accepted`
- Owners: `project team`

## Purpose

Freeze the exact gate for promoting `stark-v` from the current vendored
fail-closed row to the first supported downstream row.

## Input

- the pinned downstream contract in
  [`dn-0004-stark-v-hardening-input-and-contract.md`](./dn-0004-stark-v-hardening-input-and-contract.md)
- the attachment classification in
  [`dn-0005-stark-v-attachment-strategy.md`](./dn-0005-stark-v-attachment-strategy.md)
- the minimum generated contract in
  [`dn-0006-stark-v-generated-minimum-contract.md`](./dn-0006-stark-v-generated-minimum-contract.md)

## Current state

`stwo-metal` now has one deterministic vendored hardening row for the pinned
`stark-v` input, and that row correctly ends in:

- `generic_lane = unsupported`
- `generated_lane = required`
- `status = fail_closed`

This is a useful and correct local state, but it is not the same as supported
downstream proving.

## Decision

The current vendored fail-closed row is the highest honest internal state until
one of the following external support conditions becomes true:

1. `backend-parametric downstream surface`
   - the downstream proving and preprocessing surface stops hardcoding
     `SimdBackend`
   - generated macro output or component registration becomes backend-parametric
2. `generated-artifact producer`
   - the downstream producer emits a machine-readable artifact satisfying the
     minimum subset from `DN-0006`

No internal-only `stwo-metal` change may promote `stark-v` to supported status
without one of those conditions.

## Promotion gate

Inputs:

- vendored `stark-v` checkout
- deterministic local hardening row
- one candidate support signal from the downstream side

Outputs:

- either `supported_candidate = false` with the current fail-closed status
- or `supported_candidate = true` with one new executable downstream row

Invariants:

- support promotion remains fail-closed by default
- internal wrapper code inside `stwo-metal` does not substitute for a missing
  downstream-generated artifact
- `stwo-metal` does not claim support by editing the vendored downstream code
  into a private fork shape

Failure modes:

- the project mistakes a local workaround for a downstream support signal
- a partial downstream signal exists but still hides required generated fields
- support is claimed before the vendored row can run through the same stable
  generic/generated vocabulary

## Explicitly not sufficient for support promotion

- vendoring the downstream source alone
- a deterministic fail-closed row alone
- a local mapping note with no downstream artifact
- a private fork of `stark-v` inside `vendor/`
