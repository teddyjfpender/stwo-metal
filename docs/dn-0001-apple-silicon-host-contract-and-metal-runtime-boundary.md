# DN-0001: Apple Silicon Host Contract And Metal Runtime Boundary

- Status: `accepted`
- Authors: `project team`
- Reviewers:
- Date: `2026-03-09`
- Related work item:
  - `T2 Define the Apple Silicon host contract`
  - `T3 Design the native stwo-metal-sys Metal runtime`

## Summary

`stwo-metal` will target a Rust frontend plus a native Metal runtime owned by
`stwo-metal-sys`, with hot kernels written in `.metal`.

This note defines:

- the supported host modes for pre-Metal and Metal-enabled development
- the native runtime ownership boundary between `stwo-metal` and
  `stwo-metal-sys`
- the exact build-time pipeline for compiling `.metal` sources into a
  loadable Metal library
- the exact runtime loading model for kernels and pipeline state
- the default validation rule: bounded Metal work must be tested against the
  local vendored Stwo CPU execution

This note does not promise end-to-end proving support yet. It only defines the
host and runtime contract that future implementation work must obey.

## Implementation status snapshot

Implemented against this note:

- `STWO_METAL_MODE` host-mode handling in
  [`crates/stwo-metal-sys/build.rs`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/crates/stwo-metal-sys/build.rs)
- build-time `.metal` compilation through `xcrun metal` and `xcrun metallib`
- embedded `.metallib` loading and lazy pipeline caching inside
  [`crates/stwo-metal-sys/src/metal.rs`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/crates/stwo-metal-sys/src/metal.rs)
- the first bounded native Metal primitive:
  `BaseField` bit reversal with deterministic vendored CPU parity tests
- `SecureField` column upload, mutation, readback, and bit reversal through the
  same native Metal runtime boundary
- one bounded poly-support primitive:
  `BaseField` coset-order to circle-domain bit-reversed permutation through a
  native Metal kernel
- bounded FRI first-layer `fold_circle_into_line` from secure circle
  evaluation into the first line layer
- bounded FRI `fold_line` through repeated host-orchestrated Metal fold steps
- native `MetalLineEvaluation` and first inner-layer commitment root parity
- native first inner-layer query and decommit parity
- bounded native first inner-layer proof row
- bounded native inner-layer FRI sequence
- bounded FRI commitment slice with explicit last-layer degree-bound truncation
- bounded proof-facing inner FRI proof slice
- bounded first-layer circle commitment and decommit boundary
- bounded full FRI proof candidate
- explicit CPU bridge from Metal line values into `LineEvaluation<CpuBackend>`
  retained as a bounded validation path

Still outside the implemented surface:

- one declared proving sub-path that consumes the bounded full FRI proof
  candidate
- interpolation, evaluation, and trace-support primitives beyond the bounded
  FRI arithmetic surface
- any truthful end-to-end proving claim

The first declared T5 candidate path is:

- FRI first-layer fold from a bit-reversed secure circle evaluation into the
  first line layer

For the first implementation of that path, host orchestration may derive the
domain-specific inverse-`y` and inverse-`x` factor buffers explicitly from
vendored Stwo domain semantics and pass them into the bounded Metal kernels.
This is an explicit boundary choice, not a hidden CPU fallback for the fold
computation itself.

## Problem

Current behavior:

- [`crates/stwo-metal-sys/build.rs`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/crates/stwo-metal-sys/build.rs)
  still drives an inherited CUDA build through CMake.
- [`crates/stwo-metal-sys/Cargo.toml`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/crates/stwo-metal-sys/Cargo.toml)
  still links `stwo_cuda`.
- [`crates/stwo-metal-sys/src/raw.rs`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/crates/stwo-metal-sys/src/raw.rs)
  and
  [`crates/stwo-metal-sys/src/no_cuda_link_stubs.rs`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/crates/stwo-metal-sys/src/no_cuda_link_stubs.rs)
  define a CUDA-shaped raw boundary and stub model.

Concrete risk or pain:

- there is no truthful Apple Silicon host contract
- there is no formal rule for how `.metal` kernels enter the build
- there is no explicit runtime ownership model for devices, queues, buffers,
  or pipeline state
- there is no enforced correctness oracle besides ad hoc manual comparison

Why the current state is insufficient:

- implementation could drift into a renamed CUDA fork
- build behavior could vary silently across hosts
- future Metal kernels could land without a deterministic parity discipline
- runtime state could become ambient and hard to reason about

## Scope

In scope:

- host-mode contract for `no-metal`, development, and CI
- `.metal` source placement and build pipeline
- runtime library loading and compute pipeline creation
- ownership rules for Metal device, queue, library, pipeline, and buffers
- deterministic unit-test validation against vendored CPU execution

Out of scope:

- a full public API redesign for `stwo-metal`
- full end-to-end proving support
- throughput targets and benchmarking policy beyond the rule that correctness
  comes first
