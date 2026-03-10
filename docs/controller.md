# Controller

## Purpose

This is the active control document for `stwo-metal`.

Inputs:

- current repository state
- active blockers
- long-range sequencing from [`roadmap.md`](./roadmap.md)
- program milestones from [`program-plan.md`](./program-plan.md)

Outputs:

- one active objective
- one active tranche
- explicit blockers
- next three deliverables

Invariants:

- the public repository identity is `stwo-metal`
- backend changes must preserve proving semantics unless an approved design note
  says otherwise
- no hidden expansion of scope while the backend boundary is being cleaned up

## Current operating state

- Date opened: `2026-03-09`
- Status: `in_progress`
- Active tranche: `T8 eleventh implementation slice: parallel Blake2s commitment support has cut the end-to-end prove row substantially, so the next work is the dominant prove-values path and the next explicit PCS bridge`
- Objective:
  convert the now-truthful Metal benchmark row into benchmark-grade
  performance by reducing the dominant prove-stage bottlenecks without hiding
  any remaining host-owned or CPU-bridge boundaries
- Active design note:
  [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)
- Current owner area:
  `native performance work and bridge retirement`

## Current blockers

- the project goal had drifted toward benchmark-specific proving rows instead
  of a generic Stwo proving backend
- `MetalBackend` now implements the full Stwo `Backend` trait
- `MetalBackend` now implements the Blake2s `BackendForChannel` surface
  without a `CpuBackend` dependency, but the lifted commitment path is still
  host-owned and readback-based rather than a GPU-side hash pipeline
- internal Rust vocabulary is still CUDA-first in many places
- unchanged upstream `wide_fibonacci`, `state_machine`, `blake`, and `xor`
  example rows now prove and verify through `MetalBackend` in the acceptance
  harness
- the remaining framework-component bridge is still CPU-domain based and still
  lives only in the acceptance harness rather than a stable shared boundary
- the example-backed acceptance harness now covers single-trace, multi-tree,
  lookup-heavy, and mixed-component rows, but those bridges are still
  acceptance-local rather than stable shared boundaries
- the native performance lane is still structurally lopsided:
  `stwo-metal-sys/cuda` is a full subsystem while `stwo-metal-sys/metal`
  remains a thin frontier
- `fields.metal` and `twiddles.metal` are now compile-active and parity-tested,
  and `rfft.metal` / `ifft.metal` / `poly_utils.metal` now carry the native
  evaluate/interpolate core, and mirrored `quotients.metal`,
  `fold_circle_into_line.metal`, and `fold_line.metal` now carry the active
  native proving kernels
- `mle.metal` is now compile-active and parity-tested, and the explicit
  `MleOps` CPU bridge is retired
- `gkr.metal` is now compile-active and parity-tested for native eq-eval
  generation, next-layer construction, and bounded oracle-sum evaluation;
  the remaining host work is limited to polynomial reconstruction from the two
  native secure-field evaluations rather than a CPU oracle walk over Metal
  columns
- `prefix_sum.metal` is now compile-active and parity-tested as a support
  kernel over bit-reversed circle-domain base-field columns
- the mirrored `metal/` hot-path set recorded in `PORTING_STATUS.md` is now
  structurally complete
- the wider `FriOps` secure-column repacking and fold-accumulation path is now
  Metal-owned, so the legacy explicit `FriOps` CPU bridge is retired
- `PolyOps` point evaluation and barycentric helpers are now Metal-owned; the
  only remaining `PolyOps` CPU fallback is the bounded small-domain
  evaluate/interpolate path
- the lifted Blake2s Merkle and proof-of-work boundaries are now direct
  Metal-owned host orchestration with no `CpuBackend` dependency
- the declared `wide_fibonacci` benchmark target remains useful for
  performance, but it is not the architectural source of truth and must stop
  driving milestone sequencing
- the first Apple Silicon native trace baseline now exists for the benchmark
  north star: `wide_fibonacci_trace_generation_v1` completed at `66.61 ms`
  for `log_n_instances = 20`, `n_columns = 100`, `STWO_METAL_MODE=metal-dev`,
  `warmups = 0`, and `samples = 1`
- the end-to-end Apple Silicon benchmark row now also exists:
  `wide_fibonacci_prove_verify_v1` completed in `102272.056124 ms`, with
  `prove_ms = 102266.681958` and `verify_ms = 5.374166`, under
  `STWO_METAL_MODE=metal-dev`, `warmups = 0`, and `samples = 1`
- the benchmark boundary is now support-honest, but the measured dominant
  costs are still far from the `90 ms` north star:
  `prove_core_prove_values_ms = 71138.82325`,
  `trace_commit_merkle_ms = 16673.889875`, and
  `prove_core_composition_commit_ms = 4998.368125`
- the benchmark runner now enables the `parallel` proving surface for the
  Metal benchmark fixture, and the latest measured row used `threads = 14`
- the end-to-end row is still approximately `73.6x` slower than the current
  SIMD reference at `log_n_instances = 20` (`1390 ms`) and still far from the
  historical GPU row (`87 ms`)
- the native commitment and decommit boundary is still host-owned and
  readback-based rather than a GPU-side hash pipeline
- `AccumulationOps` and `QuotientOps` still use explicit CPU bridges over
  Metal-owned storage
- `poseidon` is currently blocked by the vendored lifted protocol's AIR-degree
  limitation, so it is not the immediate next backend row
- the only named upstream-example row still open in the current target set is
  `poseidon`, and that row is blocked by the vendored lifted protocol rather
  than by a known Metal-backend gap
- the next performance blocker is native implementation depth, not acceptance
  coverage: the hot path still lives mostly in copied CUDA source under
  `stwo-metal-sys/cuda`

## Next three deliverables

1. Turn the new `102272.056124 ms` benchmark row into the next optimization
   program by attacking `prove_core_prove_values_ms` before the now-smaller
   Merkle path.
2. Decide whether the next benchmark-facing structural win is retirement of
   the explicit `AccumulationOps` / `QuotientOps` CPU bridges or deeper PCS
   prove-values work above them.
3. Keep the acceptance-local adapter debt and the bounded small-domain
   `PolyOps` fallback explicit while T8 focuses on benchmark-grade
   performance.

## Explicitly not doing now

- pretending the copied CUDA implementation is already a complete Metal backend
- widening the claimed Metal support faster than deterministic parity can cover
- renaming every inherited CUDA symbol before the replacement slices stabilize
- treating benchmark numbers as the first proof of correctness instead of
  deterministic parity against the vendored CPU path
- re-implementing upstream example workloads when backend wiring should be the
  only change needed to prove and verify them

## Update rule

Update this file whenever the active tranche, blockers, or next three
deliverables change.
