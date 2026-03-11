#!/bin/sh

set -eu

if [ "$#" -ne 1 ]; then
  echo "usage: $0 <stark-v-checkout>" >&2
  exit 2
fi

repo=$1

prove_lib="$repo/crates/prover/src/lib.rs"
prove_rs="$repo/crates/prover/src/prover.rs"
macro_components="$repo/crates/stwo-macros/src/components.rs"
workspace_toml="$repo/Cargo.toml"

for path in "$prove_lib" "$prove_rs" "$macro_components" "$workspace_toml"; do
  if [ ! -f "$path" ]; then
    echo "missing required path: $path" >&2
    exit 1
  fi
done

require_pattern() {
  pattern=$1
  path=$2
  if ! grep -Fq "$pattern" "$path"; then
    echo "missing required pattern '$pattern' in $path" >&2
    exit 1
  fi
}

require_pattern "external/stwo/crates/stwo" "$workspace_toml"
require_pattern "SimdBackend::precompute_twiddles" "$prove_rs"
require_pattern "SimdBackend::grind" "$prove_rs"
require_pattern "Poly<stwo::prover::backend::simd::SimdBackend>" "$prove_lib"
require_pattern "MerkleProverLifted<" "$prove_lib"
require_pattern "ComponentProver<SimdBackend>" "$macro_components"
require_pattern "CircleEvaluation<SimdBackend, BaseField, BitReversedOrder>" "$macro_components"

cat <<'EOF'
stark-v attachment classification:
  generic_lane: unsupported
  generated_lane: required
  status: fail_closed
EOF
