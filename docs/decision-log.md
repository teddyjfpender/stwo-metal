# Decision Log

Use this file for durable project decisions.

Do not log implementation trivia. Log only decisions that change contracts,
sequencing, or process expectations.

## Entry template

```md
### DEC-XXXX: Title

- Date:
- Status: proposed | accepted | superseded
- Owners:
- Related design note:

Decision:

Context:

Alternatives rejected:

Impact:

Superseded by:
```

## Entries

### DEC-0133: Feed quotient accumulation from direct log-size groups before changing higher-level prove-values laws

- Date: `2026-03-11`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)

Decision:

The next semantics-preserving reduction after proof-facing sample
materialization cleanup should be to feed quotient accumulation directly from
log-size-grouped sampled columns with randomness and periodicity, instead of
building a full tree-shaped sampled-randomness structure and then regrouping it
by log size.

Context:

`prove_values` still dominated the generated wide-fibonacci benchmark after the
earlier grouping and materialization slices. The next remaining host-shaped
pass was inside quotient staging: sampled points were first expanded into a
full tree-shaped randomness structure and only afterward regrouped by log size
for `ColumnSampleBatch` creation.

Alternatives rejected:

- widen the quotient interface before removing the intermediate regroup pass
- change verifier-visible sample structure first
- treat the remaining wall as purely lower-level arithmetic without removing
  this higher-level feed duplication

Impact:

- quotient staging now constructs grouped sampled-randomness directly in
  log-size order
- one full intermediate tree-shaped regroup pass is removed from the hot path
- the next optimization slice can focus on the remaining higher-level grouping
  and later decommit staging

Superseded by:

- none

### DEC-0134: Metal FRI last-layer interpolation must not cross through CpuBackend

- Date: `2026-03-11`
- Status: `accepted`
- Owners: `project team`

Decision:

`MetalBackend` now owns the final FRI last-layer interpolation through a native
Metal line-IFFT path, and `FriProver::commit_last_layer` consumes a backend
interpolation hook instead of forcing `LineEvaluation<B>` through
`LineEvaluation<CpuBackend>`.

Context:

The remaining benchmark-critical ownership walls had shifted away from the
obvious grouped prove-values cleanup and toward a few explicit CPU-shaped
handoffs. The vendored FRI prover still converted the final last-layer line
evaluation to `CpuBackend` only to reuse a small interpolation routine. That
conversion was narrow, semantically isolated, and verifier-visible through the
last-layer polynomial, making it a good law-preserving slice to retire.

Alternatives rejected:

- leave `commit_last_layer` on `evaluation.to_cpu().interpolate()`
- add a benchmark-local or Metal-only special-case around the vendored FRI
  prover instead of moving the law into the backend abstraction
- widen the public workload surface before retiring the narrow FRI ownership
  cut

Impact:

- the final FRI last-layer polynomial for `MetalBackend` is now computed from a
  native Metal coordinate-wise line-IFFT path
- proof-level parity remains enforced by the existing Metal-vs-CPU FRI prover
  regression
- the next honest CPU-shaped ownership target is the broader FRI/PCS
  workload/handoff boundary, not this final last-layer conversion

Superseded by:

- none

### DEC-0135: Workload FRI and quotient ingress must be canonically Metal-owned

- Date: `2026-03-11`
- Status: `accepted`
- Owners: `project team`

Decision:

The canonical workload-side ingress for FRI-ready evaluations and quotient
evaluations is now Metal-owned: `CircleDomain` plus `SecureFieldVec`. CPU
evaluation ingress remains available only as a compatibility adapter that
converts into the same Metal-owned input contract before proving.

Context:

After retiring the final FRI last-layer `CpuBackend` interpolation bridge, the
next remaining ownership wall was still visible in the workload API itself:
the live workload surface still advertised `ingest_cpu_fri_ready_evaluation`,
`ingest_cpu_quotient_evaluation`, and `prove_from_cpu_*` even though the
generated lane internally wants Metal-owned columns. That made the API
semantics more CPU-shaped than the underlying proving path.

Alternatives rejected:

- leave the workload ingress CPU-shaped until the entire PCS/FRI bridge is
  rewritten
- remove CPU ingress immediately and break compatibility rows that still need
  it
- add a benchmark-only native path without fixing the shared workload contract

Impact:

- workload-side FRI-ready and quotient inputs now have a truthful
  Metal-owned canonical form
- CPU evaluation ingress still works, but only by adapting onto the native
  path
- the next honest ownership target is now above the workload contract, in the
  broader FRI/PCS handoff and prove-values pipeline

Superseded by:

- none

### DEC-0136: Bounded FRI commitment slices must not re-enter the explicit CPU line bridge

- Date: `2026-03-11`
- Status: `accepted`
- Owners: `project team`

Decision:

The bounded FRI commitment/proof slice now derives its last-layer polynomial
from the native Metal line-evaluation path directly. It must not materialize a
`LineEvaluation<CpuBackend>` through the explicit CPU line handoff bridge just
to obtain the bounded last-layer polynomial.

Context:

After retiring the final FRI last-layer `CpuBackend` interpolation bridge and
making workload-side FRI/quotient ingress canonically Metal-owned, the bounded
proof-facing commitment slice still re-entered the older explicit CPU line
bridge when constructing the last-layer polynomial. That bridge remained
correct, but it reintroduced CPU-shaped ownership inside a path that already
had a native Metal line-evaluation contract.

Alternatives rejected:

- keep the bounded commitment slice on the CPU line bridge until the entire
  handoff module is removed
- hide the bridge behind another helper without changing the ownership law
- widen the public handoff surface instead of deleting the unnecessary bridge
  usage

Impact:

- bounded FRI commitment and proof slices now derive the last-layer
  polynomial from the native Metal line-evaluation path directly
- the explicit CPU line bridge remains available only for explicit bridge
  tests and transitional compatibility rows
- the next honest ownership target is now the broader PCS/FRI proving
  pipeline and shared handoff surface above the bounded slice

Superseded by:

- none

### DEC-0132: Build proof-facing sampled values alongside samples and reuse prepared query buffers before widening prove-values interfaces

- Date: `2026-03-11`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)

Decision:

The next semantics-preserving prove-values reduction after grouped point-eval
and quotient regroup cleanup should be to build proof-facing `sampled_values`
at the same time as `samples`, and to reuse cached prepared tree-query buffers
by reference instead of cloning them per tree.

Context:

`prove_values` still dominated the generated wide-fibonacci benchmark after the
earlier decommit and grouping slices. Inspection showed two remaining
structure-level costs in `pcs/mod.rs`: proof-facing sampled values were still
rebuilt from the fully materialized `samples` tree in a second nested pass,
and prepared tree queries were still cloned into a fresh tree-shaped structure
even after query preparation had already been cached by tree log-size.

Alternatives rejected:

- introduce a new public proof-facing sample container
- widen the quotient/decommit interfaces before removing local materialization
  overhead
- claim the remaining wall is purely arithmetic before removing this repeated
  staging work

Impact:

- proof-facing sampled values are now produced during sample evaluation rather
  than reconstructed from `samples` afterward
- prepared query buffers are reused directly during tree decommit instead of
  being cloned into a temporary tree-owned structure
- the next optimization slice can focus on the remaining higher-level
  grouping and decommit scheduling rather than this materialization overhead

Superseded by:

- none

### DEC-0131: Reuse batched point-eval coefficient vectors and ordered quotient accumulations before changing prove-values contracts

- Date: `2026-03-11`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)

Decision:

The next semantics-preserving prove-values reduction after tree-decommit query
reuse should be to keep grouped point-eval coefficient vectors in their batched
schedule and to accumulate quotient numerators directly in canonical
sample-point order, rather than rebuilding coefficient lists per batch and
sorting/regrouping accumulated numerators after the fact.

Context:

`prove_values` remained the dominant wall after native trace-tree residency and
reduced decommit query staging. The next repeated CPU-shaped work lived in two
places: batched point evaluation rebuilt a temporary coefficient vector for
every grouped request, and quotient accumulation sorted and regrouped all
accumulated numerators after collection even though a canonical sample-point
order was already available.

Alternatives rejected:

- change the public prove-values contract before removing local repeated
  grouping work
- introduce a new public grouped-evaluation schedule type
- treat the remaining wall as purely quotient arithmetic without first removing
  obvious regrouping overhead

Impact:

- grouped batched evaluation now retains its coefficient vectors directly
- quotient accumulation now groups by canonical sample-point order without a
  post-sort regroup pass
- the next prove-values optimization slice can focus on the remaining
  high-level grouping and decommit staging instead of these lower repeated
  traversals

Superseded by:

- none

### DEC-0130: Reuse lifted decommit query expansion and prepared tree queries before changing prove-values grouping laws

- Date: `2026-03-11`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)

Decision:

Before changing the higher-level prove-values grouping law, the next
semantics-preserving reduction should be to cache lifted Merkle read-index
expansion by column shift and to cache prepared tree query vectors by tree
log-size.

Context:

After native standard Blake2s parent hashing and native trace-tree residency
landed, the measured wall remained `prove_values`. Inspection showed two
remaining CPU-shaped staging loops that repeated identical work without
changing proof semantics: lifted Merkle decommit rebuilt the same
query-to-read-index vectors for every same-size column, and PCS prove-values
rebuilt the same prepared tree queries for every tree of the same lifted size.

Alternatives rejected:

- widen public column or Merkle interfaces to expose staging caches
- change the prove-values grouping contract before removing lower duplicated
  staging work
- treat the remaining wall as purely arithmetic without first removing obvious
  repeated query-shaping work

Impact:

- lifted tree decommit now reuses query expansion across same-shift columns
- PCS prove-values now reuses prepared tree query vectors across trees of the
  same log-size
- the next prove-values optimization step can focus on higher-level grouping
  and quotient/decommit staging rather than repeated query-shaping overhead

Superseded by:

- none

### DEC-0128: The generated wide-fibonacci trace tree should keep standard Blake2s parent layers native until final decode

- Date: `2026-03-11`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)

Decision:

The generated wide-fibonacci benchmark row should build the standard Blake2s
trace tree natively layer-by-layer after leaves and only decode the committed
layers once at the end, instead of repeatedly bouncing each native parent
layer through a host `Vec<Blake2sHash>` before hashing the next layer.

Context:

After native parent-layer hashing landed, the benchmark still re-encoded and
re-uploaded each host materialized Merkle layer before the next native layer.
That kept the hot trace tree on a host-shaped path even though the hash
computation itself had moved onto Metal.

Alternatives rejected:

- introduce a public GPU-resident Blake2s Merkle tree API
- hide the residency optimization behind a global hash-layer cache
- leave the benchmark path on repeated host round-trips until a larger prover
  rewrite exists

Impact:

- the benchmark trace tree now keeps native standard Blake2s parent layers on
  Metal until the final committed-tree decode
- the public backend contract remains unchanged
- the next dominant benchmark wall stays prove-values, not Merkle parent
  residency

Superseded by:

- none

### DEC-0129: The next prove-values cleanup should remove duplicate staging and one extra sampled-values flattening pass before changing grouping laws

- Date: `2026-03-11`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)

Decision:

After the native trace-tree residency step, the next semantics-preserving
prove-values reduction should be to remove duplicate benchmark-local staging
construction and collapse one extra sampled-values flattening pass in the PCS
prover, rather than changing grouping semantics immediately.

Context:

The measured generated lane still spends most of its time in `prove_values`.
The clearest low-risk next step was to remove benchmark-local duplicate
`ComponentProvers` staging and one extra nested flatten traversal before
mixing sampled values into the channel.

Alternatives rejected:

- change the prove-values grouping law first
- widen the bridge API before removing duplicate staging work
- claim the remaining wall is purely arithmetic without checking host-side
  traversal costs

Impact:

- the benchmark-local prove-values entry point is leaner
- the generic prover contract stays unchanged
- the next benchmark-critical investigation can focus on grouping and tree
  decommit costs with less staging noise

Superseded by:

- none

### DEC-0127: Standard Blake2s Merkle parent hashing should move onto Metal before broader prove-values rewrites

- Date: `2026-03-11`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)

Decision:

The next benchmark-focused CPU-dependence retirement step should be native
Metal parent-layer hashing for standard Blake2s Merkle trees, while keeping
M31-output rows and unsupported leaf shapes on the existing host path.

Context:

The generated wide-fibonacci lane already uses native Metal arithmetic and
native Blake2s leaf hashing on the large trace tree, but `build_next_layer`
still hashed all Merkle parents on the host. That is on the benchmark-critical
path and can be replaced without widening the public Merkle contract because
the packed eight-word Blake2s representation is already stable inside
`stwo-metal-sys`.

Alternatives rejected:

- jump straight to a broader prove-values rewrite before removing this simpler
  host-owned Merkle hotspot
- widen the public API around a GPU-resident Blake2s hash-column type first
- force the native path for M31-output rows before parity coverage exists

Impact:

- standard Blake2s Merkle trees can retire one more benchmark-critical host
  hashing step
- the remaining Merkle bottleneck is now the host-owned hash-column
  representation and staging, not parent hashing itself
- the public backend contract remains unchanged

Superseded by:

- none

### DEC-0126: The vendored stark-v row cannot promote beyond fail-closed without an external support signal

- Date: `2026-03-11`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0007-stark-v-support-promotion-gate.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0007-stark-v-support-promotion-gate.md)

Decision:

The current vendored `stark-v` row cannot promote from fail-closed to
supported status through internal-only `stwo-metal` changes. Promotion requires
either a backend-parametric downstream proving surface or a downstream
generated artifact satisfying `DN-0006`.

Context:

After vendoring the pinned checkout and landing a deterministic fail-closed
row, the remaining temptation would be to keep moving G8 forward by adding
local adapter logic until the downstream row appears supported. That would
conflict with the frozen generic/generated contract and would turn the vendor
directory into a hidden private fork.

Alternatives rejected:

- allow internal-only support promotion through local wrappers
- patch the vendored downstream tree until it matches `stwo-metal`
- leave the support-promotion gate implicit

Impact:

- the current G8 state is explicit and bounded
- the remaining G8 blocker is external and correctly named
- `stwo-metal` stays honest about what is and is not currently supported

Superseded by:

- none

### DEC-0125: The current vendored stark-v hardening row should be executable and fail-closed

