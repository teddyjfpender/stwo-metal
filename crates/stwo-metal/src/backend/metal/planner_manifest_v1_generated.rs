use super::artifact::{MetalGeneratedRouteKind, MetalRegisteredBenchmarkOperation};
use super::planner::{MetalComponentCapability, MetalSupportTier};
use super::workload_contract::{
    MetalWorkloadOwnership, MetalWorkloadStage, MetalWorkloadStageAssignment,
};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct GeneratedMetalInventoryEntry {
    pub registration_key: &'static str,
    pub abi_family: &'static str,
    pub build_modules: &'static [&'static str],
    pub witness_hook: Option<&'static str>,
}

// Generated from the current bounded Metal workload declarations.
// schema_version: 1
// manifest_version: 1

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct GeneratedMetalPlannerComponent {
    pub component_name: &'static str,
    pub workload_family: &'static str,
    pub support_tier: MetalSupportTier,
    pub declared_capabilities: &'static [MetalComponentCapability],
    pub required_prove_capabilities: &'static [MetalComponentCapability],
    pub supported_benchmark_operations: &'static [MetalRegisteredBenchmarkOperation],
    pub supported_generated_routes: &'static [MetalGeneratedRouteKind],
    pub stage_assignments: &'static [MetalWorkloadStageAssignment],
    pub generated_inventory: GeneratedMetalInventoryEntry,
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

const FRAMEWORK_ACCEPTANCE_DECLARED_CAPABILITIES: &[MetalComponentCapability] = &[
    MetalComponentCapability::FriBlake2sSubpath,
    MetalComponentCapability::WitnessMain,
    MetalComponentCapability::WitnessInteraction,
    MetalComponentCapability::QuotientEval,
    MetalComponentCapability::PcsCommitment,
];

const FRAMEWORK_ACCEPTANCE_REQUIRED_PROVE_CAPABILITIES: &[MetalComponentCapability] = &[
    MetalComponentCapability::FriBlake2sSubpath,
    MetalComponentCapability::WitnessMain,
    MetalComponentCapability::WitnessInteraction,
    MetalComponentCapability::QuotientEval,
    MetalComponentCapability::PcsCommitment,
];

const NO_INTERACTION_ACCEPTANCE_DECLARED_CAPABILITIES: &[MetalComponentCapability] = &[
    MetalComponentCapability::FriBlake2sSubpath,
    MetalComponentCapability::WitnessMain,
    MetalComponentCapability::QuotientEval,
    MetalComponentCapability::PcsCommitment,
];

const NO_INTERACTION_ACCEPTANCE_REQUIRED_PROVE_CAPABILITIES: &[MetalComponentCapability] = &[
    MetalComponentCapability::FriBlake2sSubpath,
    MetalComponentCapability::WitnessMain,
    MetalComponentCapability::QuotientEval,
    MetalComponentCapability::PcsCommitment,
];

const FIBONACCI_EXAMPLE_SUPPORTED_BENCHMARK_OPERATIONS: &[MetalRegisteredBenchmarkOperation] = &[
    MetalRegisteredBenchmarkOperation::TraceGeneration,
    MetalRegisteredBenchmarkOperation::ProveVerify,
];

const POSEIDON_EXAMPLE_SUPPORTED_BENCHMARK_OPERATIONS: &[MetalRegisteredBenchmarkOperation] = &[];
const NO_BENCHMARK_OPERATIONS: &[MetalRegisteredBenchmarkOperation] = &[];

const FIBONACCI_EXAMPLE_SUPPORTED_GENERATED_ROUTES: &[MetalGeneratedRouteKind] = &[
    MetalGeneratedRouteKind::RegisteredProve,
    MetalGeneratedRouteKind::WorkloadBoundary,
    MetalGeneratedRouteKind::BenchmarkTraceGeneration,
    MetalGeneratedRouteKind::BenchmarkProveVerify,
];

const POSEIDON_EXAMPLE_SUPPORTED_GENERATED_ROUTES: &[MetalGeneratedRouteKind] = &[
    MetalGeneratedRouteKind::RegisteredProve,
    MetalGeneratedRouteKind::WorkloadBoundary,
];

const ACCEPTANCE_ONLY_SUPPORTED_GENERATED_ROUTES: &[MetalGeneratedRouteKind] = &[
    MetalGeneratedRouteKind::RegisteredProve,
    MetalGeneratedRouteKind::WorkloadBoundary,
];

const FIBONACCI_EXAMPLE_BUILD_MODULES: &[&str] = &[
    "planner_manifest_v1_generated",
    "witness",
    "quotient",
    "fri",
    "benchmark",
];

const POSEIDON_EXAMPLE_BUILD_MODULES: &[&str] = &[
    "planner_manifest_v1_generated",
    "fri",
    "workload",
];

const ACCEPTANCE_BRIDGE_BUILD_MODULES: &[&str] = &[
    "planner_manifest_v1_generated",
    "acceptance_bridge",
];

const FIBONACCI_EXAMPLE_STAGE_ASSIGNMENTS: &[MetalWorkloadStageAssignment] = &[
    MetalWorkloadStageAssignment {
        stage: MetalWorkloadStage::WitnessMain,
        ownership: MetalWorkloadOwnership::CpuOwned,
        detail: "The main trace witness path remains CPU-owned for the declared Fibonacci workload boundary.",
    },
    MetalWorkloadStageAssignment {
        stage: MetalWorkloadStage::WitnessInteraction,
        ownership: MetalWorkloadOwnership::NotApplicable,
        detail: "The declared Fibonacci workload boundary has no interaction trace stage.",
    },
    MetalWorkloadStageAssignment {
        stage: MetalWorkloadStage::QuotientEval,
        ownership: MetalWorkloadOwnership::CpuOwned,
        detail: "Constraint quotient evaluation remains outside the declared Metal workload boundary.",
    },
    MetalWorkloadStageAssignment {
        stage: MetalWorkloadStage::PcsCommitment,
        ownership: MetalWorkloadOwnership::CpuOwned,
        detail: "PCS commitment ownership remains CPU-side for the declared Fibonacci workload boundary.",
    },
    MetalWorkloadStageAssignment {
        stage: MetalWorkloadStage::FriBlake2s,
        ownership: MetalWorkloadOwnership::MetalNative,
        detail: "The bounded Blake2s FRI sub-path is owned by the native Metal lane for the declared Fibonacci workload boundary.",
    },
];

const POSEIDON_EXAMPLE_STAGE_ASSIGNMENTS: &[MetalWorkloadStageAssignment] = &[
    MetalWorkloadStageAssignment {
        stage: MetalWorkloadStage::WitnessMain,
        ownership: MetalWorkloadOwnership::CpuOwned,
        detail: "The main Poseidon witness path remains CPU-owned for the declared workload boundary.",
    },
    MetalWorkloadStageAssignment {
        stage: MetalWorkloadStage::WitnessInteraction,
        ownership: MetalWorkloadOwnership::CpuOwned,
        detail: "The interaction witness path remains CPU-owned for the declared Poseidon workload boundary.",
    },
    MetalWorkloadStageAssignment {
        stage: MetalWorkloadStage::QuotientEval,
        ownership: MetalWorkloadOwnership::CpuOwned,
        detail: "Constraint quotient evaluation remains outside the declared Metal workload boundary.",
    },
    MetalWorkloadStageAssignment {
        stage: MetalWorkloadStage::PcsCommitment,
        ownership: MetalWorkloadOwnership::CpuOwned,
        detail: "PCS commitment ownership remains CPU-side for the declared Poseidon workload boundary.",
    },
    MetalWorkloadStageAssignment {
        stage: MetalWorkloadStage::FriBlake2s,
        ownership: MetalWorkloadOwnership::MetalNative,
        detail: "The bounded Blake2s FRI sub-path is owned by the native Metal lane for the declared Poseidon workload boundary.",
    },
];

const STATE_MACHINE_EXAMPLE_STAGE_ASSIGNMENTS: &[MetalWorkloadStageAssignment] = &[
    MetalWorkloadStageAssignment {
        stage: MetalWorkloadStage::WitnessMain,
        ownership: MetalWorkloadOwnership::CpuOwned,
        detail: "The main state-machine trace remains upstream-owned and CPU-prepared in the acceptance lane.",
    },
    MetalWorkloadStageAssignment {
        stage: MetalWorkloadStage::WitnessInteraction,
        ownership: MetalWorkloadOwnership::CpuOwned,
        detail: "The interaction trace remains upstream-owned and CPU-prepared in the acceptance lane.",
    },
    MetalWorkloadStageAssignment {
        stage: MetalWorkloadStage::QuotientEval,
        ownership: MetalWorkloadOwnership::MetalNative,
        detail: "Constraint quotient evaluation runs through MetalBackend in the acceptance lane.",
    },
    MetalWorkloadStageAssignment {
        stage: MetalWorkloadStage::PcsCommitment,
        ownership: MetalWorkloadOwnership::MetalNative,
        detail: "PCS commitment runs through MetalBackend in the acceptance lane.",
    },
    MetalWorkloadStageAssignment {
        stage: MetalWorkloadStage::FriBlake2s,
        ownership: MetalWorkloadOwnership::MetalNative,
        detail: "FRI and Blake2s commitment stay on the Metal lane in the acceptance path.",
    },
];

const BLAKE_EXAMPLE_STAGE_ASSIGNMENTS: &[MetalWorkloadStageAssignment] =
    STATE_MACHINE_EXAMPLE_STAGE_ASSIGNMENTS;

const XOR_EXAMPLE_STAGE_ASSIGNMENTS: &[MetalWorkloadStageAssignment] = &[
    MetalWorkloadStageAssignment {
        stage: MetalWorkloadStage::WitnessMain,
        ownership: MetalWorkloadOwnership::CpuOwned,
        detail: "The XOR MLE traces remain upstream-owned and CPU/SIMD-prepared in the acceptance lane.",
    },
    MetalWorkloadStageAssignment {
        stage: MetalWorkloadStage::WitnessInteraction,
        ownership: MetalWorkloadOwnership::NotApplicable,
        detail: "The declared XOR MLE workload boundary has no separate interaction witness stage.",
    },
    MetalWorkloadStageAssignment {
        stage: MetalWorkloadStage::QuotientEval,
        ownership: MetalWorkloadOwnership::MetalNative,
        detail: "Constraint quotient evaluation runs through MetalBackend in the acceptance lane.",
    },
    MetalWorkloadStageAssignment {
        stage: MetalWorkloadStage::PcsCommitment,
        ownership: MetalWorkloadOwnership::MetalNative,
        detail: "PCS commitment runs through MetalBackend in the acceptance lane.",
    },
    MetalWorkloadStageAssignment {
        stage: MetalWorkloadStage::FriBlake2s,
        ownership: MetalWorkloadOwnership::MetalNative,
        detail: "FRI and Blake2s commitment stay on the Metal lane in the acceptance path.",
    },
];

pub const STWO_METAL_PLANNER_COMPONENTS_V1: &[GeneratedMetalPlannerComponent] = &[
    GeneratedMetalPlannerComponent {
        component_name: "fibonacci_example",
        workload_family: "wide_fibonacci",
        support_tier: MetalSupportTier::FriOnly,
        declared_capabilities: FIBONACCI_EXAMPLE_DECLARED_CAPABILITIES,
        required_prove_capabilities: FIBONACCI_EXAMPLE_REQUIRED_PROVE_CAPABILITIES,
        supported_benchmark_operations: FIBONACCI_EXAMPLE_SUPPORTED_BENCHMARK_OPERATIONS,
        supported_generated_routes: FIBONACCI_EXAMPLE_SUPPORTED_GENERATED_ROUTES,
        stage_assignments: FIBONACCI_EXAMPLE_STAGE_ASSIGNMENTS,
        generated_inventory: GeneratedMetalInventoryEntry {
            registration_key: "fibonacci_example",
            abi_family: "wide_fibonacci",
            build_modules: FIBONACCI_EXAMPLE_BUILD_MODULES,
            witness_hook: Some("ingest_cpu_wide_fibonacci_witness"),
        },
    },
    GeneratedMetalPlannerComponent {
        component_name: "poseidon_example",
        workload_family: "poseidon",
        support_tier: MetalSupportTier::FriOnly,
        declared_capabilities: POSEIDON_EXAMPLE_DECLARED_CAPABILITIES,
        required_prove_capabilities: POSEIDON_EXAMPLE_REQUIRED_PROVE_CAPABILITIES,
        supported_benchmark_operations: POSEIDON_EXAMPLE_SUPPORTED_BENCHMARK_OPERATIONS,
        supported_generated_routes: POSEIDON_EXAMPLE_SUPPORTED_GENERATED_ROUTES,
        stage_assignments: POSEIDON_EXAMPLE_STAGE_ASSIGNMENTS,
        generated_inventory: GeneratedMetalInventoryEntry {
            registration_key: "poseidon_example",
            abi_family: "poseidon",
            build_modules: POSEIDON_EXAMPLE_BUILD_MODULES,
            witness_hook: None,
        },
    },
    GeneratedMetalPlannerComponent {
        component_name: "state_machine_example",
        workload_family: "state_machine",
        support_tier: MetalSupportTier::FullWorkload,
        declared_capabilities: FRAMEWORK_ACCEPTANCE_DECLARED_CAPABILITIES,
        required_prove_capabilities: FRAMEWORK_ACCEPTANCE_REQUIRED_PROVE_CAPABILITIES,
        supported_benchmark_operations: NO_BENCHMARK_OPERATIONS,
        supported_generated_routes: ACCEPTANCE_ONLY_SUPPORTED_GENERATED_ROUTES,
        stage_assignments: STATE_MACHINE_EXAMPLE_STAGE_ASSIGNMENTS,
        generated_inventory: GeneratedMetalInventoryEntry {
            registration_key: "state_machine_example",
            abi_family: "state_machine",
            build_modules: ACCEPTANCE_BRIDGE_BUILD_MODULES,
            witness_hook: None,
        },
    },
    GeneratedMetalPlannerComponent {
        component_name: "blake_example",
        workload_family: "blake",
        support_tier: MetalSupportTier::FullWorkload,
        declared_capabilities: FRAMEWORK_ACCEPTANCE_DECLARED_CAPABILITIES,
        required_prove_capabilities: FRAMEWORK_ACCEPTANCE_REQUIRED_PROVE_CAPABILITIES,
        supported_benchmark_operations: NO_BENCHMARK_OPERATIONS,
        supported_generated_routes: ACCEPTANCE_ONLY_SUPPORTED_GENERATED_ROUTES,
        stage_assignments: BLAKE_EXAMPLE_STAGE_ASSIGNMENTS,
        generated_inventory: GeneratedMetalInventoryEntry {
            registration_key: "blake_example",
            abi_family: "blake",
            build_modules: ACCEPTANCE_BRIDGE_BUILD_MODULES,
            witness_hook: None,
        },
    },
    GeneratedMetalPlannerComponent {
        component_name: "xor_example",
        workload_family: "xor",
        support_tier: MetalSupportTier::FullWorkload,
        declared_capabilities: NO_INTERACTION_ACCEPTANCE_DECLARED_CAPABILITIES,
        required_prove_capabilities: NO_INTERACTION_ACCEPTANCE_REQUIRED_PROVE_CAPABILITIES,
        supported_benchmark_operations: NO_BENCHMARK_OPERATIONS,
        supported_generated_routes: ACCEPTANCE_ONLY_SUPPORTED_GENERATED_ROUTES,
        stage_assignments: XOR_EXAMPLE_STAGE_ASSIGNMENTS,
        generated_inventory: GeneratedMetalInventoryEntry {
            registration_key: "xor_example",
            abi_family: "xor",
            build_modules: ACCEPTANCE_BRIDGE_BUILD_MODULES,
            witness_hook: None,
        },
    },
];
