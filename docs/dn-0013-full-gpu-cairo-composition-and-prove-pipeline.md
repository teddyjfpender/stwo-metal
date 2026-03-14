# DN-0013: Full GPU Cairo Composition and Prove Pipeline

## Status

`active`

## Purpose

Close the remaining gaps between the current Metal V1 runtime and a
production-grade `prove_cairo_metal` function that executes the entire
composition polynomial on GPU without skipping any components or falling back
to SimdBackend.

Three families of work are covered:

1. **V1 program correctness** — fix the detached-register bug and row-offset
   gap so every Cairo component produces a valid, executable V1 program
2. **GPU register budget** — raise or work around the 256 ext-register ceiling
   so complex components (`add_opcode` at 489 ext) can execute on GPU
3. **End-to-end pipeline** — wire the fixed composition into a single
   `prove_cairo_metal` function and re-run benchmarks

## Inputs

### Current measured state (fib_1000, 31 Cairo components)

| Metric | Value |
| --- | --- |
| Components lowered | 31/31 (all succeed) |
| Components within GPU ext budget (≤ 256) | 22/31 |
| Components exceeding GPU ext budget | 9/31 (up to 489 ext regs) |
| SimdBackend prove time | 5059 ms |
| Metal pipeline prove time (SimdBackend composition fallback) | 5377 ms |
| Composition step (SimdBackend) | 127 ms |
| Witness generation | ~4700 ms |
| prove_values (SimdBackend) | 490 ms |

### Components exceeding the 256 ext-register GPU budget

| Component | Base regs | Ext regs | Constraints |
| --- | --- | --- | --- |
| add_opcode | 309 | 489 | 27 |
| add_opcode_small | 183 | 434 | 32 |
| jnz_opcode_taken | 150 | 312 | 15 |
| memory_id_to_big[0] | 69 | 295 | 8 |
| call_opcode_rel_imm | 109 | 245 | 17 |
| memory_address_to_id | 103 | 243 | 9 |
| add_ap_opcode | 82 | 192 | 15 (within budget but close) |
| assert_eq_opcode_double_deref | 76 | 139 | 12 |
| range_check_9_9 | 31 | 131 | 4 |

### Three previous open items

1. **CPU interpreter row offsets** — the CPU interpreter explicitly rejects
   `offset ≠ 0` (`NonZeroTraceOffsetUnsupported`). The GPU kernel ignores the
   offset entirely (never loads word 3 of the instruction). The recording
   evaluator does emit offsets (e.g. `-1` for logup cumulative sum). This means
   any component that uses `next_interaction_mask(interaction, [-1, 0])` will
   produce a V1 program that neither backend can execute correctly.

2. **No `prove_cairo_metal` function** — the `--metal` flag in `cairo_prove`
   calls `run_metal_prove` which uses SimdBackend for composition. There is no
   standalone function that replaces `prove_cairo`.

3. **Benchmark re-run** — no benchmark sweep has been run since the GPU kernel
   was wired in. The log21–23 scaling regression (7.96× per doubling) has not
   been re-measured.

## Design

### Blocker 1: Detached-Register Bug in `From<BaseField>`

**Problem.** `RecordedBaseValue::from(BaseField)` returns
`RecordedBaseValue::detached()` with `reg = u16::MAX` (65535). When this value
participates in arithmetic, the recorder emits V1 instructions with source
operand `a` or `b` = 65535. Both the CPU interpreter and GPU kernel will fail
— CPU: `BaseRegisterOutOfRange`, GPU: undefined behavior or wrong result.

The `From<BaseField>` trait is invoked whenever the FrameworkEval does e.g.:

```rust
let x: Self::F = BaseField::from(42).into(); // triggers From<BaseField>
value - x  // emits Sub(dst, value.reg, 65535) — invalid
```

Note: `AddAssign<BaseField>`, `SubAssign<BaseField>`, and
`MulAssign<BaseField>` are already handled correctly — they emit a `Const`
instruction to materialize the literal into a register first.

**Root cause.** `From<BaseField>` has no access to the `RecordingState` (the
`Rc<RefCell<RecordingState>>`) because the `From` trait has the signature
`fn from(value: BaseField) -> Self`. There is no `&self` and no state to
borrow.

