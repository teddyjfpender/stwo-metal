#!/bin/sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$repo_root"

tmp_dir="$(mktemp -d "${TMPDIR:-/tmp}/stwo-cuda-bench-check.XXXXXX")"
trap 'rm -rf "$tmp_dir"' EXIT INT TERM

STWO_CUDA_MODE=no-cuda cargo check \
  --manifest-path fixtures/standalone-benchmarks/Cargo.toml \
  --features cuda-runtime \
  --bins \
  --locked

STWO_CUDA_MODE=no-cuda \
STWO_BENCH_PLAN_ONLY=1 \
STWO_BENCH_OUTPUT_JSON="$tmp_dir/wide_fibonacci_trace.json" \
sh scripts/run_supported_wide_fibonacci_trace_benchmark.sh >/dev/null

STWO_CUDA_MODE=no-cuda \
STWO_BENCH_PLAN_ONLY=1 \
STWO_BENCH_OUTPUT_JSON="$tmp_dir/wide_fibonacci_prove.json" \
sh scripts/run_supported_wide_fibonacci_prove_benchmark.sh >/dev/null

STWO_CUDA_MODE=no-cuda \
STWO_BENCH_PLAN_ONLY=1 \
STWO_BENCH_OUTPUT_JSON="$tmp_dir/poseidon_prove.json" \
sh scripts/run_supported_poseidon_prove_benchmark.sh >/dev/null

test -f "$tmp_dir/wide_fibonacci_trace.json"
test -f "$tmp_dir/wide_fibonacci_prove.json"
test -f "$tmp_dir/poseidon_prove.json"

if ! rg -F -q '"benchmark_id": "wide_fibonacci_trace_generation_v1"' "$tmp_dir/wide_fibonacci_trace.json"; then
    echo "benchmark harness output is missing the expected benchmark id." >&2
    exit 1
fi

if ! rg -F -q '"status": "planned"' "$tmp_dir/wide_fibonacci_trace.json"; then
    echo "benchmark harness plan-only output is missing the planned status." >&2
    exit 1
fi

if ! rg -F -q '"benchmark_id": "wide_fibonacci_prove_verify_v1"' "$tmp_dir/wide_fibonacci_prove.json"; then
    echo "prove benchmark harness output is missing the expected benchmark id." >&2
    exit 1
fi

if ! rg -F -q '"status": "planned"' "$tmp_dir/wide_fibonacci_prove.json"; then
    echo "prove benchmark harness plan-only output is missing the planned status." >&2
    exit 1
fi

if ! rg -F -q '"benchmark_lane": "generated-metal"' "$tmp_dir/wide_fibonacci_prove.json"; then
    echo "prove benchmark harness plan-only output is missing the generated-metal lane." >&2
    exit 1
fi

if ! rg -F -q '"benchmark_id": "poseidon_prove_verify_v1"' "$tmp_dir/poseidon_prove.json"; then
    echo "poseidon prove benchmark harness output is missing the expected benchmark id." >&2
    exit 1
fi

if ! rg -F -q '"status": "planned"' "$tmp_dir/poseidon_prove.json"; then
    echo "poseidon prove benchmark harness plan-only output is missing the planned status." >&2
    exit 1
fi

if ! rg -F -q '"benchmark_lane": "legacy-cuda-compatible"' "$tmp_dir/poseidon_prove.json"; then
    echo "poseidon prove benchmark harness plan-only output is missing the legacy-cuda-compatible lane." >&2
    exit 1
fi

if [ "${STWO_BENCH_REQUIRE_HISTORICAL_ARTIFACTS:-0}" = "1" ]; then
    ruby scripts/check_benchmark_class_acceptance.rb \
      primary \
      docs/artifacts/benchmarks/m30-supported-row-wide-fibonacci-prove-20260308T190332Z.json >/dev/null

    ruby scripts/check_benchmark_class_acceptance.rb \
      supporting \
      docs/artifacts/benchmarks/m30-supported-row-wide-fibonacci-prove-jitter-20260308T190352Z.json >/dev/null

    ruby scripts/check_benchmark_class_acceptance.rb \
      supporting \
      docs/artifacts/benchmarks/m30-supported-row-wide-fibonacci-prove-jitter-20260308T190419Z.json >/dev/null
fi
