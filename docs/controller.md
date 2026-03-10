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
- Active tranche: `T8 sixth implementation slice: the mirrored hot-path set is complete, so the next work is benchmark activation and the remaining non-mirrored CPU bridges`
- Objective:
  return from acceptance closure to native performance work by mirroring the
  active CUDA hot-path structure into `stwo-metal-sys/metal` and porting it in
  the declared order
- Active design note:
  [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)
- Current owner area:
  `backend-completion planning and example-backed acceptance`

## Current blockers

- the project goal had drifted toward benchmark-specific proving rows instead
  of a generic Stwo proving backend
- `MetalBackend` now implements the full Stwo `Backend` trait
- `MetalBackend` now implements the Blake2s `BackendForChannel` surface through
  explicit CPU-bridge Merkle and proof-of-work boundaries
- internal Rust vocabulary is still CUDA-first in many places
- the bounded FRI commitment slice now exists, but its last-layer
  interpolation still crosses an explicit CPU bridge rather than a native
  `stwo-metal` interpolation boundary
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
  generation plus native next-layer construction, but
  `sum_as_poly_in_first_variable` still crosses an explicit CPU bridge
- `prefix_sum.metal` is now compile-active and parity-tested as a support
  kernel over bit-reversed circle-domain base-field columns
- the mirrored `metal/` hot-path set recorded in `PORTING_STATUS.md` is now
  structurally complete
- the declared `wide_fibonacci` benchmark target remains useful for
  performance, but it is not the architectural source of truth and must stop
  driving milestone sequencing
- the native commitment and decommit boundary is still host-owned and
  readback-based rather than a GPU-side hash pipeline
- `poseidon` is currently blocked by the vendored lifted protocol's AIR-degree
  limitation, so it is not the immediate next backend row
- the only named upstream-example row still open in the current target set is
  `poseidon`, and that row is blocked by the vendored lifted protocol rather
  than by a known Metal-backend gap
- the next performance blocker is native implementation depth, not acceptance
  coverage: the hot path still lives mostly in copied CUDA source under
  `stwo-metal-sys/cuda`

## Next three deliverables

1. Decide which remaining explicit CPU bridge is the next measured bottleneck
   after mirrored hot-path completion:
   `GkrOps` oracle evaluation, `FriOps`, `PolyOps`, or Blake2s lifted hashing.
2. Turn the mirrored hot-path completion into benchmark-active measurement for
   the wide-fibonacci north star instead of stopping at parity-only support.
3. Keep the remaining adapter-local and oracle-evaluation bridges explicit
   while the next benchmark-facing replacement slice is chosen.

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
