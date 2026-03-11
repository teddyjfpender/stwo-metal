#!/bin/sh

set -eu

if [ "$#" -ne 1 ]; then
  echo "usage: $0 <stark-v-checkout>" >&2
  exit 2
fi

repo=$1
root_dir=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
readiness_checker="$root_dir/scripts/check_stark_v_generated_readiness.sh"

readiness_output=$(sh "$readiness_checker" "$repo")

case "$readiness_output" in
  *"generated_artifact: absent"*)
    cat <<'EOF'
stark-v generated gap:
  missing:
    - producer_identity
    - component_identity
    - evaluation_abi
    - trace_layout
    - lookup_layout
    - degree_and_lifting_profile
    - domain_material
    - commitment_layout
    - query_layout
    - witness_hooks
    - specialization_keys
    - generated_inventory
  status: blocked
EOF
    ;;
  *)
    cat <<'EOF'
stark-v generated gap:
  missing: unknown
  status: manual_assessment_required
EOF
    ;;
esac