- Date: `2026-03-11`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0005-stark-v-attachment-strategy.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0005-stark-v-attachment-strategy.md)
  - [`dn-0006-stark-v-generated-minimum-contract.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0006-stark-v-generated-minimum-contract.md)

Decision:

The current vendored `stark-v` downstream row should exist as one deterministic
local fail-closed check, rather than only as a collection of report scripts and
notes.

Context:

After vendoring the pinned downstream input, the program had enough structure
to run a full hardening report locally, but not yet one single executable row
that asserted the expected unsupported status. That made G8 progress harder to
regress-check than the benchmark and acceptance lanes.

Alternatives rejected:

- leave the vendored downstream row as report-only
- wait for a supported row before adding any downstream regression check
- fold the fail-closed row into unrelated benchmark or acceptance scripts

Impact:

- the current vendored downstream status is now executable and regression-
  checkable
- `TD-0029` can retire because a vendored executable hardening row now exists
- the remaining G8 blocker narrows to achieving a supported row, not preserving
  the current unsupported one

Superseded by:

- none

### DEC-0124: G8 should vendor the pinned stark-v input locally before claiming downstream hardening progress

- Date: `2026-03-11`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0004-stark-v-hardening-input-and-contract.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0004-stark-v-hardening-input-and-contract.md)

Decision:

After pinning, classifying, and gap-checking `stark-v`, G8 should vendor the
exact pinned checkout locally so downstream hardening no longer depends on a
temp clone outside the repository.

Context:

The current hardening row had become deterministic in logic but still depended
on an external temp checkout path. That is too weak for a milestone intended
to harden the contract against a real downstream consumer.

Alternatives rejected:

- keep relying on an ad hoc temp checkout
- vendor only textual notes while leaving the source tree external
- claim G8 progress without preserving the inspected downstream input

Impact:

- G8 now has one local downstream input under version control
- the hardening report can run against repository-local input
- the remaining G8 blocker narrows to support, not to input preservation

Superseded by:

- none

### DEC-0123: G8 should enumerate the downstream generated gap explicitly

- Date: `2026-03-11`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0006-stark-v-generated-minimum-contract.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0006-stark-v-generated-minimum-contract.md)

Decision:

After checking generated readiness, G8 should enumerate the missing generated
subset explicitly so the first supported downstream row has a bounded gap list
rather than a generic "artifact absent" label.

Context:

The current downstream report now distinguishes generic unsupported,
generated-required, and generated-artifact-absent. The next honest tightening
is to say which required generated fields are still missing against the frozen
minimum contract.

Alternatives rejected:

- stop at a binary generated-readiness result
- leave the generated gap implicit in `DN-0006` only
- wait for a future downstream producer before stating the missing set

Impact:

- G8 now records the downstream blockage as a concrete generated gap list
- future downstream producer work can target one checked missing-field set
- the fail-closed current row remains explicit and bounded

Superseded by:

- none

### DEC-0122: G8 should check generated readiness explicitly, not infer it from attachment class alone

- Date: `2026-03-11`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0006-stark-v-generated-minimum-contract.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0006-stark-v-generated-minimum-contract.md)

Decision:

After freezing the minimum generated subset for `stark-v`, G8 should keep one
deterministic generated-readiness check so the current downstream row says
explicitly whether a machine-readable artifact is present.

Context:

`DEC-0121` defined what the first supported generated row would require. The
next honest step is to check whether the pinned checkout exposes any
machine-readable artifact signals at all, rather than assuming that generated
support is merely "not wired yet."

Alternatives rejected:

- infer generated readiness only from the fail-closed attachment result
- leave generated readiness as a manual reading of source files
- claim downstream generated support is available before checking for an
  artifact surface

Impact:

- G8 now distinguishes "generated lane required" from "generated artifact
  actually present"
- the current downstream blockage is stated more precisely
- future generated support work has one deterministic before/after check

Superseded by:

- none

### DEC-0121: The first supported `stark-v` row must come through a bounded generated subset

- Date: `2026-03-11`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0006-stark-v-generated-minimum-contract.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0006-stark-v-generated-minimum-contract.md)

Decision:

The first supported downstream `stark-v` row must come through a bounded
generated-artifact subset rather than through a speculative generic backend
swap or a one-off downstream-specific API.

Context:

`DEC-0119` fixed the current pinned snapshot as generic-lane unsupported. The
next honest move is not to leave the generated path vague; it is to freeze the
minimum generated subset that would satisfy the existing `ArtifactRegistry` and
`ExecutionPlan` seam.

Alternatives rejected:

- keep the generated downstream path unspecified
- require one artifact per tiny internal opcode helper for the first supported
  row
- add `stark-v`-specific public APIs instead of using the generated contract

Impact:

- G8 now has a concrete supported-path target even though the current pinned
  snapshot still fails closed
- future downstream work can be judged against one bounded generated subset
- the benchmark and acceptance lanes stay separate from downstream hardening

Superseded by:

- none

### DEC-0120: G8 should emit a deterministic downstream hardening report before any support claim changes

- Date: `2026-03-11`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0005-stark-v-attachment-strategy.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0005-stark-v-attachment-strategy.md)

Decision:

After pinning and classifying `stark-v`, G8 should emit one deterministic
local hardening report artifact before any attempt to widen support claims or
start a downstream executable row.

Context:

The contract checker and attachment classifier now make the current status
truthful, but the result still lives only in process output. A stable G8 needs
one rerunnable artifact that records the pinned downstream head, the contract
result, the attachment classification, and the current fail-closed status.

Alternatives rejected:

- leave the hardening result as ephemeral shell output
- start changing support claims before the current unsupported status is
  captured as an artifact
- fold the hardening report into the benchmark lane tooling

Impact:

- G8 now has one deterministic report artifact for the downstream hardening
  row
- future support changes have one stable before/after record
- the unsupported generic-lane result remains explicit until a new supported
  row exists

Superseded by:

- none

### DEC-0119: The pinned `stark-v` snapshot is not a generic-lane substitution candidate

- Date: `2026-03-11`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0005-stark-v-attachment-strategy.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0005-stark-v-attachment-strategy.md)

Decision:

The current pinned `stark-v` snapshot should be treated as
`generic_lane = unsupported`, `generated_lane = required`, and
`status = fail_closed` until it exposes either a backend-parametric proving
surface or a generated artifact that satisfies the `stwo-metal` contract.

Context:

After pinning the downstream contract in `DEC-0118`, the next honest question
was whether `stwo-metal` could claim that `stark-v` was already a generic
backend-substitution candidate. The answer is no. The downstream proving and
preprocessing surfaces are typed directly around `SimdBackend`, generated macro
output returns `ComponentProver<SimdBackend>` and
`CircleEvaluation<SimdBackend, ...>`, and the repo depends on its own vendored
`external/stwo`.

Alternatives rejected:

- claim generic-lane compatibility based only on similar public prove/verify
  function names
- silently treat generated mapping as an implicit fallback for the generic lane
- defer the strategy decision and leave G8 attachment status ambiguous

Impact:

- G8 now has one truthful executable hardening row: deterministic contract
  check plus deterministic fail-closed classification
- the next supported downstream row must come through generated mapping or a
  genuinely backend-parametric downstream proving surface
- `stwo-metal` does not widen its public API for the current downstream shape

Superseded by:

- none

### DEC-0118: G8 starts by pinning the downstream `stark-v` contract before choosing a substitution strategy

- Date: `2026-03-11`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0004-stark-v-hardening-input-and-contract.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0004-stark-v-hardening-input-and-contract.md)

Decision:

The first G8 slice should pin the real downstream `stark-v` contract and add a
deterministic local checker for it before choosing whether the first executable
hardening row is generic substitution, generated mapping, or explicit
fail-closed unsupported status.

Context:

With G6 and G7 complete, the next milestone is no longer about internal
benchmark or shim structure. It is about validating the frozen contract against
one real downstream Stwo consumer. `stark-v` is the best current candidate,
but its proving surface is SIMD-first and lives against its own vendored Stwo
snapshot. The safest first step is to pin and check the real downstream
contract shape rather than guessing the eventual integration mode.

Alternatives rejected:

- jump straight to a speculative `stwo-metal` substitution inside `stark-v`
  without pinning the downstream contract
- vendor the entire downstream repo before deciding what minimum hardening
  signal is actually required
- leave G8 as a purely documentary milestone without a deterministic local
  checker

Impact:

- G8 is now grounded in one real downstream input
- the next hardening decision will be made against the pinned contract rather
  than against memory or guesswork
- the public `stwo-metal` API stays unchanged during this first downstream
  slice

Superseded by:

- none

### DEC-0117: G7 is complete once the remaining compatibility bridges are fixture-owned rather than architecture-adjacent support crates

- Date: `2026-03-11`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0003-acceptance-bridge-law-and-ownership.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0003-acceptance-bridge-law-and-ownership.md)

Decision:

G7 is complete now that the remaining compatibility bridges live in one
fixture-owned shim crate under `fixtures/` rather than in private crates under
`support/`.

Context:

The purpose of G7 was not to pretend the framework-backed and SIMD-backed
bridges had become native Metal paths. It was to stop letting temporary
compatibility code sit next to the architecture surface. The new
`fixtures/stwo-metal-fixture-shims` crate keeps those adapters explicitly in
fixture ownership while preserving the same proving semantics and acceptance
coverage.

Alternatives rejected:

- leave the compatibility bridges in `support/` and still call them
  non-architectural
- move the bridges into the public `stwo-metal` API surface
- block G7 on retiring the deeper CPU-domain bridge logic, which is a
  different debt than fixture ownership

Impact:

- G7 is complete
- the remaining bridge debt is now about semantics and CPU-domain fallback, not
  about architecture ownership
- G8 becomes the active milestone

Superseded by:

- none

### DEC-0116: G6 is complete once the benchmark contract has separate lanes, separate artifacts, and one dual-lane report

- Date: `2026-03-11`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0002-generic-backend-and-codegen-contract.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0002-generic-backend-and-codegen-contract.md)

Decision:

G6 is complete now that the benchmark contract has explicit generated and
generic lanes, deterministic per-lane artifact generation, and one dual-lane
report surface that keeps the active optimization target explicit.

Context:

The earlier G6 slices added lane identity, a generated full-range sweep, a
bounded generic row, and a dual-lane report. The final missing piece was one
deterministic runner that produces both lanes and the combined report together.
That runner now exists, and the milestone exit condition is satisfied without
pretending the generic lane has the same execution envelope as the generated
lane.

Alternatives rejected:

- keep G6 open for more generated-lane optimization after the benchmark
  contract is already stable
- declare G6 complete without a single dual-lane runner
- force the generic lane into the same default sweep range as the generated
  lane despite the measured cost gap

Impact:

- G6 is complete
- G7 becomes the active milestone
- the generated lane remains the optimization target, but that is now a
  post-G6 performance decision rather than a benchmark-contract gap

Superseded by:

- none

### DEC-0115: Once both lanes exist, G6 optimization should target the generated lane and keep the generic lane bounded

- Date: `2026-03-11`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0002-generic-backend-and-codegen-contract.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0002-generic-backend-and-codegen-contract.md)

Decision:

After landing one dual-lane wide-fibonacci report, the active G6 optimization
target should be the `generated-metal` lane, while the `generic-metal` lane
remains a bounded correctness-and-coverage benchmark row.

Context:

The first dual-lane report now shows both benchmark regimes from live
artifacts. The generated lane supports the full `16..23` sweep and is within
range of SIMD at the low and mid logs, while the first generic row at
`log_size = 16` is still around `63.6 s`. That makes the generated lane the
only sensible near-term optimization target, but it does not remove the value
of the generic lane as a separate, explicit coverage row.

Alternatives rejected:

- keep optimizing both lanes together even though their current envelopes are
  radically different
- drop the generic lane entirely once it is shown to be much slower
- continue treating G6 as a reporting-only milestone after the dual-lane
  report already exists

Impact:

- G6 optimization is now explicitly generated-lane work
- the generic lane remains in the benchmark surface, but bounded
- the next honest work is high-log generated-lane optimization against the
  dual-lane report

Superseded by:

- none

### DEC-0114: G6 keeps the generic benchmark lane bounded until a full-range sweep is executable

- Date: `2026-03-11`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0002-generic-backend-and-codegen-contract.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0002-generic-backend-and-codegen-contract.md)

Decision:

The first `generic-metal` wide-fibonacci benchmark lane should be kept live but
bounded by default to the executable range, instead of pretending it can run
the full `log_n_instances = 16..23` sweep that the generated lane already
supports.

Context:

After `DEC-0113`, the next honest G6 step was a real generic row. That row now
exists through the upstream example-backed backend path, but the first measured
result at `log_size = 16` is roughly `63.6 s`, which makes an unconditional
full-range sweep impractical and misleading. The benchmark contract should
separate lanes honestly, not force both lanes into the same default sweep when
their current execution envelopes are radically different.

Alternatives rejected:

- keep the generic lane off entirely until it can sweep `16..23`
- keep the generic sweep default at `16..23` even though the first row already
  shows that range is not practical
- collapse the generic row back into the generated report instead of keeping
  the two lanes explicit

Impact:

- the generic lane now exists in the benchmark surface as a real measured row
- the generated lane remains the active optimization target
- the next honest G6 work is a dual-lane report surface that records both the
  generated full-range row and the bounded generic row together

Superseded by:

- none

### DEC-0113: The first G6 sweep must stabilize the lane-aware benchmark harness and record the generated-metal baseline before a generic row is added

- Date: `2026-03-11`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0002-generic-backend-and-codegen-contract.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0002-generic-backend-and-codegen-contract.md)

Decision:

The first executed G6 sweep should stabilize the lane-aware benchmark harness,
record the first generated-metal wide-fibonacci comparison table from live
artifacts, and then treat the missing generic row as the next milestone-owned
gap.

Context:

`DEC-0112` opened G6 by adding explicit benchmark-lane identity and a
deterministic wide-fibonacci sweep/report path. Running that path exposed two
practical issues that had to be fixed before the result was trustworthy:
`fixtures/standalone-pinned/Cargo.lock` had drifted enough to break the pinned
preflight, and `scripts/check_supported_benchmark_harness.sh` still assumed
older plan-only output and mandatory historical artifact JSONs. With those
repairs in place, the first generated-metal sweep now exists from live
artifacts and shows crossover ahead of SIMD at `log_size = 16` and `18`, near
parity at `17`, and a scaling deficit from `19` onward.

Alternatives rejected:

- treat the first generated-metal sweep as trustworthy without restoring the
  pinned and harness verification path
- begin the generic-lane tranche before recording the generated-metal baseline
- keep historical artifact JSON as a mandatory harness dependency even though
  the current lane-aware outputs are already sufficient to validate the active
  benchmark contract

Impact:

- the lane-aware benchmark harness is stable again
- the first generated-metal comparison table is recorded as the current G6
  baseline
- the next honest G6 work is to add a real `generic-metal` wide-fibonacci row
  through the same schema and reporting path

Superseded by:

- none

### DEC-0112: G6 benchmark separation starts with explicit lane identity and a deterministic wide-fibonacci comparison path

- Date: `2026-03-11`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0002-generic-backend-and-codegen-contract.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0002-generic-backend-and-codegen-contract.md)

Decision:

The first G6 slice should add explicit benchmark-lane identity to benchmark
JSON and land one deterministic wide-fibonacci sweep/report path, rather than
starting optimization work from ad hoc single-row measurements.

Context:

With G5 complete, the next milestone problem is no longer generated lowering.
It is separating benchmark rows so generic and generated results are not
blurred together. The existing standalone benchmark JSON had `classification`
and `dependency_row`, but those fields are too loose to serve as the stable
lane contract for later comparison tables or optimization decisions.

Alternatives rejected:

- keep using `classification` and `dependency_row` as the only lane markers
- begin optimization work before a deterministic sweep/report path exists
- build a new benchmark harness before adding the minimum stable metadata to
  the current one

Impact:

- standalone benchmark JSON now names the benchmark lane explicitly
- a deterministic wide-fibonacci sweep/report path exists for
  `log_n_instances = 16..23`
- the next honest G6 work is running that path on real artifacts and deciding
  how the future generic lane plugs into the same comparison surface

Superseded by:

- none

### DEC-0111: G5 is complete once live runtime staging law is generated-owned and benchmark work can separate lanes instead of shrinking surfaces

- Date: `2026-03-11`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0002-generic-backend-and-codegen-contract.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0002-generic-backend-and-codegen-contract.md)

Decision:

G5 is complete now that generated registrations and execution seeds own the
live runtime-planning and staging law for workload, benchmark, and bridge
paths, and the next milestone should be G6 benchmark-lane separation rather
than more surface-shrink work.

Context:

The G5 sequence started with checked generated registration inputs and widened
through runtime-planning inputs, shared execution bindings, execution seeds,
shared witness staging, quotient/FRI handoff checks, and prove-values staging.
The later G5 slices then removed transitional execution-authority surfaces and
lowered benchmark and acceptance lane checks into `stwo-metal` itself. The
remaining workload-side handoff translation is now unified through one shared
generated-seed mapping path, so the live runtime law is no longer split across
bridge-local or boundary-local fragments.

Alternatives rejected:

- keep G5 open for more public-surface cleanup after the generated execution
  law is already stable
- start G6 benchmark work without explicitly declaring G5 complete
- treat benchmark-lane reporting as incidental metadata instead of the next
  milestone-owned problem

Impact:

- G5 is complete
- G6 becomes the active milestone
- the next honest work is explicit benchmark-lane identity and a deterministic
  wide-fibonacci sweep/report path

Superseded by:

- none

### DEC-0110: Acceptance-lane Metal-capability law should live in `stwo-metal`, not in the private upstream bridge crate

- Date: `2026-03-11`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0002-generic-backend-and-codegen-contract.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0002-generic-backend-and-codegen-contract.md)

Decision:

The acceptance-lane Metal-capability check should be validated by
`MetalWorkloadBoundary` inside `stwo-metal`, and the private upstream bridge
crate should consume that validated lane instead of matching on workload plans
itself.

Context:

After `DEC-0109`, the benchmark prove-values bridge was no longer a policy
owner. The remaining analogous support-bridge rule was the upstream acceptance
lane check, which still matched directly on `MetalExecutionPlan`. That rule is
part of the workload declaration contract and should live with the workload
boundary, not in the private bridge crate.

Alternatives rejected:

- leave the acceptance-lane Metal-capability check in the private upstream
  bridge crate
- reintroduce a wider public execution-authority helper for acceptance only
- lower acceptance validation into example fixtures instead of the shared
  workload boundary

Impact:

- acceptance-lane validation now lives in `stwo-metal`
- the private upstream bridge is narrowed further to a consumer-only support
  layer
- the next honest G5 work is to lower the next workload-side staging or
  error-mapping rule that still duplicates generated seed law

Superseded by:

- none

### DEC-0109: Benchmark prove-values lane law should live in `stwo-metal`, not in the private bridge crate

- Date: `2026-03-11`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0002-generic-backend-and-codegen-contract.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0002-generic-backend-and-codegen-contract.md)

Decision:

The wide-fibonacci benchmark prove-values lane law should be validated by
`MetalWideFibonacciBenchmarkBoundary` from its generated execution seed, and
the private benchmark bridge should consume that validated result instead of
re-checking plan and stage ownership itself.

Context:

After the witness-shape law moved onto the generated execution seed in
`DEC-0108`, the next duplicated runtime rule was the benchmark prove-values
lane. The private support bridge was still checking whether the benchmark lane
was Metal-capable and whether witness/FRI ownership matched the intended
hybrid route. Those are route facts already implied by the generated execution
seed behind the benchmark declaration, so leaving them in the bridge would
make the bridge a second policy owner.

Alternatives rejected:

- keep the prove-values lane checks in the private bridge crate
- widen the public benchmark API with another richer execution-authority type
- move benchmark-specific target policy into the generated seed together with
  the shared prove-values route law

Impact:

- benchmark prove-values validation now lives in `stwo-metal`
- the private benchmark bridge is narrowed to a consumer-only support layer
- the next honest G5 work is to lower the remaining acceptance-lane and
  workload-side staging rules that still duplicate lower generated logic

Superseded by:

- none

### DEC-0108: Shared wide-fibonacci witness-shape law should live on the generated execution seed

- Date: `2026-03-11`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0002-generic-backend-and-codegen-contract.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0002-generic-backend-and-codegen-contract.md)

Decision:

The shared wide-fibonacci witness-shape law should live on the generated
execution seed rather than being duplicated across workload and benchmark
boundaries.

Context:

After the public workload-law cleanup, the next honest G5 target was a real
runtime-planning helper below the boundary surface. The workload and benchmark
paths were still each checking the same witness-shape facts: matching lengths,
power-of-two row count, and minimum column count. Those are generated-lane
facts tied to the registered witness hook, so they belong on the execution seed.

Alternatives rejected:

- keep witness-shape validation duplicated on both boundaries
- push benchmark-target-specific expected-length checks down into the generated
  seed even though those remain benchmark-lane policy
- add another boundary-local helper instead of using the generated execution
  seam already present

Impact:

- workload and benchmark boundaries now share one seed-owned witness-shape law
- the generated execution seed owns more of the runtime truth for the
  wide-fibonacci lane
- the next honest G5 task is to identify the next boundary-owned staging rule
  that still duplicates lower generated logic

Superseded by:

- none

### DEC-0107: The full stage-assignment slice should stay internal once workload law has narrowed to per-stage ownership

- Date: `2026-03-11`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0002-generic-backend-and-codegen-contract.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0002-generic-backend-and-codegen-contract.md)

Decision:

After retiring `MetalExecutionAuthority`, the public workload surface should
also drop `stage_assignments()` and the companion `MetalWorkloadStageAssignment`
export. The full assignment slice remains internal to generated inventory,
artifact registration, and execution-plan lowering.

Context:

The follow-up audit showed that no live public or fixture consumer needed the
full assignment slice once the workload boundary already exposed `plan()` and
`stage_ownership()`. Keeping the full slice public would preserve a broader
inspection surface than the current workload law actually requires.

Alternatives rejected:

- keep `stage_assignments()` public indefinitely
- remove `stage_ownership()` and force callers to scan the full slice
- delete the full assignment slice from internal planning layers as well

Impact:

- the workload-facing public law is now just `plan()` and `stage_ownership()`
- internal generated and execution-plan layers still keep the richer assignment
  data they need for lowering
- the next honest G5 work is to return from surface cleanup to lower generated
  runtime planning further

Superseded by:

- none

### DEC-0106: The transitional `MetalExecutionAuthority` object should be retired once workload law is available directly

- Date: `2026-03-11`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0002-generic-backend-and-codegen-contract.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0002-generic-backend-and-codegen-contract.md)

Decision:

Once benchmark and support paths no longer require a separate authority object,
`MetalExecutionAuthority` should be removed and execution law should live
directly on `MetalWorkloadBoundary` as `plan()`, `stage_assignments()`, and
`stage_ownership()`.

Context:

By the end of the prior slice, the authority object had no live semantic role
left beyond packaging values the workload boundary already exposed or could
expose directly. Keeping it would preserve a transitional wrapper with no
unique law. The semantics-preserving cleanup was to retire the object and move
tests to the narrower workload methods.

Alternatives rejected:

- keep `MetalExecutionAuthority` as a permanent companion type
- remove stage law from the workload boundary entirely
- shrink `stage_assignments()` before removing the redundant authority object

Impact:

- `MetalExecutionAuthority` is gone from the companion surface
- workload and benchmark paths now pin the narrower workload-law methods
- the next honest G5 task is deciding whether `stage_assignments()` still needs
  to remain public or whether `stage_ownership()` is the stable semantic unit

Superseded by:

- none

### DEC-0105: The benchmark boundary should not duplicate `MetalExecutionAuthority` once workload law is already available beneath it

- Date: `2026-03-11`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0002-generic-backend-and-codegen-contract.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0002-generic-backend-and-codegen-contract.md)

Decision:

`MetalWideFibonacciBenchmarkBoundary` should not expose its own
`execution_authority()` pass-through once it already exposes the underlying
`MetalWorkloadBoundary` and that workload boundary owns the execution-law
surface.

Context:

After the support-crate cleanup, the remaining direct authority consumers were
public workload and benchmark surfaces. The benchmark boundary carried no extra
execution-law information; it only forwarded to `workload_boundary()`. Keeping
that second path would preserve duplicate public surface area with no semantic
benefit.

Alternatives rejected:

- keep the benchmark-boundary pass-through indefinitely
- remove workload-boundary access and force all benchmark users onto a special
  benchmark-specific authority shape
- shrink the workload-boundary law first while the benchmark duplicate still
  exists

Impact:

- benchmark callers now go through `workload_boundary()` if they need the
  transitional authority type
- the remaining live public owner of `execution_authority()` is now
  `MetalWorkloadBoundary`
- the next honest G5 work is deciding whether that workload-boundary authority
  method can also disappear in favor of the narrower workload-law methods

Superseded by:

- none

### DEC-0104: Private support crates should validate directly from workload and benchmark boundaries once the root authority export is gone

- Date: `2026-03-11`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0002-generic-backend-and-codegen-contract.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0002-generic-backend-and-codegen-contract.md)

Decision:

After the root-level `MetalExecutionAuthority` export is removed, the private
benchmark and acceptance support crates should stop importing or validating via
that type and should instead validate directly from the workload and benchmark
boundaries they already receive.

Context:

Leaving the support crates on `MetalExecutionAuthority` after the root export
shrink would keep the next G5 decision muddy: it would look as if the
remaining consumer set still included private bridge validation, when in fact
the only durable question left is what the workload and benchmark public law
should expose. The clean next step was therefore to move those private crates
onto their boundary inputs directly.

Alternatives rejected:

- keep private support-crate validation on `MetalExecutionAuthority`
- add a second private replacement type just for support-crate validation
- postpone the support-crate cleanup until after changing the public workload
  or benchmark API

Impact:

- private support crates no longer depend on `MetalExecutionAuthority`
- the remaining direct consumers are now the workload/benchmark API and the
  workload-scoped companion surface
- the next honest G5 task is deciding whether one of those remaining public
  consumers can shrink without obscuring workload-stage law

Superseded by:

- none

### DEC-0103: `MetalExecutionAuthority` should live on the workload-facing companion surface, not the root companion export

- Date: `2026-03-11`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0002-generic-backend-and-codegen-contract.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0002-generic-backend-and-codegen-contract.md)

Decision:

After fixture edges and private support bridges stopped consuming
`MetalExecutionAuthority` directly from the root companion surface, the
root-level reexport should be removed. The type remains available through the
workload-facing module and through workload and benchmark boundary methods.

Context:

By the end of the prior G5 slice, `MetalExecutionAuthority` no longer carried
live fixture traffic and no longer needed to sit in the root companion export
set just to support private bridge construction. Keeping it at the root would
leave a redundant public path for a type whose semantics are workload-stage law.
The safest shrink was therefore to keep the type reachable where it is
semantically owned, but remove the duplicate root export.

Alternatives rejected:

- keep the redundant root-level export indefinitely
- remove the type from the workload-facing module as well
- shrink the workload boundary methods before removing the duplicate root path

Impact:

- `MetalExecutionAuthority` is now workload-scoped on the companion surface
- private support crates compile against that scoped path instead of the root
- the next honest G5 work is enumerating the remaining direct authority
  consumers and deciding which one can move lower or disappear next

Superseded by:

- none

### DEC-0102: Fixture edges should stop carrying `MetalExecutionAuthority` before the public authority surface shrinks

- Date: `2026-03-11`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0002-generic-backend-and-codegen-contract.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0002-generic-backend-and-codegen-contract.md)

Decision:

Once the benchmark and acceptance support bridges both had lower private lane
contracts, fixture edges should stop constructing those lanes from
`MetalExecutionAuthority` directly. After that edge cleanup, any transitional
planning helpers that no longer have live callers should be deleted rather than
preserved.

Context:

The earlier G5 slices moved the live benchmark prove-values bridge and the live
acceptance bridge below direct public-authority use, but the fixture edges were
still constructing those private lanes manually. That kept `MetalExecutionAuthority`
alive in places where the bridge boundary itself was already sufficient. The
cleanest next step was to restore boundary-based entry at the edges, then let
the compiler identify which transitional planning helpers had become dead.

Alternatives rejected:

- keep fixture-edge lane construction on `MetalExecutionAuthority`
- preserve the now-unused planning helpers behind `allow(dead_code)`
- try to shrink the public `MetalExecutionAuthority` surface before removing
  its no-longer-needed edge uses

Impact:

- fixture edges now enter private support bridges through boundary-based
  constructors only
- dead transitional planning helpers have been removed, reducing the internal
  planning seam to what live code still uses
- the next honest G5 work is to enumerate the remaining direct
  `MetalExecutionAuthority` consumers and decide whether the public companion
  surface can shrink safely

Superseded by:

- none

### DEC-0101: The upstream acceptance lane should be the next support-bridge path lowered below direct authority use

- Date: `2026-03-10`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0002-generic-backend-and-codegen-contract.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0002-generic-backend-and-codegen-contract.md)

Decision:

After lowering the benchmark prove-values bridge, the upstream acceptance lane
should be the next support-bridge path to move below direct
`MetalExecutionAuthority` use.

Context:

Once the benchmark prove-values bridge used a workspace-private validated lane
contract, the upstream acceptance lane became the clearest remaining
support-bridge consumer still building its live path directly from the public
authority surface. Lowering it next keeps the G5 progression linear: first
in-crate helper below the public surface, then benchmark support bridge, then
acceptance support bridge.

Alternatives rejected:

- leave the acceptance bridge directly on `MetalExecutionAuthority`
- switch immediately to a different benchmark-local helper
- treat the acceptance fixture edge as the final place where direct authority
  use is acceptable

Impact:

- both major private support-bridge paths now validate their own lower lane
  contracts
- the next honest G5 task is identifying the remaining direct public-authority
  consumers and deciding whether the transitional public surface can shrink

Superseded by:

- none

### DEC-0100: The first support-bridge path lowered below `MetalExecutionAuthority` should be the benchmark prove-values bridge

- Date: `2026-03-10`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0002-generic-backend-and-codegen-contract.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0002-generic-backend-and-codegen-contract.md)

Decision:

The first private support-bridge path to move below `MetalExecutionAuthority`
should be the benchmark prove-values bridge, using a workspace-private
validated lane contract.

Context:

After the first in-crate live helper moved below the public authority, the next
honest G5 target was a private support-bridge path. The benchmark prove-values
bridge was the safest candidate because it already needed only a narrow subset
of execution law and already lived in a private support crate. Lowering it
first proves that support-bridge contracts can shrink without widening the
public `stwo-metal` API or disrupting the bridge-law ownership model.

Alternatives rejected:

- lower the upstream acceptance lane first
- keep the benchmark prove-values bridge directly on `MetalExecutionAuthority`
- widen the public API with a benchmark-specific lane contract

Impact:

- one support-bridge path now stages from a lower workspace-private contract
- the next honest G5 question is whether the upstream acceptance lane is the
  next remaining public-authority consumer worth lowering

Superseded by:

- none

### DEC-0099: The first live path below `MetalExecutionAuthority` should be an in-crate helper before any support-bridge lowering

- Date: `2026-03-10`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0002-generic-backend-and-codegen-contract.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0002-generic-backend-and-codegen-contract.md)

Decision:

Before moving any private support-bridge path below `MetalExecutionAuthority`,
the project should first lower one in-crate live helper directly onto the
private registered execution seed.

Context:

`MetalExecutionAuthority` was reused successfully across benchmark,
acceptance, and workload lanes, but that still left every live path above the
same transitional public surface. The next semantics-preserving move was to
prove that one live helper could operate directly on the lower private
generated contract. Wide-fibonacci witness eligibility and hybrid-FRI lane
support were the safest candidates because they already lived inside
`stwo-metal` and required only plan and stage-law checks.

Alternatives rejected:

- move a support-bridge path below the public authority first
- keep every live path on `MetalExecutionAuthority`
- jump straight from public authority to a larger private lowering contract

Impact:

- the first live helper path now sits below `MetalExecutionAuthority`
- the next honest G5 choice is which private support-bridge path lowers next

Superseded by:

- none

### DEC-0098: Workload declarations and witness staging must obey the same reduced execution-law surface once benchmark and acceptance lanes do

- Date: `2026-03-10`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0002-generic-backend-and-codegen-contract.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0002-generic-backend-and-codegen-contract.md)

Decision:

After `MetalExecutionAuthority` is reused in benchmark and acceptance lanes,
the remaining workload-side helper checks that only need plan and stage law
must also move onto that same reduced contract before G5 lowers further.

Context:

The thirteenth G5 slice proved that benchmark prove-values, acceptance lane
validation, and benchmark witness staging could all share the same reduced
execution-law surface. Leaving workload declarations and witness staging on
`MetalWorkloadBoundary` after that would reintroduce a second default law
owner for execution capability checks.

Alternatives rejected:

- keep workload declaration checks on `MetalWorkloadBoundary`
- let witness staging be the one remaining place where workload boundaries own
  execution law
- jump below the public authority before workload code is aligned with it

Impact:

- workload-side hybrid-FRI declaration and witness staging now share the same
  reduced execution-law surface as benchmark and acceptance lanes
- the next honest G5 slice is choosing the first live helper that moves below
  the transitional public authority onto the next lower private contract

Superseded by:

- none

### DEC-0097: The reduced execution-law surface must be shared across benchmark and acceptance lanes before further lowering

- Date: `2026-03-10`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0002-generic-backend-and-codegen-contract.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0002-generic-backend-and-codegen-contract.md)

Decision:

Once `MetalExecutionAuthority` exists, the next semantics-preserving step is
to reuse it across more than one live staging lane before attempting to lower
the contract further.

Context:

The twelfth G5 slice created `MetalExecutionAuthority` and moved the shared
prove-values bridge onto it. Leaving that surface as a one-off benchmark-only
bridge would not prove that it is the right lower contract. Reusing it in the
private upstream-acceptance bridge and the benchmark witness boundary
demonstrates that the reduced execution-law surface is genuinely reusable
without exposing the full internal execution seed.

Alternatives rejected:

- keep `MetalExecutionAuthority` as a benchmark-only prove-values detail
- jump immediately to a lower private contract without validating reuse
- keep new live helpers on `MetalWorkloadBoundary`

Impact:

- benchmark prove-values, acceptance lane validation, and benchmark witness
  staging now share the same reduced execution-law surface
- the next honest G5 slice is lowering the remaining workload-side helper
  checks that only need plan and stage law

Superseded by:

- none

### DEC-0096: Live staging bridges may depend on a minimal execution-law surface before the lower private contract exists

- Date: `2026-03-10`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0002-generic-backend-and-codegen-contract.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0002-generic-backend-and-codegen-contract.md)

Decision:

When a live staging bridge still needs runtime plan and stage-law truth but
must stop depending on `MetalWorkloadBoundary`, it may temporarily consume a
minimal `MetalExecutionAuthority` surface that exposes only execution plan and
stage ownership.

Context:

The eleventh G5 slice removed benchmark-boundary ownership from the shared
prove-values staging bridge, but the bridge still depended on the broader
public workload boundary. The next semantics-preserving step is to cut that
dependency down to the smallest stable law the bridge actually needs while the
lower private generated execution contract is still being assembled.

Alternatives rejected:

- keep the bridge anchored on `MetalWorkloadBoundary`
- expose the full internal execution seed directly
- delay lowering until the full private execution contract already exists

Impact:

- live prove-values staging now depends only on plan and stage-law truth
- the next honest G5 slice is reusing that reduced execution-law surface in
  another live staging helper or lowering it further once the private contract
  is ready

Superseded by:

- none

### DEC-0095: The shared prove-values bridge must stop at workload boundary only temporarily

- Date: `2026-03-10`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0002-generic-backend-and-codegen-contract.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0002-generic-backend-and-codegen-contract.md)

Decision:

Re-anchoring the shared prove-values bridge from benchmark boundary to
`MetalWorkloadBoundary` is the right intermediate step, but it is not the end
state. The next slice must move that bridge below the public workload surface
onto the lower generated execution contract.

Context:

The tenth G5 slice moved prove-values staging into a private shared support
crate. The next safest semantics-preserving step was to remove the benchmark
boundary dependency and consume the lower workload boundary instead. That keeps
the benchmark row as a caller only, but the bridge still depends on a public
surface that sits above the canonical generated seed/binding seam.

Alternatives rejected:

- leave the shared prove-values bridge anchored on the benchmark boundary
- treat `MetalWorkloadBoundary` as the final prove-values staging authority
- widen the public `stwo-metal` API immediately with a deeper prove-values
  staging contract

Impact:

- prove-values staging is no longer benchmark-boundary owned
- the next honest G5 slice is lowering the bridge beneath the public workload
  surface onto the generated execution contract

Superseded by:

- none

### DEC-0094: The first shared PCS/prove-values bridge must move out of the fixture before it moves down

- Date: `2026-03-10`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0002-generic-backend-and-codegen-contract.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0002-generic-backend-and-codegen-contract.md)

Decision:

Once the first prove-values staging helper exists in a live benchmark row, the
next step is to move it out of the fixture into a private shared support
boundary before re-anchoring it further down onto the generated execution
contract.

Context:

The ninth G5 slice allowed the wide-fibonacci benchmark to prove and verify
through one benchmark-backed prove-values staging helper. That removed the
local recomposition inside the fixture, but the helper still lived in the
benchmark binary. Moving it into a private shared support crate keeps the main
`stwo-metal` API minimal while making the next downward refactor possible.

Alternatives rejected:

- leave the prove-values staging helper inside the benchmark fixture
- widen the public `stwo-metal` API with a benchmark-specific prove-values
  staging contract
- jump directly to lower generated bindings while the helper still belongs to a
  fixture

Impact:

- the first shared PCS/prove-values staging bridge now exists outside the
  benchmark fixture
- the next honest G5 slice is re-anchoring that bridge on the lower generated
  execution contract

Superseded by:

- none

### DEC-0093: The first prove-values staging helper may land in the benchmark lane, but the next step must lift it out

- Date: `2026-03-10`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0002-generic-backend-and-codegen-contract.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0002-generic-backend-and-codegen-contract.md)

Decision:

The first live prove-values staging helper may land in the wide-fibonacci
benchmark lane so long as it consumes the generated benchmark boundary and the
next tranche lifts that helper into a shared PCS-facing lowering boundary.

Context:

After execution seeds took over witness, evaluation, and quotient staging, the
next honest runtime seam was prove-values. The only live path that could
consume the generated contract immediately was the wide-fibonacci benchmark
fixture, because generic vendored PCS code does not depend on `stwo-metal`.
Landing the helper there is acceptable as a transitional G5 step, but leaving
it there would let fixture-owned staging become architectural again.

Alternatives rejected:

- delay prove-values staging work until a fully shared PCS boundary exists
- keep recomposing prove-values staging entirely inside the benchmark prove
  path
- treat the benchmark-local helper as a permanent endpoint

Impact:

- one live benchmark row now uses the generated boundary for trace generation
  and prove-values staging
- the next honest G5 slice is lifting that helper into a shared PCS-facing
  lowering boundary

Superseded by:

- none

### DEC-0092: Execution seeds must own workload staging truth before PCS staging grows

- Date: `2026-03-10`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0002-generic-backend-and-codegen-contract.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0002-generic-backend-and-codegen-contract.md)

Decision:

Before G5 widens into prove-values or PCS staging, the execution seed must own
the existing workload staging laws for FRI-ready and quotient-evaluation
handoff, and workload boundaries must stop carrying a parallel copy of the same
runtime plan and stage metadata.

Context:

The seventh G5 slice proved that the execution seed could drive one live
runtime helper for witness generation. The next immediate risk was allowing
evaluation and quotient staging to keep their own local capability logic,
which would recreate the split metadata problem at the next layer up. Folding
those checks and the duplicated workload boundary state into the seed keeps the
lowering path linear before PCS staging begins.

Alternatives rejected:

- let evaluation and quotient staging keep their own local plan and ownership
  checks
- start prove-values staging while workload boundaries still mirror execution
  metadata locally
- introduce a second staging-only metadata struct beside the execution seed

Impact:

- witness, evaluation, and quotient staging now share one generated runtime
  authority
- workload boundaries treat the execution seed as the runtime source of truth
- the next honest G5 slice is the first PCS prove-values staging helper

Superseded by:

- none

### DEC-0091: The first live execution-seed helper must be shared before G5 widens into later staging

- Date: `2026-03-10`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0002-generic-backend-and-codegen-contract.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0002-generic-backend-and-codegen-contract.md)

Decision:

Once the scheduling-oriented execution seed reaches one live non-declarative
helper, that helper must be shared across equivalent runtime paths before G5
widening continues into quotient or evaluation staging.

Context:

The sixth G5 slice produced one canonical execution seed and the next slice
consumed it in live witness generation. Without one shared seed-owned runtime
helper, workload and benchmark code would immediately drift back into
duplicated request construction. Normalizing that helper first keeps the next
staging work linear.

Alternatives rejected:

- let workload and benchmark witness generation keep separate seed-consuming
  request builders
- skip directly to quotient or evaluation staging while the first runtime
  helper remains duplicated
- widen runtime scheduling by adding another metadata wrapper beside the seed

Impact:

- workload and benchmark witness generation now share one checked seed-owned
  trace helper
- the next honest G5 slice is widening the same seed into quotient or
  evaluation staging

Superseded by:

- none

### DEC-0090: The first scheduling-oriented execution seed must be consumed before deeper runtime scheduling widens

- Date: `2026-03-10`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0002-generic-backend-and-codegen-contract.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0002-generic-backend-and-codegen-contract.md)

Decision:

Once a reusable generated execution binding exists, the next runtime-facing
step is a scheduling-oriented execution seed. That seed must be consumed by a
real non-declarative execution helper before the project adds deeper scheduling
policy.

Context:

The binding helper normalized workload and benchmark declaration, but runtime
scheduling still had no canonical seed. Deriving the seed first keeps runtime
policy narrow and measurable. Consuming it next prevents the seed from
becoming passive structure.

Alternatives rejected:

- jump directly to broader runtime scheduling without a canonical seed
- keep scheduling-local metadata separate from the shared binding
- treat the scheduling seed as documentation instead of live code

Impact:

- G5 now has a canonical seed for later scheduling work
- the next honest slice is the first non-declarative execution helper on top
  of that seed

Superseded by:

- none

### DEC-0089: Workload and benchmark lowering must converge on one reusable execution-binding helper before scheduling widens

- Date: `2026-03-10`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0002-generic-backend-and-codegen-contract.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0002-generic-backend-and-codegen-contract.md)

Decision:

After workload-boundary and benchmark-boundary declaration are normalized, the
next shared abstraction is one reusable execution-binding helper. Runtime
scheduling must grow from that helper instead of from parallel boundary shapes.

Context:

The previous G5 slices normalized prove planning, workload boundaries, and
benchmark declarations separately. Without one shared binding helper, the next
runtime-facing work would risk reintroducing duplicated route and inventory
handling under a different name. The binding helper now closes that gap.

Alternatives rejected:

- keep workload and benchmark lowering on separate boundary helper types
- start runtime scheduling before the shared binding exists
- introduce a second scheduling-local metadata surface

Impact:

- G5 now has one reusable generated binding shared across declaration paths
- the next honest work is the first scheduling-oriented helper on top of that
  binding

Superseded by:

- none

### DEC-0088: Benchmark declaration must consume the shared generated boundary input instead of composing route state independently

- Date: `2026-03-10`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0002-generic-backend-and-codegen-contract.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0002-generic-backend-and-codegen-contract.md)

Decision:

Once workload-boundary declaration consumes a shared generated boundary input,
benchmark declaration must consume a sibling shared benchmark-boundary input
from the same seam instead of mixing direct registration lookups with a
separately composed workload boundary.

Context:

After the third G5 slice, workload-boundary declaration was normalized, but
benchmark declaration still sat one level above it and risked drifting back
into hybrid local composition. Normalizing benchmark declaration next keeps the
generated lowering path linear and points the next honest work toward reusable
execution bindings.

Alternatives rejected:

- leave benchmark declaration as a mixed direct-registration and boundary call
- skip straight to deeper scheduling work before normalizing benchmark
  declaration
- add a benchmark-only metadata table outside the shared seam

Impact:

- workload and benchmark declaration now both consume shared generated boundary
  inputs
- the next honest G5 step is a reusable execution-binding helper for runtime
  scheduling

Superseded by:

- none

### DEC-0087: Workload-boundary declaration must consume one shared lowering input before benchmark lowering widens

- Date: `2026-03-10`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0002-generic-backend-and-codegen-contract.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0002-generic-backend-and-codegen-contract.md)

Decision:

The first broader lowering-oriented execution helper beyond prove-plan
selection is workload-boundary declaration, and it must consume one shared
workload-boundary lowering input derived from the canonical runtime-planning
and lowering inputs.

Context:

After prove-plan selection consumed the canonical runtime-planning input, the
next risk was that workload-boundary declaration would still reconstruct route
state locally. Using one shared boundary input keeps G5 linear and makes the
next honest seam clearly the benchmark declaration path above it.

Alternatives rejected:

- let workload-boundary declaration keep composing registrations directly
- jump straight to benchmark lowering without normalizing workload-boundary
  declaration first
- invent a second workload-local lowering table

Impact:

- G5 now has one broader execution helper consuming the shared generated seam
- the next honest execution-lowering work moves to benchmark declaration

Superseded by:

- none

### DEC-0086: Prove-plan selection must consume the canonical runtime-planning input

- Date: `2026-03-10`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0002-generic-backend-and-codegen-contract.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0002-generic-backend-and-codegen-contract.md)

Decision:

Once the canonical runtime-planning input exists, prove-plan selection must use
that input rather than jumping directly from checked registrations to raw
component plan inputs.

Context:

The first G5 slice created a lowering-facing registration input, but that would
still have been dead structure if the prove planner kept sidestepping it. The
smallest semantics-preserving next step is to consume that input in one real
runtime-planning helper before widening lowering any further.

Alternatives rejected:

- leave the runtime-planning input unused until later G5 slices
- keep prove-plan selection on raw `MetalComponentArtifact::as_plan_input`
- widen lowering first and normalize planning inputs later

Impact:

- G5 now has one canonical runtime-planning unit in live use
- the next honest lowering work moves beyond prove-plan selection into broader
  execution helpers

Superseded by:

- none

### DEC-0085: Runtime-facing lowering helpers must consume one canonical lowering input

- Date: `2026-03-10`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0002-generic-backend-and-codegen-contract.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0002-generic-backend-and-codegen-contract.md)

Decision:

Once a checked lowering-facing generated registration input exists, the first
runtime-facing lowering helpers must consume that input directly. They may not
reassemble ABI symbols, specialization keys, or build inventory from separate
queries.

Context:

G5 starts by turning the checked generated registration into one stable
lowering input. The next failure mode would be to leave that input unused while
runtime-facing helpers continued to pluck separate fields from artifacts or
manifests. This decision keeps the lowering contract linear and law-abiding.

Alternatives rejected:

- treat the lowering input as optional documentation only
- let each lowering helper reconstruct its own metadata view
- delay canonical lowering-input consumption until deeper runtime work already
  exists

Impact:

- the next G5 slice is constrained to consume one canonical lowering input
- generated lowering remains attached to the same stable registry seam from
  declaration through runtime planning

Superseded by:

- none

### DEC-0084: Generated lowering must start from one checked registration object rather than raw manifest fragments

- Date: `2026-03-10`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0002-generic-backend-and-codegen-contract.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0002-generic-backend-and-codegen-contract.md)

Decision:

The first lowering-oriented G5 work must start from one checked generated
registration object derived from the shared artifact registry. Lowering code
may not reach back into raw manifest fragments or reconstruct inventory fields
ad hoc.

Context:

By the end of G4, the registry carried explicit ABI symbols, build inventory,
witness hooks, and specialization keys, and declarations already consumed that
richer inventory. The next semantic risk was that lowering work would bypass
that contract and start reading raw manifest state again. Introducing one
checked registration object keeps generated lowering attached to the same seam.

Alternatives rejected:

- let lowering code read raw manifest rows directly
- keep artifact, route, and inventory as separate loose values in lowering
- delay the registration object until after G5 lowering had already widened

Impact:

- G4 now closes on a stable generated registration surface
- G5 starts from one checked lowering-facing unit instead of manifest
  reassembly

Superseded by:

- none

### DEC-0083: Workload and benchmark declarations must consume generated ABI and specialization inventory through the shared registry seam

- Date: `2026-03-10`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0002-generic-backend-and-codegen-contract.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0002-generic-backend-and-codegen-contract.md)

Decision:

Once generated registration records ABI symbols and specialization keys, the
declaration layer must consume that richer inventory through the shared
artifact-registry seam. Workload and benchmark declarations may not drop back
to route-only reasoning or introduce a second metadata lookup surface.

Context:

The first G4 slice made the internal registry richer, but the contract would
still have been fragile if declarations continued to look only at route
eligibility and workload family. The next semantics-preserving step is to keep
declarations attached to the richer inventory so later lowering slices inherit
one active contract instead of passive manifest fields.

Alternatives rejected:

- leave the richer inventory unused until later lowering work
- let workload and benchmark declarations read raw manifest details directly
- create a second generated metadata table just for declarations

Impact:

- declaration code now exercises the richer generated registration contract
- the next honest G4 work moves toward lowering-facing registration objects
  instead of more declaration-side metadata drift

Superseded by:

- none

### DEC-0082: Generated registration must carry explicit ABI symbols and specialization keys, not just route eligibility

- Date: `2026-03-10`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0002-generic-backend-and-codegen-contract.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0002-generic-backend-and-codegen-contract.md)

Decision:

The stable internal generated-artifact registry must record explicit ABI
symbols and specialization keys for each registered component in addition to
route eligibility, ABI family, build modules, and witness hooks.

Context:

G2 already established a truthful fail-closed artifact registry and execution
plan seam, but the registered inventory was still too thin. It could answer
whether a generated route existed, yet it could not state which ABI surface or
specialization dimensions a later lowering layer would rely on. G4 starts by
making that richer inventory explicit before generated lowering grows around
it.

Alternatives rejected:

- keep route eligibility as the only generated registration contract
- infer ABI symbols and specialization dimensions heuristically during lowering
- create a second generated metadata table outside the registry seam

Impact:

- generated registration is now rich enough to support later lowering work
  without inventing side metadata channels
- workload and benchmark declarations can next consume one richer inventory
  surface instead of coupling directly to manifest details

Superseded by:

- none

### DEC-0081: The registered acceptance bridge catalog may move into a private shared support crate while remaining non-public

- Date: `2026-03-10`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0003-acceptance-bridge-law-and-ownership.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0003-acceptance-bridge-law-and-ownership.md)

Decision:

The current registered acceptance bridge catalog and its framework-backed and
SIMD-backed adapter implementations may live in a private shared support crate
outside the main Cargo workspace as long as they preserve the non-public laws
from `DN-0003`.

Context:

The acceptance harness had already collapsed ad hoc bridge construction behind
one checked registered catalog, and the bridge laws were frozen. The remaining
G3 ownership gap was that the implementations still lived inside the acceptance
 harness crate itself. Moving them into a private shared bridge first, and
 later into `fixtures/stwo-metal-fixture-shims`
creates a durable shared internal home without widening the public `stwo-metal`
API and without reintroducing the vendored nested-workspace conflict that
blocked a direct move into the main crate.

Alternatives rejected:

- leave the bridge permanently in the acceptance harness
- expose the bridge as a new public `stwo-metal` contract before reuse is
  proven
- block G3 on an upstream-facing move that the current repository shape does
  not yet support cleanly

Impact:

- G3 can close honestly because the bridge is no longer acceptance-harness
  local
- the bridge remains non-public and explicitly temporary
- the next active work moves to G4 generated registration and ABI inventory

Superseded by:

- none

### DEC-0080: The acceptance bridge catalog is governed by a non-public law before any durable ownership move

- Date: `2026-03-10`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0003-acceptance-bridge-law-and-ownership.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0003-acceptance-bridge-law-and-ownership.md)

Decision:

The current acceptance bridge catalog is now governed by an explicit non-public
law. Any future move into a shared internal boundary or an upstream-facing
surface must preserve:

- registration-first construction
- upstream-owned workload semantics
- explicit CPU fallback naming
- fail-closed lane construction
- no public API inflation before reuse is proven

Context:

After the third G3 slice, the acceptance harness no longer used a loose set of
bridge helpers; it had one checked catalog. The next honest risk was that the
catalog itself could drift into an undocumented architecture surface while the
team debated whether the durable home belongs inside this repository or
upstream. Freezing the law first keeps that ownership decision bounded.

Alternatives rejected:

- move the bridge again before the contract is written down
- leave the catalog as undocumented acceptance-only glue
- promote the bridge surface to a public API immediately

Impact:

- the bridge catalog now has explicit laws and non-goals
- the next implementation work can focus on durable ownership instead of
  rediscovering the contract

Superseded by:

- none

### DEC-0079: Vendored Stwo compatibility patches may modernize stale nightly chunking idioms to preserve the pinned verification contract

- Date: `2026-03-10`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0002-generic-backend-and-codegen-contract.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0002-generic-backend-and-codegen-contract.md)

Decision:

The repository may patch the vendored Stwo snapshot in small semantics-
preserving steps when the pinned local toolchain contract drifts out of
compatibility. For this tranche, the accepted scope was limited to replacing
stale `array_chunks`-based chunking idioms so the existing pinned nightly could
compile the vendored snapshot and restore deterministic G3 verification.

Context:

After the acceptance-bridge catalog landed, full cargo verification was still
failing before the local changes ran because the vendored Stwo snapshot relied
on an older nightly chunking surface than `nightly-2025-07-14` now provides.
The repository already had a pinned nightly contract, so the smallest
correctness-preserving step was to modernize the stale vendored chunking code
instead of inventing a second local verification story or letting G3 proceed
without deterministic tests.

Alternatives rejected:

- leave G3 verification blocked and keep documenting the toolchain drift as
  passive debt
- widen the acceptance tranche without restoring cargo verification first
- make larger vendored refactors than required just to satisfy the current
  pinned toolchain

Impact:

- the pinned nightly contract is working again for the current acceptance and
  planning test surfaces
- the next honest G3 work returns to bridge law and ownership design instead of
  more toolchain triage

Superseded by:

- none

### DEC-0078: Acceptance-local bridge construction must collapse behind one checked catalog before any shared-boundary move

- Date: `2026-03-10`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0002-generic-backend-and-codegen-contract.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0002-generic-backend-and-codegen-contract.md)

Decision:

Before trying again to move acceptance bridges into `stwo-metal` or another
shared home, the acceptance harness must first collapse them behind one checked
registered-lane catalog. Framework-backed and SIMD-backed rows now construct
their local adapters only through that catalog.

Context:

After the second G3 slice, every active acceptance row already required a
registered Metal workload lane, but the bridge surface was still a set of free
constructors. An attempted crate-owned move into `stwo-metal` hit a real
nested-workspace conflict when adding vendored `stwo-constraint-framework` to
the main crate. The smallest semantics-preserving step is therefore to tighten
the acceptance-local surface first, keep the CPU-domain framework bridge
explicit, and postpone the durable ownership move until that repository-shape
constraint is addressed cleanly.

Alternatives rejected:

- keep widening a free-function acceptance bridge surface even after registered
  lanes exist
- claim that the current repository can safely host a shared framework bridge
  despite the nested-workspace conflict
- block all G3 progress until the durable shared bridge home is solved

Impact:

- acceptance rows now share one checked bridge-catalog contract
- the next honest G3 work is bridge law/ownership definition, not more lane
  registration
- the vendored workspace conflict remains explicit debt rather than hidden
  coupling

Superseded by:

- none

### DEC-0077: Acceptance workloads already in the matrix must register shared Metal lanes before building local bridges

- Date: `2026-03-10`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0002-generic-backend-and-codegen-contract.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0002-generic-backend-and-codegen-contract.md)

Decision:

The current acceptance workloads that already prove through `MetalBackend`
must no longer construct bridges from entirely local context. `wide_fibonacci`,
`state_machine`, `blake`, and `xor` now declare registered Metal workload
lanes first and only then build their remaining local adapters.

Context:

The first G3 slice proved the shape with `wide_fibonacci`: acceptance bridges
should consume the stable planning seam instead of floating outside it. The
next honest step was to widen that rule across the existing acceptance matrix
without claiming the local bridges were fully retired. Registering those lanes
keeps the architecture honest while leaving the still-local adapters explicit
in debt tracking.

Alternatives rejected:

- keep the other acceptance rows entirely local until one big adapter-rewrite
- add more unregistered acceptance bridges and defer lane registration
- claim the shared seam is the default acceptance path without widening it

Impact:

- multiple acceptance rows now consume the shared lane contract before local
  bridge construction
- the next honest G3 gap is retiring or generalizing the remaining local
  adapters themselves, not lane registration

Superseded by:

- none

### DEC-0076: The first G3 acceptance bridge must require a registered Metal workload lane

- Date: `2026-03-10`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0002-generic-backend-and-codegen-contract.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0002-generic-backend-and-codegen-contract.md)

Decision:

The first G3 acceptance slice does not try to retire every local adapter at
once. Instead, it requires the `wide_fibonacci` framework-component bridge to
be constructed only from a registered Metal workload boundary and a checked
acceptance lane.

Context:

G2 is now complete: the private artifact-registry, generated-route, policy, and
execution-plan seam exists and is adopted by planner, workload, and benchmark
declarations. The next honest architectural step is to make acceptance
integration consume that seam instead of leaving the first example bridge fully
local. `wide_fibonacci` is the safest first row because it already has a
declared workload boundary in the shared backend.

Alternatives rejected:

- keep all acceptance bridges entirely local until every example can move at
  once
- expose the acceptance lane as a new public `stwo-metal` API immediately
- treat the existing acceptance wrappers as “good enough” and skip G3

Impact:

- G2 is complete and G3 is now in progress
- the first acceptance row is tied to the stable workload-boundary contract
- the remaining framework and SIMD acceptance bridges stay explicit as debt

Superseded by:

- none

### DEC-0075: Unsupported generated support must have one explicit policy path in code before broader backend adoption

- Date: `2026-03-10`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0002-generic-backend-and-codegen-contract.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0002-generic-backend-and-codegen-contract.md)

Decision:

The design-note rule that unsupported generated support must fail closed is now
backed by one explicit private policy module in code. That policy:

- resolves a requested generated route against the registered artifact seam
- distinguishes missing artifact, schema mismatch, and unsupported route
- returns a deterministic reject/use decision before planner error mapping

Context:

Earlier G2 slices had already added schema checks, per-component inventory,
route compatibility, and execution-plan adoption for planner, workload, and
benchmark declarations. The remaining problem was that the unsupported-
generated-component rule still lived mostly in docs and error mapping. A small
private policy layer makes that rule concrete without widening the public API
before the broader backend seam is stable.

Alternatives rejected:

- leave unsupported generated support as a design-note-only rule
- encode policy independently in each caller through repeated matches
- widen the public planner API immediately to expose policy internals

Impact:

- unsupported generated support now has one canonical decision point in code
- the next honest G2 gap is broader backend adoption of the shared seam, not
  the existence of the fail-closed rule itself

Superseded by:

- none

### DEC-0074: Benchmark and workload declarations must consume generated-route support through the execution-plan seam

- Date: `2026-03-10`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0002-generic-backend-and-codegen-contract.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0002-generic-backend-and-codegen-contract.md)

Decision:

Benchmark and workload declarations must no longer reach directly into the
artifact registry for generated-route checks. They now consume the same shared
execution-plan helper layer that planner routing uses.

Context:

Earlier G2 slices had already created a private registry and lowering seam,
then added per-component generated inventory and fail-closed route support. The
remaining inconsistency was that benchmark and workload declarations still
looked directly at the registry. That left the contract truthful but only
partially adopted. Moving those declaration paths onto execution-plan helpers
narrows that gap without widening the public API.

Alternatives rejected:

- leave benchmark and workload modules with direct registry reach-through
- add a second declaration-local helper layer beside execution planning
- defer this adoption step until after generated fast-path lowering

Impact:

- planner, workload, and benchmark declarations now share the same route-check
  and error-mapping seam
- the next honest G2 gap is broader backend adoption and one explicit
  unsupported-generated-component policy path, not declaration-layer drift

Superseded by:

- none

### DEC-0073: Generated-route compatibility must be explicit and fail closed through the shared registry seam

- Date: `2026-03-10`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0002-generic-backend-and-codegen-contract.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0002-generic-backend-and-codegen-contract.md)

Decision:

Registered Metal component artifacts now declare which generated routes they
support, and the shared private registry must reject unsupported routes
explicitly:

- registered prove
- workload-boundary declaration
- declared benchmark trace generation
- declared benchmark prove/verify

Context:

After the earlier G2 slices, benchmark identity, workload-stage ownership, and
per-component generated inventory all lived in the same contract seam, but
route support was still implied by call-site convention. That left too much
room for accidental support claims. Moving route compatibility into the
registry keeps unsupported generated behavior deterministic without widening the
public API.

Alternatives rejected:

- infer supported routes from workload names or benchmark constants
- keep route support as ad hoc assertions inside benchmark and workload modules
- postpone explicit route compatibility until after full generated fast-path
  lowering

Impact:

- unsupported generated routes now fail closed through the same seam as schema
  and identity checks
- the next honest G2 gap is broader backend adoption of the seam, not route
  guessing

Superseded by:

- none

### DEC-0072: Generated inventory must be recorded per component, not as one shared registry blob

- Date: `2026-03-10`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0002-generic-backend-and-codegen-contract.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0002-generic-backend-and-codegen-contract.md)

Decision:

The private Metal artifact registry now records generated inventory on each
registered component artifact rather than as one shared registry-wide blob:

- registration key
- ABI family
- build-module inventory
- optional witness hook

Context:

Earlier G2 slices had already unified planner routing, benchmark identity, and
workload-stage ownership under one contract seam. The next honest gap was that
generated fast-path inventory still effectively behaved like shared registry
metadata, which is too weak for the codegen contract frozen in `dn-0002`.
Moving that inventory onto each component keeps the boundary truthful without
widening the public API.

Alternatives rejected:

- keep generated inventory as one shared registry-level structure
- defer per-component inventory until full generated fast-path lowering
- expose generated inventory publicly before the private seam is stable

Impact:

- component artifacts now carry richer generated metadata through the same
  private boundary as planning and workload declarations
- the next honest G2 gap is broader operation compatibility and fail-closed
  unsupported generated-operation behavior, not per-component generated
  identity

Superseded by:

- none

### DEC-0071: Workload-stage ownership metadata must live in shared contract data, not a workload-local table

- Date: `2026-03-10`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0002-generic-backend-and-codegen-contract.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0002-generic-backend-and-codegen-contract.md)

Decision:

Workload-stage ownership is now part of the same registered contract data used
for component planning:

- stage and ownership types are shared contract types
- generated registration data carries stage assignments
- workload boundaries read stage assignments from registered artifacts instead
  of maintaining a second workload-local table

Context:

After the earlier G2 slices, planner routing and benchmark identity were
already registry-backed, but workload-stage ownership still lived in a parallel
manual table inside the workload module. That left one more planning seam
outside the contract the roadmap had already frozen. Moving stage assignments
into shared contract data narrows that gap without widening the public API.

Alternatives rejected:

- keep a workload-local stage table beside the registry
- move workload-stage ownership into runtime code rather than registration data
- postpone this consolidation until after generated fast-path implementation

Impact:

- workload declarations now consume one less ad hoc planning table
- the next honest G2 gap is richer inventory and broader operation coverage,
  not workload identity or ownership duplication

Superseded by:

- none

### DEC-0070: Benchmark declaration metadata must be read from the same registered artifact seam as prove planning

- Date: `2026-03-10`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0002-generic-backend-and-codegen-contract.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0002-generic-backend-and-codegen-contract.md)

Decision:

The Metal artifact registry now owns benchmark-facing identity metadata for
declared workloads:

- workload family
- supported benchmark operations

Declared benchmark boundaries must read that metadata from the registered
artifact instead of assuming benchmark constants and planner manifests stay in
sync by convention.

Context:

After the first G2 slice, planner and workload declarations already routed
through the new private registry and execution-plan seam, but benchmark
declarations were still only indirectly connected through workload names and
hardcoded benchmark constants. This kept benchmark identity outside the same
contract the roadmap had already frozen. Moving benchmark identity into the
artifact seam widens the contract without widening the public API.

Alternatives rejected:

- keep benchmark identity purely as hardcoded constants
- create a separate benchmark registry beside the artifact registry
- wait until full generated fast-path support before tightening benchmark
  metadata ownership

Impact:

- benchmark declarations now share the same private contract boundary as prove
  planning
- the next honest G2 gap is broader operation and workload-stage coverage, not
  benchmark/workload identity drift

Superseded by:

- none

### DEC-0069: Metal planning must flow through one artifact-registry and execution-plan seam

- Date: `2026-03-10`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0002-generic-backend-and-codegen-contract.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0002-generic-backend-and-codegen-contract.md)

Decision:

The Metal backend now has one mandatory private planning seam for registered
component proving:

- `ArtifactRegistry`
  - schema/version gate
  - producer metadata
  - generated manifest ownership
  - unknown-component rejection
- `ExecutionPlan`
  - lowering from registered artifact input to backend prove planning
  - fail-closed unsupported-component behavior

Existing planner and workload declaration entry points must route through that
seam rather than reaching into generated manifests directly.

Context:

The generic/codegen contract had been frozen in docs, but the code still used a
thin direct manifest lookup path. That left the architecture truthful on paper
but not yet in the implementation. The first G2 code slice landed a private
registry and lowering layer, kept the public planner API stable, and moved the
existing Metal planner/workload declarations onto that shared path with
deterministic tests for schema mismatch and unknown-component failure.

Alternatives rejected:

- keep the current manifest lookup helpers as the long-term planning surface
- expose a new broad public planning API before the internal boundary settled
- defer the registry/lowering seam until after more benchmark or example work

Impact:

- the architecture contract is now partially implemented rather than purely
  documented
- future generated-inventory and execution-planning work has one internal seam
  to extend
- the active G2 work shifts from “define the seam” to “widen and adopt the
  seam”

Superseded by:

- none

### DEC-0068: Superseded milestone sequencing must live in one archive, not in the active planning docs

- Date: `2026-03-10`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0002-generic-backend-and-codegen-contract.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0002-generic-backend-and-codegen-contract.md)

Decision:

The old `T0` through `T8` milestone sequence is now archived in exactly one
place:

- [`milestone-archive.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/milestone-archive.md)

