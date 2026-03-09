#!/bin/sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$repo_root"

vendor_root="vendor/stwo-upstream-dev-62b228e"
vendor_manifest="$vendor_root/crates/stwo/Cargo.toml"
vendor_prover_mod="$vendor_root/crates/stwo/src/prover/mod.rs"
vendor_ledger="$vendor_root/VENDORED.md"

for required in "$vendor_manifest" "$vendor_prover_mod" "$vendor_ledger"; do
    if [ ! -f "$required" ]; then
        echo "missing vendored bridge file: $required" >&2
        exit 1
    fi
done

if ! rg -q '^pub use pcs::quotient_ops::AccumulatedNumerators;$' "$vendor_prover_mod"; then
    echo "vendored bridge re-export is missing from $vendor_prover_mod" >&2
    exit 1
fi

workspace_manifest_matches=$(
    rg -n 'workspace = true' "$vendor_manifest" || true
)
if [ -n "$workspace_manifest_matches" ]; then
    echo "vendored bridge manifest still depends on upstream workspace inheritance:" >&2
    echo "$workspace_manifest_matches" >&2
    exit 1
fi

for expected in \
    'exact commit: `62b228ed4a30ef96715e201c4c6e0742aa8bbd42`' \
    'additive public re-export in `crates/stwo/src/prover/mod.rs`:' \
    'additive `PolyOps::batch_eval_at_point` default method in `crates/stwo/src/prover/poly/circle/ops.rs`' \
    'additive `CommitmentSchemeProver::prove_values` batching path in `crates/stwo/src/prover/pcs/mod.rs`' \
    'used only when `store_polynomials_coefficients` is enabled' \
    'preserves the existing barycentric path when coefficients are not stored' \
    'manifest self-containment in `crates/stwo/Cargo.toml` so Cargo can resolve the crate outside the original upstream workspace' \
    'removed benchmark target declarations in `crates/stwo/Cargo.toml` after pruning the vendored surface to the standalone crate manifest plus `src/`' \
    'this vendored snapshot is the current short-term supported bridge row for `stwo-cuda`'
do
    if ! rg -F -q "$expected" "$vendor_ledger"; then
        echo "vendored bridge ledger is missing expected line:" >&2
        echo "$expected" >&2
        exit 1
    fi
done

supported_probe_matches=$(
    rg -n 'upstream-dev-probe' \
        crates/stwo-cuda/src \
        crates/stwo-cuda/tests \
        fixtures/standalone-pinned/Cargo.toml \
        scripts/run_standalone_pinned_validation.sh \
        docs/compatibility-matrix.md \
        .github/workflows/ci.yaml \
        || true
)
if [ -n "$supported_probe_matches" ]; then
    echo "supported vendored bridge row still references the retired probe-era boundary:" >&2
    echo "$supported_probe_matches" >&2
    exit 1
fi