- choosing the first proving primitive to implement on Metal

## Inputs, outputs, and interfaces

Inputs:

- local vendored Stwo snapshot under
  [`vendor/stwo-upstream-dev-62b228e`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/vendor/stwo-upstream-dev-62b228e)
- current `stwo-metal` and `stwo-metal-sys` crate boundaries
- `.metal` kernel sources added under `stwo-metal-sys`
- upstream Stwo skill registry and process guides:
  - [`_index.md`](https://github.com/starkware-libs/stwo/blob/dev/.claude/skills/_index.md)
  - [`rust-codebase-conventions.md`](https://github.com/starkware-libs/stwo/blob/dev/.claude/skills/rust-codebase-conventions.md)
  - [`testing-strategy.md`](https://github.com/starkware-libs/stwo/blob/dev/.claude/skills/testing-strategy.md)
  - [`soundness-review-checklist.md`](https://github.com/starkware-libs/stwo/blob/dev/.claude/skills/soundness-review-checklist.md)

Outputs:

- a truthful host-mode contract
- a native Metal kernel build artifact (`.metallib`) produced at build time
- a narrow runtime boundary owned by `stwo-metal-sys`
- deterministic parity tests against vendored CPU execution

Interfaces affected:

- [`crates/stwo-metal-sys/build.rs`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/crates/stwo-metal-sys/build.rs)
- [`crates/stwo-metal-sys/Cargo.toml`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/crates/stwo-metal-sys/Cargo.toml)
- new files under
  [`crates/stwo-metal-sys/metal`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/crates/stwo-metal-sys/metal)
- new runtime code under
  [`crates/stwo-metal-sys/src`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/crates/stwo-metal-sys/src)
- future backend integration points under
  [`crates/stwo-metal/src/backend`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/crates/stwo-metal/src/backend)

## Invariants

- The local vendored Stwo CPU execution is the semantic correctness oracle.
- Host-safe development must remain available on machines without Metal-enabled
  execution.
- `stwo-metal-sys` owns native Metal runtime details; `stwo-metal` does not
  own device, queue, or pipeline internals directly.
- Hot GPU kernels are written in `.metal` by default.
- No hidden CPU fallback is allowed in a path claimed as Metal-backed.
- Bounded Metal cuts must have deterministic unit tests before performance
  measurement matters.
- Shared mutable global runtime state is not allowed as the default ownership
  model.
- Design and review language should stay aligned with the upstream Stwo skill
  registry where that vocabulary applies cleanly.

## Process alignment with upstream Stwo skills

This note adopts the upstream Stwo skill material as process guidance for T2
and T3.

Required usage:

- use the upstream skill registry as the context-loading entry point
- use the Rust codebase conventions skill when introducing Rust-side runtime or
  backend code
- use the testing strategy skill when defining Metal parity tests and test
  placement
- use the soundness review checklist before approving any soundness-critical
  proving-path change that grows out of this runtime boundary

This note does not import upstream skills as a second semantic authority. The
semantic correctness oracle remains the local vendored CPU execution.

## Proposed host contract

### Environment variable

Introduce `STWO_METAL_MODE` as the host-mode control variable.

Supported values:

- `no-metal`
- `metal-dev`
- `metal-ci`

Default behavior:

- `target_os = macos` and `target_arch = aarch64`:
  default to `metal-dev`
- all other hosts:
  default to `no-metal`

### Mode semantics

`no-metal`:

- Metal kernels are not compiled.
- `stwo-metal-sys` emits no Metal runtime link or build requirements.
- host-safe `cargo check` and non-Metal tests must remain viable.
- any attempt to execute a Metal-backed runtime symbol fails explicitly.

`metal-dev`:

- `.metal` sources are compiled at build time if the Apple Metal toolchain is
  available.
- runtime loading and bounded local tests are allowed.
- missing toolchain or missing Metal-capable runtime fails the Metal path
  explicitly, but does not pretend success through CPU fallback.

`metal-ci`:

- `.metal` compilation is required.
- the build fails closed if the Apple Metal toolchain is unavailable.
- the CI lane must run the deterministic bounded Metal parity tests declared for
  the implemented surface.

## Proposed runtime boundary

`stwo-metal-sys` is the only crate that owns:

- Metal device discovery
- command queue creation
- buffer allocation and transfer
- loading the compiled `.metallib`
- compute pipeline state creation
- command encoding and synchronization for bounded kernel launches

`stwo-metal` owns:

- proving-facing orchestration
- conversion between Stwo data structures and the narrow runtime requests
- CPU oracle comparison in higher-level tests where the bounded surface is above
  a raw buffer transform

### Ownership rule

The default model is an explicit runtime object owned inside `stwo-metal-sys`.
It may cache:

- selected `MTLDevice`
- `MTLCommandQueue`
- loaded `MTLLibrary`
- compute pipeline states keyed by kernel name

This cache must remain encapsulated within `stwo-metal-sys`. It must not be
leaked as ambient mutable global state across the higher-level crate boundary.

## `.metal` kernel source layout

Metal kernels live under:

- [`crates/stwo-metal-sys/metal`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/crates/stwo-metal-sys/metal)

Rules:

- one bounded kernel family per file when practical
- kernel entrypoint names are stable and explicit
- input and output buffer layout assumptions must be documented next to the
  corresponding Rust launch wrapper

## Kernel compilation pipeline

Build-time compilation is owned by
[`crates/stwo-metal-sys/build.rs`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/crates/stwo-metal-sys/build.rs).

For `metal-dev` and `metal-ci` on supported hosts:

1. Discover Apple’s Metal tools through `xcrun`.
2. Compile each `.metal` source file to `.air` using:

   ```text
   xcrun metal -c <source>.metal -o <out>/<source>.air
   ```

3. Link the produced `.air` files into one library:

   ```text
   xcrun metallib <out>/*.air -o <out>/stwo-metal.metallib
   ```

4. Generate a small Rust module in `OUT_DIR` that exposes:
   - the compiled `.metallib` bytes via `include_bytes!`
   - the list of kernel entrypoint names expected by the runtime

For `no-metal`:

- skip the Metal build pipeline entirely
- generate stub behavior only

### Build artifact rule

The compiled `.metallib` is a build artifact, not a checked-in source file.
The source of truth is the `.metal` source plus the Rust launch wrappers.

## Runtime loading model

At runtime, `stwo-metal-sys`:

1. creates or resolves the selected Metal device
2. loads the embedded `.metallib` bytes into a `MTLLibrary`
3. resolves kernel functions by their declared entrypoint names
4. creates compute pipeline states lazily on first use
5. caches pipeline states inside the private runtime object

Failure to load the library, resolve a function, or create a pipeline state is
an explicit runtime error for Metal-enabled modes.

No Metal-enabled path may silently fall back to CPU while still claiming to be
the Metal path.

## Validation strategy

### Default oracle

The local vendored Stwo CPU execution is the only correctness oracle for
bounded Metal work.

### Unit-test rule

Every bounded Metal primitive must have deterministic unit tests that:

1. create fixed test inputs
2. compute the expected result through the relevant vendored CPU path
3. run the Metal primitive on the same inputs
4. compare outputs exactly, field-by-field or byte-for-byte as appropriate

### Test placement

Tests may live in:

- `crates/stwo-metal-sys` when validating raw runtime or kernel wrappers
- `crates/stwo-metal` when validating a proving-facing bounded backend surface

### Test discipline

- fixed seeds only
- no network dependencies
- no benchmark timings as a pass condition
- narrowest possible comparison surface first

### Validation ladder

The intended validation ladder is:

1. raw buffer and layout tests
2. bounded primitive parity tests
3. bounded trace or proving-subpath parity tests
4. only later, declared performance measurements

## Failure modes

- Failure mode:
  - Trigger: `STWO_METAL_MODE=metal-ci` on a host without a working Apple Metal
    toolchain
  - Detection: build-time tool discovery or compilation failure
  - Expected behavior: fail closed during build

- Failure mode:
  - Trigger: `STWO_METAL_MODE=no-metal` but a Metal-backed symbol is executed
  - Detection: stub runtime path
  - Expected behavior: explicit panic or typed runtime error naming the missing
    Metal mode

- Failure mode:
  - Trigger: `.metal` kernel signature and Rust launch wrapper disagree on
    layout or length
  - Detection: deterministic unit-test parity failure or explicit runtime
    validation
  - Expected behavior: fail the test or launch explicitly, never silently
    continue

- Failure mode:
  - Trigger: compiled `.metallib` cannot be loaded or a kernel entrypoint is
    missing
  - Detection: runtime library or function resolution failure
  - Expected behavior: explicit runtime error in Metal-enabled modes

- Failure mode:
  - Trigger: Metal-backed output differs from vendored CPU execution
  - Detection: deterministic unit tests
  - Expected behavior: block the cut; do not replace the CPU oracle

## Migration and rollout

What lands first:

- `STWO_METAL_MODE`
- `no-metal` and Metal-enabled build-mode split
- `.metal` source directory
- build-time `.metallib` generation
- explicit runtime object and one bounded kernel launch path
- deterministic CPU-oracle tests for that bounded path

What must remain compatible:

- host-safe non-Metal development
- the public `stwo-metal` repository identity
- the local vendored Stwo snapshot as semantic authority

What debt is introduced:

- none intended by design; any temporary bridge in the first implementation cut
  must be recorded in
  [`tech-debt-register.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/tech-debt-register.md)

## Non-promises

- This note does not promise a cross-platform GPU abstraction.
- This note does not promise full end-to-end proving support yet.
- This note does not promise benchmark-class performance before correctness.
- This note does not promise that every current CUDA-shaped API name is renamed
  in the same tranche.
