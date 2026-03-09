#!/bin/sh
set -eu

echo "scripts/bench.sh is retired for the standalone repo." >&2
echo "Use scripts/run_supported_wide_fibonacci_prove_benchmark.sh for the primary checked supported-row benchmark entrypoint." >&2
echo "Use scripts/run_supported_wide_fibonacci_trace_benchmark.sh for the secondary checked trace micro-benchmark." >&2
exit 1
