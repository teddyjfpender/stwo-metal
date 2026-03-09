# Vendored Upstream Bridge Snapshot

Origin:

- upstream repository: `https://github.com/starkware-libs/stwo.git`
- exact commit: `62b228ed4a30ef96715e201c4c6e0742aa8bbd42`

Retained surface:

- `crates/stwo/Cargo.toml`
- `crates/stwo/src/`
- `crates/constraint-framework/Cargo.toml`
- `crates/constraint-framework/src/`

Local delta:

- additive public re-export in `crates/stwo/src/prover/mod.rs`:
  - `pub use pcs::quotient_ops::AccumulatedNumerators;`
  - frozen as the minimal upstreamable hook artifact in [`docs/artifacts/m24-minimal-upstream-hook.patch`](/Users/theodorepender/Coding/gpu-acc/stwo-cuda/docs/artifacts/m24-minimal-upstream-hook.patch)
- additive `PolyOps::batch_eval_at_point` default method in `crates/stwo/src/prover/poly/circle/ops.rs`
  - preserves scalar `eval_at_point` semantics by default and allows backends to provide batched same-point evaluation
- additive `CommitmentSchemeProver::prove_values` batching path in `crates/stwo/src/prover/pcs/mod.rs`
  - used only when `store_polynomials_coefficients` is enabled
  - preserves the existing barycentric path when coefficients are not stored
- manifest self-containment in `crates/stwo/Cargo.toml` so Cargo can resolve the crate outside the original upstream workspace
- manifest self-containment in `crates/constraint-framework/Cargo.toml` so Cargo can resolve the crate outside the original upstream workspace
- additive point-side `next_extension_interaction_mask` override in `crates/constraint-framework/src/point.rs`
  - preserves the meaning of already-extended `SecureField` masks at OODS points
  - avoids recombining point-side extension values as if they were base-field limbs
- removed benchmark target declarations in `crates/stwo/Cargo.toml` after pruning the vendored surface to the standalone crate manifest plus `src/`

Scope rule:

- this vendored snapshot is the current short-term supported bridge row for `stwo-cuda`
- it must remain a bounded, explicit delta over upstream `starkware-libs/stwo@62b228ed4a30ef96715e201c4c6e0742aa8bbd42`
- it is not the long-term destination; the long-term target remains a pure upstream pin once the required public hook lands
