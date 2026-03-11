# stark-v Upstream Source

- Repository: `https://github.com/AntoineFONDEUR/stark-v`
- Pinned HEAD: `3a3cb4cf576d7d7e8ca82815acfb31bbc10e48ef`
- Vendored on: `2026-03-11`
- Vendoring rule: read-only downstream hardening input for `stwo-metal` G8

## Purpose

This checkout is vendored only to give `stwo-metal` one deterministic local
downstream hardening input.

It does **not** imply that the vendored `stark-v` snapshot is supported by
`stwo-metal`.

Current checked status:

- generic lane: unsupported
- generated lane: required
- status: fail_closed

See:

- [`../../docs/dn-0004-stark-v-hardening-input-and-contract.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0004-stark-v-hardening-input-and-contract.md)
- [`../../docs/dn-0005-stark-v-attachment-strategy.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0005-stark-v-attachment-strategy.md)
- [`../../docs/dn-0006-stark-v-generated-minimum-contract.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/dn-0006-stark-v-generated-minimum-contract.md)
