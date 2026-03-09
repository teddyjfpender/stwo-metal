#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${repo_root}"

if [[ -e old-stwo ]]; then
  echo "old-stwo was retired in M22 and must not exist in the active repository." >&2
  exit 1
fi

if rg -n "old-stwo/Cargo.toml|run_cuda_quotient_parity_profile.sh|run_cuda_smoke_profile.sh|run_cuda_independent_process_concurrency.sh" \
  .github/workflows/ci.yaml \
  scripts/run_remote_cuda_validation.sh \
  scripts/run_gcloud_cuda_validation.sh >/dev/null; then
  echo "Active supported-row validation still references retired old-stwo boundaries." >&2
  exit 1
fi
