#!/bin/sh

set -eu

if [ "$#" -lt 1 ] || [ "$#" -gt 2 ]; then
  echo "usage: $0 <stark-v-checkout> [output-dir]" >&2
  exit 2
fi

repo=$1
output_dir=${2:-}

root_dir=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
contract_checker="$root_dir/scripts/check_stark_v_contract.sh"
attachment_checker="$root_dir/scripts/check_stark_v_attachment.sh"
generated_checker="$root_dir/scripts/check_stark_v_generated_readiness.sh"
generated_gap_checker="$root_dir/scripts/check_stark_v_generated_gap.sh"

if [ ! -d "$repo/.git" ] && [ ! -f "$repo/Cargo.toml" ]; then
  echo "invalid stark-v checkout: $repo" >&2
  exit 1
fi

if [ -z "$output_dir" ]; then
  timestamp=$(date -u +"%Y%m%dT%H%M%SZ")
  output_dir="$root_dir/logs/hardening/$timestamp/stark-v"
fi

mkdir -p "$output_dir"

contract_output=$(sh "$contract_checker" "$repo")
attachment_output=$(sh "$attachment_checker" "$repo")
generated_output=$(sh "$generated_checker" "$repo")
generated_gap_output=$(sh "$generated_gap_checker" "$repo")

source_note="$repo/STARK_V_UPSTREAM_SOURCE.md"
if [ -d "$repo/.git" ]; then
  repo_head=$(git -C "$repo" rev-parse HEAD 2>/dev/null || echo "unknown")
elif [ -f "$source_note" ]; then
  repo_head=$(sed -n 's/^- Pinned HEAD: `\(.*\)`$/\1/p' "$source_note")
  if [ -z "$repo_head" ]; then
    repo_head="unknown"
  fi
else
  repo_head="unknown"
fi

report_md="$output_dir/stark_v_hardening_report.md"
report_json="$output_dir/stark_v_hardening_report.json"

cat >"$report_md" <<EOF
# stark-v Hardening Report

- Date (UTC): \`$(date -u +"%Y-%m-%dT%H:%M:%SZ")\`
- Repository: \`$repo\`
- HEAD: \`$repo_head\`

## Contract check

\`\`\`
$contract_output
\`\`\`

## Attachment classification

\`\`\`
$attachment_output
\`\`\`

## Generated readiness

\`\`\`
$generated_output
\`\`\`

## Generated gap

\`\`\`
$generated_gap_output
\`\`\`

## Current result

- generic lane: unsupported
- generated lane: required
- status: fail_closed

## Blocking evidence

- \`crates/prover/src/prover.rs\` hardcodes \`SimdBackend::precompute_twiddles\`
- \`crates/prover/src/prover.rs\` hardcodes \`SimdBackend::grind\`
- \`crates/prover/src/lib.rs\` reconstructs preprocessing into \`Poly<SimdBackend>\`
- \`crates/stwo-macros/src/components.rs\` emits \`ComponentProver<SimdBackend>\`
- workspace \`Cargo.toml\` points at vendored \`external/stwo\`
- no machine-readable generated artifact contract is detectable in the pinned
  checkout
- the full minimum generated subset from \`DN-0006\` is still absent

## Required next supported row

- backend-parametric proving surface, or
- generated artifact satisfying the \`stwo-metal\` registration contract
EOF

cat >"$report_json" <<EOF
{
  "repo": "$repo",
  "head": "$repo_head",
  "generic_lane": "unsupported",
  "generated_lane": "required",
  "status": "fail_closed",
  "generated_artifact": "absent"
}
EOF

echo "stark-v hardening report written to $output_dir"
