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

### TD-0017: The current framework-component Metal adapter is acceptance-local and still CPU-domain backed

- Status: `active`
- Category: `acceptance bridge`
- Introduced: `2026-03-09`
- Owner area: `T7 example proving`

Why it exists now:

The first direct `MetalBackend` upstream-example proof now uses an
acceptance-local adapter around vendored `FrameworkComponent`. This is the
smallest safe step because it avoids a nested-workspace dependency conflict in
the main `stwo-metal` crate and keeps the remaining CPU-domain quotient path
explicit, but it is not yet a reusable shared boundary for additional example
rows.

Current containment:

- `fixtures/upstream-example-acceptance/src/lib.rs`
- `fixtures/upstream-example-acceptance/tests/wide_fibonacci_prove_verify.rs`
- `fixtures/upstream-example-acceptance/tests/state_machine_prove_verify.rs`
- `fixtures/upstream-example-acceptance/tests/blake_prove_verify.rs`

Risk if left in place:

The project could stall after the first direct backend-substitution example and
accidentally treat acceptance-local glue as the long-term proving boundary.

Exit condition:

The adapter is either generalized cleanly for the next example shapes,
replaced by an upstream-facing implementation, or retired in favor of a native
Metal framework-component proving boundary.

Target retirement point:

- `T7`

### TD-0019: The current SIMD-component Metal adapter is acceptance-local and depends on coefficient-retaining trace conversion

- Status: `active`
- Category: `acceptance bridge`
- Introduced: `2026-03-09`
- Owner area: `T7 example proving`

Why it exists now:

The unchanged vendored upstream `xor` MLE-eval row now proves and verifies
through `MetalBackend` with an acceptance-local adapter around vendored
`ComponentProver<SimdBackend>`. That keeps workload logic unchanged and opens
the first mixed-component upstream row, but it still lives only in the
acceptance harness and currently depends on retained polynomial coefficients
when converting Metal-owned traces into the bridged proving view.

Current containment:

- `fixtures/upstream-example-acceptance/src/lib.rs`
- `fixtures/upstream-example-acceptance/tests/xor_mle_eval_prove_verify.rs`

Risk if left in place:

The project could confuse this local mixed-component bridge with a stable
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

- Status: `active`
- Category: `boundary consolidation`
- Introduced: `2026-03-10`
- Owner area: `post-T7 backend hardening`

Why it exists now:

The acceptance harness now has two local proving adapters: one for vendored
`FrameworkComponent` rows and one for vendored `ComponentProver<SimdBackend>`
rows. Those adapters were the smallest correctness-preserving way to prove the
named non-blocked upstream examples through `MetalBackend`, but they are still
test-local boundaries rather than a cleaner shared internal proving surface.

Current containment:

- `fixtures/upstream-example-acceptance/src/lib.rs`
- `fixtures/upstream-example-acceptance/tests/wide_fibonacci_prove_verify.rs`
- `fixtures/upstream-example-acceptance/tests/state_machine_prove_verify.rs`
- `fixtures/upstream-example-acceptance/tests/blake_prove_verify.rs`
- `fixtures/upstream-example-acceptance/tests/xor_mle_eval_prove_verify.rs`

Risk if left in place:

The project could return to native performance work with a correct backend but
keep example-backed proving dependent on test-local adapter glue, which would
blur the boundary between acceptance scaffolding and the durable proving
architecture.

Exit condition:

The current acceptance-local adapters are either retired into a shared
non-public boundary with explicit laws and ownership, replaced by an
upstream-facing proving path, or removed in favor of native Metal proving
surfaces that no longer require them.

Target retirement point:

- `T8`

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
north star: `10900.002875 ms` total, with
`prove_ms = 10899.813583000001` and `verify_ms = 0.18929200000000002`, at
`log_n_instances = 20`, `n_columns = 100`, `warmups = 0`, `samples = 1`, and
`threads = 14`. The dominant measured costs are now
`prove_core_prove_values_ms = 4686.306874999999`,
`prove_core_composition_generation_ms = 3729.947666`,
`trace_generation_ms = 1691.5720410000001`, and
`trace_commit_merkle_ms = 214.14654099999998`.

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
about `7.8x` slower than the current `log_n_instances = 20` SIMD reference
(`1390 ms`) and far from the historical GPU row (`87 ms`).

Exit condition:

The end-to-end `wide_fibonacci_prove_verify_v1` row has repeatable benchmark
measurements in the declared environment and no longer spends the majority of
its time in the currently dominant PCS sampled-value, composition-generation,
and remaining host-owned commitment stages, with progress evaluated against the
recorded phase breakdown rather than against file-presence heuristics. The
native point-evaluation lane and direct Metal-backed wide-fibonacci quotient
staging are now landed, so the next expected structural retirements are
prove-values duplication above that lane and the remaining host-owned
commitment/hash path.

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
