# Technical Debt Register

Use this file for intentional temporary compromises that are currently accepted.

This is not a bug backlog. Every entry must state why the compromise exists,
how it is contained, and what retires it.

## Entry template

```md
### TD-XXXX: Title

- Status: active | planned | retired | superseded
- Category:
- Introduced:
- Owner area:

Why it exists now:

Current containment:

Risk if left in place:

Exit condition:

Target retirement point:
```

## Active debt

### TD-0033: FRI and workload handoff surfaces still own CPU-shaped evaluation transitions above the native last-layer path

- Status: `active`
- Category: `backend ownership boundary`
- Introduced: `2026-03-11`
- Owner area: `generated-lane performance`

Why it exists now:

`MetalBackend` no longer routes the final FRI last-layer interpolation through
`CpuBackend`, workload-side FRI-ready/quotient ingress is now canonically
Metal-owned, and bounded FRI commitment/proof slices no longer re-enter the
explicit CPU line bridge. The remaining debt is that the broader workload
handoff and higher PCS/FRI proving path still contain CPU-shaped ownership and
staging transitions above that native ingress; the explicit line bridge itself
is now compatibility-only.

Current containment:

- `crates/stwo-metal/src/backend/metal/workload.rs`
- `crates/stwo-metal/src/backend/metal/handoff.rs`
- `crates/stwo-metal/src/backend/metal/witness.rs`
- `vendor/stwo-upstream-dev-62b228e/crates/stwo/src/prover/fri.rs`
- `vendor/stwo-upstream-dev-62b228e/crates/stwo/src/prover/pcs/mod.rs`

Risk if left in place:

The benchmark may stop seeing meaningful wins from smaller grouping cleanups
because the surrounding prover phases are still structured around CPU-shaped
handoff ownership rather than a GPU-owned execution contract.

Exit condition:

The remaining FRI/PCS workload and handoff boundary above the now-native
workload ingress is expressed in Metal-owned or backend-parametric terms, or
measured evidence shows those remaining CPU-shaped boundaries are no longer
material to the target rows.

Target retirement point:

- `generated-lane performance follow-up`

### TD-0034: Quotient numerator accumulation and early FRI commitments are now the dominant generated-lane hot path

- Status: `active`
- Category: `measured performance hotspot`
- Introduced: `2026-03-11`
- Owner area: `generated-lane performance`

Why it exists now:

After batching full Blake2s parent-layer construction for the generic Metal
Merkle path, then keeping quotient partial numerators packed through
`compute_quotients_and_combine`, then specializing the generic first FRI layer
fold to skip zero-destination accumulation work, then batching
partial-numerator accumulation across sample batches, then decoding packed
native Merkle layers directly from shared Metal buffers, then keeping generic
Blake2s commitment layers on a Metal-backed packed hash column across the
native commitment chain, and now finally building generic parent-layer chains
inside one Metal command buffer while removing the second packed-buffer clone
before quotient unpack, the measured `wide_fibonacci` `log20` generated-lane
profile had moved to about `854 ms` mean / `638 ms` median, while the new
steady-state benchmark contract first showed a warmed `log20` row of about
`655 ms` mean after the cold first sample was excluded and then, after
lowering the standard native Blake2s threshold to `log_size = 12`, moved that
warmed row again to about `620 ms` mean. The new contiguous eval-domain column
batch removes the quotient-side evaluation restaging pass and moves the warmed
row again to about `587 ms` mean. The dominant remaining subphases are now:

- quotient numerator accumulation before lift-and-accumulate
- the earliest FRI commitment construction rounds

Current containment:

- `crates/stwo-metal/src/backend/metal/quotient.rs`
- `crates/stwo-metal-sys/metal/quotients.metal`
- `crates/stwo-metal/src/backend/metal/accumulation.rs`
- `vendor/stwo-upstream-dev-62b228e/crates/stwo/src/prover/fri.rs`
- `crates/stwo-metal/src/backend/metal/fri.rs`
- `vendor/stwo-upstream-dev-62b228e/crates/stwo/src/prover/pcs/quotient_ops.rs`
- `vendor/stwo-upstream-dev-62b228e/crates/stwo/src/prover/vcs_lifted/prover.rs`

Risk if left in place:

The generated lane may stall above the next desired throughput band even after
quotient combination, numerator staging, generic Merkle residency, batched
parent-layer dispatch, native `AccumulationOps`, and contiguous eval-domain
quotient feed are all materially more GPU-shaped.

Exit condition:

Measured evidence shows quotient numerator accumulation and early FRI
commitment construction are no longer dominant on the target rows, or those
phases are replaced by a meaningfully more GPU-shaped execution path.

Target retirement point:

- `generated-lane performance follow-up`

### TD-0032: Standard Blake2s Merkle layers still round-trip through host hash columns between native layers

- Status: `retired`
- Category: `benchmark staging overhead`
- Introduced: `2026-03-11`
- Owner area: `generated-lane performance`

Why it exists now:

`stwo-metal` now supports native Metal Blake2s leaf hashing and native
standard Blake2s parent-layer hashing, the generated wide-fibonacci trace tree
keeps parent layers native until final decode, and the generic Blake2s lifted
Merkle path now carries a private Metal-backed packed hash column across
`build_leaves`, `build_next_layer`, and `build_merkle_layers`. The remaining
Merkle cost is no longer a whole-layer host round trip between native layers.

Current containment:

- `crates/stwo-metal/src/backend/metal/blake2s.rs`
- `crates/stwo-metal/src/backend/metal/column.rs`
- `crates/stwo-metal/src/stwo_metal/blake2s_hash_vec.rs`
- `crates/stwo-metal-sys/metal/blake2s.metal`
- `crates/stwo-metal/tests/metal_blake2s_channel_cpu_bridge.rs`
- `fixtures/standalone-benchmarks/src/bin/wide_fibonacci_prove.rs`

