#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
fixture_manifest="$repo_root/fixtures/standalone-benchmarks/Cargo.toml"

if ! grep -Fq 'stwo-constraint-framework = { path = "../../vendor/stwo-upstream-dev-62b228e/crates/constraint-framework"' "$fixture_manifest"; then
  echo "supported benchmark fixture must use the vendored stwo-constraint-framework path" >&2
  exit 1
fi

if ! grep -Fq 'stwo-constraint-framework = { path = "../../vendor/stwo-upstream-dev-62b228e/crates/constraint-framework" }' "$fixture_manifest"; then
  echo "supported benchmark fixture must patch crates.io stwo-constraint-framework to the vendored path" >&2
  exit 1
fi
