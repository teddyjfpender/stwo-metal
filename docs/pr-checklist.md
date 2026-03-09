# PR Checklist

Use this checklist for any non-trivial change.

## Scope

- [ ] The PR solves one named problem.
- [ ] Scope is minimal.
- [ ] Out-of-scope follow-ups are named explicitly.

## Design and contracts

- [ ] A design note exists if APIs, host modes, FFI, ABI, or runtime ownership
      changed.
- [ ] Inputs, outputs, invariants, and failure modes are explicit.
- [ ] Public surface changes are minimal and intentional.
- [ ] Relevant upstream Stwo skill guidance was applied for the changed domain.

## Correctness

- [ ] Deterministic tests or validation were added or rerun.
- [ ] Bug fixes include regression coverage.
- [ ] Unsupported behavior fails explicitly.
- [ ] Test design is consistent with the upstream Stwo testing-strategy skill.

## Soundness review

- [ ] If the change is soundness-critical, it was reviewed against the upstream
      Stwo soundness-review checklist.

## Process

- [ ] Docs changed with the behavior.
- [ ] New debt was added to
      [`tech-debt-register.md`](./tech-debt-register.md) if needed.
- [ ] New durable decisions were added to
      [`decision-log.md`](./decision-log.md) if needed.
- [ ] The work satisfies [`definition-of-done.md`](./definition-of-done.md).
