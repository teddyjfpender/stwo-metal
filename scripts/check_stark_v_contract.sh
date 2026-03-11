#!/bin/sh
set -eu

if [ "$#" -ne 1 ]; then
    echo "usage: sh scripts/check_stark_v_contract.sh <stark-v-checkout>" >&2
    exit 1
fi

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
stark_v_root=$1

require_file() {
    if [ ! -f "$1" ]; then
        echo "missing required file: $1" >&2
        exit 1
    fi
}

require_pattern() {
    file=$1
    pattern=$2
    if ! rg -F -q "$pattern" "$file"; then
        echo "missing required pattern '$pattern' in $file" >&2
        exit 1
    fi
}

require_file "$stark_v_root/Cargo.toml"
require_file "$stark_v_root/crates/prover/src/lib.rs"
require_file "$stark_v_root/crates/prover/src/prover.rs"
require_file "$stark_v_root/crates/prover/src/verifier.rs"
require_file "$stark_v_root/crates/bench-cli/src/main.rs"

require_pattern "$stark_v_root/Cargo.toml" "\"crates/prover\""
require_pattern "$stark_v_root/Cargo.toml" "\"crates/runner\""
require_pattern "$stark_v_root/crates/prover/src/prover.rs" "pub fn prove_rv32im"
require_pattern "$stark_v_root/crates/prover/src/verifier.rs" "pub fn verify_rv32im"
require_pattern "$stark_v_root/crates/prover/src/lib.rs" "pub use prover::prove_rv32im;"
require_pattern "$stark_v_root/crates/prover/src/lib.rs" "pub use verifier::verify_rv32im;"
require_pattern "$stark_v_root/crates/prover/src/lib.rs" "pub fn preprocess("
require_pattern "$stark_v_root/crates/bench-cli/src/main.rs" "run_with_input"
require_pattern "$stark_v_root/crates/bench-cli/src/main.rs" "prove_rv32im"
require_pattern "$stark_v_root/crates/bench-cli/src/main.rs" "verify_rv32im"

printf '%s\n' "stark-v contract check passed for $stark_v_root"
