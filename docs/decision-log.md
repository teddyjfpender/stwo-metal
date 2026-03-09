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
