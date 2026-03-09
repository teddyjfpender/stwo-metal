# WIP Policy

Keep the program narrow enough that decisions stay explicit.

## Limits

- one active tranche in [`controller.md`](./controller.md)
- at most one supporting lane
- no parallel implementation across multiple unresolved design questions

## Rules

- do design before broad implementation when contracts are unclear
- do not mix cleanup, interface redesign, and backend translation in one cut
- when a task surfaces new scope, either defer it or rewrite the controller

## Escalation rule

Pause and update the controller if a change would:

- alter the public API
- add or widen unsafe or FFI assumptions
- redefine success criteria for the first Metal milestone
