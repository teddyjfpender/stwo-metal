#!/bin/sh

set -eu

root_dir=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
report_dir=$(mktemp -d "${TMPDIR:-/tmp}/stark-v-fail-closed.XXXXXX")
trap 'rm -rf "$report_dir"' EXIT

sh "$root_dir/scripts/run_vendored_stark_v_hardening_report.sh" "$report_dir" >/dev/null

report_json="$report_dir/stark_v_hardening_report.json"
report_md="$report_dir/stark_v_hardening_report.md"

for path in "$report_json" "$report_md"; do
  if [ ! -f "$path" ]; then
    echo "missing report artifact: $path" >&2
    exit 1
  fi
done

grep -Fq '"generic_lane": "unsupported"' "$report_json"
grep -Fq '"generated_lane": "required"' "$report_json"
grep -Fq '"status": "fail_closed"' "$report_json"
grep -Fq '"generated_artifact": "absent"' "$report_json"
grep -Fq 'Generated gap' "$report_md"

echo "vendored stark-v fail-closed row passed"
