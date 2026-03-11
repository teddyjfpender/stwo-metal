# DN-0004: `stark-v` Hardening Input And Contract

- Date: `2026-03-11`
- Status: `accepted`
- Owners: `project team`

## Purpose

Freeze the first real downstream hardening input for `G8` so `stwo-metal`
validates its generic/generated contract against a concrete Stwo consumer
instead of an abstract future workload.

## Input

- downstream project: `https://github.com/AntoineFONDEUR/stark-v`
- pinned upstream HEAD at review time:
  `3a3cb4cf576d7d7e8ca82815acfb31bbc10e48ef`
- inspected files:
  - workspace root `Cargo.toml`
  - `crates/prover/src/lib.rs`
  - `crates/prover/src/prover.rs`
  - `crates/prover/src/verifier.rs`
  - `crates/prover/tests/integration.rs`
  - `crates/bench-cli/src/main.rs`

## Observed downstream surface

`stark-v` currently presents the following Stwo-facing proving contract:

- runtime input boundary:
  - `runner::run(...)`
  - `runner::run_with_input(...)`
- proving boundary:
  - `prove_rv32im(run_result, PcsConfig, &Preprocessing)`
- verification boundary:
  - `verify_rv32im(proof, PcsConfig, &Preprocessing)`
- preprocessing boundary:
  - `preprocess(PcsConfig) -> Preprocessing`

The current implementation remains SIMD-first:

- `crates/prover/src/prover.rs` imports `SimdBackend`
- many witness and component modules are generated or macro-expanded around
  `FrameworkComponent` and `LogupTraceGenerator`
- the workspace depends on its own vendored `external/stwo`

## Contract laws for `stwo-metal`

Inputs:

- one downstream proving crate with a stable public prove/verify/preprocess API
- one runner boundary that produces a deterministic execution artifact
- one Stwo dependency surface that may be vendored independently from
  `stwo-metal`

Outputs:

- one pinned contract note for downstream hardening
- one deterministic local checker for the minimum downstream contract shape
- one roadmap basis for deciding whether `stark-v` can attach through the
  generic lane, needs a generated lane, or needs an explicit fail-closed
  answer

Invariants:

- `stwo-metal` does not silently assume downstream code uses the same vendored
  Stwo snapshot
- downstream hardening does not widen the public `stwo-metal` API without a
  separate design decision
- downstream hardening remains fail-closed when the required proving entry
  points or runner surface are absent

Failure modes:

- the downstream repo drifts and no longer exposes the pinned prove/verify or
  runner surface
- the downstream repo uses a Stwo fork shape that cannot satisfy the current
  generic/generated contract without explicit adaptation
- the downstream consumer depends on SIMD-specific assumptions that are not yet
  expressible through the generated artifact contract

## Minimum required downstream contract

The first G8 acceptance check only requires:

1. `crates/prover` exists
2. `crates/runner` exists
3. `prove_rv32im` is publicly exported
4. `verify_rv32im` is publicly exported
5. `preprocess` is publicly exported
6. `bench-cli` or equivalent wiring still composes:
   `run_with_input -> prove_rv32im -> verify_rv32im`

This note does **not** claim `stwo-metal` can already replace the downstream
SIMD proving path. It only freezes the real downstream contract that G8 will
harden against.

## Next steps

- pin one local `stark-v` contract record in this repository
- keep the checker deterministic and text-structure based
- only after that decide whether the first downstream hardening slice is:
  - generic-lane substitution
  - generated artifact mapping
  - explicit fail-closed unsupported status
