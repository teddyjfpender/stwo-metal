use super::planner::{CudaComponentCapability, CudaComponentPlanInput, CudaSupportTier};

// Generated from docs/artifacts/component-registry-v1.exemplar.json and
// docs/artifacts/cuda-compatibility-manifest-v1.exemplar.json.
// schema_version: 1
// manifest_version: 1
// abi_version: 1
// generator_version: draft-planner-manifest-v1
// source_stwo.kind: git
// source_stwo.value: TBD

#[derive(Copy, Clone, Debug)]
pub struct GeneratedCudaPlannerComponent {
    pub component_name: &'static str,
    pub support_tier: CudaSupportTier,
    pub declared_capabilities: &'static [CudaComponentCapability],
    pub required_prove_capabilities: &'static [CudaComponentCapability],
}

const FIBONACCI_EXAMPLE_DECLARED_CAPABILITIES: &[CudaComponentCapability] = &[
    CudaComponentCapability::ConstraintEval,
    CudaComponentCapability::WitnessMain,
];

const FIBONACCI_EXAMPLE_REQUIRED_PROVE_CAPABILITIES: &[CudaComponentCapability] = &[
    CudaComponentCapability::ConstraintEval,
    CudaComponentCapability::WitnessMain,
];

const POSEIDON_EXAMPLE_DECLARED_CAPABILITIES: &[CudaComponentCapability] = &[
    CudaComponentCapability::ConstraintEval,
    CudaComponentCapability::WitnessMain,
    CudaComponentCapability::WitnessInteraction,
];

const POSEIDON_EXAMPLE_REQUIRED_PROVE_CAPABILITIES: &[CudaComponentCapability] = &[
    CudaComponentCapability::ConstraintEval,
    CudaComponentCapability::WitnessMain,
    CudaComponentCapability::WitnessInteraction,
];

pub const STWO_CUDA_PLANNER_COMPONENTS_V1: &[GeneratedCudaPlannerComponent] = &[
    GeneratedCudaPlannerComponent {
        component_name: "fibonacci_example",
        support_tier: CudaSupportTier::FullWitness,
        declared_capabilities: FIBONACCI_EXAMPLE_DECLARED_CAPABILITIES,
        required_prove_capabilities: FIBONACCI_EXAMPLE_REQUIRED_PROVE_CAPABILITIES,
    },
    GeneratedCudaPlannerComponent {
        component_name: "poseidon_example",
        support_tier: CudaSupportTier::FullWitness,
        declared_capabilities: POSEIDON_EXAMPLE_DECLARED_CAPABILITIES,
        required_prove_capabilities: POSEIDON_EXAMPLE_REQUIRED_PROVE_CAPABILITIES,
    },
];

pub fn planner_component_by_name(
    component_name: &str,
) -> Option<&'static GeneratedCudaPlannerComponent> {
    STWO_CUDA_PLANNER_COMPONENTS_V1
        .iter()
        .find(|component| component.component_name == component_name)
}

pub fn planner_input_for_prove(component_name: &str) -> Option<CudaComponentPlanInput<'static>> {
    let component = planner_component_by_name(component_name)?;
    Some(CudaComponentPlanInput {
        component_name: component.component_name,
        support_tier: component.support_tier,
        declared_capabilities: component.declared_capabilities,
        required_capabilities: component.required_prove_capabilities,
    })
}
