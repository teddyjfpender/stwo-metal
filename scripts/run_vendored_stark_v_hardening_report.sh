#!/bin/sh

set -eu

root_dir=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
vendored_repo="$root_dir/vendor/stark-v-pinned-3a3cb4cf576d7d7e8ca82815acfb31bbc10e48ef"

if [ ! -f "$vendored_repo/Cargo.toml" ]; then
  echo "missing vendored stark-v checkout: $vendored_repo" >&2
  exit 1
fi

exec sh "$root_dir/scripts/run_stark_v_hardening_report.sh" "$vendored_repo" "$@"
