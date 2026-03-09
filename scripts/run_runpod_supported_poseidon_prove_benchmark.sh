#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

result_dir="${STWO_BENCH_RESULT_DIR:-logs/benchmarks/m57-poseidon-post-fix}"
result_json="${STWO_BENCH_OUTPUT_JSON:-$result_dir/poseidon_prove.json}"

RUNPOD_GPU_ID="${RUNPOD_GPU_ID:-NVIDIA GeForce RTX 5090}" \
RUNPOD_TEMPLATE_ID="${RUNPOD_TEMPLATE_ID:-runpod-torch-v280}" \
RUNPOD_CLOUD_TYPE="${RUNPOD_CLOUD_TYPE:-SECURE}" \
RUNPOD_DATA_CENTER_IDS="${RUNPOD_DATA_CENTER_IDS-EU-CZ-1,EU-RO-1,EUR-IS-1,US-IL-1,US-NC-1}" \
RUNPOD_USE_PUBLIC_IP="${RUNPOD_USE_PUBLIC_IP:-0}" \
RUNPOD_CREATE_RETRIES="${RUNPOD_CREATE_RETRIES:-2}" \
RUNPOD_CREATE_RETRY_SLEEP_SEC="${RUNPOD_CREATE_RETRY_SLEEP_SEC:-10}" \
RUNPOD_REMOTE_ENV="${RUNPOD_REMOTE_ENV:-STWO_CUDA_MODE=cuda-dev STWO_BENCH_LOG_N_INSTANCES=7 STWO_BENCH_WARMUPS=1 STWO_BENCH_SAMPLES=1 STWO_BENCH_RUNNER_CLASS=benchmark-gpu-5090 STWO_BENCH_CLASSIFICATION=supported-benchmark-candidate STWO_BENCH_DEPENDENCY_ROW=vendored-upstream-bridge-v1 STWO_BENCH_RESULT_DIR=$result_dir STWO_BENCH_OUTPUT_JSON=$result_json}" \
  bash "$repo_root/scripts/run_runpod_remote.sh" \
    --pod-name "${RUNPOD_POD_NAME:-stwo-cuda-m57-poseidon}" \
    --log-label "supported-poseidon-benchmark" \
    --artifact "$result_dir" \
    -- \
    sh scripts/run_supported_poseidon_prove_benchmark.sh
