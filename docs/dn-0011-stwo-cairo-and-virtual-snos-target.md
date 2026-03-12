# DN-0011: Stwo-Cairo And Virtual SNOS Target

## Status

`accepted`

## Purpose

Define the concrete downstream hardening target that should guide post-V1
integration work without letting downstream project structure redefine the
backend contract.

## Inputs

- the current `stwo-metal` V1 runtime direction
- the `stwo-cairo` proving goal already implied by the project history
- the downstream proving-service expectation that `VIRTUAL_SNOS` proofs are the
  first concrete integration target

## Outputs

- one named downstream target for `G11`
- one rule for how `stwo-cairo` and `VIRTUAL_SNOS` affect roadmap decisions
- one fail-closed boundary for unsupported downstream integration

## Invariants

- the backend contract remains generic over Stwo-defined/codegen-defined
  components
- downstream targets validate the contract; they do not define it
- examples remain the acceptance matrix
- benchmark-local behavior must not become the downstream integration surface
- unsupported downstream rows must fail closed

## Decision

`stwo-metal` should treat `stwo-cairo` as the primary downstream proving target
after V1 runtime convergence, with `VIRTUAL_SNOS` as the first named proving
row to support and validate.

`starknet-privacy` is the concrete downstream consumer to keep in mind for that
row, because it expects `VIRTUAL_SNOS`-compatible proving behavior rather than
just a generic example benchmark.

## What this means

The program now has three distinct downstream roles:

1. Examples
   acceptance fixtures for correctness and bounded performance tracking
2. `stwo-cairo`
   the first real proving-system integration target
3. `VIRTUAL_SNOS`
   the first named downstream proving row to use when hardening the converged
   V1 runtime contract

## Required capabilities for G11

The following must exist before `stwo-metal` can honestly claim downstream
`stwo-cairo` / `VIRTUAL_SNOS` readiness:

- a stable V1 runtime contract that is already the proving authority for the
  active generated lane
- a producer/consumer mapping from `stwo-cairo` output into the V1 contract or
  an equivalent backend-parametric surface
- deterministic fail-closed behavior when a downstream artifact or capability
  is absent
- downstream integration checks that validate the targeted row without adding
  benchmark-only seams to the backend

## What this design does not promise

This note does not promise:

- immediate `stwo-cairo` support on the current pre-converged runtime
- support for arbitrary Cairo projects with no matching producer artifact
- that `stark-v` is the active downstream target

## Failure modes

- If a `stwo-cairo` proving row cannot be lowered into the V1 runtime contract,
  the integration must fail closed.
- If the first `VIRTUAL_SNOS` row requires metadata or ABI that the V1 runtime
  does not define, the correct response is to extend the contract formally,
  not to patch the benchmark path.

## Consequences

- `G11` should now be described in docs as downstream hardening for
  `stwo-cairo`, specifically `VIRTUAL_SNOS`.
- `stark-v` remains intentionally iced.
- runtime convergence work in `G9/G10` should be evaluated partly by whether it
  moves the codebase closer to supporting `stwo-cairo` inputs cleanly.
