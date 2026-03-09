#!/bin/sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$repo_root"

repo="${STWO_UPSTREAM_GITHUB_REPO:-starkware-libs/stwo}"
ref="${STWO_UPSTREAM_REF:-dev}"
mode="${STWO_UPSTREAM_HOOK_MODE:-require}"

case "$mode" in
    require|report)
        ;;
    *)
        echo "unsupported STWO_UPSTREAM_HOOK_MODE: $mode" >&2
        echo "expected one of: require, report" >&2
        exit 1
        ;;
esac

github_url="https://github.com/${repo}.git"
raw_url="https://raw.githubusercontent.com/${repo}/${ref}/crates/stwo/src/prover/mod.rs"
required_line='pub use pcs::quotient_ops::AccumulatedNumerators;'

resolved_ref="$ref"
if resolved=$(git ls-remote "$github_url" "refs/heads/$ref" 2>/dev/null | awk 'NR == 1 { print $1 }'); then
    if [ -n "$resolved" ]; then
        resolved_ref="$resolved"
    fi
fi

contents=$(curl -fsSL "$raw_url")

if printf '%s\n' "$contents" | rg -F -q "$required_line"; then
    echo "upstream hook ready:"
    echo "  repo: $repo"
    echo "  ref: $ref"
    echo "  resolved: $resolved_ref"
    echo "  required line present: yes"
    exit 0
fi

echo "upstream hook not ready:"
echo "  repo: $repo"
echo "  ref: $ref"
echo "  resolved: $resolved_ref"
echo "  required line present: no"

if [ "$mode" = "report" ]; then
    exit 0
fi

exit 1
