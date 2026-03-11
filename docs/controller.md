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
- examples remain acceptance workloads rather than the architecture surface

## Current operating state

- Date opened: `2026-03-10`
- Status: `in_progress`
- Active tranche:
  `generated-lane hotspot retirement: Metal-backed Blake2s commitment columns
  plus quotient and early-FRI follow-up`
- Objective:
  reduce the remaining generated-lane `prove_values` and early FRI commit
  costs without widening public contracts or changing proving semantics
- Active design note:
  [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)
  and
  [`dn-0002-generic-backend-and-codegen-contract.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0002-generic-backend-and-codegen-contract.md)
  and
  [`dn-0003-acceptance-bridge-law-and-ownership.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0003-acceptance-bridge-law-and-ownership.md)
  and
  [`dn-0004-stark-v-hardening-input-and-contract.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0004-stark-v-hardening-input-and-contract.md)
  and
  [`dn-0005-stark-v-attachment-strategy.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0005-stark-v-attachment-strategy.md)
  and
  [`dn-0006-stark-v-generated-minimum-contract.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0006-stark-v-generated-minimum-contract.md)
  and
  [`dn-0007-stark-v-support-promotion-gate.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0007-stark-v-support-promotion-gate.md)
- Current owner area:
  `generated-lane performance and CPU-dependence retirement`

## Current blockers

- internal Rust vocabulary is still CUDA-first in many places
- unchanged upstream `wide_fibonacci`, `state_machine`, `blake`, and `xor`
  example rows now prove and verify through `MetalBackend` in the acceptance
  harness, but those paths still rely on a non-public shared bridge with
  explicit CPU-domain fallback debt
- the remaining framework-component bridge is still CPU-domain based even
  though its ownership has moved out of the acceptance harness and into a
  private shared support boundary
- there is not yet a declared policy for how generated output becomes durable
  and hand-tunable within this repository
- generated inventory now names ABI symbols and specialization keys, is
  consumed by workload and benchmark declarations, and now feeds a reusable
  execution-binding helper plus one scheduling seed and shared witness,
  evaluation, quotient, and prove-values staging helpers, but the shared
  prove-values bridge and the acceptance and benchmark witness lanes now anchor
  themselves on a transitional public `MetalExecutionAuthority` surface instead
  of directly on a lower private generated execution contract; the first
  workload-side live helpers have now moved below that public surface onto the
  private execution seed, and both the benchmark prove-values bridge and the
  upstream acceptance lane now depend on workspace-private validated lane
  contracts instead of consuming `MetalExecutionAuthority` directly; fixture
  edges now enter those bridges through boundary-based constructors only, and
  the dead transitional planning helpers that lost all live callers have been
  removed instead of left as dormant API; the redundant root-level companion
  export for `MetalExecutionAuthority` is now gone, so the type is available
  through the workload-facing module where its semantics actually live; the
  private support crates no longer import or validate through the authority
  type at all, so the remaining direct consumers are now the workload/benchmark
  API and the workload-scoped companion reexport; the benchmark boundary no
  longer exposes its own redundant `execution_authority()` pass-through, so the
  remaining direct callers now sit on `MetalWorkloadBoundary` and tests of that
  workload-law surface; `MetalExecutionAuthority` itself is now gone, leaving
  the workload law as `plan()` and `stage_ownership()`, while the richer stage-
  assignment slice remains internal to the planning and execution-plan layers;
  the generated execution seed now also owns the shared wide-fibonacci witness
  shape law that workload and benchmark boundaries previously duplicated
- `poseidon` is currently blocked by the vendored lifted protocol's AIR-degree
  limitation, so it remains an upstream protocol blocker rather than the next
  backend row
- benchmark work remains important, but it must now follow the generic and
  generated contract instead of driving it; G5 is now complete, G6 is now
  complete with a dual-lane report surface, and G7 is now complete because the
  remaining compatibility bridges are fixture-owned rather than
  architecture-adjacent support crates
- superseded milestone history has to stay out of the active control surface so
  the new sequence remains legible
- the current acceptance rows now all consume registered workload lanes and one
  checked bridge catalog backed by a private shared support crate, which closes
  the immediate G3 ownership question but still leaves adapter retirement for
  later milestones
- the shared planning seam is now stable enough for G3 work, and deterministic
  cargo verification is restored on the pinned nightly after modernizing the
  stale vendored chunking surface
- the next decision is about how much ABI, specialization, and generated build
  inventory belongs in the stable internal artifact registry before lowering
  starts in earnest
- the wide-fibonacci generated lane now uses native Metal Blake2s leaf hashing
  and native parent-layer hashing for standard Blake2s Merkle trees, the
  benchmark trace-tree builder now keeps parent layers native until the final
  host decode, and the next prove-values slice now caches lifted Merkle query
  expansion by shift plus prepared tree-query vectors by tree log-size, and
  the next grouped scheduling slice now reuses batched point-eval coefficient
  vectors and ordered quotient accumulations without post-sort regrouping, and
  the next materialization slice now builds proof-facing `sampled_values`
  alongside `samples` while reusing prepared tree-query buffers without
  per-tree cloning, and the next quotient-feed slice now stages grouped sample
  randomness directly by log-size without a full intermediate tree-shaped
  regroup pass, but the Merkle contract still materializes host
  `Vec<Blake2sHash>` layers for the committed tree and `prove_values` still
  dominates the benchmark row
