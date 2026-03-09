#!/bin/sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$repo_root"

if [ -e crates/stwo-cuda-sys/cuda/legacy_request_compat.cuh ]; then
    echo "legacy request compatibility bridge still exists:"
    echo "crates/stwo-cuda-sys/cuda/legacy_request_compat.cuh"
    exit 1
fi

legacy_matches=$(
    rg -n "legacy_request_compat|legacy_trace_evaluations|legacy_random_coeff_powers|legacy_denominator_inverses|legacy_eval_ptr" \
        crates/stwo-cuda-sys/cuda \
        || true
)

if [ -n "$legacy_matches" ]; then
    echo "legacy request compatibility helpers are still referenced in the native CUDA surface:"
    echo "$legacy_matches"
    exit 1
fi

mutable_signature_matches=$(
    rg -n -P "(?<!const )\\bvoid \\*eval\\b|(?<!const )\\bqm31 \\*random_coeff_powers\\b|(?<!const )\\bm31 \\*denominator_inverses\\b|\\bm31 \\*\\*trace[012]_evaluations\\b" \
        crates/stwo-cuda-sys/cuda \
        -g '*.[ch]u*' \
        || true
)

if [ -n "$mutable_signature_matches" ]; then
    echo "mutable request/evaluator signatures remain in the native CUDA surface:"
    echo "$mutable_signature_matches"
    exit 1
fi
