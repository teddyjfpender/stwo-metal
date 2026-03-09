#!/bin/sh
set -eu

if rg -n 'todo!\(|unimplemented!\(' crates/stwo-cuda/src/backend/cuda -g '*.rs'; then
  echo "raw todo!/unimplemented!() remain under crates/stwo-cuda/src/backend/cuda" >&2
  exit 1
fi
