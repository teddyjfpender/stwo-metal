#!/bin/sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$repo_root"

mode="${STWO_METAL_MODE:-metal-prod}"
results_root="${STWO_BENCH_RESULTS_ROOT:-logs/benchmarks}"
timestamp="$(date -u +%Y%m%dT%H%M%SZ)"
result_dir="${STWO_BENCH_RESULT_DIR:-$results_root/$timestamp/wide_fibonacci_metal_generic}"
warmups="${STWO_BENCH_WARMUPS:-0}"
samples="${STWO_BENCH_SAMPLES:-1}"
n_columns="${STWO_BENCH_N_COLUMNS:-100}"
runner_class="${STWO_BENCH_RUNNER_CLASS:-manual-metal-generic}"
classification="${STWO_BENCH_CLASSIFICATION:-generic-benchmark-candidate}"
dependency_row="${STWO_BENCH_DEPENDENCY_ROW:-metal-generic-lane-v1}"
lane="${STWO_BENCH_LANE:-generic-metal}"
log_sizes="${STWO_BENCH_LOG_SIZES:-16}"

mkdir -p "$result_dir"

for log_size in $log_sizes; do
    result_json="$result_dir/wide_fibonacci_prove_log${log_size}.json"
    STWO_METAL_MODE="$mode" \
    STWO_BENCH_LOG_N_INSTANCES="$log_size" \
    STWO_BENCH_N_COLUMNS="$n_columns" \
    STWO_BENCH_WARMUPS="$warmups" \
    STWO_BENCH_SAMPLES="$samples" \
    STWO_BENCH_RUNNER_CLASS="$runner_class" \
    STWO_BENCH_CLASSIFICATION="$classification" \
    STWO_BENCH_DEPENDENCY_ROW="$dependency_row" \
    STWO_BENCH_LANE="$lane" \
    STWO_BENCH_RESULT_DIR="$result_dir" \
    STWO_BENCH_OUTPUT_JSON="$result_json" \
    sh scripts/run_supported_wide_fibonacci_prove_benchmark.sh
done

ruby scripts/render_wide_fibonacci_comparison_table.rb \
  --lane "$lane" \
  --metal-label "Metal generic" \
  --summary "Wide Fibonacci, Blake2s channel (SIMD vs Metal generic lane)" \
  "$result_dir"/wide_fibonacci_prove_log*.json > "$result_dir/wide_fibonacci_comparison.md"
printf '%s\n' "$result_dir/wide_fibonacci_comparison.md"
