# DN-0009: V1 Post-Composition Sampled-Values ABI

- Status: `accepted`
- Date: `2026-03-11`
- Owners: `project team`
- Related roadmap items:
  - [`roadmap.md`](./roadmap.md)
  - [`program-plan.md`](./program-plan.md)
  - [`dn-0008-metal-evaluation-program-v1.md`](./dn-0008-metal-evaluation-program-v1.md)

## Summary

`MetalEvaluationProgramV1` now owns generated-lane composition generation for
the active `wide_fibonacci` benchmark row, but the next migration step fails
at the post-composition proof boundary.

The current live proof flow exposes secure-field sampled-value masks after
composition generation. The current V1 runtime contract only accepts explicit
base-field trace interactions. Those two shapes are not the same contract.

The next correct step is to define one explicit post-composition sampled-values
ABI for V1-adjacent execution, rather than forcing benchmark-local rewrites
that reinterpret proof samples ad hoc.

## Problem

The generated `wide_fibonacci` row now has these truths at once:

- selected `MetalEvaluationProgramV1` execution can lower, validate, dispatch,
  and execute the generated component
- selected V1 execution now owns composition generation for the generated prove
  core
- the remaining proof flow after composition generation still consumes sampled
  values through the older proof ABI

The failed migration attempt showed the mismatch concretely:

- post-composition `proof.sampled_values[MAIN_TRACE_IDX]` entries are not plain
  base-field trace values
- they arrive as secure-field values with nonzero upper limbs
- the current V1 trace-interaction contract cannot consume that shape directly

So the remaining live proof flow cannot honestly move onto V1 by pretending the
sampled-values ABI is just another trace-interaction view.

## Scope

This note specifies:

- the problem boundary after V1-owned composition generation
- the required next ABI step
- invariants for that new ABI
- fail-closed behavior
- migration constraints

This note does not specify:

- the final on-device instruction set for sampled-value execution
- the final `.metal` kernel implementation for post-composition proving
- the final overlay generator for that path

## Design target

The target is one explicit shared ABI between:

- selected `MetalEvaluationProgramV1` composition authority
- the remaining proof flow that consumes sampled values after composition
  generation

That ABI must be:

- explicit
- validated
- stable
- fail-closed
- shared by both:
  - reference execution
  - selected runtime / overlay execution

## Inputs, outputs, invariants, failure modes

### Inputs

- one validated `MetalEvaluationProgramV1`
- one generated sampled-values view for the live proof boundary
- ordered `random_coeff_powers`
- explicit specialization data needed to interpret those sampled values

### Outputs

- one post-composition evaluation result that is semantically equivalent to the
  existing host proof flow for the same sampled values

### Invariants

- no hidden CPU fallback inside the selected runtime row
- no benchmark-local reinterpretation of sampled values
- sampled-values ordering remains semantic and stable
- constraint accumulation order remains unchanged
- generic/reference and generated/overlay paths consume the same sampled-values
  ABI

### Failure modes

- unsupported sampled-values ABI version
- unsupported sample shape
- unsupported secure-field degree
- inconsistent sampled-values section lengths
- sampled-values specializations that do not match the lowered program

All of these must fail closed before hot-path execution.

## Contract shape

The new step is not “replace trace interactions with secure values everywhere.”
The safer contract is:

1. keep `MetalEvaluationProgramV1` for row-evaluation semantics
2. introduce one explicit post-composition sampled-values view
3. lower that sampled-values view into the inputs required by the remaining
   proof phase

That means the V1 family becomes:

- `trace interaction ABI`
  - used for pre-composition evaluation
- `sampled-values ABI`
  - used for post-composition proof continuation

Both remain part of the same validated producer/consumer family, but they are
not the same record shape.

## Minimum V1-adjacent sampled-values ABI requirements

The first version must:

- name tree index, column index, and sample position explicitly
- preserve secure-field limbs without collapsing them into fake base values
- preserve ordering exactly as expected by the live proof flow
- be serializable and hashable under the same fail-closed discipline as V1

The first version may stay narrow:

- only the active generated `wide_fibonacci` row is required
- the first runtime consumer may be reference-first
- device execution may remain a later widening step

## Migration rule

The next G10 migration step must not:

- reinterpret secure-field sampled values as base trace interactions
- hide the mismatch in benchmark-local glue
- claim broader selected-runtime prove-path ownership before this ABI exists

The next G10 migration step must:

- define the sampled-values ABI explicitly
- validate it explicitly
- use it in one backend-owned post-composition proof boundary
- keep the benchmark binary as reporting glue only

## Acceptance rule

This design is complete enough to proceed when:

- one backend-owned post-composition proof boundary consumes the explicit
  sampled-values ABI
- that boundary matches the existing host proof semantics on deterministic test
  vectors
- the active `wide_fibonacci` row no longer needs benchmark-local ad hoc
  sampled-value reinterpretation

## Alternatives rejected

- keep forcing the remaining proof flow through the trace-interaction ABI
- reinterpret secure-field sampled values as base values locally in the
  benchmark boundary
- declare G10 complete with V1-owned composition generation alone

## Impact

- the next honest G10 work becomes an ABI-design and implementation slice,
  rather than another speculative benchmark-local migration
- selected-runtime prove-path ownership remains truthful
- benchmark regressions caused by duplicate or invalid authority paths become
  easier to identify and contain