**Fix.** Two options, in order of preference:

**Option A — Deferred materialization.** Change `RecordedBaseValue` to carry
an optional constant value alongside the register index:

```rust
struct RecordedBaseValue {
    reg: u16,
    state: Rc<RefCell<RecordingState>>,
    pending_const: Option<u32>,  // new field
}
```

`From<BaseField>` sets `reg = u16::MAX` and `pending_const = Some(value.0)`.
In `emit_binary`, before emitting the instruction, check if `self` or `rhs`
has `pending_const.is_some()`. If so, materialize via
`st.alloc_base_reg()` + `Const` instruction, update the register, clear
`pending_const`. This is transparent to all callers.

For `Zero::zero()` and `One::one()`: set `pending_const = Some(0)` and
`pending_const = Some(1)` respectively.

**Option B — Thread-local state.** Store the recording state in a
thread-local so `From<BaseField>` can access it without `&self`. This is
more invasive and makes the recorder non-reentrant.

**Recommendation:** Option A. It requires changes only to `RecordedBaseValue`
and `RecordedExtValue` in `recording_eval_v1.rs`. No changes to the V1 ABI,
interpreter, or GPU kernel.

**Acceptance criteria:**
- [ ] `From<BaseField>` for `RecordedBaseValue` produces a valid V1 program
  (no instruction references register 65535)
- [ ] `Zero::zero()` and `One::one()` produce valid V1 programs
- [ ] `add_opcode` (489 ext) lowers to a V1 program that the CPU interpreter
  executes without error (ignoring row-offset issues)
- [ ] All 31 Cairo components from fib_1000 lower and execute on CPU
  interpreter with synthetic row-0-only data

### Blocker 2: Row-Offset Support

**Problem.** Cairo components use `next_interaction_mask(interaction, [-1, 0])`
for logup cumulative-sum constraints. The recording evaluator correctly emits
`TraceCol { offset: -1 }` in the `imm` field. But:

- CPU interpreter: rejects with `NonZeroTraceOffsetUnsupported`
- GPU kernel: ignores `imm` field entirely (always reads current row)

**Fix.** Both the CPU interpreter and GPU kernel need offset support.

**CPU interpreter** (`interpret_metal_evaluation_program_v1` in
`eval_program_v1.rs`):

```rust
// Current: rejects non-zero offsets
if inst.imm != 0 {
    return Err(NonZeroTraceOffsetUnsupported { offset: inst.imm });
}

// Fixed: wrap-around row access
let offset = inst.imm as isize;
let target_row = ((row as isize + offset).rem_euclid(n_rows as isize)) as usize;
// Then: trace_values[target_row] instead of trace_values[row]
```

The wrap-around matches the circular evaluation domain semantics of the STARK
protocol: at row 0, offset -1 reads row (n_rows - 1).

**GPU kernel** (`eval_program_v1.metal`):

The kernel currently reads 3 words per base instruction. Load word 3 (the
`imm` field) and apply it as a signed row offset:

```metal
// Current: word_base + 3 is never loaded
int offset = as_type<int>(base_insts[word_base + 3u]);
uint target_row = uint((int(row_index) + offset + int(row_count)) % int(row_count));
// Then: use target_row instead of row_index in trace_value lookup
```

**Performance note.** The extra modular arithmetic per TraceCol instruction is
negligible: it is one integer add + one integer modulo per column access. Most
columns use offset=0, so a branch can skip the modulo entirely:

```metal
uint target_row = (offset == 0) ? row_index :
    uint((int(row_index) + offset + int(row_count)) % int(row_count));
```

**Ext instructions with offsets.** Check whether `RecordingEvaluator` also
emits ext-field instructions with non-zero offsets. In the current recording
evaluator, `next_extension_interaction_mask` calls `next_interaction_mask`
which emits base-field `TraceCol` instructions, then builds ext values from
those. So ext-field offset access is decomposed into base-field column reads
with offsets — no separate ext-offset opcode is needed.

**Acceptance criteria:**
- [ ] CPU interpreter handles offset=-1, offset=0, offset=1 correctly with
  wrap-around semantics
- [ ] GPU kernel loads and applies the `imm` offset field
- [ ] A component with logup (cumulative sum at offset -1) executes correctly
  on both CPU and GPU