The active planning docs must stay operational and forward-looking:

- [`roadmap.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/roadmap.md)
- [`program-plan.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/program-plan.md)
- [`controller.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/controller.md)

Context:

Once the generic backend and codegen contract was frozen, the active docs still
carried too much superseded sequencing context. That made the repository harder
to operate because current planning, historical milestone memory, and
architecture reset were all mixed together. A single archive keeps the old path
auditable without letting it continue to steer day-to-day work.

Alternatives rejected:

- leave the old milestone history spread across roadmap, plan, and controller
- delete the milestone history entirely
- keep adding historical notes inline to active docs as new resets happen

Impact:

- the docs directory now has one explicit archive surface for old sequencing
- the active planning docs are shorter and easier to operate from
- future architecture resets must either update the active docs or add one
  archive entry, rather than mixing both concerns everywhere

Superseded by:

- none

### DEC-0067: Examples are the acceptance matrix, while the architecture is generic backend substitution plus generated proving artifacts

- Date: `2026-03-10`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0002-generic-backend-and-codegen-contract.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0002-generic-backend-and-codegen-contract.md)

Decision:

The active architecture is now explicitly split into three layers:

- generic backend substitution for correctness and coverage
- generated fast-path support driven by Stwo/framework/codegen proving
  artifacts
- temporary example-specific wrappers only as compatibility shims

Examples remain useful, but only as the acceptance matrix. They are not the
long-term integration surface and must not define the backend API.

Context:

The Metal bring-up proved that upstream examples are valuable for acceptance,
but it also showed the cost of letting benchmark rows and example-local seams
drive the implementation strategy. The engineering review converged on a more
durable distinction:

- the backend should be generic over Stwo-defined and codegen-defined proving
  components
- the scalable performance story requires a machine-readable producer artifact
  plus generated registration, ABI, and build inventory
- example-specific wrappers should stay temporary and explicit

That contract is now frozen in
[`dn-0002-generic-backend-and-codegen-contract.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0002-generic-backend-and-codegen-contract.md).

