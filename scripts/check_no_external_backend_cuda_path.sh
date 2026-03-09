#!/usr/bin/env sh
set -eu

if git grep -n "stwo::prover::backend::cuda" -- crates 2>/dev/null
then
  echo
  echo "Direct stwo::prover::backend::cuda usage is not allowed."
  echo "Move external callers onto the stwo-cuda companion package boundary."
  exit 1
fi

echo "No direct stwo::prover::backend::cuda usage detected."
