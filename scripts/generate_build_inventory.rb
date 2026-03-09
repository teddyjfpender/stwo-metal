#!/usr/bin/env ruby
# frozen_string_literal: true

require "json"
require "pathname"
require "set"

if ARGV.length != 2
  warn "usage: ruby scripts/generate_build_inventory.rb <registry.json> <output.cmake>"
  exit 1
end

registry_path = Pathname(ARGV[0]).expand_path
output_path = Pathname(ARGV[1]).expand_path
script_path = Pathname(__FILE__).expand_path
repo_root = script_path.dirname.parent
allowed_root = repo_root.join("crates/stwo-cuda-sys/cuda").realpath
registry = JSON.parse(registry_path.read)

def expect(condition, message)
  return if condition

  warn message
  exit 1
end

expect(registry["schema_version"] == 1, "registry schema_version must be 1")
components = registry.fetch("components")
expect(components.is_a?(Array) && !components.empty?, "registry components must be a non-empty array")

source_paths = Set.new
components.each do |component|
  entrypoints = component.fetch("entrypoints")
  entrypoints.each_value do |entrypoint|
    source_unit = entrypoint.fetch("source_unit")
    next unless source_unit.end_with?(".cu")

    source_path = repo_root.join(source_unit).cleanpath
    expect(source_path.to_s.start_with?(allowed_root.to_s + "/"), "source unit outside approved root: #{source_unit}")
    expect(File.exist?(source_path), "missing source unit #{source_unit}")
    source_paths.add(source_path)
  end
end

expect(!source_paths.empty?, "no exemplar source units were discovered")

relative_sources = source_paths.to_a.sort.map do |source_path|
  source_path.relative_path_from(allowed_root).to_s
end

content = +""
content << "# Generated from #{registry_path.relative_path_from(repo_root)}.\n"
content << "# schema_version: #{registry.fetch('schema_version')}\n"
content << "# generator_version: draft-build-inventory-v1\n"
content << "# source_stwo.kind: #{registry.fetch('source_stwo').fetch('kind')}\n"
content << "# source_stwo.value: #{registry.fetch('source_stwo').fetch('value')}\n\n"
content << "set(\n"
content << "    STWO_CUDA_COMPONENT_INVENTORY_V1_EXEMPLAR_SOURCES\n"
relative_sources.each do |source|
  content << "    #{source}\n"
end
content << ")\n"

output_path.dirname.mkpath
output_path.write(content)
