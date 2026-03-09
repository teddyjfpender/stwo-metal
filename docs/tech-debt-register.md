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

The bounded Metal lane can now fold into the first line layer, but it does not
yet own a native `SecureColumnByCoords<MetalBackend>` or native inner-layer
Merkle commitment surface. The current boundary therefore materializes the line
evaluation on CPU and commits there explicitly.

Current containment:

- `crates/stwo-metal/src/backend/metal/handoff.rs`

Risk if left in place:

The T5 candidate path remains support-honest, but the proving surface still
exits Metal before commitment, which limits both performance and how far the
backend can truthfully claim native proving support.

Exit condition:

The first inner-layer commitment no longer depends on CPU materialization and
has an approved native Metal proving-facing boundary with deterministic parity.

Target retirement point:

- `T5`
