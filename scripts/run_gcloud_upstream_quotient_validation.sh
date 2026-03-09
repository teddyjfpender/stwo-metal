#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REMOTE_VALIDATION_SCRIPT="${repo_root}/scripts/run_remote_upstream_quotient_validation.sh" \
  "${repo_root}/scripts/run_gcloud_cuda_validation.sh" "$@"
