# DN-0008: Metal Evaluation Program V1

- Status: `accepted`
- Date: `2026-03-11`
- Owners: `project team`
- Related roadmap items:
  - [`roadmap.md`](./roadmap.md)
  - [`program-plan.md`](./program-plan.md)
  - [`dn-0002-generic-backend-and-codegen-contract.md`](./dn-0002-generic-backend-and-codegen-contract.md)

## Summary

`stwo-metal` should stop treating benchmark-specialized proving paths as the
long-term architecture surface for generated support.

The next stable contract is `MetalEvaluationProgramV1`:

- one fully lowered Stwo or framework/codegen proving component
- canonicalized in Rust before device execution
- validated and hashed before dispatch
- executed on Metal through either:
  - a generic interpreter lane for correctness, or
  - an optional generated overlay lane for performance

Examples remain the acceptance matrix. They do not define the device ABI or the
execution model.

## Problem

The repository now has two useful but incomplete truths:

- the generated `wide_fibonacci` lane can outperform the old SIMD reference on
  warmed runs
- the current fast path is still shaped too much by benchmark-specific seams
  and backend-local specialization

That is not a stable long-term interface.

What is still missing is a formal producer/consumer contract that says:

- what exact lowered artifact Stwo/codegen emits
- what exact subset `stwo-metal` consumes
- what the host/device ABI is
- how validation and fail-closed behavior work
- how the generic interpreter lane and generated overlay lane coexist

Without that contract, performance work risks hard-coding workload behavior
instead of building a durable Metal backend.

## Scope

This note specifies:

- the V1 semantic contract for a lowered Metal proving program
- the V1 host/device ABI records
- validation and fail-closed rules
- execution modes
- the acceptance matrix role
- the migration path from the current benchmark-specialized lane to the generic
  and generated architecture

This note does not specify:

- the final `.metal` implementation of every instruction
- the final serialized container format on disk
- the final overlay code generator
- downstream `stark-v` enablement

## Design target

The target is not “arbitrary Rust workload inference.”

The target is:

- generic over Stwo-defined and codegen-defined proving components
- driven by a machine-readable lowered artifact
- executed through a stable Metal runtime contract
- optionally accelerated by generated overlays keyed by semantic identity

## V1 semantic contract

`MetalEvaluationProgramV1` means:

- one fully lowered `FrameworkEval::evaluate()` instance or equivalent
- after canonicalization of:
  - parameters
  - trace and preprocessed source references
  - intermediates
  - finalized constraints
- with no host callback path in the selected device row
- with no silent fallback inside the selected device row

The Rust-side lowering pipeline is:

1. run an info-discovery pass to learn mask structure, preprocessed columns, and
   constraint count
2. run an expression-capture pass to collect ordered intermediates and
   finalized constraints
3. canonicalize parameter names into dense base/ext slot indices
4. canonicalize every source read into an explicit source reference
5. lower base/ext expression DAGs into dense register definitions
6. emit ordered `constraint_roots[]`

The V1 device result is one secure-field row accumulator per row, using the
same accumulation law as the host path:

- constraint `i` is accumulated with `random_coeff_powers[i]`
- ordering is semantic and stable
- constraint order must not change after lowering

## V1 execution assumptions

V1 intentionally requires:

- `PREFINALIZED_LOGUP`
- `secure_ext_degree = 4`
- explicit ordered constraint accumulation only

V1 intentionally excludes:

- dynamic host callbacks
- a general effect machine for row interactions
- hidden CPU fallback in the chosen Metal row

Those richer capabilities remain reserved for later revisions.

## Host/device ABI

The V1 ABI must be:

- pointer-free
- little-endian
- sectioned
- stable under semantic hashing

### Header

```c
struct StwoMetalPlanHeaderV1 {
  uint32_t magic;              // "STP1"
  uint16_t abi_major;          // 1
  uint16_t abi_minor;          // 0
  uint32_t n_sections;
  uint32_t flags;              // PREFINALIZED_LOGUP, DEBUG_PRESENT, ...
  uint64_t semantic_hash;      // canonical semantic payload only
  uint64_t capability_bits;    // BASE_INV, EXT_MUL, PREFINALIZED_LOGUP, ...
  uint32_t n_interactions;
  uint32_t n_base_params;
  uint32_t n_ext_params;
  uint32_t n_constraints;
  uint32_t max_base_regs;
  uint32_t max_ext_regs;
  uint32_t secure_ext_degree;  // 4 in V1
  uint32_t reserved[8];
};
```

### Section descriptor

```c
struct StwoMetalSectionDescV1 {
  uint32_t kind;
  uint32_t elem_size;
  uint64_t offset_bytes;
  uint64_t count;
};
```

### Required sections

- `BASE_CONSTS`
- `EXT_CONSTS`
- `BASE_INSTS`
- `EXT_INSTS`
- `CONSTRAINT_ROOTS`

### Optional sections

- `DEBUG_STRINGS`
- `PARAM_DEBUG_MAP`
- `NODE_DEBUG_MAP`

### Semantic hash law

The semantic hash includes:

- opcodes
- operands
- constants
- parameter-slot ordering
- constraint-root ordering
- semantic flags

The semantic hash excludes:

- debug sections
- physical buffer locations
- host-only path annotations

That hash is the overlay lookup key.

## ABI struct rules

Shared boundary structs must be:

- `#[repr(C)]`
- fixed-width
- POD-like

Allowed field forms:

- `u32`
- `u64`
- `[u32; N]`
- `[u64; N]`

Disallowed field forms:

- references
- slices
- `Vec`
- `String`
- trait objects
- implicit-layout Rust enums
- cross-language `bool`
- ambiguous vector layouts in shared buffers

Rust layout checks and Metal reflection checks are both required. `#[repr(C)]`
is necessary but not sufficient.

## Instruction model

V1 uses a register-based interpreter, not a stack machine.

### Base instructions

The base instruction family must cover:

- trace reads
- preprocessed-column reads
- parameter reads
- constant reads
- add/sub/mul/neg/inv

### Extension instructions

The extension instruction family must cover:

- secure-field assembly from four base registers
- parameter reads
- constant reads
- add/sub/mul/neg

### Constraint roots

`constraint_roots[]` is ordered and semantic:

- position `i` equals the semantic constraint index
- position `i` is the `random_coeff_powers[i]` slot used for accumulation
- no post-lowering renumbering is allowed

## Validation rules

Validation must happen before execution and must fail closed.

### Required validation checks

- supported ABI major
- `secure_ext_degree == 4`
- `PREFINALIZED_LOGUP` present
- only supported opcodes
- register ids in range
- `constraint_roots` in range
- parameter counts match runtime context
- plan fits backend register budgets
- semantic hash and checksum match payload

### Failure modes

- incompatible ABI
  - trigger: unsupported major version or malformed section table
  - expected behavior: reject before planning
- unsupported semantics
  - trigger: unsupported opcode or missing required flag
  - expected behavior: reject before execution
- capacity overflow
  - trigger: register budgets or section counts exceed backend limits
  - expected behavior: reject before execution
- overlay mismatch
  - trigger: semantic hash or capability profile mismatch
  - expected behavior: do not use overlay; fall back only to the generic Metal
    interpreter lane if that lane validates
- unsupported row
  - trigger: neither generic interpreter nor generated overlay supports the
    artifact
  - expected behavior: fail closed; outer dispatch may select another proving
    mode, but the Metal row must not hide that decision internally

## Execution modes

V1 has two execution modes.

### 1. Generic interpreter lane

Purpose:

- correctness
- broad semantic coverage
- acceptance and bring-up

Properties:

- executes `MetalEvaluationProgramV1` directly
- stable baseline for parity and law tests
- not assumed to be the fastest lane

### 2. Generated overlay lane

Purpose:

- production performance
- backend-specific specialization

Properties:

- selected by `(abi_major, semantic_hash, capability_profile)`
- may use generated Rust registration
- may use generated Metal kernel stubs or adapters
- must preserve the exact semantic contract of the generic lane

## Dispatch law

Dispatch is:

1. build canonical program
2. validate against V1 generic Metal capability profile
3. look up optional generated overlay by semantic identity
4. run the overlay if found and valid
5. otherwise run the generic Metal interpreter if valid
6. otherwise fail closed

There is no silent CPU fallback inside a selected Metal lane.

## Acceptance matrix

Examples remain acceptance workloads only.

The current matrix is:

- `wide_fibonacci`
- `state_machine`
- `blake`
- `xor`
- `poseidon` when the upstream protocol blocker clears

Acceptance duties:

- verify semantic parity of generic interpreter lane
- verify semantic parity of generated overlay lane
- verify unsupported rows fail closed honestly

Benchmarks remain separate from acceptance:

- generic lane benchmark rows
- generated lane benchmark rows

## Migration plan

### Stage 1: freeze the contract

Land:

- this design note
- `metal-eval-abi` Rust boundary module or crate
- validator error enum
- semantic hash rules

Compatibility:

- existing benchmark-specialized paths remain valid
- examples remain the active acceptance matrix

### Stage 2: generic interpreter bring-up

Land:

- `MetalEvaluationProgramV1` lowering in Rust
- generic interpreter execution path in `.metal`
- parity tests against host evaluators

Compatibility:

- existing specialized benchmark paths may stay for performance comparison
- they stop being the architecture source of truth

### Stage 3: overlay registration

Land:

- overlay lookup keyed by semantic hash and capability profile
- generated ABI and inventory registration
- fail-closed overlay selection

Compatibility:

- generic interpreter remains the correctness fallback inside the Metal family

### Stage 4: benchmark migration

Land:

- benchmark rows reported separately for:
  - generic interpreter lane
  - generated overlay lane
- migration of the current hand-optimized `wide_fibonacci` row onto the same
  semantic contract

Compatibility:

- current benchmark-specialized helpers are allowed only as transitional
  generated-lane shims
- those helpers must be retired once the program/overlay path carries the row

### Stage 5: downstream hardening

Land:

- the same program contract applied to a real downstream workload such as
  `stark-v`

Compatibility:

- if downstream artifacts or backend-parametric surfaces are absent, the row
  remains fail-closed

## Verification plan

Verification must be layered.

### Expression level

- lowered node evaluation matches Rust expression evaluation on randomized
  assignments

### Program level

- canonical lowered program matches host symbolic lowering for intermediates and
  constraints

### Row level

- host row evaluator and Metal generic interpreter produce identical row
  accumulators

### Packed level

- SIMD/CPU unpacked rows match Metal row results

### Negative validation

- unsupported opcode
- explicit non-prefinalized logup in V1
- out-of-range register id
- bad constraint root
- wrong extension degree
- corrupted semantic hash or checksum

### ABI checks

- Rust `size_of` and `align_of`
- Metal reflection for buffer size, alignment, and struct-member layout

## What remains unchanged

- verifier semantics stay unchanged
- examples remain acceptance rows
- generated outputs must remain durable and hand-tunable
- unsupported generated rows fail closed
- the vendored Stwo snapshot remains the semantic authority
