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
