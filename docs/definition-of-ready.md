# Definition Of Ready

A work item is ready only when implementation can start without silent guessing.

## Required checks

- [ ] Problem statement is one sentence.
- [ ] Output is named.
- [ ] Target files or boundaries are named.
- [ ] Acceptance check is named.
- [ ] Dependencies and blockers are named.
- [ ] Owner is named.
- [ ] It is clear whether a design note is required.

## Additional required checks when applicable

- [ ] API, ABI, host-mode, or unsafe changes have an approved design note.
- [ ] Temporary bridges or compromises have a planned debt entry.
- [ ] Failure modes are named if the work touches build, FFI, runtime, or
      memory ownership.

## Ready card template

```md
Title:

Owner:

Problem:

Target boundary or files:

Output:

Acceptance check:

Dependencies:

Blockers:

Design note required: yes | no

Design note link:

Debt entry required: yes | no
```
