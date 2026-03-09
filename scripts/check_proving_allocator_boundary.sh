#!/bin/sh

set -eu

target_files="
crates/stwo-cuda-sys/cuda/quotients.cu
crates/stwo-cuda-sys/cuda/eval_at_point.cu
crates/stwo-cuda-sys/cuda/evaluate_poseidon_constraint.cu
crates/stwo-cuda-sys/cuda/evaluate_wide_fibonacci.cu
crates/stwo-cuda-sys/cuda/prefix_sum.cu
crates/stwo-cuda-sys/cuda/fold_line.cu
crates/stwo-cuda-sys/cuda/fold_circle_into_line.cu
crates/stwo-cuda-sys/cuda/twiddles.cu
crates/stwo-cuda-sys/cuda/rfft.cu
crates/stwo-cuda-sys/cuda/ifft.cu
crates/stwo-cuda-sys/cuda/gkr.cu
crates/stwo-cuda-sys/cuda/accumulate.cu
crates/stwo-cuda-sys/cuda/poseidon252_starknet.cu
"

legacy_malloc_hits="$(
  rg -n "\\bcuda_malloc<|\\bclone_to_device<|\\bcuda_alloc_zeroes_uint32_t\\(|\\bcuda_free_memory\\(" ${target_files} || true
)"

if [ -n "$legacy_malloc_hits" ]; then
  echo "legacy generic allocator helpers remain in the proving allocator boundary files:"
  echo "$legacy_malloc_hits"
  exit 1
fi
