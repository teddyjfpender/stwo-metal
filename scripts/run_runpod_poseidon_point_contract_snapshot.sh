#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

RUNPOD_GPU_ID="${RUNPOD_GPU_ID:-NVIDIA GeForce RTX 5090}" \
RUNPOD_TEMPLATE_ID="${RUNPOD_TEMPLATE_ID:-runpod-torch-v280}" \
RUNPOD_CLOUD_TYPE="${RUNPOD_CLOUD_TYPE:-SECURE}" \
RUNPOD_DATA_CENTER_IDS="${RUNPOD_DATA_CENTER_IDS-US-IL-1,US-NC-1,EU-CZ-1,EU-RO-1,EUR-IS-1,EUR-IS-2,EUR-NO-1}" \
RUNPOD_USE_PUBLIC_IP="${RUNPOD_USE_PUBLIC_IP:-0}" \
RUNPOD_CREATE_RETRIES="${RUNPOD_CREATE_RETRIES:-10}" \
RUNPOD_CREATE_RETRY_SLEEP_SEC="${RUNPOD_CREATE_RETRY_SLEEP_SEC:-15}" \
RUNPOD_REMOTE_ENV="${RUNPOD_REMOTE_ENV:-STWO_CUDA_MODE=cuda-dev STWO_CUDA_DEBUG_PROVE_VALUES=1}" \
  bash "$repo_root/scripts/run_runpod_remote.sh" \
    --pod-name "${RUNPOD_POD_NAME:-stwo-cuda-m35-poseidon}" \
    --log-label "poseidon-point-contract" \
    -- \
    cargo test --manifest-path fixtures/standalone-benchmarks/Cargo.toml \
      --bin poseidon_prove \
      tests::supported_poseidon_prove_core_snapshot \
      --features cuda-runtime \
      --locked \
      -- \
      --ignored \
      --exact \
      --nocapture
