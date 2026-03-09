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
- [ ] Any declared workload boundary states witness, quotient, PCS, and FRI
      ownership explicitly instead of hiding hybrid execution behind one label.
- [ ] Proof-facing FRI slices validate their bounded last-layer polynomial
      semantics against the vendored CPU oracle, including the configured
      degree-bound truncation.
- [ ] Any caller-supplied challenge material used by a bounded proof slice is
      named explicitly and tracked as debt until a transcript-owned boundary
      replaces it.

## Milestone done

- [ ] Milestone exit condition in [`program-plan.md`](./program-plan.md) is
      satisfied.
- [ ] The controller is advanced to the next active tranche.
- [ ] Residual debt is explicit.
- [ ] Obsolete process docs created only for the finished milestone are removed.

## Not done

The work is not done if any of the following are true:

- behavior changed but validation did not
- a new compromise exists but no debt entry records it
- the implementation depends on assumptions that are not written down
- the docs still describe the previous state
