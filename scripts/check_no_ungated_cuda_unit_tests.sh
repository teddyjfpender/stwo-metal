#!/bin/sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$repo_root"

matches=$(
    rg -n '#\[cfg\(test\)\]|#\[cfg\(all\(test, not\(target_arch = "wasm32"\)\)\)\]' \
        crates/stwo-cuda/src/stwo_cuda/base_field_vec.rs \
        crates/stwo-cuda/src/stwo_cuda/bindings.rs \
        crates/stwo-cuda/src/stwo_cuda/secure_field_vec.rs \
        crates/stwo-cuda/src/backend/cuda/accumulation.rs \
        crates/stwo-cuda/src/backend/cuda/blake2s.rs \
        crates/stwo-cuda/src/backend/cuda/column.rs \
        crates/stwo-cuda/src/backend/cuda/fri.rs \
        crates/stwo-cuda/src/backend/cuda/lookups/gkr.rs \
        crates/stwo-cuda/src/backend/cuda/lookups/mle.rs \
        crates/stwo-cuda/src/backend/cuda/poly.rs \
        crates/stwo-cuda/src/backend/cuda/poseidon252.rs \
        crates/stwo-cuda/src/backend/cuda/quotient.rs \
        || true
)

if [ -n "$matches" ]; then
    echo "found CUDA in-crate unit tests that are not gated by stwo_cuda_link:"
    echo "$matches"
    exit 1
fi
