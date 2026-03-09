#!/bin/sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$repo_root"

if [ -d "crates/stwo/src/stwo_cuda/cuda" ]; then
    echo "raw/native CUDA sources still live under crates/stwo/src/stwo_cuda/cuda"
    exit 1
fi

if [ -d "crates/stwo/src/stwo_cuda" ]; then
    echo "typed CUDA wrapper ownership still lives under crates/stwo/src/stwo_cuda"
    exit 1
fi

if [ -d "crates/stwo/src/prover/backend/cuda" ]; then
    echo "Rust CUDA backend ownership still lives under crates/stwo/src/prover/backend/cuda"
    exit 1
fi

if ! [ -d "crates/stwo-cuda-sys/cuda" ]; then
    echo "expected raw/native CUDA sources under crates/stwo-cuda-sys/cuda"
    exit 1
fi

ffi_hits=$(
    (
        if [ -f crates/stwo/src/stwo_cuda/bindings.rs ] || [ -f crates/stwo/src/stwo_cuda/bindings_airs.rs ]; then
            rg -n 'extern "C"' \
                crates/stwo/src/stwo_cuda/bindings.rs \
                crates/stwo/src/stwo_cuda/bindings_airs.rs \
                || true
            rg -n -F 'link(name = "stwo_cuda")' \
                crates/stwo/src/stwo_cuda/bindings.rs \
                crates/stwo/src/stwo_cuda/bindings_airs.rs \
                || true
        fi
    )
)

if [ -n "$ffi_hits" ]; then
    echo "raw CUDA extern declarations still live under crates/stwo:"
    echo "$ffi_hits"
    exit 1
fi

build_hits=$(
    if [ -f crates/stwo/build.rs ]; then
        rg -n 'cmake::Config|cargo:rustc-link-lib=static=stwo_cuda|cargo:rustc-link-search=native=|src/stwo_cuda/cuda|CUDAToolkit_ROOT|CUDA_HOME|CUDA_PATH' \
            crates/stwo/build.rs \
            || true
    fi
)

if [ -n "$build_hits" ]; then
    echo "raw/native build ownership still lives in crates/stwo/build.rs:"
    echo "$build_hits"
    exit 1
fi

if rg -n 'stwo-cuda-sys' crates/stwo/Cargo.toml >/dev/null 2>&1; then
    echo "crates/stwo/Cargo.toml still depends directly on stwo-cuda-sys"
    exit 1
fi
