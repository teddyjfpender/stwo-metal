#!/usr/bin/env ruby
# frozen_string_literal: true

require "pathname"
require "tempfile"

if ARGV.length != 2
  warn "usage: ruby scripts/validate_build_inventory_artifacts.rb <registry.json> <generated_inventory.cmake>"
  exit 1
end

registry_path = Pathname(ARGV[0]).expand_path
generated_path = Pathname(ARGV[1]).expand_path
script_path = Pathname(__FILE__).expand_path
repo_root = script_path.dirname.parent
generator_path = repo_root.join("scripts/generate_build_inventory.rb")

Tempfile.create(["build-inventory", ".cmake"]) do |tempfile|
  expected_path = Pathname(tempfile.path)
  system("ruby", generator_path.to_s, registry_path.to_s, expected_path.to_s, exception: true)

  expected = expected_path.read
  actual = generated_path.read

  if expected != actual
    warn "build inventory artifact drift detected in #{generated_path}"
    exit 1
  end
end

puts "build inventory artifacts ok"