Alternatives rejected:

- continue using acceptance examples as the de facto architecture
- keep widening benchmark-local or example-local seams instead of freezing the
  producer/consumer contract
- promise one magical backend that efficiently proves arbitrary Rust workloads
  without Stwo-owned metadata

Impact:

- the roadmap is now sequenced around generic substitution, generated fast
  path, and eventual `stark-v` hardening
- benchmark work is no longer the active architecture driver
- unsupported generated components must fail closed instead of quietly falling
  back inside claimed GPU rows

Superseded by:

- none

### DEC-0066: Standard Blake2s lifted trace leaves must hash directly from Metal column buffers in 16-column blocks

- Date: `2026-03-10`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)

Decision:

The standard Blake2s lifted leaf path for wide trees now treats direct
Metal-buffer hashing as a hard benchmark boundary:

- standard Blake2s lifted leaves must no longer flatten wide trace trees into a
  staged buffer before hashing
- wide trees now hash directly from Metal column buffers in 16-column chunks,
  which aligns each dispatch with one 64-byte Blake2s compression block
- the grouped quotient side of `prove_values` now avoids the extra
  sort-and-group churn pass before `ColumnSampleBatch` construction

Context:

After grouped PCS sample filling and cached point-evaluation staging landed, the
remaining commitment wall was still the large trace-tree leaf path. The bounded
Metal Blake2s leaf kernel only covered small standard trees, so the real
`wide_fibonacci` trace tree still paid a wide flatten-and-stage tax before
hashing. Once the direct wide-tree path was added and the quotient grouping pass
was tightened, the best measured production row moved to
`wide_fibonacci_prove_verify_v1 = 1456.654041 ms`, with
`prove_ms = 1456.38`, `verify_ms = 0.274041`,
`prove_core_prove_values_ms = 885.230166`,
`trace_commit_ms = 225.42575000000002`, and
`trace_commit_merkle_ms = 48.477041`.

Alternatives rejected:

- keep widening the old flattened-leaf path instead of replacing it with a
  direct Metal-buffer boundary
