# DN-0003: Acceptance Bridge Law And Ownership Boundary

- Date: `2026-03-10`
- Status: `accepted`
- Owners: `project team`
- Related roadmap milestone:
  - `G3`
- Related decisions:
  - [`DEC-0078`](./decision-log.md)
  - [`DEC-0079`](./decision-log.md)

## Purpose

Freeze the smallest non-public contract that the current acceptance bridge
catalog is allowed to satisfy while `stwo-metal` decides where the durable
shared ownership should live.

This note exists to prevent the acceptance harness from growing into a second
informal backend architecture.

## Inputs

- one declared `MetalWorkloadBoundary`
- one checked registered acceptance lane derived from that boundary
- one unchanged vendored upstream proving component shape:
  - `FrameworkComponent<E>`
  - `dyn ComponentProver<SimdBackend>`

## Outputs

- one `ComponentProver<MetalBackend>` adapter per supported upstream component
  shape
- no new workload semantics
- no new public `stwo-metal` API requirement

## Laws

1. `Registration first`

Bridge construction must start from a registered Metal-capable workload
boundary. No acceptance adapter may be created from ad hoc local constants or
unregistered workload context.

2. `Semantics stay upstream-owned`

The bridge may change backend wiring only. It must not rewrite workload logic,
trace layout, verifier semantics, or challenge flow.

3. `CPU fallback must stay explicit`

If a bridge still depends on CPU-domain evaluation, coefficient retention, or
other non-Metal proving support, that dependency must remain named in code and
tracked in the debt register.

4. `No public contract inflation`

The bridge law is non-public. `stwo-metal` must not expose a stable public
adapter API until the durable ownership decision is made and the bridge proves
reusable beyond the current acceptance matrix.

5. `Fail closed`

If the workload boundary is not Metal-capable, bridge construction must reject
it explicitly. Silent CPU fallback is not allowed at the bridge-construction
layer.

## Non-goals

- generating fast-path code
- replacing the generic backend contract
- treating examples as the architecture surface
- promising that all acceptance-local bridge logic should move into the main
  crate unchanged

## Durable ownership options

### Option A: non-public shared boundary inside this repository

Use when:

- the bridge laws are stable
- the vendored workspace layout no longer blocks a clean dependency shape
- the bridge remains specific to this repository’s backend integration needs

### Option B: upstream-facing boundary

Use when:

- the bridge law is naturally owned by vendored Stwo framework/component
  surfaces
- multiple backend families would benefit from the same proving adapter shape
- keeping the bridge local would duplicate framework semantics

## Rejected option

### Option C: leave the catalog permanently acceptance-local

Rejected because it would let test scaffolding masquerade as architecture and
would blur the line between correctness fixtures and the durable proving
surface.

## Failure modes

- workload boundary is CPU-only:
  reject at lane construction
- framework-backed component still needs CPU-domain quotient evaluation:
  allow only as explicit local bridge debt
- SIMD-backed component still needs coefficient-retaining trace conversion:
  allow only as explicit local bridge debt
- vendored workspace layout blocks crate-owned move:
  keep the bridge local and record the ownership blocker explicitly

## Exit condition

This note retires when the current acceptance bridge catalog is either:

- replaced by a shared non-public boundary with the same laws, or
- replaced by an upstream-facing proving path that preserves the same laws

