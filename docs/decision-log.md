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