- move immediately to a fully GPU-side upper Merkle tree while the first-order
  leaf staging tax was still present
- leave the quotient-side `prove_values` regrouping churn in place while
  claiming the next prove-values wall was purely arithmetic

Impact:

- the best measured end-to-end row is now about `1.05x` slower than the SIMD
  `log20` reference instead of `1.09x` slower
- `trace_commit_merkle_ms` is no longer a first-order benchmark wall on the
  measured wide-fibonacci row
- the next honest optimization target is the remaining `prove_values` wall and
  the composition-generation / upper commitment orchestration around it

Superseded by:

- none

### DEC-0065: PCS sampled-value scheduling and large-domain point-evaluation staging must stay grouped and Metal-backed

- Date: `2026-03-10`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)

Decision:

The benchmark-active sampled-value path now treats grouped scheduling and
Metal-backed staging as hard constraints:

- `prove_values` request grouping must write batched evaluation results
  directly into final `PointSample` slots instead of building a flat request
  list and then rescattering the same values
- large-domain `batch_eval_at_point` must reuse flattened Metal coefficient
  staging across repeated point-query groups instead of reflattening the same
  coefficient sets on every grouped call
- bounded standard Blake2s leaf construction now has an explicit Metal kernel
  and large standard Blake2s trees are the next direct Metal-buffer target

Context:

After direct trace slicing, Metal-backed quotient staging, native FFT/IFFT, and
the native point-evaluation lane were all in place, the measured
`wide_fibonacci_prove_verify_v1` row was no longer dominated by obvious buffer
readback bugs. The profile still showed two first-order walls:

- `prove_core_prove_values_ms`
- large-tree lifted Blake2s `build_leaves`

The vendored PCS sampled-value path still flattened requests, regrouped them,
evaluated them, and then scattered the results back into the same sample
structure. At the same time, repeated grouped point queries were reflattening
the same Metal-backed coefficient sets. After retiring those duplicate staging
and scatter steps, the current production-grade row is now
`wide_fibonacci_prove_verify_v1 = 1521.303625 ms`, with
`prove_ms = 1520.994583`, `verify_ms = 0.309042`,
`prove_core_prove_values_ms = 893.510667`,
`trace_commit_ms = 307.475084`, and
`trace_commit_merkle_ms = 147.948042`.

Alternatives rejected:

- keep the PCS request flatten/regroup/scatter shape because the backend call
  was already grouped
- chase only new kernels while leaving repeated coefficient flattening inside
  the benchmark-active `PolyOps` path
- call the bounded Metal Blake2s leaf path “done” before separating the small
  standard-tree win from the still-host-owned large trace-tree path

Impact:

- the measured end-to-end row is now about `1.09x` slower than the SIMD
  `log20` reference instead of `1.12x` slower
- `prove_core_prove_values_ms` dropped materially without changing proof
  semantics or widening the public API
- the next honest walls are the remaining grouped prove-values work above PCS
  and the still-wide standard-tree leaf staging that is now the next direct
  Metal-buffer target

Superseded by:

- none

### DEC-0064: Native FFT/IFFT commitment work must stay on Metal buffers end-to-end

- Date: `2026-03-10`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)

Decision:

The benchmark-active FFT/poly commitment lane now treats buffer ownership as a
hard constraint:

- once a trace or evaluation column is already Metal-backed, native `rfft` and
  `ifft` work must operate on cloned Metal buffers directly
- no intermediate `to_vec()` / `from_slice()` host bounce is acceptable inside
  the benchmark-active trace commitment lane

Context:

After direct trace-column slicing made trace generation benchmark-faithful, the
next measured benchmark wall inside the trace half was commitment work. The
native FFT/IFFT path still forced a full host materialization and re-upload
before launching the native kernels, even though the values were already in
Metal-owned buffers. Retiring that hidden round-trip moved the measured row to
`wide_fibonacci_prove_verify_v1 = 2823.8264590000003 ms`, with
`prove_ms = 2823.629792`, `trace_generation_ms = 65.444416`,
`trace_commit_ms = 409.924625`, `trace_commit_interpolation_ms = 51.244667`,
and `trace_commit_extension_ms = 121.42966700000001`.

Alternatives rejected:

- keep the host bounce because the kernels themselves were already native
- move directly to a GPU-side hash program while the FFT/poly lane still
  carried obvious avoidable host materialization
- bundle this behavior under an implicit optimizer rule rather than recording
  it as a benchmark-facing boundary law

Impact:

- the measured end-to-end row is now about `2.03x` slower than the SIMD
  `log20` reference instead of `2.23x` slower
- trace commitment is no longer hiding avoidable host transport inside the
  native FFT path
- the next honest wall is even clearer: `prove_values` now dominates the
  production-grade wide-fibonacci row

Superseded by:

- none

### DEC-0063: Release-grade benchmark work now treats direct Metal trace slicing and indexed column queries as mandatory boundaries, and the next measured wall is `prove_values`

- Date: `2026-03-10`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)

Decision:

The active benchmark-grade Metal boundary now requires:

- direct coordinate-wise `AccumulationOps` and quotient accumulation over
  Metal-owned buffers rather than row-wise `SecureField` reconstruction
- indexed query reads for `MetalBaseFieldVec::batch_at` so decommit queries do
  not force whole-column readback when only a small query set is needed
- direct slicing of native wide-fibonacci trace columns out of the flat Metal
  trace buffer rather than element-wise host reads followed by re-upload

The next measured optimization target is no longer trace generation. It is the
`prove_values` phase, with trace commitment the next measured cost behind it.

Context:

The prior production-grade row had already fallen from the old debug-inflated
measurements to the `4859.213000 ms` range, but it still carried two hidden
host taxes:

- wide-fibonacci trace columns were being reconstructed by repeated
  `buffer_get` reads and then uploaded back into Metal-owned columns
- base-field batch queries in the lifted decommit path still defaulted to full
  host materialization when no cache existed

After retiring those taxes and keeping the direct Metal-owned PCS accumulation
refactor in place, the benchmark moved to
`wide_fibonacci_prove_verify_v1 = 3105.399292 ms`, with
`prove_ms = 3105.208917`, `verify_ms = 0.19037500000000002`,
`trace_generation_ms = 66.355792`, `trace_commit_ms = 616.476209`, and
`prove_core_prove_values_ms = 1881.806083`.

Alternatives rejected:

- keep treating the old trace materialization path as “native enough” because
  the kernel itself was already Metal-backed
- keep optimizing host commitment code first after the benchmark showed that
  trace generation had fallen out of the dominant-cost set
- add a benchmark-only semantic shortcut rather than first retiring the hidden
  host round-trips at the actual runtime boundary

Impact:

- the benchmark row is now about `2.23x` slower than the SIMD `log20`
  reference instead of `7.8x` slower
- direct trace generation is no longer a first-order blocker in the benchmark
  optimization program
- the next honest tranche is the measured `prove_values` wall, with trace
  commitment and composition generation behind it

Superseded by:

- none

### DEC-0062: Production benchmark measurements now require `cargo_profile = release`, `STWO_METAL_MODE = metal-prod`, and Metal-backed wide-fibonacci quotient staging

- Date: `2026-03-10`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)

Decision:

The benchmark contract for non-plan Apple Silicon measurements is now:

- run the standalone Metal benchmark binaries in the Rust `release` profile
- use `STWO_METAL_MODE=metal-prod` unless an explicit diagnostic override is set
- treat non-release or `metal-dev` benchmark rows as diagnostic data, not as the
  primary performance baseline
- keep wide-fibonacci quotient accumulation on Metal-backed trace buffers during
  composition generation instead of reading the evaluation columns back to host
  slices first

Context:

The previous benchmark narrative overstated the slowdown because the project
had been treating `cargo run`-style dev-profile rows as if they were
production measurements. At the same time, the wide-fibonacci composition path
was still forcing a full host materialization of the trace evaluations before
entering the native quotient kernel. After enforcing the benchmark contract and
keeping quotient staging on Metal-backed buffers, the measured production-grade
row is now `wide_fibonacci_prove_verify_v1 = 10900.002875 ms`, with
`prove_ms = 10899.813583000001`, `verify_ms = 0.18929200000000002`,
`prove_core_prove_values_ms = 4686.306874999999`, and
`prove_core_composition_generation_ms = 3729.947666`.

Alternatives rejected:

- keep treating dev-profile or `metal-dev` runs as the benchmark source of truth
- add unchecked fast-math flags and call that “production mode” without first
  proving correctness and measuring the real release baseline
- leave wide-fibonacci quotient staging host-backed while trying to optimize the
  same phase at higher levels

Impact:

- the active benchmark baseline is now truthful and directly comparable against
  SIMD and historical GPU rows
- the project’s remaining performance gap is now understood as roughly `7.8x`
  to the SIMD `log_n_instances = 20` row, not the old debug-inflated `31x`
- the next benchmark-facing structural target stays above `PolyOps`:
  PCS prove-values duplication and the remaining host-owned commitment/hash path

Superseded by:

- none

### DEC-0061: Native Metal point evaluation is now a benchmark-active `PolyOps` surface, and coefficient staging must stay on Metal when possible

- Date: `2026-03-10`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)

Decision:

`MetalBackend` now treats large-domain circle-polynomial point evaluation as a
native Metal optimization surface. The active rule is:

- use the mirrored `.metal` batch reduction lane for large-domain point
  evaluation
- stage coefficients through Metal-owned buffers rather than host readback and
  re-upload when batched evaluation is already operating on Metal-backed
  columns
- allow secure-coordinate polynomial evaluation to batch through the same
  backend surface instead of forcing four independent coordinate calls

Context:

After retiring the larger PCS bridges and consolidating host readbacks, the
end-to-end wide-fibonacci row was still dominated by prove-values work. The
first native point-evaluation cut exposed a correctness bug in the Metal
reduction kernel, which was traced to a missing shared-memory read/write
barrier. Once corrected, the native point-evaluation lane became parity-tested
at both the partial-chunk and full multi-stage levels, and the best measured
benchmark row on the current tree improved to
`wide_fibonacci_prove_verify_v1 = 43262.562624 ms`, with
`prove_ms = 43257.033541`, `verify_ms = 5.529083`, and
`prove_core_prove_values_ms = 19427.014834`.

Alternatives rejected:

- keep point evaluation CPU-owned while claiming the next benchmark work is
  deeper prover orchestration
- ship a broken fully native reduction path rather than prove parity first
- accept a pseudo-native path that still materializes coefficients on the host
  before every batch

Impact:

- native point evaluation is now a real benchmark-facing Metal surface rather
  than a planned future slice
- secure polynomial evaluation may route through `batch_eval_at_point` to
  amortize coordinate work
- the next honest benchmark work remains above this layer:
  further PCS prove-values duplication and the host-owned commitment/hash path

Superseded by:

- none

### DEC-0060: Host-readback consolidation is now an explicit optimization rule, and the next prove-values step must be native point evaluation

- Date: `2026-03-10`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)

Decision:

While the lifted Blake2s commitment and parts of the prove-values path remain
host-owned, `stwo-metal` now treats one-shot host materialization, cached
readback views, and batched queried-value reads as the default optimization
law for those surfaces. That law is now considered exploited enough that the
next benchmark-facing prove-values step must be a native point-evaluation lane
rather than more host clone cleanup.

Context:

The measured benchmark row fell materially after consolidating host readbacks
across lifted Blake2s leaves, PCS quotient work, and cached base-field views.
The follow-on borrowed host-view cleanup and batched queried-value read slice
kept the semantics clean but only nudged the row to
`wide_fibonacci_prove_verify_v1 = 45616.501417 ms`, with
`prove_ms = 45611.008417`, `verify_ms = 5.492999999999999`,
`prove_core_prove_values_ms = 21835.479`, and
`trace_commit_merkle_ms = 9168.181416`. That is still about `32.8x` slower
than the current SIMD `log_n_instances = 20` reference.

Alternatives rejected:

- keep pursuing small host clone and cache cleanups without first acknowledging
  that the remaining prove-values cost is now mostly algorithmic
- rewrite the benchmark contract around a new workload instead of using the
  measured `wide_fibonacci` row as the active optimization reference
- hide the plateau inside a benchmark note without changing the sequencing
  expectation for T8

Impact:

- host-owned commitment and prove-values code now has an explicit optimization
  rule for future maintenance and review
- the next honest performance tranche is a native Metal point-evaluation path
  mirroring the CUDA-side `eval_at_point` / `batch_eval_at_points` capability
- adapter cleanup and the bounded small-domain `PolyOps` fallback stay explicit
  debt rather than getting folded into performance claims

Superseded by:

- none

### DEC-0059: `AccumulationOps` and `QuotientOps` now stay on direct Metal-owned PCS storage rather than `CpuBackend`

- Date: `2026-03-10`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)

Decision:

`MetalBackend` now performs `AccumulationOps` and `QuotientOps` directly over
Metal-owned secure-column and evaluation storage. The legacy capability names
are retained for API stability, but the `CpuBackend` dependency for these PCS
surfaces is retired.

Context:

After enabling deterministic parallel proving and reducing the host-side
Merkle path, the measured end-to-end row was dominated by PCS prove-values
work. The explicit `AccumulationOps` and `QuotientOps` bridges were no longer
the smallest honest boundary: both operations are deterministic host logic over
already-materialized Metal-owned buffers, so they could be retired without
introducing a new kernel or changing proof semantics.

Alternatives rejected:

- keep `AccumulationOps` and `QuotientOps` on `CpuBackend` while claiming the
  remaining prove-values bottleneck was mostly deeper PCS work
- widen immediately into larger commitment or transcript refactors before
  first removing these direct host-side backend dependencies
- hide bridge retirement inside a benchmark-only diff without updating the
  supported backend contract

Impact:

- the measured end-to-end benchmark row drops again, and
  `prove_core_prove_values_ms` becomes materially smaller than the previous
  baseline; the current rerun is `wide_fibonacci_prove_verify_v1 =
  64588.605875 ms`, with `prove_ms = 64582.99775`, `verify_ms = 5.608125`,
  and `prove_core_prove_values_ms = 30791.174583`
- the capability model now truthfully reports these surfaces as supported
  rather than explicit CPU bridges
- the next measured blockers are the prove-values layer above PCS,
  host-owned commitment/hash work, and the bounded small-domain `PolyOps`
  fallback

Superseded by:

- none

### DEC-0058: Batch point evaluation is now a benchmark-active `PolyOps` optimization surface

- Date: `2026-03-10`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)

Decision:

`MetalBackend::batch_eval_at_point` now evaluates point queries directly over
Metal-owned polynomial storage and uses deterministic parallel iteration when
the `parallel` feature is enabled. This is now part of the benchmark-active
`PolyOps` surface for the wide-fibonacci prove row.

Context:

Once the parallel Blake2s tranche landed, the measured benchmark row showed
that `prove_core_prove_values_ms` dominated the remaining cost. The vendored
default `batch_eval_at_point` path performed repeated single-point evaluation
without taking advantage of available deterministic parallelism, even though
the benchmark fixture already enabled the `parallel` feature.

Alternatives rejected:

- jump directly to new kernels before exploiting the obvious deterministic
  parallel structure in the existing host-side point-evaluation law
- treat single-point evaluation as sufficient while the benchmark row still
  spent most of its time in prove-values work
- optimize Merkle hashing first even though prove-values had become the larger
  measured blocker

Impact:

- the benchmark-active `PolyOps` path now has a truthful batch-evaluation
  surface instead of serial repeated single-point calls
- the PCS prove-values baseline is materially lower, which makes the next
  structural prove-values work easier to measure honestly

Superseded by:

- none

### DEC-0057: The standalone Metal benchmark fixture enables deterministic parallel proving support before deeper PCS optimization

- Date: `2026-03-10`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)

Decision:

The standalone Metal benchmark fixture now enables `stwo-metal`'s `parallel`
feature, and the Metal Blake2s lifted Merkle path now uses deterministic
parallel iteration for leaf hashing and next-layer hashing when that feature is
enabled. The first measured result after this change is
`wide_fibonacci_prove_verify_v1 = 102272.056124 ms`, with
`prove_ms = 102266.681958` and `verify_ms = 5.374166`, at
`log_n_instances = 20`, `n_columns = 100`, `STWO_METAL_MODE=metal-dev`,
`warmups = 0`, `samples = 1`, and `threads = 14`.

Context:

After the benchmark boundary was closed end to end through `MetalBackend`, the
measured prove row was still dominated by host-owned commitment work, especially
the Blake2s lifted Merkle stages. The Metal benchmark fixture was also still
building without the `parallel` proving surface, which meant the host-side
proof machinery was leaving obvious deterministic parallelism unused.

Alternatives rejected:

- jump directly to larger PCS or quotient refactors before turning on the
  already-supported parallel proving surface
- enable parallel benchmarking without making the Metal Blake2s host path
  actually honor it
- treat the previous serial benchmark number as the only meaningful baseline
  after a large host-side proving bottleneck was still plainly exposed

Impact:

- the end-to-end benchmark row improved materially, and
  `trace_commit_merkle_ms` dropped from the prior serial baseline to a much
  smaller but still non-trivial cost
- `prove_core_prove_values_ms` is now the dominant measured blocker
- the next structural choice should focus on PCS prove-values work and the
  explicit `AccumulationOps` / `QuotientOps` bridges rather than on the Merkle
  leaf path first

Superseded by:

- none

### DEC-0056: The `wide_fibonacci_prove` benchmark boundary is now closed through `MetalBackend`, and the remaining work is performance debt rather than execution debt

- Date: `2026-03-10`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)

Decision:

The standalone `wide_fibonacci_prove` row now executes end to end through
`MetalBackend` and verifies successfully on Apple Silicon. The old benchmark
boundary debt is therefore retired. The benchmark target remains open as a
performance problem, not as an execution-boundary problem. The first closure
measurement for that row was:
`wide_fibonacci_prove_verify_v1 = 213731.915833 ms`, with
`prove_ms = 213726.729125` and `verify_ms = 5.186708`, at
`log_n_instances = 20`, `n_columns = 100`, `STWO_METAL_MODE=metal-dev`,
`warmups = 0`, and `samples = 1`. The active optimization baseline now lives
in `DEC-0057`.

Context:

The prior benchmark activation tranche had already recorded a native trace
baseline, but the full prove row still carried a named non-Metal execution
gap. After retiring the remaining wider FRI, point-evaluation, and Blake2s
`CpuBackend` dependencies, the benchmark runner could be switched to
`MetalBackend` truthfully and measured as an end-to-end row.

Alternatives rejected:

- keep `TD-0012` active after the runner already executes through
  `MetalBackend`
- treat the first end-to-end Apple Silicon number as if it already satisfied
  the `90 ms` reference goal
- record the benchmark result without splitting benchmark-boundary closure from
  performance debt

Impact:

- `TD-0012` is retired
- `T6` is now complete
- the next T8 work is measured optimization of the dominant prove stages, not
  benchmark-boundary closure
- the recorded prove-phase breakdown is now the optimization source of truth

Superseded by:

- none

### DEC-0055: Blake2s lifted Merkle and proof-of-work support no longer depends on `CpuBackend`

- Date: `2026-03-10`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)

Decision:

The lifted Blake2s Merkle leaf construction, next-layer hashing, and channel
proof-of-work grind loop are now implemented directly for `MetalBackend`
without delegating to `CpuBackend`.

Context:

Once the mirrored native hot-path set was complete and the benchmark row was
close to executable, the remaining Blake2s trait debt had become a misleading
signal: the work was host-side Blake2s hashing over Metal-owned columns, but
the code still routed through `CpuBackend`. That obscured the real bottleneck
and overstated the remaining bridge count.

Alternatives rejected:

- keep the lifted Blake2s and proof-of-work path on `CpuBackend` while claiming
  the benchmark row was nearly end-to-end
- move directly to benchmark closure without retiring the misleading Blake2s
  bridge
- invent a GPU-side Blake2s kernel before first retiring the unnecessary CPU
  backend dependency

Impact:

- the legacy Blake2s capability names are retained for API stability, but they
  now describe direct support rather than CPU bridging
- the next benchmark-facing bottlenecks are measured host-owned commitment work
  and explicit `AccumulationOps` / `QuotientOps` bridges

Superseded by:

- none

### DEC-0054: Point evaluation and barycentric `PolyOps` are now Metal-owned, leaving only the bounded small-domain fallback

- Date: `2026-03-10`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)

Decision:

`PolyOps::eval_at_point`, `barycentric_weights`,
`barycentric_eval_at_point`, and `eval_at_point_by_folding` now execute
directly over Metal-owned polynomial and evaluation storage instead of
delegating to `CpuBackend`. The only explicit `PolyOps` CPU path that remains
is the bounded small-domain evaluate/interpolate fallback.

Context:

After `extend` and `split_at_mid` were retired from `CpuBackend`, the next
smallest truthful Poly step was to port the direct algebraic point-evaluation
and barycentric helpers. Those routines are deterministic host logic over
Metal-owned buffers and did not require a new kernel to retire the CPU bridge.

Alternatives rejected:

- leave point evaluation and barycentric helpers on `CpuBackend` while
  claiming Poly support was close enough for benchmark closure
- widen directly into larger commitment or benchmark work before narrowing the
  remaining Poly trait debt
- add speculative new kernels before proving the direct Metal-owned host logic
  was sufficient

Impact:

- `PolyOps` no longer depends on `CpuBackend` for point evaluation or
  barycentric helpers
- the remaining explicit `PolyOps` CPU path is now small, named, and bounded

Superseded by:

- none

### DEC-0053: The wider `FriOps` secure-column repacking and fold-accumulation bridge is retired from `CpuBackend`

- Date: `2026-03-10`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)

Decision:

The wider `FriOps` path is now Metal-owned: secure-column repacking stays on
Metal-owned storage, `fold_circle_into_line` accumulates against Metal-owned
line values without `CpuBackend`, and the old legacy `FriOps` CPU bridge is
retired.

Context:

`FriOps::decompose` had already moved off `CpuBackend`, which made the
remaining FRI bridge more obviously artificial. The next semantics-preserving
step was to repack secure columns and accumulate folded values directly over
Metal-owned storage rather than continuing to bounce through CPU vectors.

Alternatives rejected:

- keep describing `FriOps` as bridged even after the remaining bridge was only
  a storage conversion artifact
- jump directly to benchmark conclusions without retiring the wider FRI bridge
- hide the repacking and fold-accumulation move inside a larger benchmark-only
  patch

Impact:

- the legacy `FriOps` capability name now refers to retired CPU bridging
  rather than an active dependency
- the wider benchmark-facing proving gap moves away from FRI and toward
  commitment and accumulation bottlenecks

Superseded by:

- none

### DEC-0049: Mirrored hot-path completion may stop at parity-tested support for `prefix_sum`, and the next native decision moves from file presence to benchmark bottlenecks

- Date: `2026-03-10`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)

Decision:

`prefix_sum.metal` is now compile-active and parity-tested as a support kernel
for bit-reversed circle-domain base-field columns. That is enough to mark the
mirrored hot-path set complete in `PORTING_STATUS.md`; the next T8 choice is no
longer “which mirrored file is missing?” but “which remaining explicit CPU
bridge is the real measured bottleneck for the benchmark-active path?”

Context:

After native `mle` and the bounded native `gkr` subset landed, `prefix_sum`
was the only mirrored hot-path file still scaffold-only. It is useful support
in the copied CUDA subsystem, but not yet the architecture driver for the
current proving row. Completing it as parity-tested support keeps the mirror
truthful without over-claiming benchmark ownership.

Alternatives rejected:

- leave `prefix_sum.metal` scaffold-only and keep `PORTING_STATUS.md`
  permanently incomplete
- claim `prefix_sum` as benchmark-critical ownership before measurement shows
  it is on the active bottleneck path
- keep sequencing T8 by file presence after the mirrored hot-path set is
  already complete

Impact:

- `PORTING_STATUS.md` now has no scaffold-only mirrored hot-path files
- the next honest native tranche is benchmark activation plus retirement of the
  next measured explicit CPU bridge
- mirrored file completion is no longer the gating question for T8

Superseded by:

- none

### DEC-0052: `FriOps::decompose` is retired from `CpuBackend` before the wider FRI repacking bridge

- Date: `2026-03-10`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)

Decision:

`FriOps::decompose` now executes directly over Metal-owned secure-column values
instead of delegating to `CpuBackend`. The explicit `FriOps` bridge is
therefore narrowed to secure-column repacking into the bounded fold kernels and
the host-side accumulation path inside `fold_circle_into_line`.

Context:

After narrowing `PolyOps` and recording the first native trace benchmark
baseline, the next semantically small FRI step was the decomposition law:
compute the alternating-half lambda and shift the two domain halves by that
lambda. That logic is explicit in the vendored CPU backend and can be ported
without inventing a new `.metal` kernel or changing the fold interfaces.

Alternatives rejected:

- keep `decompose` on `CpuBackend` while claiming the remaining FRI bridge was
  only about repacking
- jump straight to a wider fold/repacking rewrite before retiring this smaller
  algebraic handoff
- create a new native kernel before proving the direct Metal-owned host
  implementation was enough

Impact:

- `FriOps` no longer depends on `CpuBackend` for decomposition
- the remaining explicit `FriOps` bridge is smaller and easier to measure
- the next benchmark-facing proving bridges are now secure-column repacking in
  `FriOps`, point-evaluation/barycentric `PolyOps`, and Blake2s lifted hashing

Superseded by:

- none

### DEC-0051: Benchmark activation records the first Apple Silicon native wide-fibonacci trace baseline before the prove row is closed

- Date: `2026-03-10`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)

Decision:

The project records the first benchmark-active Apple Silicon trace result for
the declared north-star row without overstating prove closure:
`wide_fibonacci_trace_generation_v1` completed in `66.61 ms` at
`log_n_instances = 20`, `n_columns = 100`, `STWO_METAL_MODE=metal-dev`,
`warmups = 0`, and `samples = 1`.

Context:

Once the mirrored hot-path set was complete, benchmark work needed to become
measured rather than merely planned. The trace benchmark can already execute
through the native Metal path, while the full prove benchmark still contains
the explicit non-Metal remainder tracked in `TD-0012`.

Alternatives rejected:

- keep benchmark activation purely theoretical after the mirrored hot-path set
  was complete
- treat the trace-only number as if it were already comparable to the full
  `90 ms` RTX 4090 prove/verify target
- wait for every remaining proving bridge to retire before collecting any
  Apple Silicon performance evidence

Impact:

- the project now has a real Apple Silicon benchmark baseline for the native
  trace row
- `wide_fibonacci_prove_verify_v1` remains an open benchmark-completion task
  rather than an implied success
- future performance work can compare bridge-retirement slices against a
  recorded native Metal starting point

Superseded by:

- none

### DEC-0050: The next post-GKR bridge-retirement step narrows `PolyOps` before benchmark activation

- Date: `2026-03-10`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)

Decision:

`PolyOps::extend` and `PolyOps::split_at_mid` now stay inside Metal-owned
base-field storage instead of delegating to the vendored CPU backend. The
remaining explicit `PolyOps` bridge is narrowed to point evaluation and
barycentric helpers while the project turns the mirrored native set into
benchmark-active measurement.

Context:

Once `gkr.metal` owned the last native lookup-oracle walk, the next smallest
truthful `PolyOps` step was to remove the easy structural CPU handoffs before
opening a wider point-evaluation or FRI replacement tranche. `extend` is only
zero-padding, and `split_at_mid` is only coefficient partitioning, so both can
be retired without changing algebraic semantics.

Alternatives rejected:

- leave `extend` and `split_at_mid` on `CpuBackend` while moving directly to
  benchmark runs
- widen the slice into barycentric evaluation or full `FriOps::decompose`
  before the structural `PolyOps` bridge had been narrowed
- claim `PolyOps` was mostly native without retiring the CPU backend from
  these deterministic storage transformations

Impact:

- `PolyOps` no longer depends on `CpuBackend` for zero-padding or midpoint
  coefficient splits
- the remaining explicit `PolyOps` bridge is now easier to measure honestly
  against the wide-fibonacci proving path
- the next benchmark-facing bridge candidates remain `FriOps`,
  point-evaluation/barycentric `PolyOps`, and Blake2s lifted hashing

Superseded by:

- none

### DEC-0049: The mirrored `gkr.metal` tranche retires the remaining native GKR oracle CPU bridge before benchmark activation

- Date: `2026-03-10`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)

Decision:

`gkr.metal` now owns the bounded oracle-sum evaluation used by
`GkrOps::sum_as_poly_in_first_variable` for `GrandProduct`, `LogUpGeneric`,
`LogUpMultiplicities`, and `LogUpSingles`. The Metal runtime computes the
native `eval_at_0` and `eval_at_2` values, and the host only performs the
bounded `correct_sum_as_poly_in_first_variable` reconstruction step.

Context:

After the mirrored hot-path set became compile-active and parity-tested, the
largest remaining lookup-side CPU bridge was the oracle evaluation inside
`GkrOps`. That bridge was also close enough to the wide-fibonacci proving path
to matter for honest benchmark activation, so it was the right next retirement
step before turning attention to broader `FriOps` and `PolyOps` surfaces.

Alternatives rejected:

- leave `sum_as_poly_in_first_variable` on the CPU while claiming lookup
  mirroring was effectively complete
- move straight to benchmark activation without first retiring the last native
  GKR oracle walk over Metal-owned columns
- widen the change into a full `FriOps` or Blake2s bridge tranche before the
  smaller GKR retirement was parity-locked

Impact:

- `GkrOps` no longer performs CPU oracle evaluation over Metal-owned columns
- `gkr.metal` now covers native eq-eval generation, next-layer construction,
  and bounded oracle-sum evaluation
- the next measured bridge candidates are broader `FriOps`, `PolyOps`, and
  Blake2s lifted hashing surfaces

Superseded by:

- none

### DEC-0048: The next mirrored lookup tranche retires native MLE completely and narrows GKR to the remaining oracle-evaluation bridge

- Date: `2026-03-10`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)

Decision:

`mle.metal` is now compile-active and parity-tested for both base-field and
secure-field `fix_first_variable`, so the explicit `MleOps` CPU bridge is
retired. `gkr.metal` is also now compile-active and parity-tested for
`gen_eq_evals` and `next_layer` across every current `Layer` variant, but
`sum_as_poly_in_first_variable` remains an explicit CPU bridge until the
oracle-evaluation boundary has its own bounded native replacement.

Context:

After the mirrored quotient and fold files landed, the smallest truthful
bridge-retirement step was `MleOps`. The natural adjacent step inside the same
lookup tranche was not all of `GkrOps`, but the parts that are already
pointwise layer transforms with clear deterministic CPU oracles: eq-eval
generation and next-layer construction.

Alternatives rejected:

- leave `mle.metal` scaffold-only while moving directly to `prefix_sum`
- claim full `GkrOps` native support before the sumcheck-oracle boundary is
  implemented
- keep the old broad `GkrOps` CPU-bridge wording after native eq-evals and
  next-layer support had landed

Impact:

- the compile-active mirrored hot-path set now includes `mle.metal` and
  `gkr.metal`
- `MleOps` no longer depends on an explicit CPU bridge
- `GkrOps` is now a narrower and more truthful CPU bridge, limited to
  `sum_as_poly_in_first_variable`
- `prefix_sum.metal` is the only remaining scaffold-only mirrored hot-path
  file in `PORTING_STATUS.md`

Superseded by:

- none

### DEC-0047: Native quotient and fold kernels must live in mirrored Metal file names before the remaining support tranche proceeds

- Date: `2026-03-10`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)

Decision:

The active native proving kernels now live in the mirrored files
`quotients.metal`, `fold_circle_into_line.metal`, and `fold_line.metal`. The
older non-mirrored files `quotient.metal` and `fri.metal` remain in the tree
only as retained history and are no longer part of the compile-active Metal
set.

Context:

After the native FFT core landed, the next structural mismatch was that the
active quotient and fold kernels still lived in non-mirrored file names. That
made the Metal subtree harder to compare against the CUDA source and kept the
T8 status file less truthful than it should be. Re-homing those kernels fixes
the structure without churning the runtime ABI.

Alternatives rejected:

- keep compiling the old `fri.metal` and `quotient.metal` files indefinitely
- rename runtime kernel entry points as part of the structural move
- postpone the mirrored proving-file move until after `prefix_sum`, `mle`, or
  `gkr`

Impact:

- the compile-active Metal set is now aligned more closely with the mirrored
  CUDA file structure
- the next honest T8 tranche is the remaining support-kernel set:
  `prefix_sum`, `mle`, and `gkr`
- the old non-mirrored files are retained only as historical context

Superseded by:

- none

### DEC-0046: The second compile-active native T8 replacement lands the mirrored FFT core before quotient and fold work

- Date: `2026-03-10`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)

Decision:

After native twiddle precompute, the next compile-active mirrored T8 files are
`poly_utils.metal`, `rfft.metal`, and `ifft.metal`. The core
`MetalBackend::evaluate_into` and `MetalBackend::interpolate` paths now use
this native FFT/poly lane before the project moves into mirrored quotient and
fold files.

Context:

Once twiddle generation was native, the next highest-value CPU bridge to
retire was the FFT/poly execution core itself. Porting the mirrored FFT files
before quotient and fold work keeps the benchmark-facing proving path ordered
around reusable primitives rather than workload-specific shortcuts.

Alternatives rejected:

- move directly into quotient or fold files while evaluate/interpolate still
  bridge through the vendored CPU backend
- land only `rfft` without the matching `ifft` and shared poly helper file
- claim broader `PolyOps` completion than the current native scope supports

Impact:

- the compile-active mirrored Metal set now includes the FFT/poly core
- the remaining `PolyOps` bridge is narrower and explicitly about point
  evaluation, barycentric helpers, and host-side orchestration
- the next honest native tranche is mirrored `quotients` plus fold files

Superseded by:

- none

### DEC-0045: The first compile-active native T8 replacement retires the Metal twiddle CPU bridge before FFT/poly kernels are attempted

- Date: `2026-03-10`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)

Decision:

The first real native T8 replacement is `MetalBackend::precompute_twiddles`.
`fields.metal` and `twiddles.metal` are now compile-active, and twiddle plus
inverse-twiddle generation is retired from the previous CPU bridge before any
attempt to port `rfft`, `ifft`, or the rest of the FFT/poly path.

Context:

The mirrored native subsystem already existed structurally, but the generic
`PolyOps` path still depended on CPU twiddle materialization. That was the
smallest hot-path bridge to retire first because it sits below unchanged
upstream example proving and above the next FFT/poly tranche. Porting twiddles
before FFT kernels keeps the next tranche honest and measurable.

Alternatives rejected:

- start `rfft` or `ifft` while twiddle precompute still crosses the CPU bridge
- widen the native port to multiple FFT/poly files before the first compile-active
  `fields` and `twiddles` replacement is parity-tested
- leave `fields.metal` scaffold-only while claiming `twiddles.metal` support

Impact:

- `fields.metal` and `twiddles.metal` are now compile-active native sources
- `MetalBackend::precompute_twiddles` now validates native output directly
  against the vendored CPU oracle
- the next honest T8 tranche is `rfft` / `ifft`, not more twiddle scaffolding

Superseded by:

- none

### DEC-0044: Native performance work returns through a mirrored Metal subsystem that follows the CUDA hot-path structure file-for-file where replacement is intended

- Date: `2026-03-10`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)

Decision:

Now that the named non-blocked upstream examples prove and verify through
`MetalBackend`, the project returns to native performance work by mirroring the
active CUDA hot-path structure under `crates/stwo-metal-sys/metal`. The next
native tranche uses the same conceptual file names for the intended Metal
replacements, starting with `fields`, `twiddles`, `rfft`, `ifft`,
`poly_utils`, `quotients`, `fold_circle_into_line`, `fold_line`,
`prefix_sum`, `mle`, and `gkr`.

Context:

The acceptance milestone answered the architectural question: `stwo-metal` can
already prove real Stwo rows through backend substitution. The next bottleneck
is native implementation depth. The copied CUDA subsystem is large and already
organizes the hot path into reviewable files, while the current Metal subtree
is still a thin frontier. Mirroring those hot-path names is the cleanest way
to make parity review, benchmark planning, and file-by-file port tracking
explicit.

Alternatives rejected:

- continue growing the Metal subtree opportunistically without matching the
  CUDA source boundaries
- return to benchmark work without first making the native subsystem shape
  explicit
- treat the existing thin Metal frontier as sufficient structure for the next
  performance tranche

Impact:

- `T8` becomes the active native-port milestone
- the roadmap now declares a specific mirrored hot-path set and port order
- scaffolded Metal files are acceptable, but only if their status stays
  explicit and they are not confused with compile-active support

Superseded by:

- none

### DEC-0043: Mixed-component upstream rows may use an acceptance-local SIMD-component Metal bridge when the framework-backed row already remains explicit

- Date: `2026-03-09`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)

Decision:

The unchanged vendored upstream `xor` MLE-eval row may prove and verify
through `MetalBackend` by combining the existing acceptance-local
framework-component adapter with a second acceptance-local adapter for
vendored `ComponentProver<SimdBackend>` rows. This is accepted only as a local
acceptance boundary, not as a stable public or upstream-owned support claim.

Context:

After `wide_fibonacci`, `state_machine`, and `blake`, the next honest example
shape was not another framework-only row. The vendored `xor` MLE-eval path
mixes one framework-backed component with one non-framework prover component,
so it was the smallest truthful place to validate whether direct
`MetalBackend` substitution generalized beyond the framework-only adapter.

Alternatives rejected:

- stop after the framework-backed examples and treat mixed-component rows as a
  future concern
- fork the vendored `xor` workload into a bespoke Metal-only harness
- claim the new SIMD-component bridge as a stable shared proving interface

Impact:

- the named upstream example set now proves and verifies through `MetalBackend`
  for every non-blocked row in the current vendored snapshot
- `TD-0019` is now the explicit debt entry for the acceptance-local
  SIMD-component bridge
- the next honest question moves from example onboarding to bridge retirement
  and milestone-closure semantics

Superseded by:

- none

### DEC-0042: Lookup-heavy upstream rows may use a vendored proving-setup seam to preserve the stock Blake transcript flow during MetalBackend substitution

- Date: `2026-03-09`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)

Decision:

The unchanged vendored upstream `blake` example may prove and verify through
`MetalBackend` by exposing a small vendored proving-setup seam that replays the
example’s stock statement-mixing and interaction-element transcript flow, while
keeping workload logic and verifier logic upstream-owned.

Context:

`blake` was the first named row in the acceptance matrix that combined a
lookup-heavy workload with protocol sequencing beyond the earlier
single-trace and multi-tree framework rows. The example’s statement mixing and
interaction-element draws had to stay transcript-faithful during backend
substitution, which made a small vendored setup seam safer than a larger
acceptance-only reconstruction.

Alternatives rejected:

- reconstruct the `blake` proving row in the acceptance crate from many
  private vendored details
- defer `blake` until a stable shared framework-component proving boundary
  exists
- hide the transcript replay requirement inside an undocumented test-local
  hack

Impact:

- `blake` now proves and verifies through `MetalBackend` with unchanged
  workload logic
- the acceptance harness now covers a lookup-heavy upstream row
- the remaining gaps narrow to acceptance-local bridge retirement and the
  upstream `poseidon` blocker

Superseded by:

- none

### DEC-0041: Multi-tree upstream examples may use the same acceptance-local MetalBackend adapter pattern as the first single-trace row

- Date: `2026-03-09`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)

Decision:

The unchanged vendored upstream `state_machine` example now proves and verifies
through direct `MetalBackend` substitution in the acceptance harness. This
reuses the same acceptance-local framework-component adapter pattern as the
first `wide_fibonacci` row, while preserving the upstream protocol sequencing
for multi-tree trace commitments, statement mixing, interaction traces, and
verification.

Context:

After `wide_fibonacci` proved that the first direct backend-substitution row
worked, the next honest question was whether the result generalized beyond a
single-trace component. `state_machine` was the smallest stronger example in
the vendored set because it has multiple trees, multiple framework-backed
components, and upstream-owned statement checks, but it still fits the current
adapter model.

Alternatives rejected:

- stop after the first single-trace example and assume the adapter generalizes
- jump directly to `blake` or `xor` before proving the multi-tree framework
  shape works
- treat the new row as native framework-component support when the adapter is
  still acceptance-local and CPU-domain backed

Impact:

- direct `MetalBackend` acceptance now covers both a single-trace and a
  multi-tree upstream example
- `TD-0017` becomes the main remaining bridge-debt entry for framework-backed
  example substitution
- the next honest example rows are shaped by lookup-heavy and non-framework
  proving surfaces rather than the already-proven basic framework path

Superseded by:

- none

### DEC-0040: The first unchanged upstream example may prove directly through `MetalBackend` via an acceptance-local framework-component adapter

- Date: `2026-03-09`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)

Decision:

The first unchanged vendored upstream `wide_fibonacci` example now proves and
verifies through direct `MetalBackend` substitution in the acceptance harness.
This is accepted through an explicit acceptance-local adapter around vendored
`FrameworkComponent`, which preserves workload semantics and keeps the
remaining CPU-domain constraint-quotient bridge named and local instead of
restoring the earlier outer CPU prove helper.

Context:

After `MetalBackend` satisfied the generic `Backend` and Blake2s
`BackendForChannel` contracts, the remaining blocker for unchanged
framework-backed examples was the upstream `ComponentProver` implementation
surface. Adding `stwo-constraint-framework` directly to the `stwo-metal`
workspace created a nested-workspace conflict, so the smallest
semantics-preserving step was to place the adapter in the acceptance crate
where that dependency already exists cleanly, then prove the first example
through the stock `prove`/`verify` path.

Alternatives rejected:

- keep the old outer CPU prove helper even though direct backend substitution
  had become possible
- claim a stable public `stwo-metal` adapter API before the framework bridge
  proves reusable beyond the first example
- block the first direct `MetalBackend` example until a native Metal
  framework-component implementation exists

Impact:

- `wide_fibonacci` now has a truthful direct `MetalBackend` prove/verify row
  in the acceptance matrix
- `TD-0015` retires because the explicit outer CPU prove bridge is gone
- the remaining blocker narrows to the acceptance-local framework adapter and
  its explicit CPU-domain quotient-evaluation bridge

Superseded by:

- none

### DEC-0039: Blake2s `BackendForChannel` support is accepted through explicit CPU-bridge Merkle and proof-of-work boundaries

