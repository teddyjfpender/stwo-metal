#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${repo_root}"

GCP_PROJECT="${GCP_PROJECT:-stwo-cuda-gpu-20260306}" \
GCP_ZONE="${GCP_ZONE:-us-central1-a}" \
GCP_MACHINE_TYPE="${GCP_MACHINE_TYPE:-n1-standard-4}" \
GCP_GPU_TYPE="${GCP_GPU_TYPE:-nvidia-tesla-t4}" \
GCP_GPU_COUNT="${GCP_GPU_COUNT:-1}" \
REMOTE_VALIDATION_SCRIPT="${REMOTE_VALIDATION_SCRIPT:-${repo_root}/scripts/run_remote_poseidon_point_contract_snapshot.sh}" \
  bash "${repo_root}/scripts/run_gcloud_cuda_validation.sh"