Risk if left in place:

Before retirement, the benchmark could continue to trail SIMD even after more
arithmetic kernels were native, because Merkle commitment still paid a final
host materialization cost before the committed tree entered the broader prover
contract.

Exit condition:

The benchmark-critical Merkle path keeps the committed tree GPU-resident
through the next consumer boundary, or measured evidence shows that further
Merkle staging elimination is no longer material.

Target retirement point:

- `2026-03-11 generic Blake2s packed-layer residency slice`

### TD-0030: The pinned `stark-v` snapshot is still SIMD-shaped and therefore unsupported on the generic lane

- Status: `active`
- Category: `downstream compatibility gap`
- Introduced: `2026-03-11`
- Owner area: `G8 downstream hardening`

Why it exists now:

The current pinned `stark-v` repo exposes the right high-level prove/verify
surface, but its proving and preprocessing implementation still hardcodes
`SimdBackend`, its generated component surface still emits
`ComponentProver<SimdBackend>` and `CircleEvaluation<SimdBackend, ...>`, and
the workspace depends on its own vendored `external/stwo`. That means the
current downstream row is not yet a truthful generic `MetalBackend`
substitution candidate.

Current containment:

- `docs/dn-0005-stark-v-attachment-strategy.md`
- `docs/dn-0006-stark-v-generated-minimum-contract.md`
- `scripts/check_stark_v_attachment.sh`
- `scripts/check_stark_v_generated_readiness.sh`
- `scripts/check_stark_v_generated_gap.sh`
- `scripts/run_stark_v_hardening_report.sh`

Risk if left in place:

The program could overstate G8 completion or claim downstream generic support
where the current pinned consumer is still structurally SIMD-specific.

Exit condition:

The pinned downstream row either:

- exposes a genuinely backend-parametric proving surface, or
- emits a generated artifact that satisfies the `stwo-metal` generated
  registration contract

Target retirement point:

- `G8`

### TD-0031: The vendored `stark-v` row is hard-blocked on an external support signal for promotion

- Status: `active`
- Category: `external downstream dependency`
- Introduced: `2026-03-11`
- Owner area: `G8 downstream hardening`

Why it exists now:

The vendored `stark-v` row now has a deterministic local fail-closed check, but
the first supported row cannot come from internal-only `stwo-metal` changes.
Promotion requires either a backend-parametric downstream surface or a
downstream-generated artifact satisfying `DN-0006`.

Current containment:

- `docs/dn-0007-stark-v-support-promotion-gate.md`
- `scripts/check_vendored_stark_v_fail_closed.sh`
- `vendor/stark-v-pinned-3a3cb4cf576d7d7e8ca82815acfb31bbc10e48ef`

Risk if left in place:

The project could misclassify an external dependency as a missing internal
implementation step and start widening local wrapper code in ways that violate
the frozen contract.

Exit condition:

One real downstream support signal exists and is exercised by a new executable
row:

- backend-parametric downstream proving surface, or
- generated artifact satisfying the `stwo-metal` registration contract

Target retirement point:

- `G8`

### TD-0029: Vendored `stark-v` input is still not executed through `stwo-metal`

- Status: `retired`
- Category: `downstream hardening gap`
- Introduced: `2026-03-11`
- Owner area: `G8 downstream hardening`

Why it exists now:

`stark-v` is now the pinned G8 downstream input, and its minimum proving
contract is documented and checked. The downstream repo is now vendored
locally, but no executable hardening row has yet been landed through
`stwo-metal`.

Current containment:

- `docs/dn-0004-stark-v-hardening-input-and-contract.md`
- `scripts/check_stark_v_contract.sh`
- `scripts/run_vendored_stark_v_hardening_report.sh`
- `scripts/check_vendored_stark_v_fail_closed.sh`
- `vendor/stark-v-pinned-3a3cb4cf576d7d7e8ca82815acfb31bbc10e48ef`

Risk if left in place:

The roadmap could claim downstream hardening progress without actually running
an executable downstream row or making an explicit fail-closed decision.

Exit condition:

One executable and explicitly fail-closed `stark-v` hardening row exists and
is tracked through the same generic/generated contract vocabulary as the rest
of the program.

Target retirement point:

- `G8`

### TD-0027: The generated-metal wide-fibonacci row still falls behind SIMD from `log_size = 19` onward

- Status: `active`
- Category: `benchmark scaling regression`
- Introduced: `2026-03-11`
- Owner area: `G6 lane-separated benchmarking`

Why it exists now:

The first live generated-metal wide-fibonacci comparison table is now landed,
and it shows that the generated row crosses over ahead of SIMD at
`log_size = 16` and `18`, sits near parity at `17`, but loses throughput and
elapsed-time scaling from `19` onward.

Current containment:

- `logs/benchmarks/20260311T081850Z/wide_fibonacci_metal_generated/wide_fibonacci_comparison.md`
- `scripts/run_supported_wide_fibonacci_metal_sweep.sh`
- `scripts/render_wide_fibonacci_comparison_table.rb`
- `fixtures/standalone-benchmarks/src/bin/wide_fibonacci_prove.rs`

Risk if left in place:

The project could optimize the generated lane without a crisp statement of
where it already wins and where it still loses, making later G6 optimization
claims hard to compare honestly.

Exit condition:

Lane-separated measurement either shows the generated-metal row scaling at or
above SIMD across the supported wide-fibonacci sweep, or a narrower
benchmark-contract note explicitly limits the supported generated target range.

Target retirement point:

- `G6`

### TD-0028: The generic-metal wide-fibonacci benchmark lane is only practical in a bounded range

- Status: `active`
- Category: `generic lane benchmark envelope`
- Introduced: `2026-03-11`
- Owner area: `G6 lane-separated benchmarking`

