#!/usr/bin/env ruby
# frozen_string_literal: true

require "json"
require "set"

if ARGV.length != 2
  warn "usage: ruby scripts/validate_abi_artifacts.rb <registry.json> <abi.json>"
  exit 1
end

registry_path, abi_path = ARGV
registry = JSON.parse(File.read(registry_path))
abi = JSON.parse(File.read(abi_path))

def expect(condition, message)
  return if condition

  warn message
  exit 1
end

def expect_keys(object, keys, label)
  missing = keys.reject { |key| object.key?(key) }
  expect(missing.empty?, "#{label} is missing keys: #{missing.join(', ')}")
end

def align_up(value, alignment)
  ((value + alignment - 1) / alignment) * alignment
end

expect_keys(registry, %w[schema_version components], "registry")
expect_keys(
  abi,
  %w[abi_version generator_version source_stwo registry_schema_version generated_outputs host_abi status_codes layouts components],
  "abi"
)

expect(registry["schema_version"] == 1, "registry schema_version must be 1")
expect(abi["abi_version"] == 1, "abi abi_version must be 1")
expect(abi["registry_schema_version"] == registry["schema_version"], "abi registry_schema_version must match registry schema_version")

host_abi = abi["host_abi"]
expect_keys(host_abi, %w[data_model pointer_width_bits endianness], "host_abi")
expect(host_abi["data_model"] == "lp64", "host_abi.data_model must be lp64")
expect(host_abi["pointer_width_bits"] == 64, "host_abi.pointer_width_bits must be 64")
expect(host_abi["endianness"] == "little", "host_abi.endianness must be little")

generated_outputs = abi["generated_outputs"]
expect_keys(generated_outputs, %w[rust_module c_header], "generated_outputs")
generated_outputs.each_value do |path|
  expect(File.exist?(path), "generated output #{path} does not exist")
end

status_names = Set.new
status_codes = Set.new
abi["status_codes"].each do |status|
  expect_keys(status, %w[name code], "status_code")
  expect(status_names.add?(status["name"]), "duplicate status name #{status['name']}")
  expect(status_codes.add?(status["code"]), "duplicate status code #{status['code']}")
end

primitive_layouts = {
  "u32" => { "size" => 4, "alignment" => 4 },
  "bool" => { "size" => 1, "alignment" => 1 },
  "ptr_u32" => { "size" => 8, "alignment" => 8 },
  "ptr_const_u32" => { "size" => 8, "alignment" => 8 },
  "ptr_const_ptr_const_u32" => { "size" => 8, "alignment" => 8 },
  "ptr_const_void" => { "size" => 8, "alignment" => 8 }
}

layout_names = Set.new
layouts = {}
abi["layouts"].each do |layout|
  expect_keys(layout, %w[name kind size alignment fields], "layout")
  expect(layout["kind"] == "struct", "layout #{layout['name']} must have kind struct")
  expect(layout_names.add?(layout["name"]), "duplicate layout #{layout['name']}")
  layouts[layout["name"]] = layout
end

required_layouts = %w[
  StwoCudaM31AbiV1
  StwoCudaCm31AbiV1
  StwoCudaQm31AbiV1
  StwoCudaLookupElements16AbiV1
  StwoCudaCommonEvalAbiV1
  StwoCudaWideFibonacciEvalAbiV1
  StwoCudaPoseidonEvalAbiV1
  StwoCudaConstraintEvalRequestV1
  StwoCudaWideFibonacciTraceRequestV1
  StwoCudaPoseidonTraceRequestV1
  StwoCudaPoseidonInteractionTraceRequestV1
]
required_layouts.each do |name|
  expect(layouts.key?(name), "required layout #{name} is missing")
end

resolved = {}
resolving = Set.new

