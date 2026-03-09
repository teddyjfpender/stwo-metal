#!/bin/sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$repo_root"

matches=$(
    {
        rg -n "pub use_assert_evaluator" crates/stwo-cuda/src/backend/cuda/adapter.rs || true
        if [ -f crates/stwo-cuda/src/lib.rs ]; then
            rg -n "use_assert_evaluator" crates/stwo-cuda/src/lib.rs || true
        fi
    }
)

if [ -n "$matches" ]; then
    echo "found drift of the debug-only assert-evaluator flag into the supported Rust surface:"
    echo "$matches"
    exit 1
fi