Why it exists now:

The first real `generic-metal` wide-fibonacci row now exists, but the measured
`log_size = 16` result is already roughly `63.6 s`, which makes the generated
lane's full `16..23` sweep range impractical as a default generic-lane
contract.

Current containment:

- `scripts/run_supported_wide_fibonacci_generic_sweep.sh`
- `scripts/render_wide_fibonacci_comparison_table.rb`
- `fixtures/standalone-benchmarks/src/bin/wide_fibonacci_prove.rs`
- `logs/benchmarks/20260311T083107Z/wide_fibonacci_metal_generic/wide_fibonacci_comparison.md`

Risk if left in place:

The project could either hide the generic lane entirely or keep trying to run
an unrealistic default sweep, making G6 reporting noisy and hard to trust.

Exit condition:

The generic lane either becomes fast enough to support a broader default sweep
or the benchmark contract is explicitly narrowed and documented as a bounded
generic coverage row with a stable published range.

Target retirement point:

- `G6`

### TD-0026: Generated ABI and specialization inventory is registered but not yet consumed broadly enough by declarations and lowering

- Status: `retired`
- Category: `generated registration adoption`
- Introduced: `2026-03-10`
- Owner area: `G4 generated fast-path registration`

Why it exists now:

The internal generated-artifact registry now records explicit ABI symbols and
specialization keys. Workload and benchmark declarations consume that richer
inventory through the shared planning seam, and one lowering-facing generated
registration object now derives from the same boundary.

Current containment:

- `crates/stwo-metal/src/backend/metal/artifact.rs`
- `crates/stwo-metal/src/backend/metal/planner_manifest_v1_generated.rs`
- `crates/stwo-metal/src/backend/metal/execution_plan.rs`
- `crates/stwo-metal/src/backend/metal/workload.rs`
- `crates/stwo-metal/src/backend/metal/benchmark.rs`
- `docs/controller.md`
- `docs/program-plan.md`

Risk if left in place:

This registration-adoption debt is now retired. The next remaining gap is G5
lowering depth, not whether the richer generated registration contract is
active.

Exit condition:

Workload declarations, benchmark declarations, and the first lowering-facing
registration object all consume ABI and specialization inventory through the
same stable planning seam.

Target retirement point:

- `G5`

### TD-0024: The frozen generated-artifact contract is not yet implemented as a stable backend registry and execution-plan boundary

- Status: `retired`
- Category: `architecture implementation gap`
- Introduced: `2026-03-10`
- Owner area: `generic/generated backend planning`

Why it exists now:

The project froze the generic backend and codegen contract first, then widened
the private implementation until one stable internal artifact-registry and
execution-plan seam existed in code with schema checks, per-component
inventory, generated-route compatibility, and one explicit fail-closed policy
path.

Current containment:

- `docs/dn-0002-generic-backend-and-codegen-contract.md`
- `docs/roadmap.md`
- `docs/program-plan.md`
- `docs/controller.md`
- `crates/stwo-metal/src/backend/metal/artifact.rs`
- `crates/stwo-metal/src/backend/metal/execution_plan.rs`

Risk if left in place:

The implementation risk this entry tracked has now been retired. Remaining
integration risk has shifted to acceptance-adapter cleanup rather than the
existence of the planning seam itself.

Exit condition:

`stwo-metal` owns a stable internal artifact-registry boundary and a stable
execution-plan boundary across the broader backend registration path, with
explicit schema/version checks and documented unsupported-component behavior.

Target retirement point:

- `G2`

### TD-0001: Internal backend vocabulary is still CUDA-first

- Status: `active`
- Category: `boundary naming`
- Introduced: `2026-03-09`
- Owner area: `Rust backend boundary`

Why it exists now:

The repo identity has been corrected to `stwo-metal`, but internal modules and
types still use names such as `CudaBackend`, `stwo_cuda`, and
`stwo_cuda_link`. Renaming them before the backend-neutral interface is designed
would create churn without locking the right abstraction.

Current containment:

- `crates/stwo-metal/src/backend/cuda`
- `crates/stwo-metal/src/stwo_cuda`
- cfg gates and tests that still reference CUDA naming

Risk if left in place:

The public direction of the project stays clear, but internal design work can
accidentally inherit CUDA-specific concepts into the long-term Metal boundary.

Exit condition:

An approved boundary-neutral naming design lands and the internal Rust surface
is migrated to match it.

Target retirement point:

- `T1`

### TD-0002: Native runtime ownership is still the inherited CUDA build

- Status: `active`
- Category: `native runtime bridge`
- Introduced: `2026-03-09`
- Owner area: `stwo-metal-sys`

Why it exists now:

`stwo-metal-sys` still contains the copied CUDA native build, CMake files, and
the `stwo_cuda` static library target. This is acceptable only as a temporary
placeholder while the Metal runtime boundary is designed.

Current containment:

- `crates/stwo-metal-sys/cuda`
- `crates/stwo-metal-sys/build.rs`

Risk if left in place:

The project could drift into a renamed CUDA fork rather than a bounded Metal
port.

Exit condition:

The Metal runtime design is approved and the inherited CUDA native build is
either removed or isolated behind an explicit transition plan.

Target retirement point:

- `T3`

### TD-0003: CI and helper tooling have not yet been curated for `stwo-metal`

- Status: `active`
- Category: `tooling hygiene`
- Introduced: `2026-03-09`
- Owner area: `project operations`

Why it exists now:

The docs surface was reset first. Copied scripts and CI definitions still
contain CUDA-era assumptions and naming.

Current containment:

- `.github/`
- `scripts/`

Risk if left in place:

Contributors may infer unsupported process or runtime guarantees from stale
tooling.

Exit condition:

The active CI and helper scripts are audited against the `stwo-metal` host and
backend plan.

Target retirement point:

- `T2`

### TD-0004: The Metal runtime surface is still primitive-specific rather than backend-neutral

- Status: `active`
- Category: `runtime boundary shape`
- Introduced: `2026-03-09`
- Owner area: `stwo-metal-sys`

Why it exists now:

The native Metal runtime currently exposes bounded helpers for `u32`,
`u32x4`, and one explicit poly-order permutation. This is deliberate for the
first proving-surface slices, but it is not yet the stable backend-neutral ABI
we ultimately want.

Current containment:

- `crates/stwo-metal-sys/src/metal.rs`
- `crates/stwo-metal-sys/metal`
- `crates/stwo-metal/src/stwo_metal`

Risk if left in place:

Future ports could accumulate one-off runtime entrypoints instead of converging
on a coherent Metal ABI for proving operations.

Exit condition:

An approved runtime-boundary refactor groups the bounded helpers into a smaller
and more durable Metal ABI surface without losing deterministic parity tests.

Target retirement point:

- `T5`

### TD-0005: FRI domain factor generation is still host-derived rather than owned by the Metal runtime

- Status: `active`
- Category: `host orchestration bridge`
- Introduced: `2026-03-09`
- Owner area: `T5 bounded FRI bring-up`

Why it exists now:

The bounded Metal FRI slices currently derive inverse-`y` and inverse-`x`
factor buffers on the Rust host from vendored Stwo domain semantics and pass
them into native Metal kernels. This keeps the first proving-path cuts narrow
and explicit while the proving-facing runtime boundary is still being shaped.

Current containment:

- `crates/stwo-metal/src/backend/metal/fri.rs`

Risk if left in place:

The arithmetic remains correct, but the supported Metal story could stall at a
host-prepared twiddle bridge instead of converging on a more durable runtime
boundary for domain-derived factors.

Exit condition:

The declared proving path owns its domain-factor generation or equivalent
twiddle materialization at a stable runtime boundary without changing bounded
CPU-oracle parity guarantees.

Target retirement point:

- `T5`

### TD-0006: The first inner FRI-layer commitment still leaves Metal through an explicit CPU bridge

- Status: `active`
- Category: `proving-facing bridge`
- Introduced: `2026-03-09`
- Owner area: `T5 bounded FRI bring-up`

Why it exists now:

The bounded Metal lane originally needed an explicit CPU bridge to keep the
first inner-layer commitment honest before a native `stwo-metal` commitment
surface existed. The bridge remains available as a validation path even though
the native commitment boundary now exists.

Current containment:

- `crates/stwo-metal/src/backend/metal/handoff.rs`

Risk if left in place:

The bridge is now contained and explicit, but keeping it around too long risks
leaving two proving-facing paths alive when only one should remain primary.

Exit condition:

The native first inner-layer query and decommit path is stable enough that the
CPU bridge is no longer needed even as a validation surface.

Target retirement point:

- `T5`

### TD-0007: Native first inner-layer commitment is still host-owned and root-focused

- Status: `active`
- Category: `native commitment boundary`
- Introduced: `2026-03-09`
- Owner area: `T5 bounded FRI bring-up`

Why it exists now:

The native `stwo-metal` commitment boundary currently reads folded line values
back to the host and builds the lifted Merkle tree there. It now supports
native decommit semantics on top of that host-owned tree, but the hashing path
itself is still not GPU-side.

Current containment:

- `crates/stwo-metal/src/backend/metal/line.rs`

Risk if left in place:

The proving path can now commit and decommit honestly for the first inner
layer, but host-owned hashing may become an accidental long-term ceiling if the
team never explicitly decides whether that is acceptable for T5.

Exit condition:

The team has explicitly decided whether host-owned hashing is an acceptable T5
endpoint or has replaced it with a GPU-side hash path.

Target retirement point:

- `T5`

### TD-0008: Last-layer interpolation in the bounded FRI commitment slice is still CPU-bridged

- Status: `active`
- Category: `proving-facing bridge`
- Introduced: `2026-03-09`
- Owner area: `T5 bounded FRI bring-up`

Why it exists now:

The bounded `MetalFriCommitmentSlice` now packages the native inner-layer FRI
sequence with last-layer interpolation, but that interpolation still
materializes a `LineEvaluation<CpuBackend>` through the explicit handoff bridge
before deriving the bounded `LinePoly`.

Current containment:

- `crates/stwo-metal/src/backend/metal/commitment_slice.rs`
- `crates/stwo-metal/src/backend/metal/handoff.rs`

Risk if left in place:

The proof-facing slice is now truthful about its degree-bound output, but the
last-layer polynomial contract could remain partially CPU-owned instead of
converging on a clearer native `stwo-metal` interpolation boundary.

Exit condition:

The bounded Metal FRI path derives the last-layer polynomial through a native
`stwo-metal` interpolation boundary, or the team explicitly accepts the
CPU-bridged interpolation as the T5 endpoint.

Target retirement point:

- `T5`

### TD-0009: The bounded full FRI proof candidate still depends on caller-supplied folding alphas

- Status: `retired`
- Category: `transcript ownership`
- Introduced: `2026-03-09`
- Owner area: `T5 bounded FRI bring-up`

Why it existed:

The bounded Metal FRI proof slice initially packaged the first-layer and
inner-layer proof surfaces into the vendored `ExtendedFriProof` shape, but it
still took the first-layer alpha and inner-layer alphas as explicit caller
inputs instead of deriving them from a transcript-owned channel boundary.

Current containment:

- `crates/stwo-metal/src/backend/metal/proof.rs`
- `crates/stwo-metal/src/backend/metal/proof_slice.rs`

Risk if left in place:

The bounded proof object shape would have remained truthful, but the supported
Metal story could have stalled at a proof-construction helper rather than a
declared proving sub-path with transcript ownership aligned to vendored Stwo
semantics.

Exit condition:

The bounded FRI lane owns its challenge flow at an explicit transcript boundary
without weakening deterministic CPU-oracle parity.

Target retirement point:

- `T5`

### TD-0010: The declared bounded FRI sub-path is still FRI-only and not yet workload-complete

- Status: `retired`
- Category: `workload integration`
- Introduced: `2026-03-09`
- Owner area: `T5 bounded proving-path bring-up`

Why it existed:

`stwo-metal` now has a declared bounded Blake2s FRI proving sub-path, but that
declared path still stops at the FRI boundary. It does not yet own quotient,
trace, or PCS integration for one declared Stwo workload.

Current containment:

- `crates/stwo-metal/src/backend/metal/subpath.rs`
- `crates/stwo-metal/src/backend/metal/prover.rs`

Risk if left in place:

The project could over-index on a truthful FRI lane while still lacking one
declared Stwo workload boundary that demonstrates where FRI plugs into the rest
of the prover.

Exit condition:

One declared Stwo workload boundary consumes the declared bounded FRI sub-path
with explicit quotient, trace, and PCS ownership and deterministic CPU-oracle
parity.

Target retirement point:

- `T5`

### TD-0011: The executable hybrid workload still begins after quotient accumulation

- Status: `active`
- Category: `workload handoff`
- Introduced: `2026-03-09`
- Owner area: `T5 workload handoff`

Why it exists now:

`stwo-metal` now has a declared hybrid workload boundary with explicit witness,
quotient, PCS, and FRI ownership, plus an explicit CPU-owned
wide-fibonacci witness handoff feeding the native Metal trace boundary. A
bounded native wide-fibonacci quotient primitive now exists for the benchmark
row, but the declared executable hybrid workload still begins only once a
CPU-owned quotient evaluation already exists.

Current containment:

- `crates/stwo-metal/src/backend/metal/workload.rs`
- `crates/stwo-metal/src/backend/metal/subpath.rs`
- `fixtures/standalone-benchmarks/src/bin/wide_fibonacci_prove.rs`

Risk if left in place:

The project could sound more workload-complete than it really is, even though
the declared executable hybrid path still begins after quotient accumulation.

Exit condition:

One declared workload owns a native quotient-accumulation boundary on top of
the witness-owned handoff, so the executable Metal workload no longer begins
after a precomputed CPU-owned quotient evaluation.

Target retirement point:

- `T5`

### TD-0012: The wide-fibonacci prove benchmark still bridges native Metal quotient output into the inherited CUDA-era proving lane

- Status: `retired`
- Category: `benchmark execution boundary`
- Introduced: `2026-03-09`
- Owner area: `T5 benchmark-target alignment`

Why it existed:

`stwo-metal` now declares the log-size-20 wide-fibonacci benchmark target with
an explicit `90 ms` RTX 4090 reference goal, and the standalone
`wide_fibonacci_trace` benchmark now enters through a native `.metal`
trace-generation path. `wide_fibonacci_prove` now generates its trace and
accumulates its quotient through native Metal paths, but it still bridges the
quotient output back into the inherited CUDA proving lane for pre-FRI PCS
commitment and the rest of the proving flow. The first Apple Silicon native
trace baseline is now measured at `66.61 ms` for `log_n_instances = 20`,
`n_columns = 100`, `STWO_METAL_MODE=metal-dev`, `warmups = 0`, and
`samples = 1`, but that number is trace-only and does not close the prove
benchmark boundary.

Current containment:

- `crates/stwo-metal/src/backend/metal/benchmark.rs`
- `fixtures/standalone-benchmarks/src/bin/wide_fibonacci_prove.rs`

Risk if left in place:

The project could overstate benchmark progress if the executable prove row
starts in Metal but still quietly depends on the inherited CUDA proving lane
after quotient accumulation.

Exit condition:

The executable `wide_fibonacci_prove` row no longer needs the Metal-to-CUDA
quotient-output bridge, and any remaining non-Metal stages are explicitly
named rather than implicitly inherited from CUDA.

Target retirement point:

- `T6`

Retired by:

- `DEC-0056`

### TD-0013: The target upstream example acceptance set is not yet vendored in the local snapshot

- Status: `retired`
- Category: `acceptance-input gap`
- Introduced: `2026-03-09`
- Owner area: `T5a planning correction`

Why it existed:

The project was rebaselined around proving upstream Stwo examples with
`MetalBackend` unchanged except for backend wiring, but the current local
vendored snapshot under `vendor/` did not expose the upstream
`crates/examples` tree directly. That meant the target acceptance set existed
as named scope, not yet as a fully local executable matrix.

Current containment:

- `docs/roadmap.md`
- `docs/program-plan.md`
- `docs/controller.md`

Resolution:

The upstream `stwo-examples` source is now pinned locally under
`vendor/stwo-upstream-dev-62b228e/crates/examples` with recorded source
provenance, so `T7` now has an auditable local input.

### TD-0014: The vendored upstream example crate is pinned from a separately recorded upstream commit and a locally adapted manifest

- Status: `active`
- Category: `acceptance-input pin`
- Introduced: `2026-03-09`
- Owner area: `T7 acceptance harness`

Why it exists now:

The vendored upstream example crate is now present locally, but it was copied
from a separately recorded upstream commit and given a local `Cargo.toml` so it
can build against the vendored `stwo` and `stwo-constraint-framework` crates in
this repository. The example workload logic is intended to remain upstream-owned,
but the pin still relies on that explicit local adaptation.

Current containment:

- `vendor/stwo-upstream-dev-62b228e/crates/examples/Cargo.toml`
- `vendor/stwo-upstream-dev-62b228e/crates/examples/STWO_UPSTREAM_SOURCE.md`

