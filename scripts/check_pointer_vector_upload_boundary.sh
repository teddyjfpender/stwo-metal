#!/bin/sh

set -eu

raw_upload_hits="$(
  rg -n "copy_device_pointer_vec_from_host_to_device\\(" crates/stwo-cuda/src/backend/cuda \
    | grep -v "pointer_vec.rs" || true
)"

if [ -n "$raw_upload_hits" ]; then
  echo "raw pointer-vector upload FFI escaped the wrapper boundary:"
  echo "$raw_upload_hits"
  exit 1
fi

raw_release_hits="$(
  rg -n "cuda_release_uploaded_pointer_vec\\(" crates/stwo-cuda/src/backend/cuda \
    | grep -v "pointer_vec.rs" || true
)"

if [ -n "$raw_release_hits" ]; then
  echo "raw pointer-vector release FFI escaped the wrapper boundary:"
  echo "$raw_release_hits"
  exit 1
fi

legacy_free_hits="$(
  rg -n "cuda_free_memory\\(" \
    crates/stwo-cuda/src/backend/cuda/quotient.rs \
    crates/stwo-cuda/src/backend/cuda/poly.rs \
    crates/stwo-cuda/src/backend/cuda/blake2s.rs \
    crates/stwo-cuda/src/backend/cuda/poseidon252.rs || true
)"

if [ -n "$legacy_free_hits" ]; then
  echo "legacy generic free remains in the migrated pointer-vector callers:"
  echo "$legacy_free_hits"
  exit 1
fi
