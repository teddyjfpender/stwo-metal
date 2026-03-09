#!/usr/bin/env ruby
# frozen_string_literal: true

root_manifest = File.read("Cargo.toml")
matrix = File.read("docs/compatibility-matrix.md")

workspace_version = root_manifest[/\[workspace\.package\].*?version\s*=\s*"([^"]+)"/m, 1]
abort("Could not determine workspace package version from Cargo.toml") if workspace_version.nil?

expected_row = [
  "| companion-package-v1 | `supported` | `stwo` workspace version `#{workspace_version}` via companion crate `stwo-cuda` | `1` | `1` |"
]

expected_row.each do |snippet|
  next if matrix.include?(snippet)

  abort("Compatibility matrix is missing expected companion row fragment:\n#{snippet}")
end

required_notes = [
  "companion package boundary is now real for current non-`stwo` workspace consumers",
  "`cargo test -p stwo-cuda --features=\"prover\" --locked`",
  "`TD-0009`"
]

required_notes.each do |snippet|
  next if matrix.include?(snippet)

  abort("Compatibility matrix is missing required note snippet:\n#{snippet}")
end

puts "M11 companion compatibility matrix is aligned with the workspace package boundary."
