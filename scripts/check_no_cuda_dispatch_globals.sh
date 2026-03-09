#!/usr/bin/env sh
set -eu

if git grep -n "g_should_accumulate_host" -- crates/stwo-cuda-sys/cuda; then
  echo
  echo "Mutable host-global accumulation state is not allowed in crates/stwo-cuda-sys/cuda."
  echo "Use explicit request-scoped execution policy instead of g_should_accumulate_host."
  exit 1
fi

echo "No forbidden CUDA dispatch globals detected."