- Date: `2026-03-09`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)

Decision:

`MetalBackend` now implements the Blake2s `BackendForChannel` surface through
explicit CPU-bridge `ColumnOps<Blake2sHash>`, lifted Merkle, and proof-of-work
boundaries. This is accepted because it completes the Blake2s proving channel
contract without hiding the fact that the current hash and grind surfaces still
execute on the host side.

Context:

After the lookup tranche, `MetalBackend` satisfied the generic Stwo `Backend`
trait but still could not enter the standard proving API because
`BackendForChannel` remained open. The acceptance harness uses the Blake2s M31
channel, so the smallest useful next slice was the Blake2s-specific
channel-backed surface rather than a broader or speculative hash migration.

Alternatives rejected:

- leave Blake2s channel support unresolved and keep the acceptance bridge
  blocked at the generic `prove` entrypoint
- describe the new channel surface as native Metal hashing while the Merkle and
  grind boundaries are still explicit CPU bridges
- widen channel support before proving the Blake2s acceptance route matters

Impact:

- `MetalBackend` now satisfies both the generic Stwo `Backend` trait and the
  Blake2s `BackendForChannel` surface
- the remaining explicit CPU prove bridge is now blocked by the upstream
  component-prover layer, not by backend or channel infrastructure
- `TD-0015` narrows again to the component-prover acceptance gap

Superseded by:

- none

### DEC-0038: Lookup bridges complete the generic `MetalBackend` contract

- Date: `2026-03-09`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)

Decision:

`MetalBackend` now implements `MleOps` and `GkrOps` through explicit CPU
bridges over Metal-owned multilinear and lookup-layer storage, and it now
declares `impl Backend for MetalBackend {}`. This tranche is accepted because
it closes the remaining generic backend trait gap without changing prover
semantics.

Context:

After `FriOps` landed, the remaining missing generic backend trait was the
lookup layer. The smallest semantics-preserving step was to convert the
multilinear and GKR surfaces into the vendored CPU backend and back, verify
them against the CPU oracle, and then make the backend completion explicit
through a compile-time `Backend` assertion.

Alternatives rejected:

- leave `MetalBackend` trait-complete everywhere except the lookup layer
- skip the explicit `Backend` implementation even after the required trait
  slices existed
- delay the lookup tranche until a native Metal lookup representation exists

Impact:

- `MetalBackend` now satisfies the generic Stwo `Backend` trait
- the next honest proving blocker moved from generic backend completion to the
  Blake2s `BackendForChannel` surface
- acceptance planning can now focus on channel and component-prover seams

Superseded by:

- none

### DEC-0037: `FriOps` is accepted as a bridge-backed `MetalBackend` slice when the fold kernels are Metal-owned and the remaining repacking and decomposition are explicit

- Date: `2026-03-09`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)

Decision:

`MetalBackend` now implements `FriOps`. This slice is accepted even though it
is still bridge-backed, because the fold surface reuses the existing bounded
Metal FRI kernels while keeping the current secure-column repacking and the
`decompose` fallback explicit in the capability model and parity tests.

Context:

After landing `PolyOps`, `AccumulationOps`, and `QuotientOps`, the next honest
missing trait in the Stwo backend contract was `FriOps`. The current backend
column representation does not yet match the packed secure-field representation
used by the bounded Metal FRI helpers, so a fully native trait implementation
would have required a larger representation refactor. The smallest
semantics-preserving step was to expose `FriOps` now, make the repacking rule
explicit, and verify the trait methods directly against the vendored CPU
backend.

Alternatives rejected:

- leave `FriOps` unimplemented until a fully native packed representation
  exists
- implement `FriOps` entirely on the CPU and ignore the bounded Metal fold
  kernels already present in the repo
- describe the new trait slice as direct native Metal support while the
  repacking and `decompose` fallback are still bridge-backed

Impact:

- `MetalBackend` advances to the final missing shared lookup/backend trait gap:
  `GkrOps`
- the acceptance bridge is now blocked by lookup and channel-backed surfaces,
  not by FRI trait absence
- `TD-0015` narrows again toward the remaining `GkrOps` and
  `BackendForChannel` requirements

Superseded by:

- none

### DEC-0036: `MetalBackend` may advance trait-by-trait through explicit CPU bridges so long as each bridge is named and parity-tested

- Date: `2026-03-09`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)

Decision:

`MetalBackend` now advances beyond `ColumnOps` by implementing `PolyOps`,
`AccumulationOps`, and `QuotientOps` through explicit CPU bridges over
Metal-owned columns, secure-column storage, and evaluations. These slices are
accepted because they retire a generic backend gap trait by trait, keep the
bridge explicit in the capability model, and add deterministic parity tests
against the vendored CPU backend.

Context:

The first upstream-example prove/verify acceptance row proved that native Metal
trace generation can feed an unchanged vendored workload, but the proving path
still depended on a broad CPU prover bridge. The smallest semantics-preserving
way to shrink that bridge was not another example harness; it was to implement
the next shared backend traits in order. `PolyOps`, `AccumulationOps`, and
`QuotientOps` were the first honest tranche because they widen backend
coverage without overclaiming direct end-to-end Metal proving support.

Alternatives rejected:

- keep the CPU bridge broad and unnamed until every remaining backend trait is
  ready
- add more upstream-example acceptance rows before shared backend support
  materially widens
- describe the new slices as native Metal support when they still rely on the
  vendored CPU backend for the underlying trait execution

Impact:

- `MetalBackend` now satisfies more of the generic Stwo backend contract
  without changing proof semantics
- the capability model now distinguishes the new bridge-backed traits
- `TD-0015` narrows from a broad backend gap toward the remaining
  `FriOps`/`GkrOps`/channel-backed requirements
- the next honest tranche is `FriOps`, not another workload-specific proving
  seam

Superseded by:

- none

### DEC-0001: Reset `stwo-metal` documentation to a process-only clean slate

- Date: `2026-03-09`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - none

Decision:

The copied CUDA-era documentation archive is removed from `docs/`. The active
documentation set is now limited to current process control, planning, review,
decision, debt, and template documents.

Context:

`stwo-metal` started as a direct copy of `stwo-cuda`. Keeping milestone history,
benchmark archives, and CUDA restoration notes in the active docs surface would
blur planning and misstate current project goals.

Alternatives rejected:

- preserve the copied archive under `docs/` as reference history
- keep a mixed tree with both active process docs and stale milestone notes

Impact:

- contributors now have one clean process surface
- historical CUDA development is no longer treated as active guidance
- new project decisions start from `DEC-0001`

Superseded by:

- none

### DEC-0033: The upstream example set is pinned locally, and the first T7 implementation slice starts with unchanged wide-fibonacci example wiring

- Date: `2026-03-09`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)

Decision:

The upstream `stwo-examples` source is now pinned locally inside the vendored
snapshot, with recorded source provenance. The first `T7` implementation slice
uses the unchanged upstream `wide_fibonacci` example to feed the current native
Metal trace boundary through an isolated acceptance fixture rather than a
benchmark-specific proving row.

Context:

After the roadmap correction, the next missing input was an auditable local copy
of the upstream acceptance workloads. The next missing implementation seam was
an example-backed slice that proved backend wiring could consume an unchanged
upstream workload without introducing another bespoke benchmark harness.

Alternatives rejected:

- continue with the acceptance set only as a documentation promise
- wire the first `T7` slice through another standalone benchmark instead of a
  vendored example
- claim end-to-end example proving before `MetalBackend` satisfies the full
  Stwo `Backend` contract

Impact:

- `TD-0013` is retired because the example source is now pinned locally
- `T7` moves from planned to in-progress
- the first example-backed execution evidence is trace-boundary parity for the
  vendored upstream `wide_fibonacci` example
- the next honest example-backed tranche is the first prove/verify boundary,
  not more bespoke trace plumbing

Superseded by:

- none

### DEC-0034: The first T7 prove/verify boundary uses native Metal trace generation with an explicit CPU prover bridge

- Date: `2026-03-09`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)

Decision:

The first honest prove/verify boundary for `T7` is now the unchanged vendored
upstream `wide_fibonacci` component driven by native Metal trace generation and
then bridged explicitly into the stock CPU prover and verifier. This is
accepted as real progress because the workload logic stays upstream-owned and
the bridge is named as a temporary backend-completion gap rather than hidden as
direct `MetalBackend` support.

Context:

After vendoring the upstream example set and validating trace parity, the next
missing seam was the first prove/verify boundary from unchanged example code.
`MetalBackend` still does not satisfy the full Stwo `BackendForChannel`
contract, so direct backend substitution was not yet truthful. The smallest
semantics-preserving step was to reuse the native Metal trace path, materialize
that trace into ordinary CPU circle evaluations, and run the stock prover and
verifier unchanged.

Alternatives rejected:

- keep T7 at trace-only acceptance without any prove/verify execution evidence
- add another benchmark-specific proving harness instead of using the vendored
  upstream example component
- describe the new hybrid boundary as direct `MetalBackend` support before the
  backend trait contract exists

Impact:

- `wide_fibonacci` moves to in-progress in the acceptance matrix with a real
  prove/verify boundary
- the current example-backed proving path is explicitly hybrid and bridge-backed
- `TD-0015` is introduced to track retirement of the CPU prover bridge
- the next honest tranche is to generalize the acceptance harness and begin the
  backend-completion slice that can retire that bridge

Superseded by:

- none

### DEC-0035: Single-trace Blake2s upstream acceptance rows use one shared CPU-bridge harness

- Date: `2026-03-09`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)

Decision:

The acceptance crate now exposes one shared single-trace Blake2s CPU-bridge
prove/verify helper. Upstream-example acceptance rows that still need the CPU
prover bridge should use that helper instead of embedding bespoke proving code
inside each test file.

Context:

Once the first `wide_fibonacci` prove/verify boundary existed, the next risk
was accidental proliferation of near-duplicate hybrid harnesses. That would
slow future example onboarding and make it harder to tell whether a new row was
exercising shared backend support or just adding another test-local bridge.

Alternatives rejected:

- keep the first prove/verify wiring entirely local to one test file
- add a wide-fibonacci-specific helper instead of a reusable single-trace
  bridge harness
- move on to a second example before extracting the shared acceptance pattern

Impact:

- acceptance harness logic is now reusable for future single-trace Blake2s
  upstream examples
- the next honest tranche is no longer harness cleanup; it is the first direct
  backend-completion slice at the `PolyOps` boundary that can retire the CPU
  prove bridge
- `wide_fibonacci` remains the only example using the shared helper so far

Superseded by:

- none

### DEC-0032: The primary deliverable is generic Stwo proving with `MetalBackend`, and upstream examples are the acceptance workloads

- Date: `2026-03-09`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)

Decision:

`stwo-metal` is rebaselined around one primary deliverable: prove Stwo traces
with `MetalBackend` and verify them through the standard verifier, without
rewriting workload logic beyond backend wiring. Upstream Stwo examples are the
target acceptance workloads for that deliverable.

Context:

The project had started to drift toward benchmark-first proving rows, especially
around wide-fibonacci. Those bounded slices produced useful primitives, but
they are not the product definition. The actual goal is backend completion
against unchanged Stwo workloads, with benchmark rows used only as supporting
performance evidence once the backend path is truthful.

Alternatives rejected:

- continue using benchmark-specific proving rows as the primary milestone driver
- treat workload-specific rewrites as acceptable substitutes for backend
  completeness
- leave the acceptance target implicit instead of naming upstream examples and
  backend wiring as the intended seam

Impact:

- the controller, roadmap, program plan, and done criteria are rebaselined
  around generic backend completion
- a new formal milestone is opened for proving upstream examples with
  `MetalBackend` unchanged except for backend wiring
- further bespoke benchmark-path expansion is frozen until this correction is
  written down
- the benchmark north star remains in the plan, but no longer defines
  completion by itself

Superseded by:

- none

### DEC-0031: Wide-fibonacci quotient accumulation is now native Metal, and the remaining benchmark bridge moves to pre-FRI PCS commitment

- Date: `2026-03-09`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)

Decision:

`stwo-metal` now treats bounded wide-fibonacci quotient accumulation as a
native `.metal` proving primitive with deterministic CPU-oracle parity. The
remaining `wide_fibonacci_prove` bridge is no longer at trace generation; it is
now the explicit Metal-to-CUDA quotient-output handoff before pre-FRI PCS
commitment and the rest of the inherited proving lane.

Context:

The benchmark north-star row was still starting in native Metal and then
crossing back into CUDA before quotient work. That made quotient accumulation
the most leverageful next replacement because it removes more of the remaining
bridge while staying narrow enough to validate directly against the vendored CPU
algebra. The new primitive is intentionally bounded to the wide-fibonacci row
and does not yet claim a general quotient backend for every workload.

Alternatives rejected:

- keep quotient accumulation planned-only and move directly to pre-FRI PCS
  commitment
- declare generic quotient support before a benchmark-aligned bounded primitive
  exists
- hide the remaining output bridge inside the benchmark component without
  logging that the seam moved

Impact:

- `wide_fibonacci_prove` now enters native Metal for both trace generation and
  quotient accumulation
- the next honest benchmark-aligned tranche is pre-FRI PCS commitment, not
  another quotient placeholder
- `TD-0012` narrows from a trace bridge to a quotient-output bridge
- `TD-0011` remains active because the declared workload surface still begins
  from a CPU-owned quotient evaluation even though one benchmark row no longer
  does

Superseded by:

- none

### DEC-0002: Public repository identity moves to `stwo-metal` before backend replacement

- Date: `2026-03-09`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - none

Decision:

The Cargo and repository identity use `stwo-metal` and `stwo-metal-sys` now,
even though internal implementation vocabulary still contains inherited CUDA
names until a backend-neutral boundary is designed.

Context:

The public project surface needed to become truthful immediately, but a full
internal rename before design work would create churn without clarifying the
target architecture.

Alternatives rejected:

- keep the public crate names as `stwo-cuda`
- force a complete internal rename before the Metal boundary is specified

Impact:

- external planning and fixtures use the new project identity
- internal CUDA naming is now explicit migration debt rather than ambient repo
  identity

Superseded by:

- none

### DEC-0003: Default roadmap direction is Rust host orchestration plus native Metal plus `.metal` kernels

- Date: `2026-03-09`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - none

Decision:

The default planning direction for `stwo-metal` is:

- Rust frontend and host orchestration
- native Metal runtime ownership on the host
- `.metal` compute kernels for the hot path

Deterministic unit tests against the local vendored Stwo CPU execution are the
default correctness oracle for bounded Metal work.

Context:

The most direct route to a truthful and high-performance Apple Silicon backend
is to stay close to native Metal semantics, write hot kernels in `.metal`, and
validate each bounded cut against the vendored CPU reference before caring about
throughput.

Alternatives rejected:

- treating non-native GPU stacks as part of the default roadmap
- treating benchmarks as the first correctness signal
- introducing a second semantic authority beside the vendored CPU reference

Impact:

- the roadmap now plans around a native Metal host and `.metal` kernel path
- deterministic CPU-reference parity is part of the default path, not optional
- the next required design work is the host contract and native runtime
  boundary, not broad kernel experimentation

Superseded by:

- none

### DEC-0004: T2 and T3 will execute against a single formal basis and a single correctness oracle

- Date: `2026-03-09`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)

Decision:

