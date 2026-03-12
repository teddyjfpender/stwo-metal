# Documentation Set

This directory is the clean process surface for `stwo-metal`.

It is intentionally small. We keep only documents that define how the project
is planned, controlled, reviewed, and changed.

## Active documents

- [`roadmap.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/roadmap.md):
  long-range architecture and milestone map
- [`controller.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/controller.md):
  current execution control document
- [`program-plan.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/program-plan.md):
  milestone-level program route
- [`definition-of-ready.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/definition-of-ready.md):
  start gate for work
- [`definition-of-done.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/definition-of-done.md):
  finish gate for slices and milestones
- [`decision-log.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/decision-log.md):
  durable architectural and process decisions
- [`tech-debt-register.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/tech-debt-register.md):
  explicit temporary compromises
- [`wip-policy.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/wip-policy.md):
  concurrency limits and focus rules
- [`pr-checklist.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/pr-checklist.md):
  merge gate
- [`design-note-template.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/design-note-template.md):
  contract-changing design template
- [`work-item-template.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/work-item-template.md):
  slice or task template
- [`milestone-archive.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/milestone-archive.md):
  superseded milestone history kept out of the active control path

## Current design note

- [`dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0001-apple-silicon-host-contract-and-metal-runtime-boundary.md):
  formal basis for T2 and T3
- [`dn-0002-generic-backend-and-codegen-contract.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0002-generic-backend-and-codegen-contract.md):
  formal basis for the generic backend, generated fast path, and acceptance-fixture split
- [`dn-0003-acceptance-bridge-law-and-ownership.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0003-acceptance-bridge-law-and-ownership.md):
  formal basis for the current private acceptance bridge catalog and its durable ownership rules
- [`dn-0008-metal-evaluation-program-v1.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0008-metal-evaluation-program-v1.md):
  formal basis for the next stable lowered-program ABI, validator, and
  execution-mode contract
- [`dn-0009-v1-post-composition-sampled-values-abi.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0009-v1-post-composition-sampled-values-abi.md):
  formal basis for the next shared ABI step after V1-owned composition
  generation
- [`dn-0010-generated-row-convergence-and-runtime-optimization.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0010-generated-row-convergence-and-runtime-optimization.md):
  formal basis for collapsing the generated row onto shared V1 runtime paths
  and optimizing from that cleaner baseline
- [`dn-0011-stwo-cairo-and-virtual-snos-target.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0011-stwo-cairo-and-virtual-snos-target.md):
  formal basis for the downstream `stwo-cairo` target and the first
  `VIRTUAL_SNOS` hardening row

## Rules

- Historical milestone history may live here only if it is explicitly archived
  and clearly marked as non-authoritative for current work.
- If a document stops guiding current work, delete it rather than preserving it
  as passive history.
- Decisions, debt, and active control state must stay synchronized.
