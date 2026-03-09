# Upstream Source Pin

- Source repository: `/Users/theodorepender/Coding/stwo-upstream`
- Source commit: `a8fcf4bdde3778ae72f1e6cfe61a38e2911648d2`
- Imported on: `2026-03-09`

Notes:

- `src/` and `benches/` were copied from the upstream `crates/examples` crate.
- `Cargo.toml` was adapted locally so this vendored crate can build against the
  vendored `stwo` and `stwo-constraint-framework` crates in this repository.
- Example workload logic is intended to remain upstream-owned. Local changes
  should stay limited to vendoring, pinning, and backend-wiring acceptance
  harnesses.
