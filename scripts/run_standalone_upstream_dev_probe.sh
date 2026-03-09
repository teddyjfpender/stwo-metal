#!/bin/sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$repo_root"

mode="${STWO_CUDA_MODE:-no-cuda}"

STWO_CUDA_MODE="$mode" cargo check \
  --manifest-path fixtures/standalone-upstream-dev-probe/Cargo.toml \
  --tests \
  --locked
