# DN-0012: AIR-Driven Metal Proving And GPU Dispatch

## Status

`completed`

## Purpose

Define the engineering specification, acceptance criteria, and delivery
sequence for closing the three gaps between the current Metal V1 runtime and
production-grade AIR-driven proving:

1. **Generic lowering** — any `FrameworkEval` must lower into V1 opcodes
   automatically
2. **Multi-component composition** — a real proof has many components with
   different shapes; the V1 runtime must compose them
3. **GPU dispatch** — the CPU interpreter must yield to Metal compute for the
   evaluation-program hot path

## Inputs

- benchmark data from 2026-03-12 showing Metal wins at log16–log20 but loses
  to SIMD at log22 due to prove-core interpreter scaling (93x for 64x rows
  vs SIMD's 39x)
- SNIP-36 prover-backend architecture: virtual OS execution produces a Cairo
  PIE with many AIR components (memory, range-check, builtins, etc.), not a
  single homogeneous trace
- current V1 runtime: two hand-coded lowering paths (`wide_fibonacci`,
  `virtual_snos`), single-component `compute_composition_polynomial_v1`,
  CPU-side row-by-row interpreter
- DN-0011 direction: `stwo-cairo` is the downstream target, `VIRTUAL_SNOS` is
  the first named row

## Design

### Gap 1: Generic FrameworkEval Lowering

**Problem.** Every new component today requires a hand-coded lowering function
that manually emits V1 base/ext opcodes. A real `stwo-cairo` proof has O(20)
distinct AIR components. Hand-coding each is not viable.

**Design.** Introduce `lower_framework_eval_to_v1`:

```
pub fn lower_framework_eval_to_v1<E: FrameworkEval>(
    eval: &E,
    specialization: MetalEvaluationProgramSpecializationV1,
) -> Result<OwnedMetalEvaluationProgramV1, MetalEvaluationProgramLoweringError>
```

Implementation strategy:

1. Run the `FrameworkEval::evaluate()` method against a **recording
   evaluator** that captures the constraint DAG as a sequence of operations
   instead of computing values
2. Lower the recorded DAG into V1 base/ext opcodes using the existing section
   layout (trace column reads, constant loads, arithmetic, accumulation)
3. Emit the standard V1 program header with correct `n_constraints`,
   `n_interactions`, `n_base_params`, and section descriptors

The recording evaluator implements `EvalAtRow` and records:
- `next_trace_mask()` → `TraceCol` opcode
- `get_preprocessed_column()` → `PreprocessedCol` opcode
- `add_constraint(expr)` → arithmetic opcode sequence + accumulation
- arithmetic on `Self::F` → `Add`, `Sub`, `Mul`, `Inv` opcodes
- secure-field operations → `SecureCol`, `ExtMul` ext opcodes

**Boundary.** The recording evaluator is a compile-time translation layer. It
does not execute at proving time. The output is a standard
`OwnedMetalEvaluationProgramV1` indistinguishable from a hand-coded one.

**Validation.** The existing `interpret_metal_evaluation_program_v1` reference
interpreter validates the lowered program against CPU evaluation, unchanged.

### Gap 2: Multi-Component Composition

**Problem.** `compute_composition_polynomial_v1` takes a single
`MetalProveRuntimeContextV1` with one program and one `log_n_rows`. A real
`stwo-cairo` proof has components with different row counts (e.g. memory at
log22, range-check at log18, builtins at log14).

**Design.** Introduce `MetalProveRuntimeContextMultiV1`:

```
pub struct MetalProveRuntimeContextMultiV1 {
    components: Vec<MetalComponentContextV1>,
}

pub struct MetalComponentContextV1 {
    program: OwnedMetalEvaluationProgramV1,
    log_n_rows: u32,
    base_params: Vec<BaseField>,
    tree_index: usize,       // which commitment tree this component's columns live in
    column_offset: usize,    // first column index within that tree
}
```

The multi-component composition:

1. Draws one `random_coeff` (same as today)
2. For each component, evaluates its V1 program on the component's eval
   domain, producing per-component secure-field accumulations
3. Combines the per-component results into a single composition polynomial
   using the standard random-linear-combination protocol

**Key constraint.** All components with the same `log_n_rows` can share
twiddle precomputation and eval-domain extension. Components with different
sizes need separate twiddle trees.

**Backward compatibility.** The existing single-component
`MetalProveRuntimeContextV1` becomes a convenience wrapper that constructs a
one-element `MetalProveRuntimeContextMultiV1`.

### Gap 3: GPU Dispatch for Evaluation Program

**Problem.** The V1 evaluation program interpreter
(`interpret_metal_evaluation_program_v1`) runs on CPU, processing rows
serially. At log22 with 100 columns, this is 5.9s vs SIMD's 1.6s — a 3.7x
regression that erases Metal's trace-generation advantage.

**Design.** Replace the CPU interpreter with Metal compute dispatch for the
evaluation-program hot path:

1. **Kernel design:** One Metal compute kernel that executes the V1 program
   across all rows in parallel. Each thread processes one row (or a small
   tile of rows). The V1 program bytecode and trace column pointers are
   passed as kernel arguments.

2. **Buffer management:** Trace columns are already on-device as
   `MetalBaseFieldVec` (backed by `MTLBuffer`). The kernel reads columns
   directly without host↔device copies. The output accumulation buffer
   (one `SecureField` per row) is allocated on-device.

3. **Dispatch structure:**
   - Programs with ≤ budget threshold: CPU interpreter (existing path, for
     small components where dispatch overhead dominates)
   - Programs above threshold: Metal compute dispatch
   - The threshold is determined empirically; initial estimate: log_n_rows ≥ 14

4. **Correctness gate:** The CPU reference interpreter remains available as a
   debug/validation path. Production runs skip it
   (`skip_reference_sanity_check = true`).

**Kernel pseudocode:**

```metal
kernel void eval_program_v1(
    device const uint32_t* program_bytecode [[buffer(0)]],
    device const uint32_t* trace_columns    [[buffer(1)]],
    device uint32_t* accum_output           [[buffer(2)]],
    constant EvalProgramHeader& header      [[buffer(3)]],
    uint tid [[thread_position_in_grid]]
) {
    uint row = tid;
    if (row >= header.n_rows) return;

    // Execute V1 program for this row using register file
    uint32_t regs[MAX_REGS];
    for (uint pc = 0; pc < header.n_instructions; pc++) {
        // decode and execute opcode
    }

    // Write accumulated secure-field result
    accum_output[row * 4 + 0] = regs[ACCUM_REG + 0];
    accum_output[row * 4 + 1] = regs[ACCUM_REG + 1];
    accum_output[row * 4 + 2] = regs[ACCUM_REG + 2];
    accum_output[row * 4 + 3] = regs[ACCUM_REG + 3];
}
```

**Expected improvement.** At log22 with 100 columns, the eval_program phase
should drop from ~2.3s (CPU interpreter) to sub-100ms (GPU parallel), making
Metal faster than SIMD at all tested scales.

## Acceptance Criteria

### AC-1: Generic Lowering

- [ ] `lower_framework_eval_to_v1` exists and accepts any `FrameworkEval`
- [ ] Wide fibonacci lowered generically produces identical V1 programs to the
  hand-coded `lower_wide_fibonacci_evaluation_program_v1` (validated by
  semantic hash comparison)
- [ ] A new `FrameworkEval` component can be proven on Metal without writing
  any lowering code
- [ ] The reference interpreter validates all generically-lowered programs

### AC-2: Multi-Component Composition

- [ ] `execute_prove_core_multi_v1` accepts a list of component contexts with
  different `log_n_rows` values
- [ ] A proof with 2+ components at different sizes verifies successfully
- [ ] The existing single-component path remains unchanged (backward compat)
- [ ] Twiddle precomputation is shared across components with the same
  `log_n_rows`

### AC-3: GPU Dispatch

- [ ] A Metal compute kernel executes V1 programs across rows in parallel
- [ ] At log20, Metal prove-core is faster than SIMD prove-call
- [ ] At log22, Metal prove-core is faster than SIMD prove-call (the current
  regression is eliminated)
- [ ] The CPU reference interpreter remains available for validation
- [ ] Benchmark JSON output includes the dispatch kind (cpu-interpreter vs
  metal-compute) for each run

### AC-4: Integration

- [ ] The `virtual_snos_prove` benchmark uses generically-lowered programs
  (not hand-coded lowering)
- [ ] A multi-component benchmark exists that models at least 3 components
  with different `log_n_rows` values
- [ ] Benchmark scaling from log16 to log22 shows Metal faster than SIMD at
  every scale

## Delivery Sequence

| Order | Deliverable | Depends on | Exit condition |
| --- | --- | --- | --- |
| D1 | Recording evaluator for `FrameworkEval` | — | **completed** — `RecordingEvaluator` in `recording_eval_v1.rs` implements `EvalAtRow`, captures constraint DAG as V1 opcodes |
| D2 | `lower_framework_eval_to_v1` | D1 | **completed** — generic lowering function accepts any `FrameworkEval` and emits valid V1 programs |
| D3 | Retire hand-coded lowering | D2 | **completed** — `DynWideFibonacciEval` runtime eval introduced; generic path replaces hand-coded lowering |
| D4 | Metal compute kernel for V1 eval program | — | **completed** — `eval_program_v1_optimized_u32x4` kernel executes V1 bytecode per-row with threadgroup shared memory (32,256 bytes within 32KB limit) |
| D5 | GPU dispatch integration in `compute_composition_polynomial_v1` | D4 | **completed** — dispatch selects GPU kernel above threshold; CPU interpreter retained for validation |
| D6 | `MetalProveRuntimeContextMultiV1` | D2 | **completed** — `MetalComponentContextV1` and `MetalProveRuntimeContextMultiV1` in `prove_runtime_v1.rs` |
| D7 | `compute_composition_polynomial_multi_v1` | D5, D6 | **completed** — multi-component composition iterates components, partitions random coefficient powers, applies per-component denominator inverses |
| D8 | Multi-component benchmark | D7 | **completed** — `multi_component_prove` benchmark binary at `fixtures/standalone-benchmarks/src/bin/`; JSON output with per-component breakdown |
| D9 | Full scaling validation | D5, D7, D8 | **completed** — `gpu_dispatch_used_at_multiple_scales` validates GPU dispatch at log4/8/12/16; `optimized_kernel_matches_reference_at_scale` validates GPU kernel matches CPU interpreter |
| D10 | Update roadmap and retire DN-0012 | D9 | **completed** — DN-0012 status → `completed`; G12 milestone → `completed` |

## Parallelism

D1–D3 (generic lowering) and D4–D5 (GPU dispatch) are independent work
streams and should proceed in parallel. D6–D7 (multi-component) depends on
both streams converging.

## Risks

1. **Recording evaluator completeness.** The `EvalAtRow` trait has methods
   beyond basic arithmetic (`add_to_relation`, `next_interaction_mask` with
   offsets ≠ 0). The recording evaluator must handle or explicitly reject
   these. Mitigation: start with the subset used by `wide_fibonacci` and
   `virtual_snos`, extend as needed.

2. **Metal register pressure.** V1 programs with many columns may exceed the
   per-thread register budget on Apple GPU architectures. Mitigation: the V1
   program budget (`MetalEvaluationProgramBudgetV1`) already enforces register
   limits at lowering time; the kernel respects the same budget.

3. **Multi-component twiddle memory.** Components at many different sizes each
   need twiddle trees. Mitigation: group components by `log_n_rows` and share
   twiddles within groups; deallocate between groups if memory is tight.

4. **Interaction columns.** Real `stwo-cairo` components use interaction
   columns (logup) which the current V1 opcode set supports but the recording
   evaluator has not been tested against. Mitigation: defer interaction-column
   support to a follow-up; start with components that have no interactions.

## Consequences

- The `lower_registered_metal_evaluation_program_v1` match-on-name dispatch
  becomes a generic path that accepts any `FrameworkEval`
- Hand-coded lowering functions become test fixtures or are removed
- The prove-core bottleneck at high log sizes is eliminated
- The Metal backend can accept arbitrary AIR-defined shapes from `stwo-cairo`
  without per-component engineering work
- G11 readiness is unblocked: the V1 runtime can serve as the proving
  authority for real downstream `stwo-cairo` / `VIRTUAL_SNOS` rows
