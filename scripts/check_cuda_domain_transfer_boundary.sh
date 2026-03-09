#!/bin/sh

set -eu

target_file="crates/constraint-framework/src/cuda_domain.rs"

mask_body="$(
  awk '
    /fn next_interaction_mask/ { in_fn = 1 }
    in_fn { print }
    in_fn && /^    fn / && !/fn next_interaction_mask/ { exit }
    in_fn && /^    super::logup_proxy!\(\);/ { exit }
  ' "$target_file"
)"

if printf '%s\n' "$mask_body" | rg -n '\.to_cpu\(' -; then
  echo "hidden to_cpu() calls remain inside CudaDomainEvaluator::next_interaction_mask" >&2
  echo "use explicit CudaDomainTraceMaterialization instead of per-access host copies" >&2
  exit 1
fi

echo "CudaDomainEvaluator transfer boundary is explicit."
