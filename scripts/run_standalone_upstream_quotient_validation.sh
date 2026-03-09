#!/bin/sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$repo_root"

mode="${STWO_CUDA_MODE:-no-cuda}"
validate_mode="${STWO_UPSTREAM_QUOTIENT_VALIDATE_MODE:-test}"

case "$validate_mode" in
  check)
    STWO_CUDA_MODE="$mode" cargo check \
      --manifest-path fixtures/standalone-upstream-dev-quotient/Cargo.toml \
      --tests \
      --locked
    ;;
  test)
    STWO_CUDA_MODE="$mode" cargo test \
      --manifest-path fixtures/standalone-upstream-dev-quotient/Cargo.toml \
      --locked
    ;;
  *)
    echo "unsupported STWO_UPSTREAM_QUOTIENT_VALIDATE_MODE: $validate_mode" >&2
    exit 1
    ;;
esac
