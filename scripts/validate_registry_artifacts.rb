#!/usr/bin/env ruby
# frozen_string_literal: true

require "json"
require "set"

if ARGV.length != 2
  warn "usage: ruby scripts/validate_registry_artifacts.rb <registry.json> <manifest.json>"
  exit 1
end

registry_path, manifest_path = ARGV
registry = JSON.parse(File.read(registry_path))
manifest = JSON.parse(File.read(manifest_path))

def expect(condition, message)
  return if condition

  warn message
  exit 1
end

def expect_keys(object, keys, label)
  missing = keys.reject { |key| object.key?(key) }
  expect(missing.empty?, "#{label} is missing keys: #{missing.join(', ')}")
end

def fnv1a_eval_id_gen(value)
  fnv_offset_basis = 0x811C9DC5
  fnv_prime = 0x01000193

  hash = fnv_offset_basis
  value.each_byte do |byte|
    hash ^= byte
    hash = (hash * fnv_prime) & 0xFFFF_FFFF
  end
  hash
end

expect_keys(registry, %w[schema_version generator_version source_stwo components], "registry")
expect_keys(manifest, %w[manifest_version registry_schema_version generator_version source_stwo abi_version cuda components], "manifest")

expect(registry["schema_version"] == 1, "registry schema_version must be 1")
expect(manifest["manifest_version"] == 1, "manifest manifest_version must be 1")
expect(manifest["abi_version"] == 1, "manifest abi_version must be 1")
expect(
  manifest["registry_schema_version"] == registry["schema_version"],
  "manifest registry_schema_version must match registry schema_version"
)

allowed_support_tiers = Set[
  "constraint-eval-only",
  "constraint-and-interaction",
  "full-witness",
  "unsupported"
]

component_names = Set.new
eval_id_tags = Set.new
eval_id_values = Set.new

registry["components"].each do |component|
  expect_keys(
    component,
    %w[component_name eval_identity eval_layout trace_layout constraint_metadata entrypoints native_sources],
    "registry component"
  )
  expect(component_names.add?(component["component_name"]), "duplicate component_name #{component['component_name']}")

  eval_identity = component["eval_identity"]
  expect_keys(eval_identity, %w[eval_id_tag eval_id_hash eval_id_value], "eval_identity")
  expect(eval_id_tags.add?(eval_identity["eval_id_tag"]), "duplicate eval_id_tag #{eval_identity['eval_id_tag']}")
  expect(eval_id_values.add?(eval_identity["eval_id_value"]), "duplicate eval_id_value #{eval_identity['eval_id_value']}")
  expect(eval_identity["eval_id_hash"] == "fnv1a_32", "unsupported eval_id_hash #{eval_identity['eval_id_hash']}")
  expect(
    eval_identity["eval_id_value"] == fnv1a_eval_id_gen(eval_identity["eval_id_tag"]),
    "eval_id_value #{eval_identity['eval_id_value']} does not match fnv1a_32(eval_id_tag) for #{eval_identity['eval_id_tag']}"
  )

  eval_layout = component["eval_layout"]
  expect_keys(eval_layout, %w[rust_type repr fields], "eval_layout")
  expect(eval_layout["fields"].is_a?(Array) && !eval_layout["fields"].empty?, "eval_layout.fields must be a non-empty array")

  trace_layout = component["trace_layout"]
  expect_keys(trace_layout, %w[trace_trees], "trace_layout")
  expect(trace_layout["trace_trees"].is_a?(Array) && !trace_layout["trace_trees"].empty?, "trace_layout.trace_trees must be a non-empty array")

  entrypoints = component["entrypoints"]
  expect(entrypoints.key?("constraint_eval"), "entrypoints.constraint_eval must be present")
end

registry_component_names = component_names

manifest["components"].each do |component|
  expect_keys(component, %w[component_name support_tier capabilities ci_expectations notes], "manifest component")
  expect(
    registry_component_names.include?(component["component_name"]),
    "manifest component #{component['component_name']} is not present in registry"
  )
  expect(
    allowed_support_tiers.include?(component["support_tier"]),
    "unsupported support_tier #{component['support_tier']}"
  )
end

puts "registry artifacts ok"
