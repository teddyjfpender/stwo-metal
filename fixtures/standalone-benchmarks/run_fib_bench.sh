#!/usr/bin/env bash
# Benchmark fib(n) across Metal and SIMD backends.
#
# Usage: ./run_fib_bench.sh [--sizes "32768 65536 ..."] [--warmup N] [--iters N] [--metal-only|--simd-only]
#
# Outputs a formatted comparison table with cycles/second throughput.
# Each size runs (warmup + iters) iterations; only measurement iterations
# are used for the reported median.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
TEST_DATA="$SCRIPT_DIR/test_data"

# Defaults.
FIB_SIZES="32768 65536 131072 262144 524288 1048576 2097152 4194304"
N_WARMUP=1
N_ITERS=3
RUN_METAL=true
RUN_SIMD=true

# Parse args.
while [[ $# -gt 0 ]]; do
    case $1 in
        --sizes) FIB_SIZES="$2"; shift 2;;
        --warmup) N_WARMUP="$2"; shift 2;;
        --iters) N_ITERS="$2"; shift 2;;
        --metal-only) RUN_SIMD=false; shift;;
        --simd-only) RUN_METAL=false; shift;;
        *) echo "Unknown flag: $1"; exit 1;;
    esac
done

TOTAL_RUNS=$((N_WARMUP + N_ITERS))

# Build the cairo_prove binary.
echo "Building cairo_prove with metal-runtime + cairo-prove (release)..."
cd "$SCRIPT_DIR"
STWO_METAL_MODE=metal-prod cargo build --features metal-runtime,cairo-prove --bin cairo_prove --release 2>&1 | tail -3
BINARY="$SCRIPT_DIR/target/release/cairo_prove"

echo ""
echo "Configuration: warmup=$N_WARMUP, measurement=$N_ITERS, metal=$RUN_METAL, simd=$RUN_SIMD"
echo ""

# Results file (JSON lines for post-processing).
RESULTS_FILE="$SCRIPT_DIR/bench_results_$(date +%Y%m%d_%H%M%S).jsonl"

# Build bench flags.
BENCH_FLAGS=""
if $RUN_METAL; then BENCH_FLAGS="$BENCH_FLAGS --metal"; fi
if $RUN_SIMD; then BENCH_FLAGS="$BENCH_FLAGS --verify"; fi

# Compute statistics from a JSON array of floats.
# Usage: compute_stats '[1.0,2.0,3.0]' warmup_count
# Outputs: median min max (of measurement runs only, after dropping warmup)
compute_stats() {
    python3 -c "
import json, sys
times = json.loads('$1')
warmup = $2
measure = times[warmup:]
if not measure:
    print('- - -')
    sys.exit()
measure.sort()
n = len(measure)
if n % 2 == 1:
    median = measure[n // 2]
else:
    median = (measure[n // 2 - 1] + measure[n // 2]) / 2
print(f'{median:.1f} {measure[0]:.1f} {measure[-1]:.1f}')
"
}

# Header.
echo ""
echo "╔═══════════════════════════════════════════════════════════════════════════════════════════════════════════╗"
echo "║                                   fib(n) Proving Benchmark Results                                     ║"
echo "╠═══════════════════════════════════════════════════════════════════════════════════════════════════════════╣"
printf "║ %-11s │ %10s │ %10s %10s %10s │ %10s %10s %10s │ %7s ║\n" \
    "fib(n)" "cycles" "Metal(ms)" "kHz" "min-max" "SIMD(ms)" "kHz" "min-max" "speedup"
echo "╠═══════════════════════════════════════════════════════════════════════════════════════════════════════════╣"

for N in $FIB_SIZES; do
    INPUT="$TEST_DATA/cairo_fib_$N/prover_input.json"
    if [ ! -f "$INPUT" ]; then
        echo "  SKIP fib($N): $INPUT not found" >&2
        continue
    fi

    echo "  benchmarking fib($N)..." >&2

    # Run all iterations in one process (warmup + measurement).
    OUTPUT=$("$BINARY" --bench "$TOTAL_RUNS" $BENCH_FLAGS "$INPUT" 2>&1)
    JSON_LINE=$(echo "$OUTPUT" | grep "^BENCH_JSON " | sed 's/^BENCH_JSON //')

    if [ -z "$JSON_LINE" ]; then
        echo "  ERROR: no BENCH_JSON output for fib($N)" >&2
        continue
    fi

    CYCLES=$(echo "$JSON_LINE" | python3 -c "import json,sys; print(json.load(sys.stdin)['cycles'])")
    METAL_ARR=$(echo "$JSON_LINE" | python3 -c "import json,sys; d=json.load(sys.stdin); print(json.dumps(d['metal_ms']) if d['metal_ms'] else 'null')")
    SIMD_ARR=$(echo "$JSON_LINE" | python3 -c "import json,sys; d=json.load(sys.stdin); print(json.dumps(d['simd_ms']) if d['simd_ms'] else 'null')")

    METAL_MEDIAN="-"; METAL_KHZ="-"; METAL_RANGE="-"
    SIMD_MEDIAN="-"; SIMD_KHZ="-"; SIMD_RANGE="-"
    SPEEDUP="-"

    if [ "$METAL_ARR" != "null" ]; then
        read METAL_MEDIAN METAL_MIN METAL_MAX <<< $(compute_stats "$METAL_ARR" "$N_WARMUP")
        METAL_KHZ=$(python3 -c "print(f'{$CYCLES / ($METAL_MEDIAN / 1000) / 1000:.1f}')")
        METAL_RANGE="${METAL_MIN}-${METAL_MAX}"
    fi

    if [ "$SIMD_ARR" != "null" ]; then
        read SIMD_MEDIAN SIMD_MIN SIMD_MAX <<< $(compute_stats "$SIMD_ARR" "$N_WARMUP")
        SIMD_KHZ=$(python3 -c "print(f'{$CYCLES / ($SIMD_MEDIAN / 1000) / 1000:.1f}')")
        SIMD_RANGE="${SIMD_MIN}-${SIMD_MAX}"
    fi

    if [ "$METAL_MEDIAN" != "-" ] && [ "$SIMD_MEDIAN" != "-" ]; then
        SPEEDUP=$(python3 -c "print(f'{$SIMD_MEDIAN / $METAL_MEDIAN:.2f}x')")
    fi

    printf "║ %-11s │ %10s │ %10s %10s %10s │ %10s %10s %10s │ %7s ║\n" \
        "fib($N)" "$CYCLES" "$METAL_MEDIAN" "$METAL_KHZ" "$METAL_RANGE" "$SIMD_MEDIAN" "$SIMD_KHZ" "$SIMD_RANGE" "$SPEEDUP"

    # Write JSON line for post-processing.
    echo "{\"n\":$N,\"cycles\":$CYCLES,\"metal_median_ms\":\"$METAL_MEDIAN\",\"metal_khz\":\"$METAL_KHZ\",\"simd_median_ms\":\"$SIMD_MEDIAN\",\"simd_khz\":\"$SIMD_KHZ\",\"speedup\":\"$SPEEDUP\"}" >> "$RESULTS_FILE"
done

echo "╚═══════════════════════════════════════════════════════════════════════════════════════════════════════════╝"
echo ""
echo "Results saved to: $RESULTS_FILE"
