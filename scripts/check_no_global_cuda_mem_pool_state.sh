#!/bin/sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$repo_root"

matches=$(
    rg -n "\\bg_mem_pool\\b|\\bg_mem_pool_initialized\\b|cudaMemPoolCreate\\(" \
        crates/stwo-cuda-sys/cuda/cuda_mem_pool.cuh \
        crates/stwo-cuda-sys/cuda/cuda_mem_pool.cu \
        || true
)

if [ -n "$matches" ]; then
    echo "process-global custom CUDA mem-pool state remains in the supported allocator boundary:"
    echo "$matches"
    exit 1
fi
