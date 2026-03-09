#!/bin/sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$repo_root"

mode="${STWO_CUDA_MODE:-no-cuda}"
validate_mode="${STWO_STANDALONE_VALIDATE_MODE:-check}"

case "$validate_mode" in
  check)
    STWO_CUDA_MODE="$mode" cargo check \
      --manifest-path fixtures/standalone-pinned/Cargo.toml \
      --tests \
      --locked
    ;;
  test)
    STWO_CUDA_MODE="$mode" cargo test \
      --manifest-path fixtures/standalone-pinned/Cargo.toml \
      --locked
    ;;
  *)
    echo "unsupported STWO_STANDALONE_VALIDATE_MODE: $validate_mode" >&2
    exit 1
    ;;
esac
