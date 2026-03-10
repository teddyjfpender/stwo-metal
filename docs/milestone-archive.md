# Milestone Archive

## Purpose

This file preserves superseded milestone sequencing so the active planning
documents can stay small and operational.

This archive is not the source of truth for current work. The active sources of
truth are:

- [`roadmap.md`](./roadmap.md)
- [`program-plan.md`](./program-plan.md)
- [`controller.md`](./controller.md)

## Archived milestone system

The previous program was tracked through milestone families `T0` through `T8`.
That sequence was useful for the initial Metal bring-up, but it is now
superseded by the generic/generated contract sequence `G0` through `G8`.

Archived historical milestones:

| Milestone | Archived meaning | Current disposition |
| --- | --- | --- |
| `T0` | isolate repo identity and clean copied docs/process | completed and retained as foundation |
| `T1` | freeze backend-neutral Rust boundary and direction | completed and absorbed into `G0` |
| `T2` | define Apple Silicon host contract | completed and retained as foundation |
| `T3` | design `stwo-metal-sys` runtime replacement | completed and retained as foundation |
| `T4` | land first Metal primitive path | completed and retained as foundation |
| `T5` | prove one bounded Stwo trace path through Metal | completed and retained as foundation |
| `T5a` | planning correction toward backend-first completion | completed and superseded by `G0/G1` |
| `T6` | restore one truthful end-to-end supported workload | completed and retained as foundation |
| `T7` | prove upstream examples with backend wiring only | completed for current non-blocked rows and retained as acceptance foundation |
| `T8` | mirror and port CUDA hot path into Metal for benchmark-grade work | completed as a structural native-port foundation; no longer the active planning frame |

## Why the sequence was archived

The archived sequence overfit to the initial bring-up phase:

- examples were doing too much architectural work
- benchmark work was too close to the planning centerline
- native porting activity was clearer than the long-term producer/consumer
  contract

Once the team aligned on the generic backend plus generated fast-path model,
the old sequence became historical context rather than the right active control
surface.

## What remains valid from the archived sequence

The following results remain active foundation and are not being undone:

- repository/process cleanup
- Apple Silicon host/runtime boundary
- mirrored Metal native hot-path structure
- example-backed acceptance coverage for current non-blocked rows
- benchmark-faithful wide-fibonacci proving row

## What no longer uses the archived sequence

The following are now governed by `G0` through `G8` instead:

- backend API shape
- codegen/producer contract
- generated inventory planning
- execution-plan lowering
- benchmark-lane separation
- `stark-v` hardening
