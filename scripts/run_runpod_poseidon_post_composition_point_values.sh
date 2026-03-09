#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

RUNPOD_GPU_ID="${RUNPOD_GPU_ID:-NVIDIA GeForce RTX 5090}" \
RUNPOD_TEMPLATE_ID="${RUNPOD_TEMPLATE_ID:-runpod-torch-v280}" \
RUNPOD_CLOUD_TYPE="${RUNPOD_CLOUD_TYPE:-COMMUNITY}" \
RUNPOD_REMOTE_ENV="${RUNPOD_REMOTE_ENV:-STWO_CUDA_MODE=cuda-dev}" \
  bash "$repo_root/scripts/run_runpod_remote.sh" \
    --pod-name "${RUNPOD_POD_NAME:-stwo-cuda-m45-poseidon-post-composition}" \
    --log-label "poseidon-post-composition-point-values" \
    -- \
    cargo test --manifest-path fixtures/standalone-benchmarks/Cargo.toml \
      --bin poseidon_prove \
      tests::supported_poseidon_trace_point_values_survive_composition_commit \
      --features cuda-runtime \
      --locked \
      -- \
      --ignored \
      --exact \
      --nocapture
