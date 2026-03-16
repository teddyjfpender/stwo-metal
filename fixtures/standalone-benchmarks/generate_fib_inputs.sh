#!/usr/bin/env bash
# Generate fib(n) prover inputs for benchmarking.
# Usage: ./generate_fib_inputs.sh
#
# Prerequisites:
#   - cairo-compile (Cairo0 compiler) in PATH
#   - generate_prover_input built from vendor/stwo-cairo

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
TEST_DATA="$SCRIPT_DIR/test_data"
GEN_BIN="$REPO_ROOT/vendor/stwo-cairo/stwo_cairo_prover/target/release/generate_prover_input"

# Build generate_prover_input if needed.
if [ ! -f "$GEN_BIN" ]; then
    echo "Building generate_prover_input..."
    cargo build --manifest-path "$REPO_ROOT/vendor/stwo-cairo/stwo_cairo_prover/Cargo.toml" \
        --bin generate_prover_input --release
fi

FIB_SIZES=(32768 65536 131072 262144 524288 1048576 2097152 4194304)

for N in "${FIB_SIZES[@]}"; do
    DIR="$TEST_DATA/cairo_fib_$N"
    mkdir -p "$DIR"

    # Skip if prover_input.json already exists.
    if [ -f "$DIR/prover_input.json" ]; then
        echo "[$N] prover_input.json already exists, skipping"
        continue
    fi

    echo "[$N] Generating fib.cairo..."
    cat > "$DIR/fib.cairo" <<CAIRO
%builtins output pedersen range_check ecdsa bitwise ec_op keccak poseidon range_check96 add_mod mul_mod

func main{
    output_ptr: felt*,
    pedersen_ptr,
    range_check_ptr,
    ecdsa_ptr,
    bitwise_ptr,
    ec_op_ptr,
    keccak_ptr,
    poseidon_ptr,
    range_check96_ptr,
    add_mod_ptr,
    mul_mod_ptr,
}() {
    let result = fib(1, 1, $N);
    assert [output_ptr] = result;
    let output_ptr = output_ptr + 1;
    return ();
}

func fib(first_element: felt, second_element: felt, n: felt) -> felt {
    jmp fib_body if n != 0;
    return second_element;

    fib_body:
    return fib(second_element, first_element + second_element, n - 1);
}
CAIRO

    echo "[$N] Compiling with cairo-compile --proof_mode..."
    cairo-compile --proof_mode "$DIR/fib.cairo" --output "$DIR/compiled.json"

    echo "[$N] Generating prover_input.json (this may take a while for large n)..."
    "$GEN_BIN" --program "$DIR/compiled.json" --output "$DIR/prover_input.json"

    # Extract and display cycle count.
    CYCLES=$(python3 -c "
import json
with open('$DIR/prover_input.json') as f:
    d = json.load(f)
st = d['state_transitions']['casm_states_by_opcode']
total = sum(len(v) for v in st.values() if isinstance(v, list))
print(total)
")
    echo "[$N] Done — $CYCLES total cycles"
    echo ""
done

echo "=== All inputs generated ==="
echo ""
for N in "${FIB_SIZES[@]}"; do
    DIR="$TEST_DATA/cairo_fib_$N"
    if [ -f "$DIR/prover_input.json" ]; then
        SIZE=$(du -h "$DIR/prover_input.json" | cut -f1)
        CYCLES=$(python3 -c "
import json
with open('$DIR/prover_input.json') as f:
    d = json.load(f)
st = d['state_transitions']['casm_states_by_opcode']
total = sum(len(v) for v in st.values() if isinstance(v, list))
print(total)
")
        printf "  fib(%7d): %s cycles, %s on disk\n" "$N" "$CYCLES" "$SIZE"
    fi
done
