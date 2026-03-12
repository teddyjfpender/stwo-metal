# DN-0010: Generated Row Convergence And Runtime Optimization

- Date: `2026-03-12`
- Status: `accepted`
- Owners: `project team`
- Related:
  - [`dn-0008-metal-evaluation-program-v1.md`](./dn-0008-metal-evaluation-program-v1.md)
  - [`dn-0009-v1-post-composition-sampled-values-abi.md`](./dn-0009-v1-post-composition-sampled-values-abi.md)

## Purpose

Freeze the next engineering contract for the generated Metal lane after the
first successful V1 migration work.

Inputs:

- the current `wide_fibonacci` generated-lane benchmark sweep at
  `log16..23`
- the active V1 runtime contract
- the current backend-owned generated benchmark flow in `stwo-metal`

Outputs:

- the convergence rules for moving the generated row onto shared V1 runtime
  paths
- the boundary between backend-owned runtime logic and benchmark-only
  orchestration
- the optimization order once the generated row is running on the cleaner
  runtime baseline

## Benchmark context

The current generated-Metal sweep is:

| Log(Size) | Prove SIMD ms | Prove Metal ms | Speedup |
| --- | --- | --- | --- |
| 16 | `199.00` | `94.10` | `2.11x` |
| 17 | `267.00` | `145.70` | `1.83x` |
| 18 | `450.00` | `250.18` | `1.80x` |
| 19 | `757.00` | `481.31` | `1.57x` |
| 20 | `1390.00` | `921.14` | `1.51x` |
| 21 | `2670.00` | `2179.63` | `1.22x` |
| 22 | `5166.00` | `6493.18` | `0.80x` |
| 23 | `11014.00` | `32530.12` | `0.34x` |

The useful reading is:

- the generated lane is already good enough at `log16..20` to justify
  architecture-first cleanup instead of only micro-optimizing kernels
- the generated lane still falls off badly at `log21..23`, which strongly
  suggests that remaining runtime/orchestration overhead and scaling behavior
  still matter
- the next work must preserve the faster low-log rows while improving the
  high-log scaling law

## Decision

The next generated-lane work must proceed in three ordered phases:

1. collapse more of the live generated row onto shared V1 runtime paths
2. remove the remaining specialized benchmark-only orchestration that still
   defines generated-lane behavior
3. optimize the backend-owned runtime family from that cleaner baseline

The benchmark harness remains a measurement surface only. It must not continue
to define benchmark-local prove semantics once the same behavior can live under
the V1 runtime family.

## Scope

This design covers:

- the active generated `wide_fibonacci` row
- the `MetalEvaluationProgramV1` runtime family
- the sampled-values ABI/runtime family
- the backend-owned generated benchmark flow

This design does not yet promise:

- full generic-lane performance parity
- downstream `stark-v` hardening
- arbitrary-code Rust-to-Metal lowering

## Semantic contract

### Inputs

- one validated generated benchmark workload supported by the current V1
  artifact contract
- one selected `MetalEvaluationProgramV1` runtime or overlay lane
- one backend-owned post-composition sampled-values contract
- benchmark execution parameters such as `log_size`, warmups, and samples

### Outputs

- one backend-owned generated benchmark run result
- deterministic timing breakdowns for:
  - trace generation
  - trace commit
  - V1 runtime execution
  - composition generation
  - prove-values
  - finishing and decommit
  - verification
- one explicit classification of whether each phase is:
  - shared V1 runtime
  - generated overlay
  - temporary compatibility bridge

### Invariants

- proof semantics do not change
- verifier behavior does not change
- the generated row must fail closed when the selected V1 contract is absent,
  invalid, or incompatible
- benchmark-only code may report results, but it must not redefine generated
  prove semantics
- any remaining compatibility bridge must be named and tracked as debt

### Failure modes

- invalid V1 program or sampled-values ABI: fail closed before proving
- unsupported overlay selection: fall back only to the explicit supported V1
  runtime mode, never to hidden benchmark-local semantics
- unsupported generated row shape: fail closed with explicit runtime error

## Phase 1: Collapse onto shared V1 runtime paths

### Goal

The generated row should use the same stable V1/runtime family for as much of
the live prove path as possible.

### Required moves

- keep composition generation on selected `MetalEvaluationProgramV1`
- keep post-composition sampled-values on the sampled-values ABI/runtime family
- move any remaining generated-row-specialized execution helpers toward shared
  V1/runtime entry points instead of benchmark-specific routing
- make the selected runtime/overlay dispatch the only semantic authority for
  generated program execution

### Exit condition

The generated row no longer has benchmark-only execution law for any live prove
 phase that already has a backend-owned V1/runtime contract.

## Phase 2: Reduce specialized benchmark-only orchestration

### Goal

The benchmark binary should become a thin reporting surface.

### Required moves

- keep run orchestration, timed iteration execution, and result ownership
  inside `stwo-metal`
- remove benchmark-local sequencing logic that still exists only for the
  generated row
- keep the binary responsible only for:
  - CLI/environment parsing
  - invoking the backend-owned run API
  - rendering JSON and summary output

### Exit condition

The live generated benchmark row is backend-owned end to end, and the benchmark
binary is no longer the main semantic authority for generated proving.

## Phase 3: Optimize the backend-owned runtime family

### Goal

Optimize only after the generated row is running through the cleaner shared
runtime shape.

### First optimization targets

1. composition-generation path inside the V1 runtime
2. post-composition sampled-values evaluation and decommit path
3. trace-commit path, if it remains material after convergence

### Measurement rules

- always report cold-start and warmed steady-state separately
- optimize the shared runtime family first, not a benchmark-local surrogate
- keep before/after evidence for every retained performance change
- reject optimizations that improve one cold sample but regress the warmed
  steady-state contract

## Engineering specification

### Public API rule

Expose only backend-owned benchmark/run/result surfaces. Do not expose internal
runtime planning or overlay inventory unless callers need it for correctness.

### Runtime ownership rule

If a phase can already be expressed under:

- `MetalEvaluationProgramV1`, or
- the sampled-values ABI/runtime family

then that phase must not remain benchmark-local.

### Compatibility rule

Temporary compatibility bridges are allowed only when:

- they are the smallest stable boundary available
- they are named explicitly in docs and debt
- they are not misrepresented as part of the final runtime architecture

### Overlay rule

Generated overlays remain semantically identical to the shared V1 runtime
family. They may change performance but not meaning.

## Acceptance and verification

Acceptance for this design means:

- the generated `wide_fibonacci` benchmark row runs through backend-owned
  orchestration and shared V1/runtime contracts
- low-log rows remain competitive with or faster than SIMD
- high-log rows are measured against the same runtime family, not a different
  benchmark-local path
- all migrated phases remain covered by deterministic tests

Required verification classes:

- V1 ABI validation tests
- sampled-values ABI validation tests
- benchmark-boundary law tests
- public-surface tests
- release benchmark reproduction at the current reference log sizes

## Retirement conditions

This design note is complete when:

- the generated row’s remaining specialized orchestration is retired or reduced
  to reporting-only code
- the active generated benchmark path is driven primarily by shared V1/runtime
  execution surfaces
- optimization work is measured against that converged runtime family rather
  than the older mixed path