Risk if left in place:

The acceptance surface could drift subtly from the exact upstream example crate
shape if the local manifest adaptation or source pin is not maintained
carefully.

Exit condition:

The example crate is pinned in a way that no longer requires a separate local
manifest adaptation, or the project deliberately adopts and documents that
adapted vendoring model as stable process.

Target retirement point:

- `T7`

### TD-0015: The first upstream-example prove/verify boundary still depends on an explicit CPU prover bridge

- Status: `retired`
- Category: `backend-completion gap`
- Introduced: `2026-03-09`
- Owner area: `T7 example proving`

Why it existed:

The unchanged vendored upstream `wide_fibonacci` example now proves and
verifies through a real acceptance fixture, but the proving path still crosses
an explicit CPU bridge after native Metal trace generation. `MetalBackend` now
implements `PolyOps`, `AccumulationOps`, and `QuotientOps` through explicit CPU
bridges, and it now implements `FriOps` through an explicit bridge-backed FRI
trait slice. It now also implements `MleOps`, `GkrOps`, the generic Stwo
`Backend` trait, and the Blake2s `BackendForChannel` surface. The remaining
bridge exists because the vendored upstream `FrameworkComponent` still only
implements `ComponentProver` for `CpuBackend` and `SimdBackend`, so unchanged
framework-backed examples cannot yet enter the stock proving path through
`MetalBackend`.

Current containment:

- `crates/stwo-metal/src/backend/metal/witness.rs`
- `crates/stwo-metal/src/backend/metal/poly.rs`
- `crates/stwo-metal/src/backend/metal/accumulation.rs`
- `crates/stwo-metal/src/backend/metal/quotient.rs`
- `crates/stwo-metal/src/backend/metal/fri.rs`
- `crates/stwo-metal/src/backend/metal/lookups.rs`
- `crates/stwo-metal/src/backend/metal/blake2s.rs`
- `fixtures/upstream-example-acceptance/src/lib.rs`
- `fixtures/upstream-example-acceptance/tests/wide_fibonacci_prove_verify.rs`

Risk if left in place:

The project could overclaim example-backed proving support while the critical
prove path still depends on CPU backend substitution. It also limits how much
performance signal the acceptance harness can provide for the eventual
log-size-20 wide-fibonacci target.

Exit condition:

At least one accepted upstream example proves and verifies through a direct
`MetalBackend` path, with no explicit CPU prover bridge required after native
Metal trace generation and with the remaining upstream component-prover seam
resolved cleanly enough to make backend substitution truthful.

Target retirement point:

- `T7`

Retired by:

- `DEC-0040`

### TD-0016: Vendored framework-backed components still lack a truthful `ComponentProver<MetalBackend>` path

- Status: `active`
- Category: `upstream integration gap`
- Introduced: `2026-03-09`
- Owner area: `T7 example proving`

Why it exists now:

The vendored upstream framework component layer currently implements
`ComponentProver` only for `CpuBackend` and `SimdBackend`. The first unchanged
upstream example now proves through a local acceptance adapter, but there is
still no shared or upstream-owned truthful `ComponentProver<MetalBackend>`
surface for framework-backed examples.

Current containment:

- `vendor/stwo-upstream-dev-62b228e/crates/constraint-framework/src/prover/component_prover.rs`
- `fixtures/upstream-example-acceptance/src/lib.rs`
- `fixtures/upstream-example-acceptance/tests/wide_fibonacci_prove_verify.rs`
- `fixtures/upstream-example-acceptance/tests/state_machine_prove_verify.rs`
- `fixtures/upstream-example-acceptance/tests/blake_prove_verify.rs`

Risk if left in place:

The project could treat the first acceptance-local adapter as full closure and
stop short of a reusable path for other framework-backed examples. That would
leave the remaining CPU-domain bridge hidden in test-local code instead of
driving the next honest backend-completion tranche.

Exit condition:

One truthful `ComponentProver<MetalBackend>` path exists for framework-backed
upstream examples, either by an approved vendored upstream refactor or by a
clean local adapter boundary that does not fork workload semantics.

Target retirement point:

- `T7`

### TD-0017: The current framework-component Metal adapter is privately shared and still CPU-domain backed

- Status: `active`
- Category: `acceptance bridge`
- Introduced: `2026-03-09`
- Owner area: `T7 example proving`

Why it exists now:

The first direct `MetalBackend` upstream-example proofs now use a privately
shared adapter around vendored `FrameworkComponent` in
`fixtures/stwo-metal-fixture-shims`. That remains the smallest safe step
because it avoids a nested-workspace dependency conflict in the main
`stwo-metal` crate and keeps the remaining CPU-domain quotient path explicit.
The framework-backed rows still consume one registered acceptance bridge
catalog, and the adapter is no longer harness-local, but it is still
non-public and still CPU-domain backed.

Current containment:

- `fixtures/stwo-metal-fixture-shims/src/lib.rs`
- `fixtures/upstream-example-acceptance/tests/state_machine_prove_verify.rs`
- `fixtures/upstream-example-acceptance/tests/blake_prove_verify.rs`

Risk if left in place:

The project could stall after the first direct backend-substitution examples
and accidentally treat the private compatibility bridge as the long-term
proving boundary.

Exit condition:

The adapter is either generalized cleanly for the next example shapes,
replaced by an upstream-facing implementation, or retired in favor of a native
Metal framework-component proving boundary.

Target retirement point:

- `T7`

### TD-0019: The current SIMD-component Metal adapter is privately shared and depends on coefficient-retaining trace conversion

- Status: `active`
- Category: `acceptance bridge`
- Introduced: `2026-03-09`
- Owner area: `T7 example proving`

Why it exists now:

