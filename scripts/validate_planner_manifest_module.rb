#!/usr/bin/env ruby
# frozen_string_literal: true

require "pathname"
require "tempfile"

if ARGV.length != 3
  warn "usage: ruby scripts/validate_planner_manifest_module.rb <registry.json> <manifest.json> <generated.rs>"
  exit 1
end

registry_path = Pathname(ARGV[0]).expand_path
manifest_path = Pathname(ARGV[1]).expand_path
generated_path = Pathname(ARGV[2]).expand_path
script_path = Pathname(__FILE__).expand_path
repo_root = script_path.dirname.parent
generator_path = repo_root.join("scripts/generate_planner_manifest_module.rb")

def normalize_semantic_rust(content)
  content.gsub(/[[:space:]]+/, "")
end

Tempfile.create(["planner-manifest-module", ".rs"]) do |tempfile|
  expected_path = Pathname(tempfile.path)
  system(
    "ruby",
    generator_path.to_s,
    registry_path.to_s,
    manifest_path.to_s,
    expected_path.to_s,
    exception: true
  )

  expected = expected_path.read
  actual = generated_path.read

  if expected != actual
    if normalize_semantic_rust(expected) == normalize_semantic_rust(actual)
      warn "planner manifest module formatting-only drift ignored in #{generated_path}"
    else
      warn "planner manifest module drift detected in #{generated_path}"
      exit 1
    end
  end
end

puts "planner manifest module ok"
