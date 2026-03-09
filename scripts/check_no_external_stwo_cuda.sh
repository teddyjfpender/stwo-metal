#!/usr/bin/env sh
set -eu

if git grep -n "stwo::stwo_cuda" -- crates 2>/dev/null; then
  echo
  echo "Direct stwo::stwo_cuda usage is not allowed."
  echo "Move callers onto the stwo-cuda companion package boundary."
  exit 1
fi

echo "No direct stwo::stwo_cuda usage detected."
