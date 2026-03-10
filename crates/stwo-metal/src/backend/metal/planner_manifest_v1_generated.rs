use super::planner::{MetalComponentCapability, MetalSupportTier};

// Generated from the current bounded Metal workload declarations.
// schema_version: 1
// manifest_version: 1

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct GeneratedMetalPlannerComponent {
    pub component_name: &'static str,
    pub support_tier: MetalSupportTier,
    pub declared_capabilities: &'static [MetalComponentCapability],
    pub required_prove_capabilities: &'static [MetalComponentCapability],
}

const FIBONACCI_EXAMPLE_DECLARED_CAPABILITIES: &[MetalComponentCapability] =
    &[MetalComponentCapability::FriBlake2sSubpath];

const FIBONACCI_EXAMPLE_REQUIRED_PROVE_CAPABILITIES: &[MetalComponentCapability] = &[
    MetalComponentCapability::FriBlake2sSubpath,
    MetalComponentCapability::WitnessMain,
    MetalComponentCapability::QuotientEval,
    MetalComponentCapability::PcsCommitment,
];

const POSEIDON_EXAMPLE_DECLARED_CAPABILITIES: &[MetalComponentCapability] =
    &[MetalComponentCapability::FriBlake2sSubpath];

const POSEIDON_EXAMPLE_REQUIRED_PROVE_CAPABILITIES: &[MetalComponentCapability] = &[
    MetalComponentCapability::FriBlake2sSubpath,
    MetalComponentCapability::WitnessMain,
    MetalComponentCapability::WitnessInteraction,
    MetalComponentCapability::QuotientEval,
    MetalComponentCapability::PcsCommitment,
];

pub const STWO_METAL_PLANNER_COMPONENTS_V1: &[GeneratedMetalPlannerComponent] = &[
    GeneratedMetalPlannerComponent {
        component_name: "fibonacci_example",
        support_tier: MetalSupportTier::FriOnly,
        declared_capabilities: FIBONACCI_EXAMPLE_DECLARED_CAPABILITIES,
        required_prove_capabilities: FIBONACCI_EXAMPLE_REQUIRED_PROVE_CAPABILITIES,
    },
    GeneratedMetalPlannerComponent {
        component_name: "poseidon_example",
        support_tier: MetalSupportTier::FriOnly,
        declared_capabilities: POSEIDON_EXAMPLE_DECLARED_CAPABILITIES,
        required_prove_capabilities: POSEIDON_EXAMPLE_REQUIRED_PROVE_CAPABILITIES,
    },
];
