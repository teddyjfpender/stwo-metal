#!/bin/sh

set -eu

target_file="crates/stwo-cuda-sys/cuda/pedersen_table_init.cu"

legacy_hits="$(
  rg -n "\\bcuda_mem_pool_allocate<|\\bcuda_mem_pool_free\\(|\\bcuda_malloc<|\\bclone_to_device<|\\bcuda_free_memory\\(" "${target_file}" || true
)"

if [ -n "${legacy_hits}" ]; then
  echo "legacy generic allocator helpers remain in pedersen_table_init.cu:"
  echo "${legacy_hits}"
  exit 1
fi
