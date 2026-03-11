#!/bin/sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$repo_root"

results_root="${STWO_BENCH_RESULTS_ROOT:-logs/benchmarks}"
timestamp="$(date -u +%Y%m%dT%H%M%SZ)"
parent_dir="${STWO_BENCH_RESULT_DIR:-$results_root/$timestamp/wide_fibonacci_dual_lane}"
generated_dir="$parent_dir/generated"
generic_dir="$parent_dir/generic"
report_path="$parent_dir/wide_fibonacci_dual_lane_report.md"

mkdir -p "$generated_dir" "$generic_dir"

STWO_BENCH_RESULT_DIR="$generated_dir" \
sh scripts/run_supported_wide_fibonacci_metal_sweep.sh >/dev/null

STWO_BENCH_RESULT_DIR="$generic_dir" \
sh scripts/run_supported_wide_fibonacci_generic_sweep.sh >/dev/null

ruby scripts/render_wide_fibonacci_dual_lane_report.rb \
  "$generated_dir" \
  "$generic_dir" > "$report_path"

printf '%s\n' "$report_path"