The unchanged vendored upstream `xor` MLE-eval row now proves and verifies
through `MetalBackend` with a privately shared adapter around vendored
`ComponentProver<SimdBackend>`. That keeps workload logic unchanged and opens
the first mixed-component upstream row, but it still depends on retained
polynomial coefficients when converting Metal-owned traces into the bridged
proving view. The row constructs that adapter only through the shared
registered acceptance bridge catalog, but the adapter remains a private
compatibility boundary rather than a stable generated or native path.

Current containment:

- `fixtures/stwo-metal-fixture-shims/src/lib.rs`
- `fixtures/upstream-example-acceptance/tests/xor_mle_eval_prove_verify.rs`

Risk if left in place:

The project could confuse this private mixed-component bridge with a stable
shared backend boundary and stop short of a reusable or upstream-owned path
for non-framework prover components.

Exit condition:

One truthful shared or upstream-owned `ComponentProver<MetalBackend>` path
exists for the non-framework example shapes currently covered by the local
SIMD-component bridge, or the local adapter is retired in favor of a native
Metal replacement boundary.

Target retirement point:

- `T7`

### TD-0020: Acceptance-local Metal adapters still need retirement into a cleaner shared boundary

- Status: `retired`
- Category: `boundary consolidation`
- Introduced: `2026-03-10`
- Owner area: `post-T7 backend hardening`

Why it existed:

The acceptance harness originally had two local proving adapters: one for
vendored `FrameworkComponent` rows and one for vendored
`ComponentProver<SimdBackend>` rows. Those adapters were the smallest
correctness-preserving way to prove the named non-blocked upstream examples
through `MetalBackend`, but they were still test-local boundaries rather than
a cleaner shared internal proving surface.

Current containment:

- `fixtures/stwo-metal-fixture-shims/src/lib.rs`
- `fixtures/upstream-example-acceptance/tests/wide_fibonacci_prove_verify.rs`
- `fixtures/upstream-example-acceptance/tests/state_machine_prove_verify.rs`
- `fixtures/upstream-example-acceptance/tests/blake_prove_verify.rs`
- `fixtures/upstream-example-acceptance/tests/xor_mle_eval_prove_verify.rs`

Risk if left in place:

This ownership-only debt is now retired. Remaining compatibility risk is
tracked more precisely in `TD-0017` and `TD-0019`.

Exit condition:

The registered bridge catalog and its current framework-backed and SIMD-backed
adapters move into a shared non-public boundary with explicit laws and durable
ownership.

Target retirement point:

- `T8`

### TD-0025: The vendored Stwo snapshot currently requires an older nightly feature surface than the installed local toolchains provide

- Status: `retired`
- Category: `verification environment`
- Introduced: `2026-03-10`
- Owner area: `acceptance verification`

Why it existed:

The current vendored Stwo snapshot relied on nightly feature usage around
`array_chunks` and related APIs that no longer matched the repository-pinned
toolchain contract. That meant full acceptance-matrix cargo verification could
fail before any local acceptance-bridge changes were exercised.

Resolution:

- `vendor/stwo-upstream-dev-62b228e/crates/stwo`
- `rust-toolchain.toml`

The vendored chunking surface is now modernized enough that the pinned nightly
again supports the private planning tests, the acceptance harness unit tests,
and the current non-blocked acceptance example matrix.

### TD-0021: The end-to-end wide-fibonacci benchmark row is support-honest but still far from the declared `90 ms` target

- Status: `active`
- Category: `benchmark performance gap`
- Introduced: `2026-03-10`
- Owner area: `T8 benchmark optimization`

Why it exists now:

`wide_fibonacci_prove_verify_v1` now executes end to end through
`MetalBackend` and verifies successfully on Apple Silicon, so the old
benchmark-boundary debt is retired. The benchmark contract is now explicit:
non-plan benchmark measurements must use `cargo_profile = release` and
`STWO_METAL_MODE=metal-prod` unless an override is set for diagnostics. Even
under that production-grade contract, the row is still far from the declared
north star: best measured `1456.654041 ms` total, with `prove_ms = 1456.38`
and `verify_ms = 0.274041`, at `log_n_instances = 20`, `n_columns = 100`,
`warmups = 0`, `samples = 1`, and `threads = 14`. The benchmark has since
landed native standard Blake2s parent hashing, kept the generated trace tree
native through final decode, and reduced repeated lifted decommit and
tree-query staging. A recent `log_n_instances = 20` production rerun with
`warmups = 0`, `samples = 3`, and `threads = 14` came in at
`prove_ms mean = 1162.3086803333333`, `prove_ms median = 956.267291`,
`prove_core_prove_values_ms mean = 561.6742503333334`, and
`trace_commit_merkle_ms mean = 78.27522233333333`. After reusing grouped
point-eval coefficient vectors and canonical quotient accumulation order, a
follow-up `log_n_instances = 20` production rerun came in at
`prove_ms mean = 1153.4602223333334`, `prove_ms median = 925.868`,
`prove_core_prove_values_ms mean = 570.164667`, and
`trace_commit_merkle_ms mean = 73.39211100000001`. The dominant measured cost
is still `prove_values`, not Merkle staging. After building proof-facing
`sampled_values` alongside `samples` and reusing prepared query buffers
without per-tree cloning, the next `log_n_instances = 20` production rerun
came in at `prove_ms mean = 1152.4745003333335`,
`prove_ms median = 942.208834`, `prove_core_prove_values_ms mean = 563.6594163333333`,
and `trace_commit_merkle_ms mean = 77.41322233333334`. That is a
semantics-preserving staging cleanup, but not a material movement of the
dominant wall. After feeding quotient accumulation directly from grouped
sampled-randomness by log size, the next `log_n_instances = 20` production
rerun came in at `prove_ms mean = 1152.5688746666667`,
`prove_ms median = 952.858083`, `prove_core_prove_values_ms mean = 560.2786666666667`,
and `trace_commit_merkle_ms mean = 74.00340233333333`. That again confirms
the remaining wall is higher-level prove-values work, not this regroup pass.

