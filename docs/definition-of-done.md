# Definition Of Done

Work is done only when the code, contract, and process state agree.

## Slice done

- [ ] Scope matches the ready card or approved design note.
- [ ] Tests or validation are deterministic and relevant to the changed
      boundary.
- [ ] New failure modes are documented or explicitly rejected.
- [ ] Docs changed with the behavior or contract.
- [ ] New temporary debt is recorded in
      [`tech-debt-register.md`](./tech-debt-register.md).
- [ ] Decisions that change sequencing or contracts are recorded in
      [`decision-log.md`](./decision-log.md).
- [ ] Metal-backed slices state exactly which surfaces are native Metal, which
      remain planned, and which hosts are expected to pass parity tests.
- [ ] Any explicit host-prepared bridge used by a Metal slice is named in the
      docs and tracked as debt instead of being treated as implicit support.
- [ ] If a native Metal file is added only as part of a mirrored scaffold, its
      status is recorded explicitly so file presence is not confused with
      compile-active support.
- [ ] If the slice changes backend architecture, it states whether the change
      belongs to the generic lane, generated lane, or a temporary compatibility
      shim.
- [ ] If the slice introduces or changes generated support, it defines schema
      compatibility, fail-closed behavior, and generated inventory ownership
      explicitly.
- [ ] If the slice introduces generated output, that output is durable,
      reviewable, and compatible with later hand tuning.
- [ ] If the slice claims example progress, the example remains an acceptance
      workload rather than becoming the implementation strategy.
- [ ] If the slice changes benchmark behavior, it states whether the measured
      row belongs to the generic lane or the generated lane.

## Milestone done

- [ ] Milestone exit condition in [`program-plan.md`](./program-plan.md) is
      satisfied.
- [ ] The controller is advanced to the next active tranche.
- [ ] Residual debt is explicit.
- [ ] Obsolete process docs created only for the finished milestone are removed.
- [ ] Any milestone claiming example-backed support names the accepted example
      set and records whether the examples are upstream-owned in this repo or
      still pending vendoring.
- [ ] If a milestone is complete for all non-blocked rows but not for the full
      named example set, the remaining upstream protocol blockers are recorded
      explicitly instead of being misclassified as Metal-backend gaps.
- [ ] Any milestone claiming generated fast-path support names the producer
      artifact contract it consumes and the unsupported-component behavior when
      that artifact is absent or incompatible.
- [ ] Any milestone claiming a stable backend architecture separates acceptance
      fixtures, generic backend substitution, and generated fast-path support
      explicitly.

## Not done

The work is not done if any of the following are true:

- behavior changed but validation did not
- a new compromise exists but no debt entry records it
- the implementation depends on assumptions that are not written down
- the docs still describe the previous state
- upstream-example progress is claimed even though workload-specific rewrites
  are carrying the result
- generated support is claimed even though the component still depends on
  undocumented example-specific glue or silent CPU fallback