- [ ] CPU interpreter reference output matches GPU kernel output for
  components with offsets

### Blocker 3: GPU Register Budget (256 ext regs → 512+)

**Problem.** The Metal kernel declares fixed-size register arrays:

```metal
thread uint base_regs[1024];           // 4 KB
thread StwoMetalQm31 ext_regs[256];    // 4 KB (256 × 16 bytes)
```

Complex Cairo components need up to 489 ext registers. The hardware limit is
per-thread register pressure, not a hardcoded constant. Apple M-series GPUs
support up to ~32 KB of thread memory (occupancy-dependent).

**Analysis.** The register arrays dominate per-thread memory:

| Configuration | Base regs | Ext regs | Thread memory | Notes |
| --- | --- | --- | --- | --- |
| Current | 1024 × 4B = 4 KB | 256 × 16B = 4 KB | 8 KB | 22/31 components fit |
| Proposed | 512 × 4B = 2 KB | 512 × 16B = 8 KB | 10 KB | All 31 components fit |
| Maximum | 1024 × 4B = 4 KB | 512 × 16B = 8 KB | 12 KB | All fit, keeps base headroom |

At 10-12 KB per thread with threadgroup shared memory for instruction cache
(32 KB), this stays within Apple GPU per-thread limits. Occupancy may drop
from ~4 waves to ~2-3 waves per SIMD group, but the alternative (no GPU
execution at all) is worse.

**Fix — two-tier kernel approach:**

1. **Kernel variant A (current):** 1024 base / 256 ext — used for components
   that fit. Maximum GPU occupancy.

2. **Kernel variant B (new):** 512 base / 512 ext — used for components that
   exceed 256 ext regs. Slightly lower occupancy.

The dispatch logic (`select_metal_evaluation_program_dispatch_v1`) selects the
kernel variant based on the program's header `max_ext_regs`:

```rust
if program.header().max_ext_regs <= 256 {
    dispatch_kernel_a(program, runtime)
} else if program.header().max_ext_regs <= 512 {
    dispatch_kernel_b(program, runtime)
} else {
    Err(RegisterBudgetExceeded { ... })
}
```

**Register allocation optimization.** Additionally, investigate reducing
register usage in the lowered V1 programs via a liveness-based register
allocator. Currently the recording evaluator uses a simple monotonic allocator
(each new value gets the next register). A linear-scan allocator that reuses
dead registers could bring `add_opcode` from 489 ext to well under 256.

This is a larger project and should be a separate deliverable after the two-
tier kernel provides immediate coverage.

**Acceptance criteria:**
- [ ] Kernel variant B compiles and executes with 512 ext registers
- [ ] All 31 fib_1000 components execute on GPU (variant A or B)
- [ ] Dispatch automatically selects the correct variant
- [ ] Performance regression from lower occupancy is < 20% per component
  vs variant A for components that fit in both

### Pipeline: `prove_cairo_metal`

Once blockers 1–3 are resolved, wire the full pipeline.

**Architecture:**

```
prove_cairo_metal<MC>(input, params) -> CairoProof<MC::H>
  │
  ├── witness gen: create_cairo_claim_generator (unchanged, CPU)
  ├── trace commit: CommitmentSchemeProver<SimdBackend, MC>
  │     (Phase 1 — SimdBackend for now; MetalBackend later)
  │
  ├── lower: lower_framework_eval_to_v1 per component
  │     (all 31 components, one-time compile)
  │
  ├── composition: compute_metal_composition_poly
  │     per-component V1 execution on GPU (variant A or B)
  │     + accumulate quotients via IFFT → sum coefficients → SimdBackend poly
  │     (Phase 2 — this is the GPU-accelerated step)
  │
  ├── commit composition poly
  ├── OODS point draw + sample points
  ├── prove_values (SimdBackend)
  │     (Phase 3–5: FRI + decommit remain SimdBackend)
  │
  └── assemble CairoProof
```

**Key observation.** The composition step (127 ms on SimdBackend) is only 2.5%
of total prove time (5059 ms). The bottleneck is witness generation (4700 ms)
and prove_values/FRI (490 ms). Accelerating composition to GPU will not
produce a meaningful speedup on this test input.

**The case for Metal composition regardless:**