Current containment:

- `fixtures/standalone-benchmarks/src/bin/wide_fibonacci_prove.rs`
- `crates/stwo-metal/src/backend/metal/blake2s.rs`
- `crates/stwo-metal/src/backend/metal/accumulation.rs`
- `crates/stwo-metal/src/backend/metal/quotient.rs`
- `crates/stwo-metal/src/backend/metal/poly.rs`
- `crates/stwo-metal/src/backend/metal/column.rs`
- `crates/stwo-metal/src/backend/metal/handoff.rs`
- `crates/stwo-metal/src/stwo_metal/base_field_vec.rs`
- `vendor/stwo-upstream-dev-62b228e/crates/stwo/src/prover/vcs_lifted/prover.rs`
- `vendor/stwo-upstream-dev-62b228e/crates/stwo/src/prover/pcs/mod.rs`
- `docs/controller.md`
- `docs/roadmap.md`

Risk if left in place:

The project could confuse benchmark-boundary closure with performance closure,
or it could optimize the wrong layer without using the measured phase
breakdown. That would make the `90 ms` reference goal look arbitrary instead of
turning it into a disciplined optimization program. The row is also still
about `1.05x` slower than the current `log_n_instances = 20` SIMD reference
(`1390 ms`) and far from the historical GPU row (`87 ms`).

Exit condition:

The end-to-end `wide_fibonacci_prove_verify_v1` row has repeatable benchmark
measurements in the declared environment and no longer spends the majority of
its time in the currently dominant grouped PCS sampled-value,
composition-generation, and remaining upper commitment stages, with progress
evaluated against the recorded phase breakdown rather than against
file-presence heuristics. The native point-evaluation lane, grouped sampled
value scheduler, direct wide-tree standard Blake2s leaf path, native
parent-layer hashing, native trace-tree residency, repeated
tree-decommit/query-preparation staging reductions, and the first grouped
point-eval/quotient-regroup reduction are now landed, so the next expected
structural retirements are the remaining higher-level prove-values grouping
work above that lane and the upper commitment/hash path. The direct grouped
quotient-feed path is now landed as well, so the remaining hot work is not the
obvious tree-shaped regrouping layer anymore.

Target retirement point:

- `T8`

### TD-0022: The bounded small-domain `PolyOps` fallback is still explicitly CPU-backed

- Status: `active`
- Category: `bounded algebra fallback`
- Introduced: `2026-03-10`
- Owner area: `T8 benchmark optimization`

Why it exists now:

`PolyOps` point evaluation, barycentric helpers, zero-padding extend,
split-at-mid, and batch point evaluation are now Metal-owned over Metal-backed
storage, but the bounded small-domain evaluate/interpolate fallback still
delegates to `CpuBackend`. This is the smallest safe remaining fallback in the
trait surface after the larger prove-values PCS bridges were retired.

Current containment:

- `crates/stwo-metal/src/backend/metal/poly.rs`
- `crates/stwo-metal/src/backend/metal/capability.rs`

Risk if left in place:

The supported `PolyOps` story could look more complete than it really is, and
the benchmark row could still pay avoidable small-domain fallback costs if
future prove-values work reaches that path more often than expected.

Exit condition:

The bounded small-domain evaluate/interpolate fallback is either retired into
Metal-owned storage logic or explicitly accepted as a non-benchmark-critical
host fallback with measured evidence that it does not matter for the target
rows.

Target retirement point:

- `T8`

### TD-0023: Lifted Blake2s leaf construction is still host-owned and dominates current commitment cost

- Status: `retired`
- Category: `host-owned hash path`
- Introduced: `2026-03-10`
- Owner area: `T8 benchmark optimization`

Why it existed:

The benchmark row had reached the point where large lifted Blake2s leaf
construction on the wide trace tree was still a first-order commitment cost.
The bounded Metal leaf kernel only covered small standard trees, so the
benchmark-critical wide tree still paid a flatten-and-stage host path.

Current containment:

- `crates/stwo-metal/src/backend/metal/blake2s.rs`
- `vendor/stwo-upstream-dev-62b228e/crates/stwo/src/prover/vcs_lifted/prover.rs`
- `fixtures/standalone-benchmarks/src/bin/wide_fibonacci_prove.rs`
- `docs/controller.md`

Risk if left in place:

The project would have kept chasing arithmetic wins while a first-order
commitment wall remained outside the native polynomial and FRI lanes.

Exit condition:

The large-tree lifted Blake2s leaf path is no longer a first-order trace
commitment cost on the production `wide_fibonacci_prove_verify_v1` row.

Target retirement point:

- `T8`

### TD-0018: Poseidon acceptance is blocked by a vendored lifted-protocol AIR-degree limit

- Status: `active`
- Category: `upstream protocol limit`
- Introduced: `2026-03-09`
- Owner area: `T7 acceptance planning`

Why it exists now:

The vendored upstream `poseidon` example already marks its lifted proving path
unsupported because the current lifted protocol does not support the example's
AIR degree shape. That means the current stop on `poseidon` is not a pure
Metal-backend gap.

Current containment:

- `vendor/stwo-upstream-dev-62b228e/crates/examples/src/poseidon/mod.rs`
- `docs/roadmap.md`

Risk if left in place:

The project could waste time trying to force `poseidon` into the next Metal
acceptance tranche and misclassify an upstream protocol limitation as a Metal
port failure.

Exit condition:

The vendored upstream snapshot supports the required lifted protocol path for
the `poseidon` example, or the project explicitly chooses a different accepted
Poseidon proving route.

Target retirement point:

- `T7`
