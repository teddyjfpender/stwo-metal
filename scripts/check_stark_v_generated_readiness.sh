#!/bin/sh

set -eu

if [ "$#" -ne 1 ]; then
  echo "usage: $0 <stark-v-checkout>" >&2
  exit 2
fi

repo=$1

if [ ! -f "$repo/Cargo.toml" ]; then
  echo "missing workspace Cargo.toml: $repo/Cargo.toml" >&2
  exit 1
fi

has_pattern() {
  pattern=$1
  path=$2
  grep -Fq "$pattern" "$path"
}

cargo_toml="$repo/Cargo.toml"
prove_lib="$repo/crates/prover/src/lib.rs"
prove_rs="$repo/crates/prover/src/prover.rs"

for path in "$cargo_toml" "$prove_lib" "$prove_rs"; do
  if [ ! -f "$path" ]; then
    echo "missing required path: $path" >&2
    exit 1
  fi
done

artifact_signal_present=false

if rg -n "registration_key|abi_symbols|specialization_keys|schema_version" "$repo" -g '!target' >/dev/null 2>&1; then
  artifact_signal_present=true
fi

if [ "$artifact_signal_present" = true ]; then
  cat <<'EOF'
stark-v generated readiness:
  generated_artifact: detected
  generated_registration: review_required
  status: manual_assessment_required
EOF
  exit 0
fi

cat <<'EOF'
stark-v generated readiness:
  generated_artifact: absent
  generated_registration: unavailable
  status: blocked
EOF
