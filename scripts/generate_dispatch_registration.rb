#!/usr/bin/env ruby
# frozen_string_literal: true

require "json"
require "pathname"
require "set"

if ARGV.length != 2
  warn "usage: ruby scripts/generate_dispatch_registration.rb <registry.json> <output.cuh>"
  exit 1
end

registry_path = Pathname(ARGV[0]).expand_path
output_path = Pathname(ARGV[1]).expand_path
script_path = Pathname(__FILE__).expand_path
repo_root = script_path.dirname.parent
registry = JSON.parse(registry_path.read)

def expect(condition, message)
  return if condition

  warn message
  exit 1
end

def normalized_dispatch_symbol(component_name)
  "stwo_cuda_dispatch_constraint_eval_#{component_name}_v1"
end

expect(registry["schema_version"] == 1, "registry schema_version must be 1")
components = registry.fetch("components")
expect(components.is_a?(Array) && !components.empty?, "registry components must be a non-empty array")

component_names = Set.new
eval_id_values = Set.new
wrapper_symbols = Set.new

entries = components.sort_by { |component| component.fetch("component_name") }.map do |component|
  component_name = component.fetch("component_name")
  expect(component_name.match?(/\A[a-z0-9_]+\z/), "unsupported component_name #{component_name.inspect}")
  expect(component_names.add?(component_name), "duplicate component_name #{component_name}")

  eval_identity = component.fetch("eval_identity")
  eval_id_value = eval_identity.fetch("eval_id_value")
  expect(eval_id_values.add?(eval_id_value), "duplicate eval_id_value #{eval_id_value}")

  entrypoints = component.fetch("entrypoints")
  constraint_eval = entrypoints.fetch("constraint_eval")
  expect(!constraint_eval.fetch("symbol").to_s.empty?, "missing constraint_eval symbol for #{component_name}")

  wrapper_symbol = normalized_dispatch_symbol(component_name)
  expect(wrapper_symbols.add?(wrapper_symbol), "duplicate generated wrapper symbol #{wrapper_symbol}")
  [component_name, wrapper_symbol]
end

content = +""
content << "#ifndef STWO_CUDA_COMPONENT_DISPATCH_V1_GENERATED_H\n"
content << "#define STWO_CUDA_COMPONENT_DISPATCH_V1_GENERATED_H\n\n"
content << "// Generated from #{registry_path.relative_path_from(repo_root)}.\n"
content << "// schema_version: #{registry.fetch('schema_version')}\n"
content << "// generator_version: draft-dispatch-v1\n"
content << "// source_stwo.kind: #{registry.fetch('source_stwo').fetch('kind')}\n"
content << "// source_stwo.value: #{registry.fetch('source_stwo').fetch('value')}\n\n"
content << "#define STWO_CUDA_COMPONENT_DISPATCH_V1_CASES(X) \\\n"
entries.each_with_index do |(component_name, wrapper_symbol), index|
  suffix = index == entries.length - 1 ? "\n" : " \\\n"
  content << "    X(\"#{component_name}\", #{wrapper_symbol})#{suffix}"
end
content << "\n#endif // STWO_CUDA_COMPONENT_DISPATCH_V1_GENERATED_H\n"

output_path.dirname.mkpath
output_path.write(content)
