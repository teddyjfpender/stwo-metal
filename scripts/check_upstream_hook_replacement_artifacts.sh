#!/bin/sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$repo_root"

hook_patch="docs/artifacts/m24-minimal-upstream-hook.patch"
manifest_patch="docs/artifacts/m24-supported-row-cutover-manifests.patch"
runbook="docs/ops/m24-pure-upstream-cutover-runbook.md"
surface="docs/mappings/m24-pure-upstream-cutover-surface.md"

for required in "$hook_patch" "$manifest_patch" "$runbook" "$surface"; do
    if [ ! -f "$required" ]; then
        echo "missing upstream-hook replacement artifact: $required" >&2
        exit 1
    fi
done

if ! rg -F -q 'pub use pcs::quotient_ops::AccumulatedNumerators;' "$hook_patch"; then
    echo "minimal upstream hook patch does not contain the required re-export." >&2
    exit 1
fi

if rg -F -q 'pub mod pcs;' "$hook_patch"; then
    echo "minimal upstream hook patch widens visibility beyond the agreed re-export." >&2
    exit 1
fi

if ! rg -F -q 'crates/stwo-cuda/Cargo.toml' "$manifest_patch"; then
    echo "manifest cutover artifact does not include stwo-cuda manifest rewiring." >&2
    exit 1
fi

if ! rg -F -q 'fixtures/standalone-pinned/Cargo.toml' "$manifest_patch"; then
    echo "manifest cutover artifact does not include standalone pinned fixture rewiring." >&2
    exit 1
fi

for required_line in \
    "cargo check --workspace --tests --features='tracing, prover' --locked" \
    "env STWO_CUDA_MODE=no-cuda sh scripts/run_standalone_pinned_validation.sh" \
    "GCP_PROJECT=stwo-cuda-gpu-20260306 ./scripts/run_gcloud_cuda_validation.sh"
do
    if ! rg -F -q "$required_line" "$runbook" "$surface"; then
        echo "cutover artifact set is missing required validation command:" >&2
        echo "$required_line" >&2
        exit 1
    fi
done
