#!/usr/bin/env ruby
# frozen_string_literal: true

require "json"
require "pathname"
require "set"

if ARGV.length != 3
  warn "usage: ruby scripts/generate_planner_manifest_module.rb <registry.json> <manifest.json> <output.rs>"
  exit 1
end

registry_path = Pathname(ARGV[0]).expand_path
manifest_path = Pathname(ARGV[1]).expand_path
output_path = Pathname(ARGV[2]).expand_path
script_path = Pathname(__FILE__).expand_path
repo_root = script_path.dirname.parent

registry = JSON.parse(registry_path.read)
manifest = JSON.parse(manifest_path.read)

def expect(condition, message)
  return if condition

  warn message
  exit 1
end

def enum_variant_for_capability(name)
  {
    "constraint_eval" => "CudaComponentCapability::ConstraintEval",
    "witness_main" => "CudaComponentCapability::WitnessMain",
    "witness_interaction" => "CudaComponentCapability::WitnessInteraction",
  }.fetch(name) do
    raise KeyError, "unsupported capability #{name.inspect}"
  end
end

def enum_variant_for_support_tier(name)
  {
    "constraint-eval-only" => "CudaSupportTier::ConstraintEvalOnly",
    "constraint-and-interaction" => "CudaSupportTier::ConstraintAndInteraction",
    "full-witness" => "CudaSupportTier::FullWitness",
    "unsupported" => "CudaSupportTier::ConstraintEvalOnly",
  }.fetch(name) do
    raise KeyError, "unsupported support tier #{name.inspect}"
  end
end

def const_prefix_for_component(name)
  name.upcase
end

expect(registry["schema_version"] == 1, "registry schema_version must be 1")
expect(manifest["manifest_version"] == 1, "manifest manifest_version must be 1")
expect(manifest["abi_version"] == 1, "manifest abi_version must be 1")

registry_components = registry.fetch("components").to_h do |component|
  [component.fetch("component_name"), component]
end

planner_components = manifest.fetch("components").sort_by { |component| component.fetch("component_name") }.map do |manifest_component|
  component_name = manifest_component.fetch("component_name")
  registry_component = registry_components.fetch(component_name) do
    raise KeyError, "manifest component #{component_name.inspect} is not present in registry"
  end

  declared_capabilities = manifest_component.fetch("capabilities")
  declared_set = Set.new
  declared_capabilities.each do |capability|
    expect(declared_set.add?(capability), "duplicate capability #{capability.inspect} for #{component_name}")
  end

  required_prove_capabilities = ["constraint_eval"]
  entrypoints = registry_component.fetch("entrypoints")
  required_prove_capabilities << "witness_main" if entrypoints.key?("witness_main")
  required_prove_capabilities << "witness_interaction" if entrypoints.key?("witness_interaction")

  {
    component_name: component_name,
    support_tier: manifest_component.fetch("support_tier"),
    declared_capabilities: declared_capabilities,
    required_prove_capabilities: required_prove_capabilities,
  }
end

content = +""
content << "use super::planner::{CudaComponentCapability, CudaComponentPlanInput, CudaSupportTier};\n\n"
content << "// Generated from #{registry_path.relative_path_from(repo_root)} and\n"
content << "// #{manifest_path.relative_path_from(repo_root)}.\n"
content << "// schema_version: #{registry.fetch('schema_version')}\n"
content << "// manifest_version: #{manifest.fetch('manifest_version')}\n"
content << "// abi_version: #{manifest.fetch('abi_version')}\n"
content << "// generator_version: draft-planner-manifest-v1\n"
content << "// source_stwo.kind: #{manifest.fetch('source_stwo').fetch('kind')}\n"
content << "// source_stwo.value: #{manifest.fetch('source_stwo').fetch('value')}\n\n"
content << "#[derive(Copy, Clone, Debug)]\n"
content << "pub struct GeneratedCudaPlannerComponent {\n"
content << "    pub component_name: &'static str,\n"
content << "    pub support_tier: CudaSupportTier,\n"
content << "    pub declared_capabilities: &'static [CudaComponentCapability],\n"
content << "    pub required_prove_capabilities: &'static [CudaComponentCapability],\n"
content << "}\n\n"

planner_components.each do |component|
  prefix = const_prefix_for_component(component[:component_name])

  content << "const #{prefix}_DECLARED_CAPABILITIES: &[CudaComponentCapability] = &[\n"
  component[:declared_capabilities].each do |capability|
    content << "    #{enum_variant_for_capability(capability)},\n"
  end
  content << "];\n\n"

  content << "const #{prefix}_REQUIRED_PROVE_CAPABILITIES: &[CudaComponentCapability] = &[\n"
  component[:required_prove_capabilities].each do |capability|
    content << "    #{enum_variant_for_capability(capability)},\n"
  end
  content << "];\n\n"
end

content << "pub const STWO_CUDA_PLANNER_COMPONENTS_V1: &[GeneratedCudaPlannerComponent] = &[\n"
planner_components.each do |component|
  prefix = const_prefix_for_component(component[:component_name])
  content << "    GeneratedCudaPlannerComponent {\n"
  content << "        component_name: #{component[:component_name].inspect},\n"
  content << "        support_tier: #{enum_variant_for_support_tier(component[:support_tier])},\n"
  content << "        declared_capabilities: #{prefix}_DECLARED_CAPABILITIES,\n"
  content << "        required_prove_capabilities: #{prefix}_REQUIRED_PROVE_CAPABILITIES,\n"
  content << "    },\n"
end
content << "];\n\n"

content << "pub fn planner_component_by_name(\n"
content << "    component_name: &str,\n"
content << ") -> Option<&'static GeneratedCudaPlannerComponent> {\n"
content << "    STWO_CUDA_PLANNER_COMPONENTS_V1\n"
content << "        .iter()\n"
content << "        .find(|component| component.component_name == component_name)\n"
content << "}\n\n"

content << "pub fn planner_input_for_prove(component_name: &str) -> Option<CudaComponentPlanInput<'static>> {\n"
content << "    let component = planner_component_by_name(component_name)?;\n"
content << "    Some(CudaComponentPlanInput {\n"
content << "        component_name: component.component_name,\n"
content << "        support_tier: component.support_tier,\n"
content << "        declared_capabilities: component.declared_capabilities,\n"
content << "        required_capabilities: component.required_prove_capabilities,\n"
content << "    })\n"
content << "}\n"

output_path.dirname.mkpath
output_path.write(content)