resolve_layout = lambda do |type_name|
  return primitive_layouts[type_name] if primitive_layouts.key?(type_name)
  return resolved[type_name] if resolved.key?(type_name)

  layout = layouts[type_name]
  expect(!layout.nil?, "unknown layout type #{type_name}")
  expect(resolving.add?(type_name), "cyclic layout reference involving #{type_name}")

  offset = 0
  max_alignment = 1
  field_names = Set.new

  layout["fields"].each do |field|
    expect_keys(field, %w[name type offset], "#{type_name} field")
    expect(field_names.add?(field["name"]), "duplicate field #{field['name']} in #{type_name}")

    field_layout = resolve_layout.call(field["type"])
    array_len = field.fetch("array_len", 1)
    expect(array_len.is_a?(Integer) && array_len >= 1, "invalid array_len for #{type_name}.#{field['name']}")

    field_alignment = field_layout["alignment"]
    offset = align_up(offset, field_alignment)
    expect(field["offset"] == offset, "unexpected offset for #{type_name}.#{field['name']}: expected #{offset}, got #{field['offset']}")

    field_size = field_layout["size"] * array_len
    offset += field_size
    max_alignment = [max_alignment, field_alignment].max
  end

  expected_size = align_up(offset, max_alignment)
  expect(layout["alignment"] == max_alignment, "unexpected alignment for #{type_name}: expected #{max_alignment}, got #{layout['alignment']}")
  expect(layout["size"] == expected_size, "unexpected size for #{type_name}: expected #{expected_size}, got #{layout['size']}")

  resolved[type_name] = { "size" => expected_size, "alignment" => max_alignment }
  resolving.delete(type_name)
  resolved[type_name]
end

required_layouts.each do |name|
  resolve_layout.call(name)
end

common_eval = layouts["StwoCudaCommonEvalAbiV1"]
expect(common_eval["fields"].length == 2, "StwoCudaCommonEvalAbiV1 must have exactly 2 fields")
expect(common_eval["fields"][0]["name"] == "eval_id", "common eval field 0 must be eval_id")
expect(common_eval["fields"][0]["type"] == "u32", "common eval eval_id must be u32")
expect(common_eval["fields"][1]["name"] == "log_n_rows", "common eval field 1 must be log_n_rows")
expect(common_eval["fields"][1]["type"] == "u32", "common eval log_n_rows must be u32")

%w[StwoCudaWideFibonacciEvalAbiV1 StwoCudaPoseidonEvalAbiV1].each do |layout_name|
  first_field = layouts[layout_name]["fields"].first
  expect(first_field["type"] == "StwoCudaCommonEvalAbiV1", "#{layout_name} must begin with StwoCudaCommonEvalAbiV1")
  expect(first_field["offset"] == 0, "#{layout_name} common header must be at offset 0")
end

registry_components = {}
registry["components"].each do |component|
  expect_keys(component, %w[component_name eval_identity], "registry component")
  registry_components[component["component_name"]] = component
end

allowed_capabilities = Set[
  "constraint_eval",
  "witness_main",
  "witness_interaction"
]

component_names = Set.new
abi["components"].each do |component|
  expect_keys(component, %w[component_name eval_identity eval_layout capabilities], "abi component")
  expect(component_names.add?(component["component_name"]), "duplicate abi component #{component['component_name']}")

  registry_component = registry_components[component["component_name"]]
  expect(!registry_component.nil?, "abi component #{component['component_name']} is not present in registry")

  registry_eval_identity = registry_component["eval_identity"]
  abi_eval_identity = component["eval_identity"]
  expect_keys(abi_eval_identity, %w[eval_id_tag eval_id_value], "abi eval_identity")
  expect(
    abi_eval_identity["eval_id_tag"] == registry_eval_identity["eval_id_tag"],
    "abi eval_id_tag mismatch for #{component['component_name']}"
  )
  expect(
    abi_eval_identity["eval_id_value"] == registry_eval_identity["eval_id_value"],
    "abi eval_id_value mismatch for #{component['component_name']}"
  )

  expect(layouts.key?(component["eval_layout"]), "unknown eval_layout #{component['eval_layout']}")

  capability_names = Set.new
  component["capabilities"].each do |capability_name, capability|
    expect(allowed_capabilities.include?(capability_name), "unsupported capability #{capability_name}")
    expect(capability_names.add?(capability_name), "duplicate capability #{capability_name} for #{component['component_name']}")
    expect_keys(capability, %w[request_layout export_symbol], "#{component['component_name']} capability #{capability_name}")
    expect(layouts.key?(capability["request_layout"]), "unknown request_layout #{capability['request_layout']}")
    expect(!capability["export_symbol"].to_s.empty?, "empty export_symbol for #{component['component_name']} capability #{capability_name}")
  end
end

puts "abi artifacts ok"
