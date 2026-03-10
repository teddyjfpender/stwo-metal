#!/bin/sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$repo_root"

mode="${STWO_METAL_MODE:-metal-prod}"
plan_only="${STWO_BENCH_PLAN_ONLY:-0}"
log_n_instances="${STWO_BENCH_LOG_N_INSTANCES:-20}"
n_columns="${STWO_BENCH_N_COLUMNS:-100}"
warmups="${STWO_BENCH_WARMUPS:-1}"
samples="${STWO_BENCH_SAMPLES:-5}"
runner_class="${STWO_BENCH_RUNNER_CLASS:-manual-gpu}"
classification="${STWO_BENCH_CLASSIFICATION:-supported-benchmark-candidate}"
dependency_row="${STWO_BENCH_DEPENDENCY_ROW:-vendored-upstream-bridge-v1}"
results_root="${STWO_BENCH_RESULTS_ROOT:-logs/benchmarks}"
timestamp="$(date -u +%Y%m%dT%H%M%SZ)"
result_dir="${STWO_BENCH_RESULT_DIR:-$results_root/$timestamp}"
result_json="${STWO_BENCH_OUTPUT_JSON:-$result_dir/wide_fibonacci_trace.json}"

if [ "$plan_only" != "1" ] && [ "$mode" = "no-metal" ]; then
    echo "wide_fibonacci_trace benchmark requires STWO_METAL_MODE != no-metal unless STWO_BENCH_PLAN_ONLY=1." >&2
    exit 1
fi

if [ "$plan_only" != "1" ] && [ "${STWO_BENCH_SKIP_PREFLIGHT:-0}" != "1" ]; then
    env STWO_CUDA_MODE=no-cuda sh scripts/run_standalone_pinned_validation.sh
fi

mkdir -p "$result_dir"

gpu_name="unknown"
gpu_driver="unknown"
if command -v nvidia-smi >/dev/null 2>&1; then
    gpu_info="$(nvidia-smi --query-gpu=name,driver_version --format=csv,noheader | head -n 1 || true)"
    if [ -n "$gpu_info" ]; then
        gpu_name="$(printf '%s' "$gpu_info" | cut -d',' -f1 | sed 's/[[:space:]]*$//')"
        gpu_driver="$(printf '%s' "$gpu_info" | cut -d',' -f2 | sed 's/^[[:space:]]*//')"
    fi
fi

cuda_toolkit="unknown"
if command -v nvcc >/dev/null 2>&1; then
    cuda_toolkit="$(nvcc --version | tail -n 1 | sed 's/^[[:space:]]*//')"
fi

cpu_name="unknown"
if command -v sysctl >/dev/null 2>&1; then
    cpu_name="$(sysctl -n machdep.cpu.brand_string 2>/dev/null || true)"
fi
if [ -z "$cpu_name" ] || [ "$cpu_name" = "unknown" ]; then
    if command -v lscpu >/dev/null 2>&1; then
        cpu_name="$(LC_ALL=C lscpu | awk -F: '/Model name/ {sub(/^[ \t]+/, "", $2); print $2; exit}')"
    fi
fi
if [ -z "$cpu_name" ] || [ "$cpu_name" = "unknown" ]; then
    cpu_name="$(awk -F: '/model name/ {sub(/^[ \t]+/, "", $2); print $2; exit}' /proc/cpuinfo 2>/dev/null || true)"
fi
if [ -z "$cpu_name" ]; then
    cpu_name="unknown"
fi

thread_count="${RAYON_NUM_THREADS:-}"
if [ -z "$thread_count" ]; then
    if command -v nproc >/dev/null 2>&1; then
        thread_count="$(nproc)"
    elif command -v sysctl >/dev/null 2>&1; then
        thread_count="$(sysctl -n hw.ncpu 2>/dev/null || true)"
    fi
fi
case "$thread_count" in
    ''|*[!0-9]*)
        thread_count="1"
        ;;
esac

runtime_feature_args=""
if [ "$plan_only" != "1" ]; then
    runtime_feature_args="--features metal-runtime"
fi

command_string="cargo run --release --manifest-path fixtures/standalone-benchmarks/Cargo.toml --bin wide_fibonacci_trace $runtime_feature_args"
git_commit="${STWO_BENCH_GIT_COMMIT:-unknown}"
if [ "$git_commit" = "unknown" ] && git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    git_commit="$(git rev-parse HEAD)"
fi

STWO_METAL_MODE="$mode" \
STWO_BENCH_OUTPUT_JSON="$result_json" \
STWO_BENCH_PLAN_ONLY="$plan_only" \
STWO_BENCH_LOG_N_INSTANCES="$log_n_instances" \
STWO_BENCH_N_COLUMNS="$n_columns" \
STWO_BENCH_WARMUPS="$warmups" \
STWO_BENCH_SAMPLES="$samples" \
STWO_BENCH_RUNNER_CLASS="$runner_class" \
STWO_BENCH_CLASSIFICATION="$classification" \
STWO_BENCH_DEPENDENCY_ROW="$dependency_row" \
STWO_BENCH_GIT_COMMIT="$git_commit" \
STWO_BENCH_COMMAND="$command_string" \
STWO_BENCH_HOSTNAME="$(hostname)" \
STWO_BENCH_OS="$(uname -s)" \
STWO_BENCH_ARCH="$(uname -m)" \
STWO_BENCH_CPU="$cpu_name" \
STWO_BENCH_THREADS="$thread_count" \
STWO_BENCH_GPU_NAME="$gpu_name" \
STWO_BENCH_GPU_DRIVER="$gpu_driver" \
STWO_BENCH_CUDA_TOOLKIT="$cuda_toolkit" \
cargo run --release \
  --manifest-path fixtures/standalone-benchmarks/Cargo.toml \
  --bin wide_fibonacci_trace \
  ${runtime_feature_args:+$runtime_feature_args} \
  --locked

ruby scripts/render_benchmark_table.rb "$result_json"
