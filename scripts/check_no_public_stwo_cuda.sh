#!/bin/sh
set -eu

if [ -d crates/stwo/src/stwo_cuda ]; then
  echo "typed CUDA wrapper ownership still lives under crates/stwo/src/stwo_cuda" >&2
  exit 1
fi

if [ -f crates/stwo/src/lib.rs ]; then
  if rg -n '^pub mod stwo_cuda;|^mod stwo_cuda;' crates/stwo/src/lib.rs; then
    echo "stwo must not retain a crate-root stwo_cuda module." >&2
    exit 1
  fi
fi

if rg -n 'stwo::stwo_cuda' crates -g '*.rs'; then
  echo "Direct code dependencies on stwo::stwo_cuda are not allowed." >&2
  echo "Depend on stwo-cuda exports instead." >&2
  exit 1
fi

echo "No stwo-owned stwo_cuda module or direct stwo::stwo_cuda dependency remains."