The formal basis for T2 and T3 is
[`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md).
New bounded Metal work must validate against the local vendored Stwo CPU
execution as its correctness oracle.

Context:

The project needed a concrete governing document for host modes, runtime
ownership, `.metal` compilation, runtime loading, and deterministic validation
rules before implementation starts.

Alternatives rejected:

- begin T2/T3 implementation from roadmap prose alone
- allow each primitive cut to invent its own validation oracle
- mix performance-first experimentation into the first runtime boundary cuts

Impact:

- T2 and T3 now have a single formal design basis
- vendored CPU execution is the default correctness authority for bounded Metal
  work
- future implementation cuts can be reviewed against explicit host and runtime
  contracts

Superseded by:

- none

### DEC-0005: `stwo-metal` adopts upstream Stwo skills as process guidance for domain vocabulary and review discipline

- Date: `2026-03-09`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)

Decision:

`stwo-metal` will use the upstream Stwo skill registry as process guidance for
domain vernacular, testing focus, and soundness review discipline. This guides
how we plan and review work; it does not replace the local vendored CPU oracle
or the local design documents.

Context:

The upstream Stwo repo maintains a skill registry and focused guides for Rust
conventions, testing strategy, soundness review, and mathematical domains. We
want those practices and vocabulary in `stwo-metal` without importing the old
`stwo-cuda` planning noise.

Alternatives rejected:

- keep `stwo-metal` process language independent from upstream Stwo practices
- rely on informal memory of upstream conventions instead of naming them in the
  plan

Impact:

- roadmap and design notes can cite the upstream skill registry as process input
- PR review now has an explicit upstream testing and soundness alignment rule
- future theory-grounded work should load the relevant upstream skill before
  making changes

Superseded by:

- none

### DEC-0006: `DN-0001` is accepted and governs the initial Metal runtime implementation

- Date: `2026-03-09`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)

Decision:

`DN-0001` is no longer draft planning material. It is accepted as the governing
host and runtime contract for the initial `stwo-metal-sys` Metal build and
execution path.

Context:

The repository now contains a native Metal build pipeline, embedded `.metallib`
loading, runtime ownership inside `stwo-metal-sys`, and deterministic parity
tests for the first bounded Metal primitive. The design note is now describing
implemented behavior, not speculative structure.

Alternatives rejected:

- keep the design note in draft state while implementation is already relying
  on it
- allow the initial Metal runtime to evolve without an accepted governing note

Impact:

- T2 and T3 are now governed by an accepted design note
- runtime implementation and future review can treat `DN-0001` as binding
  project contract

Superseded by:

- none

### DEC-0007: The first bounded Metal primitive is `BaseField` bit reversal with vendored CPU parity

- Date: `2026-03-09`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)

Decision:

The first completed T4 slice is a native Metal `BaseField` bit-reversal path
with deterministic parity tests against the vendored CPU implementation.

Context:

The project needed a narrow primitive that exercised the real Metal build,
library-loading, buffer, and kernel-dispatch boundary without forcing a false
claim that the full proving backend was already ported.

Alternatives rejected:

- start with a broader quotient, FRI, or trace-generation primitive
- claim full Metal backend support before any bounded primitive had parity
  validation

Impact:

- T4 is complete
- the active tranche can move from planning and runtime setup into bounded
  proving-surface growth
- the next bounded slices are `SecureField` columns and one poly or FRI support
  primitive

Superseded by:

- none

### DEC-0008: `SecureField` column support is part of the supported bounded Metal surface

- Date: `2026-03-09`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)

Decision:

`SecureField` column upload, mutation, readback, and bit reversal are now part
of the supported bounded Metal surface, with deterministic parity tests against
the vendored CPU implementation.

Context:

The first `BaseField` slice was not enough to support future proving-path work.
The next stable shape needed to preserve `SecureField` column semantics before
growing into FRI and trace-support work.

Alternatives rejected:

- keep `SecureField` support outside the bounded Metal claim
- skip parity tests and rely on the `BaseField` lane as a proxy

Impact:

- bounded Metal column support now includes both `BaseField` and `SecureField`
- future proving-path work can build on stable secure-field vector semantics

Superseded by:

- none

### DEC-0009: The first bounded poly-support Metal primitive is coset-order to circle-domain bit-reversed permutation

- Date: `2026-03-09`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)

Decision:

The first bounded poly-support primitive on the Metal lane is `BaseField`
coset-order to circle-domain bit-reversed permutation, implemented as a native
Metal kernel with deterministic vendored CPU parity.

Context:

The project needed one poly-adjacent primitive that was smaller and safer than
FRI folding or interpolation, but still directly useful for later polynomial
and evaluation-order work.

Alternatives rejected:

- jump straight from column support to FRI folding arithmetic
- treat order permutation as implementation trivia instead of a named bounded
  contract

Impact:

- T5 now has a bounded poly-support primitive to build on
- the next step can focus on choosing the first declared proving sub-path and
  the next required FRI or trace-support slice

Superseded by:

- none

### DEC-0010: The first declared T5 proving sub-path candidate is the FRI first-layer fold

- Date: `2026-03-09`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)

Decision:

The first declared T5 proving sub-path candidate is the FRI first-layer fold
from a bit-reversed secure circle evaluation into the first line layer.

Context:

The project needed a proving-path target that is part of the current Stwo flow,
smaller than full interpolation or trace generation, and directly useful for
growing toward a truthful bounded Metal row.

Alternatives rejected:

- use deprecated FRI decomposition as the first declared proving sub-path
- jump directly to a wider trace-generation or quotient path

Impact:

- the next bounded Metal primitive is fixed:
  first-layer `fold_circle_into_line`
- future T5 work can be evaluated against one explicit proving-path target

Superseded by:

- none

### DEC-0011: The bounded FRI arithmetic surface now includes first-layer fold and line fold

- Date: `2026-03-09`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)

Decision:

The supported bounded Metal FRI surface now includes:

- first-layer `fold_circle_into_line` from secure circle evaluation into line
  evaluation
- repeated `fold_line` over line evaluations through native Metal kernels with
  deterministic vendored CPU parity

Context:

The first declared T5 proving sub-path could not move beyond planning until the
two arithmetic folds used by `FriProver::commit_inner_layers` existed on the
Metal lane with explicit CPU-oracle validation.

Alternatives rejected:

- stop after the first circle-to-line fold and treat the remaining line folds
  as future trivia
- widen the claim to a full FRI prover path before the repeated line fold was
  parity-tested

Impact:

- the bounded Metal lane now covers the core FRI arithmetic transitions for the
  first declared T5 candidate
- the next honest gap is no longer arithmetic; it is the proving-facing
  materialization and commitment boundary after the folds

Superseded by:

- none

### DEC-0012: The next T5 boundary after bounded FRI arithmetic is explicit line-evaluation handoff

- Date: `2026-03-09`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)

Decision:

The next required T5 boundary is an explicit handoff from the bounded Metal FRI
arithmetic surface into the first inner FRI-layer commitment. That handoff must
name whether the first proving row uses an explicit CPU Merkle commitment
bridge or a native Metal commitment boundary.

Context:

After first-layer fold and line fold landed, the remaining blocker in the first
declared T5 path is not another arithmetic kernel. It is how the folded line
values become a proving-facing `LineEvaluation` and commitment input without
hiding CPU fallback behavior.

Alternatives rejected:

- keep adding arithmetic kernels without freezing the proving-facing handoff
- blur CPU readback or commitment fallback into the supported Metal story

Impact:

- T5 sequencing now points at the next real contract boundary
- any future CPU bridge in this area must be explicit, bounded, and logged as
  debt rather than implicit support

Superseded by:

- none

### DEC-0013: The first inner FRI-layer commitment handoff is implemented only as an explicit CPU bridge

- Date: `2026-03-09`
- Status: `superseded`
- Owners: `project team`
- Related design note:
  - [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)

Decision:

The current proving-facing boundary after bounded Metal FRI arithmetic is an
explicit CPU bridge:

- Metal line values are materialized into `LineEvaluation<CpuBackend>`
- the first inner-layer Merkle commitment is computed on that CPU view

Context:

The bounded Metal lane now covers the arithmetic required to reach the first
inner FRI layer, but it does not yet own a native `SecureColumnByCoords` or
Merkle commitment surface. The project still needs a truthful handoff so work
can keep moving without silently claiming native commitment support.

Alternatives rejected:

- hide the CPU readback and commitment path behind generic conversions
- claim the first inner-layer commitment is already native Metal

Impact:

- the first inner-layer commitment boundary is now explicit, testable, and
  support-honest
- the next T5 boundary is the native replacement for this CPU bridge

Superseded by:

- `DEC-0014`

### DEC-0014: The first inner FRI-layer commitment now has a native `stwo-metal` boundary

- Date: `2026-03-09`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)

Decision:

The first inner FRI-layer commitment is no longer defined only by a CPU bridge.
`stwo-metal` now owns a native proving-facing boundary consisting of:

- `MetalLineEvaluation` for folded line-domain values
- a native `stwo-metal` commitment object built directly from that line
  evaluation

This boundary must continue to match the vendored CPU Merkle root for the same
folded values.

Context:

The explicit CPU bridge was sufficient to keep the first commitment step honest,
but it was not a stable or truthful long-term boundary for the Metal proving
path. The project needed a direct `stwo-metal` commitment surface before
growing into decommitment or wider proving support.

Alternatives rejected:

- keep the CPU bridge as the only proving-facing commitment path
- claim a GPU-side Merkle pipeline before a stable native host boundary exists

Impact:

- the first inner-layer commitment root can now be produced without crossing
  into `CpuBackend`
- the explicit CPU bridge is retained only as a bounded validation surface
- the next T5 boundary is native query and decommit support on top of this
  commitment object

Superseded by:

- none

### DEC-0015: The first inner FRI layer now has a native decommit boundary

- Date: `2026-03-09`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)

Decision:

The native `stwo-metal` line-commitment surface now supports first inner-layer
query and decommit operations that mirror the vendored CPU FRI semantics for:

- grouped decommitment positions
- omitted-value `fri_witness` ordering
- Merkle hash witness ordering

Context:

After the native first inner-layer commitment boundary landed, the remaining
gap in that layer was the ability to answer FRI queries and produce a truthful
decommitment without crossing back through the CPU bridge.

Alternatives rejected:

- keep the CPU bridge as the only decommitment path
- expose a custom witness format that diverges from the existing FRI proof
  model

Impact:

- the first inner FRI layer now has native commit and decommit support
- the next T5 boundary is to package that surface into a bounded proof-facing
  row

Superseded by:

- none

### DEC-0016: The first inner FRI layer is now packaged as a bounded native proof row

- Date: `2026-03-09`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)

Decision:

The native first inner FRI layer is now exposed as a bounded proof-facing row
with:

- a stable commitment root
- a stable fold step
- a stable decommit API that returns the vendored FRI layer proof shape

Context:

Raw native commit and decommit helpers were not yet a truthful proving-facing
surface. The project needed one explicit row abstraction before it could grow
into a multi-layer native inner FRI sequence.

Alternatives rejected:

- keep the native inner-layer surface as loose functions only
- jump straight to a wider sequence API before the first row had a stable
  boundary

Impact:

- the first inner FRI layer now has a single proof-facing row abstraction
- the next T5 boundary is the bounded native inner-layer sequence

Superseded by:

- none

### DEC-0017: The native first inner FRI row now extends to a bounded inner-layer sequence

- Date: `2026-03-09`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)

Decision:

`stwo-metal` now packages the native first inner FRI row into a bounded
inner-layer sequence that owns:

- the ordered native inner-layer rows
- the native decommit path for each row
- the resulting last line evaluation after the configured fold schedule

Context:

One isolated proof-facing row was not yet enough to represent the bounded FRI
commit progression used by the prover. The project needed a deterministic
sequence abstraction that mirrors the vendored CPU fold schedule and carries the
final line evaluation forward.

Alternatives rejected:

- stop at a single native row and leave multi-layer sequencing to ad hoc caller
  logic
- widen the claim to a full FRI commitment slice before the inner-layer
  schedule itself was explicit

Impact:

- the bounded native Metal FRI surface now covers the inner-layer schedule, not
  just one row
- the next T5 boundary is the bounded FRI commitment slice

Superseded by:

- none

### DEC-0018: The bounded native inner-layer sequence now extends to a bounded FRI commitment slice

- Date: `2026-03-09`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)

Decision:

`stwo-metal` now packages the bounded native inner-layer FRI sequence into a
bounded FRI commitment slice that owns:

- the ordered inner-layer sequence
- the bounded last-layer polynomial
- the explicit last-layer degree-bound truncation contract

Context:

The native inner-layer sequence carried the correct folded evaluations, but it
was not yet a truthful commitment-stage boundary because the last-layer
polynomial contract remained implicit. The project needed one explicit slice
that mirrors the vendored CPU commitment stage up to, but not including, the
first-layer proof surface.

Alternatives rejected:

- stop at the inner-layer sequence and leave last-layer interpolation implicit
  in caller code
- widen the claim to a full FRI proof before the bounded commitment-stage slice
  was explicit

Impact:

- the bounded Metal FRI lane now has an explicit commitment-stage slice
- last-layer degree-bound truncation is now part of the proof-facing contract,
  not an ambient implementation detail
- the next T5 boundary is a bounded proof-facing inner FRI proof slice

Superseded by:

- none

### DEC-0019: The bounded FRI commitment slice now extends to an inner proof-facing FRI proof slice

- Date: `2026-03-09`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)

Decision:

`stwo-metal` now packages the bounded FRI commitment slice into an honest
inner proof-facing FRI proof slice that owns:

- the bounded inner-layer proofs
- the bounded last-layer polynomial
- an explicit contract that first-layer FRI proof support is still out of scope

Context:

The commitment slice made the last-layer polynomial explicit, but callers still
needed to assemble proof-shaped inner-layer output themselves. The project
needed one stable proof-facing wrapper for the bounded inner FRI surface
without implying that the first-layer circle commitment path already exists.

Alternatives rejected:

- stop at the commitment slice and leave proof-shaped inner-layer packaging to
  ad hoc caller code
- expose the result as a full `FriProof` even though the first layer is not yet
  present

Impact:

- the bounded Metal lane now has an honest inner proof-facing FRI slice
- the next T5 boundary is the first-layer circle commitment and decommit
  surface needed to form a bounded full FRI proof candidate

Superseded by:

- none

### DEC-0020: The bounded Metal lane now includes a first-layer FRI proof boundary

- Date: `2026-03-09`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)

Decision:

`stwo-metal` now owns a bounded first-layer FRI proof boundary consisting of:

- a first-layer circle commitment root
- a first-layer decommit API that mirrors the vendored FRI proof shape
- a fold handoff from the committed first layer into the native Metal line lane

Context:

The inner proof slice made the later FRI layers explicit, but the bounded Metal
path still lacked the first-layer proof boundary needed before a truthful full
FRI proof candidate could exist.

Alternatives rejected:

- keep the first-layer proof boundary implicit in test scaffolding
- widen the claim to a full FRI proof before the first-layer proof surface was
  explicit

Impact:

- the bounded Metal lane now covers both sides of the FRI proof split:
  first-layer proof and inner-layer proof
- the next T5 boundary is to compose those pieces into a bounded full FRI proof
  candidate

Superseded by:

- none

### DEC-0021: The bounded Metal FRI lane now packages a full proof candidate shape

- Date: `2026-03-09`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)

Decision:

`stwo-metal` now packages the bounded first-layer proof boundary and the
bounded inner proof slice into the vendored `ExtendedFriProof` shape for one
explicit FRI lane.

This decision does not claim a full proving-path integration yet. The bounded
proof candidate still takes caller-supplied folding alphas and still relies on
host-owned hashing plus a CPU-bridged last-layer interpolation boundary.

Context:

Once both proof halves existed, the project needed one honest wrapper that
matched the vendored FRI proof structure so later work can integrate a declared
proving sub-path without inventing another proof representation.

Alternatives rejected:

- keep the first-layer and inner-layer proof surfaces separate and force callers
  to assemble the vendored proof shape manually
- claim a truthful end-to-end proving path before transcript ownership and
  declared sub-path integration exist

Impact:

- the bounded Metal lane now has a full FRI proof candidate shape
- the next T5 boundary is one declared proving sub-path that consumes it with
  explicit transcript and unsupported-edge accounting

Superseded by:

- none

### DEC-0022: The bounded Metal FRI lane now owns transcript-driven commit and decommit

- Date: `2026-03-09`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)

Decision:

`stwo-metal` now exposes a bounded transcript-owned FRI prover that:

- mixes first-layer and inner-layer roots in vendored order
- draws folding alphas from the channel instead of taking them from the caller
- mixes the bounded last-layer polynomial into the channel
- draws query positions from that same transcript and returns the vendored
  `FriDecommitResult` / `ExtendedFriProof` shapes

Context:

The bounded full FRI proof candidate had the right proof shape, but it still
depended on caller-supplied folding challenges. The project needed a truthful
transcript boundary before the bounded Metal lane could be treated as a proving
sub-path rather than a proof-packaging helper.

Alternatives rejected:

- keep the bounded proof candidate as a caller-driven challenge helper
- add a transcript wrapper that returned a custom local proof result instead of
  the vendored FRI result shapes

Impact:

- the bounded Metal FRI lane now owns transcript-driven commit and decommit
- `TD-0009` is retired for the bounded FRI lane
- the next T5 boundary is one declared Stwo proving sub-path that consumes this
  transcript-owned lane and names the remaining unsupported edges

Superseded by:

- none

### DEC-0023: The bounded Metal FRI lane now has one declared Blake2s sub-path

- Date: `2026-03-09`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)

Decision:

`stwo-metal` now exposes one declared bounded proving sub-path:

- a Blake2s-backed FRI sub-path built on the transcript-owned Metal FRI prover

This declared sub-path is intentionally narrow. It does not yet claim quotient,
trace, PCS, or end-to-end workload support.

Context:

The transcript-owned prover made the FRI lane truthful, but the project still
needed one named entry point that represented the currently supported path
rather than forcing callers to compose low-level pieces themselves.

Alternatives rejected:

- keep the bounded FRI lane only as low-level building blocks
- declare a broader Stwo workload path before quotient, trace, and PCS
  ownership are explicit

Impact:

- `stwo-metal` now has one declared bounded proving sub-path
- the next T5 boundary is to bind that declared FRI sub-path into one declared
  Stwo workload boundary
- `TD-0010` records that the declared sub-path is still FRI-only

Superseded by:

- none

### DEC-0024: The first declared Stwo workload boundary is an explicit hybrid FRI boundary

- Date: `2026-03-09`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)

Decision:

`stwo-metal` now exposes a manifest-driven workload planning and boundary layer
for exemplar Stwo workloads. The first declared workload boundary is explicitly
hybrid:

- the bounded Blake2s FRI sub-path is Metal-owned
- witness, quotient, and PCS stages remain explicitly CPU-owned or
  not-applicable per workload
- the supported plans are named as `CpuOnly`, `MetalFriHybrid`, or
  `MetalFull`

Context:

The bounded Metal FRI lane now had a truthful proving sub-path, but the
project still lacked a stable workload contract describing where that sub-path
fits into a real Stwo workload. The next safe step was to make stage ownership
explicit instead of silently widening the support claim.

Alternatives rejected:

- keep the declared FRI sub-path disconnected from workload planning
- declare a full workload path before witness, quotient, and PCS ownership had
  been written down
- blur hybrid execution into a generic "Metal-supported" label

Impact:

- exemplar workloads now have explicit planning and stage-ownership contracts
- `TD-0010` is retired because the declared FRI sub-path now sits inside one
  declared Stwo workload boundary
- the next T5 boundary is an executable handoff from a CPU-owned workload
  artifact into that declared hybrid boundary

Superseded by:

- none

### DEC-0025: The first executable workload handoff starts at a CPU-owned FRI-ready evaluation

- Date: `2026-03-09`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)

Decision:

The first executable workload handoff for the declared hybrid workload
boundary is:

- a CPU-owned `SecureEvaluation<CpuBackend, BitReversedOrder>`
- on a canonic circle domain
- materialized into a `MetalSecureFieldVec` only at the explicit workload
  handoff boundary

The workload API names this as a FRI-ready evaluation handoff. It does not
claim witness, quotient, or PCS generation has moved.

Context:

The hybrid workload boundary made stage ownership explicit, but callers still
had to pass an already-uploaded Metal column with no named contract. The next
truthful step was to freeze the earliest executable handoff we currently own
without pretending earlier workload stages are already ported.

Alternatives rejected:

- keep taking raw `MetalSecureFieldVec` inputs with no workload-handoff contract
- claim witness, quotient, or PCS ownership had already crossed into the
  executable Metal path

Impact:

- `stwo-metal` now has an executable workload handoff with deterministic
  CPU-oracle parity
- the remaining workload gap is narrower and explicit: the executable boundary
  still starts after witness, quotient, and PCS preparation
- `TD-0011` remains active for that earlier-stage gap

Superseded by:

- none

### DEC-0026: The first quotient-owned workload handoff is explicit and executable

- Date: `2026-03-09`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)

Decision:

`stwo-metal` now exposes an explicit CPU-owned quotient evaluation handoff for
the declared hybrid workload boundary. The executable Metal workload can now
start from:

- a CPU-owned quotient evaluation on a canonic circle domain
- a named workload boundary for `fibonacci_example`
- deterministic parity against the vendored CPU FRI prover on that quotient
  artifact

Context:

The prior executable handoff started at a generic FRI-ready evaluation. The
next upstream-aligned step was to name the quotient-to-FRI boundary directly,
because that is the actual proving seam used by the vendored PCS flow.

Alternatives rejected:

- keep the executable workload handoff only as a generic FRI-ready evaluation
- claim native quotient accumulation before an explicit quotient-owned handoff
  existed

Impact:

- the executable workload boundary now uses quotient terminology aligned to the
  vendored prover flow
- the next remaining workload gap is earlier than quotient evaluation:
  witness-owned artifacts and quotient accumulation still stay outside the
  executable Metal path
- `TD-0011` narrows to that earlier-stage gap

Superseded by:

- none

### DEC-0027: The first benchmark north star is the wide-fibonacci log-size-20 row

- Date: `2026-03-09`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)

Decision:

The first explicit benchmark objective for `stwo-metal` is the wide-fibonacci
row represented by the existing standalone fixtures:

- benchmark ids:
  `wide_fibonacci_trace_generation_v1` and `wide_fibonacci_prove_verify_v1`
- `log_n_instances = 20`
- `n_columns = 100`
- project-supplied reference goal: approach `90 ms`
- reference platform label: RTX 4090 CUDA history

This is recorded as a benchmark north star, not as a correctness oracle and
not as a present support claim for Metal.

Context:

The project needed its performance direction pinned to one named workload row
instead of a generic promise to be "fast on Metal". The existing standalone
benchmark fixtures already encode the relevant workload dimensions, so the next
clean step was to make that objective explicit in the planning and boundary
surface.

Alternatives rejected:

- keep the benchmark target implicit in fixture defaults and script names
- treat the benchmark number as a correctness gate for early Metal slices
- declare a wider benchmark portfolio before one north star row was fixed

Impact:

- the benchmark-facing objective is now explicit in code and docs
- workload planning can prioritize the wide-fibonacci witness and prove path
  intentionally
- the remaining benchmark gap is explicit: the prove benchmark still remains
  outside the native Metal lane

Superseded by:

- none

### DEC-0028: Wide-fibonacci trace generation enters the Metal lane through a native `.metal` kernel and retargeted trace fixture

- Date: `2026-03-09`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)

Decision:

The bounded wide-fibonacci witness and trace-generation boundary now enters the
Metal lane through a native `.metal` kernel that writes a contiguous
column-major trace buffer. The standalone `wide_fibonacci_trace` benchmark
fixture is retargeted to that Metal path and now reports `STWO_METAL_MODE`
explicitly in its runner metadata.

Context:

The declared benchmark north star was still entering through the inherited CUDA
witness and trace-generation path, which made the trace benchmark misleading as
an indicator of Metal bring-up progress. The narrowest truthful cut was to move
trace generation first, keep the output layout explicit, and validate the
native result against a deterministic CPU recurrence.

Alternatives rejected:

- leave the trace benchmark on the CUDA path until the full prove benchmark is
  native
- emulate the old per-column CUDA pointer shape instead of declaring one
  contiguous Metal trace layout
- claim a full Metal prove benchmark before the earlier proving seams are
  actually replaced

Impact:

- the bounded Metal surface now includes native wide-fibonacci trace generation
- the standalone trace benchmark measures a real Metal `.metal` trace path
- the remaining benchmark and workload gap narrows to the earlier prove-path
  seams, starting with `wide_fibonacci_prove`

Superseded by:

- none

### DEC-0029: The wide-fibonacci prove benchmark now starts from native Metal trace generation and an explicit Metal-to-CUDA bridge

- Date: `2026-03-09`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)

Decision:

`wide_fibonacci_prove` now generates its main trace through the native Metal
wide-fibonacci trace boundary and then crosses an explicit bridge into the
existing `CudaBackend` trace-evaluation row. The benchmark no longer uses the
inherited CUDA trace-generation path, but it also does not pretend the rest of
the proving lane is native Metal yet.

Context:

After retargeting the standalone trace benchmark, the next honest benchmark
slice was to move the prove runner’s trace-generation phase onto the same Metal
boundary. The rest of the prove runner still depends on inherited
CUDA-prover-owned interpolation, commitment, quotient, and proof generation, so
the bridge needed to be explicit and narrow.

Alternatives rejected:

- leave `wide_fibonacci_prove` fully on the inherited CUDA trace path
- claim the full prove benchmark is native Metal after only replacing trace
  generation
- widen the bridge implicitly by hiding the Metal-to-CUDA handoff inside an
  unnamed helper

Impact:

- the prove benchmark now measures a real native Metal trace-generation phase
- the remaining benchmark gap is narrower and explicitly named:
  the Metal-to-CUDA trace-evaluation bridge and the earlier proving seams
- the next honest tranche is to move the workload boundary earlier than that
  bridge, before quotient accumulation

Superseded by:

- none

### DEC-0030: Quotient accumulation is the next native proving-stage replacement after the witness-owned handoff

- Date: `2026-03-09`
- Status: `accepted`
- Owners: `project team`
- Related design note:
  - [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md)

Decision:

After defining the workload-level CPU-owned wide-fibonacci witness handoff that
feeds the native Metal trace boundary, the next native replacement target is
quotient accumulation, not pre-FRI PCS commitment.

Context:

The earlier proving seam is now explicit: CPU-owned witness inputs feed native
Metal trace generation before the prove benchmark bridges back into the
inherited CUDA lane. The two plausible next boundaries were quotient
accumulation and pre-FRI PCS commitment. Quotient accumulation is earlier in
the prover flow, removes more of the remaining Metal-to-CUDA bridge, and
matches the existing explicit quotient-evaluation handoff already present in
the workload surface.

Alternatives rejected:

- jump to pre-FRI PCS commitment before quotient accumulation is native
- leave the next native replacement unspecified after landing the witness
  handoff

Impact:

- the next honest tranche is fixed: native quotient accumulation on top of the
  witness-owned handoff
- pre-FRI PCS commitment remains explicit future work, not the immediate next
  target
- `TD-0011` narrows to the quotient-accumulation gap instead of a generic
  earlier-witness gap

Superseded by:

- none