- the current wide-fibonacci generated benchmark still trails SIMD from
  `log_size = 19` onward, so remaining prove-values and commitment staging
  costs must stay explicit and measured
- the final FRI last-layer interpolation no longer forces a `CpuBackend`
  conversion for `MetalBackend`; that bridge is now native Metal line-IFFT
  plus final host coefficient decode, so the next CPU-shaped ownership wall is
  the broader FRI/PCS workload and handoff boundary rather than
  `commit_last_layer`
- the workload-side FRI-ready and quotient-evaluation ingress paths are now
  canonically Metal-owned, with CPU evaluation ingress retained only as a
  compatibility adapter onto the native path; the next remaining ownership
  wall is therefore the higher FRI/PCS bridge and prove-values pipeline above
  those workload contracts rather than the workload ingress surface itself
- the bounded FRI commitment/proof slice no longer re-enters the explicit
  CPU line handoff to derive the last-layer polynomial, so that bridge now
  remains only for explicit bridge tests and transitional rows rather than the
  bounded proof-facing path itself
- the explicit CPU line handoff now adapts onto a canonical native Metal line
  evaluation/commitment path, so the next ownership/performance wall is the
  shared PCS/FRI proving pipeline above that thinner bridge layer
- the generic Blake2s lifted Merkle path can now batch full Metal parent-layer
  construction from one packed leaf buffer, and the quotient path now keeps
  partial numerators packed through `compute_quotients_and_combine` while the
  generic FRI prover now has a dedicated first-layer fold hook for fresh line
  evaluations; on the `wide_fibonacci` generated lane this moved `log20` to
  about `1157 ms` mean and then `1132 ms` mean across successive bounded
  slices, with `compute_quotients_and_combine` dropping sharply on warm runs;
  the next quotient/FRI slice then batched partial-numerator accumulation
  across all sample batches in one Metal launch and decoded packed native
  Merkle layers without an intermediate host copy, moving the generated
  `log20` row to about `1066 ms` mean / `842 ms` median; the current slice now
  keeps generic Blake2s commitment layers on a Metal-backed packed-hash column
  across the commitment path, which moved the current generated `log20` row to
  about `934 ms` mean / `707 ms` median while dropping
  `trace_commit_merkle_ms` to about `29 ms`; the remaining measured interior
  walls are now quotient numerator accumulation before lift-and-accumulate and
  the earliest FRI commitment rounds rather than Merkle layer decode
- `AccumulationOps` for `MetalBackend` no longer routes through host slices for
  `accumulate` or `lift_and_accumulate`; both now execute through native Metal
  coordinate-buffer kernels, and the standard Blake2s native commitment
  threshold now reaches down to `log_size = 16`; the measured generated
  `wide_fibonacci` `log20` row is now about `927 ms` mean / `674 ms` median,
  with `prove_core_prove_values_ms` about `427 ms` mean and
  `trace_commit_merkle_ms` about `29 ms` mean; the next measured walls are the
  numerator-accumulation kernel itself and the first FRI commitment rounds,
  not hidden CPU accumulation helpers
- the generic Blake2s commitment path now builds full parent-layer chains from
  one packed native leaf layer inside one Metal command buffer, and the batched
  quotient feeder no longer clones the same packed partial-numerator slice a
  second time before secure-coordinate unpack; the measured generated
  `wide_fibonacci` `log20` row is now about `854 ms` mean / `638 ms` median,
  with `prove_core_prove_values_ms` about `405 ms` mean; the remaining hot
  wall is no longer repeated Merkle dispatch setup or quotient unpack
  duplication, but numerator accumulation plus the first FRI fold/commit band

## Next three deliverables

1. Reduce numerator accumulation in the generated `prove_values` lane from the
   new `~854 ms` / `~638 ms` baseline, with the next measured target still
   being the quotient numerator kernel before lift-and-accumulate.
2. Reduce the first FRI fold and commit band from the same baseline now that
   repeated Merkle parent-layer dispatch is no longer the dominant early-commit
   tax.
3. Keep downstream `stark-v` hardening iced unless an external support signal
   appears.

## Explicitly not doing now

- using acceptance examples as the architecture surface
- adding new benchmark-local seams before the planning boundary is frozen
- promising generated support before the producer/consumer contract exists in
  code
- re-implementing upstream example workloads when backend wiring should remain
  the only change needed to prove and verify them
- treating benchmark numbers as the first proof of correctness instead of
  deterministic parity against the vendored CPU path

## Update rule

Update this file whenever the active tranche, blockers, or next three
deliverables change.
