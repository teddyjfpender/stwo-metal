#!/bin/sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$repo_root"

mode="${STWO_CUDA_MODE:-no-cuda}"
validate_mode="${STWO_UPSTREAM_ACCUMULATION_VALIDATE_MODE:-check}"
manifest="fixtures/standalone-upstream-dev-accumulation/Cargo.toml"

case "$validate_mode" in
  check)
    STWO_CUDA_MODE="$mode" cargo check \
      --manifest-path "$manifest" \
      --tests \
      --locked
    ;;
  test)
    STWO_CUDA_MODE="$mode" cargo test \
      --manifest-path "$manifest" \
      tests::accumulation_probe_lift_and_accumulate_matches_cpu \
      -- --exact --nocapture
    ;;
  *)
    echo "unsupported STWO_UPSTREAM_ACCUMULATION_VALIDATE_MODE: $validate_mode" >&2
    exit 1
    ;;
esac