1. On larger inputs (more rows, more components), composition scales as
   O(n × k) where n = rows and k = constraints. At production scales
   (log20+ per component, 50+ components), composition will dominate.

2. The Metal kernel already shows 2× speedup at log16–log20 on wide_fibonacci.
   With fixed register budgets and row offsets, this speedup applies directly
   to Cairo components.

3. The pipeline proves the architecture works end-to-end, enabling future
   acceleration of FRI and prove_values on MetalBackend.

**Acceptance criteria:**
- [ ] `prove_cairo_metal` (or equivalent `--metal` path in `cairo_prove`)
  produces a verified proof using GPU composition for ALL 31 components
- [ ] No components fall back to SimdBackend for composition
- [ ] Proof matches SimdBackend proof (verified by standard verifier)
- [ ] Timing breakdown shows composition step, prove_values step, and total

### Benchmarks

Re-run the benchmark sweep after all fixes land:

1. **wide_fibonacci log16–log23** — Metal generated vs SimdBackend. This
   validates that the GPU kernel + threadgroup shared memory optimization
   hasn't regressed.

2. **cairo_prove fib_1000 --metal --verify** — end-to-end Cairo proof with
   GPU composition. Compare Metal total vs SimdBackend total.

3. **Per-component composition breakdown** — time each component's V1
   execution separately. Identify which components benefit most from GPU.

4. **Scaling test** — if a larger Cairo input is available (e.g., fib_10000 or
   a real VIRTUAL_SNOS trace), run it to see composition-dominated behavior.

**Acceptance criteria:**
- [ ] Benchmark results recorded in `logs/benchmarks/` with timestamp
- [ ] wide_fibonacci log16–log20 shows Metal ≥ 1.5× faster than SimdBackend
- [ ] cairo_prove fib_1000 shows full GPU composition with no SimdBackend
  fallback for any component

## Delivery Sequence

| Order | Deliverable | Depends on | Estimate |
| --- | --- | --- | --- |
| D1 | Fix `From<BaseField>` detached-register bug (Option A) | — | Small: ~100 lines in `recording_eval_v1.rs` |
| D2 | Row-offset support in CPU interpreter | — | Small: ~20 lines in `eval_program_v1.rs` |
| D3 | Row-offset support in GPU kernel | D2 (for validation) | Small: ~10 lines in `eval_program_v1.metal` |
| D4 | Kernel variant B (512 ext regs) | — | Medium: new kernel entry + dispatch |
| D5 | Wire `compute_metal_composition_poly` with fixed programs | D1, D2, D3, D4 | Medium: replace SimdBackend fallback |
| D6 | End-to-end verified `cairo_prove --metal` | D5 | Small: testing + validation |
| D7 | Benchmark re-run | D6 | Small: run and record |

**Parallelism.** D1, D2, and D4 are independent and can proceed in parallel.
D3 depends on D2 for validation. D5 depends on D1+D2+D3+D4 converging.

## Risks

1. **Apple GPU thread memory limits.** 512 ext registers (8 KB) + 512 base
   registers (2 KB) + threadgroup shared (32 KB) may exceed the per-thread
   budget on some Apple GPU generations. Mitigation: test on M1/M2/M3; fall
   back to CPU interpreter for unsupported hardware.

2. **Register allocator complexity.** A liveness-based allocator for V1
   programs is a significant project. Mitigation: the two-tier kernel covers
   all current components; the allocator is a future optimization.

3. **Row-offset performance.** Adding offset modular arithmetic to every
   TraceCol access could slow the GPU kernel. Mitigation: branch on offset==0
   (majority case) to avoid the modulo.

4. **Larger Cairo inputs.** The fib_1000 test has only 31 components at
   log4–log20. Production Cairo traces may have 50+ components at log20+.
   Mitigation: test with larger inputs as they become available.

## Consequences

- Every Cairo component can execute on GPU without SimdBackend fallback
- The `prove_cairo_metal` pipeline is verified end-to-end
- The Metal V1 runtime is validated against real stwo-cairo AIR components
- The GPU kernel handles row offsets, enabling logup and interaction
  constraints
- The two-tier register budget covers all known Cairo component sizes
- Benchmark data provides a clear picture of Metal vs SimdBackend at
  production scales
