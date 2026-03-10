use super::planner::{
    MetalComponentCapability, MetalComponentPlanInput, MetalOperationKind, MetalSupportTier,
};
use super::planner_manifest_v1_generated::{
    GeneratedMetalInventoryEntry, GeneratedMetalPlannerComponent, STWO_METAL_PLANNER_COMPONENTS_V1,
};
use super::workload_contract::MetalWorkloadStageAssignment;

pub const STWO_METAL_ARTIFACT_SCHEMA_VERSION_V1: u16 = 1;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct MetalProducerIdentity {
    pub producer_name: &'static str,
    pub producer_version: &'static str,
    pub provenance: &'static str,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct MetalGeneratedInventory {
    pub manifest_module: &'static str,
    pub manifest_lookup_fn: &'static str,
    pub manifest_version: u16,
    pub registration_key: &'static str,
    pub abi_family: &'static str,
    pub abi_symbols: &'static [&'static str],
    pub build_modules: &'static [&'static str],
    pub witness_hook: Option<&'static str>,
    pub specialization_keys: &'static [&'static str],
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum MetalRegisteredBenchmarkOperation {
    TraceGeneration,
    ProveVerify,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum MetalGeneratedRouteKind {
    RegisteredProve,
    WorkloadBoundary,
    BenchmarkTraceGeneration,
    BenchmarkProveVerify,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct MetalComponentArtifact {
    pub schema_version: u16,
    pub producer: MetalProducerIdentity,
    pub component_name: &'static str,
    pub workload_family: &'static str,
    pub support_tier: MetalSupportTier,
    pub declared_capabilities: &'static [MetalComponentCapability],
    pub required_prove_capabilities: &'static [MetalComponentCapability],
    pub supported_benchmark_operations: &'static [MetalRegisteredBenchmarkOperation],
    pub supported_generated_routes: &'static [MetalGeneratedRouteKind],
    pub stage_assignments: &'static [MetalWorkloadStageAssignment],
    pub generated_inventory: MetalGeneratedInventory,
}

impl MetalComponentArtifact {
    pub fn as_plan_input(self, operation: MetalOperationKind) -> MetalComponentPlanInput<'static> {
        let required_capabilities = match operation {
            MetalOperationKind::Prove => self.required_prove_capabilities,
        };

        MetalComponentPlanInput {
            component_name: self.component_name,
            support_tier: self.support_tier,
            declared_capabilities: self.declared_capabilities,
            required_capabilities,
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum MetalArtifactLookupError<'a> {
    SchemaMismatch { expected: u16, found: u16 },
    UnknownComponent { component_name: &'a str },
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum MetalArtifactSupportError<'a> {
    Lookup(MetalArtifactLookupError<'a>),
    UnsupportedGeneratedRoute {
        component_name: &'a str,
        route: MetalGeneratedRouteKind,
    },
}

#[derive(Copy, Clone, Debug)]
pub struct MetalArtifactRegistry {
    schema_version: u16,
    producer: MetalProducerIdentity,
    artifacts: &'static [GeneratedMetalPlannerComponent],
}

pub const STWO_METAL_ARTIFACT_REGISTRY_V1: MetalArtifactRegistry = MetalArtifactRegistry {
    schema_version: STWO_METAL_ARTIFACT_SCHEMA_VERSION_V1,
    producer: MetalProducerIdentity {
        producer_name: "stwo-metal",
        producer_version: "planner-manifest-v1",
        provenance: "crates/stwo-metal/src/backend/metal/planner_manifest_v1_generated.rs",
    },
    artifacts: STWO_METAL_PLANNER_COMPONENTS_V1,
};

impl MetalArtifactRegistry {
    pub const fn schema_version(&self) -> u16 {
        self.schema_version
    }

    pub fn require_schema<'a>(
        &self,
        expected_schema_version: u16,
    ) -> Result<(), MetalArtifactLookupError<'a>> {
        if self.schema_version != expected_schema_version {
            return Err(MetalArtifactLookupError::SchemaMismatch {
                expected: expected_schema_version,
                found: self.schema_version,
            });
        }

        Ok(())
    }

    pub fn artifact_by_name<'a>(
        &self,
        component_name: &'a str,
        expected_schema_version: u16,
    ) -> Result<MetalComponentArtifact, MetalArtifactLookupError<'a>> {
        self.require_schema(expected_schema_version)?;

        let component = self
            .artifacts
            .iter()
            .find(|component| component.component_name == component_name)
            .ok_or(MetalArtifactLookupError::UnknownComponent { component_name })?;

        Ok(self.component_artifact(component))
    }

    pub fn artifact_supporting_generated_route<'a>(
        &self,
        component_name: &'a str,
        expected_schema_version: u16,
        route: MetalGeneratedRouteKind,
    ) -> Result<MetalComponentArtifact, MetalArtifactSupportError<'a>> {
        let artifact = self
            .artifact_by_name(component_name, expected_schema_version)
            .map_err(MetalArtifactSupportError::Lookup)?;

        if !artifact.supported_generated_routes.contains(&route) {
            return Err(MetalArtifactSupportError::UnsupportedGeneratedRoute {
                component_name,
                route,
            });
        }

        Ok(artifact)
    }

    pub fn generated_inventory_by_name<'a>(
        &self,
        component_name: &'a str,
        expected_schema_version: u16,
    ) -> Result<MetalGeneratedInventory, MetalArtifactLookupError<'a>> {
        self.artifact_by_name(component_name, expected_schema_version)
            .map(|artifact| artifact.generated_inventory)
    }

    fn component_artifact(
        &self,
        component: &GeneratedMetalPlannerComponent,
    ) -> MetalComponentArtifact {
        MetalComponentArtifact {
            schema_version: self.schema_version,
            producer: self.producer,
            component_name: component.component_name,
            workload_family: component.workload_family,
            support_tier: component.support_tier,
            declared_capabilities: component.declared_capabilities,
            required_prove_capabilities: component.required_prove_capabilities,
            supported_benchmark_operations: component.supported_benchmark_operations,
            supported_generated_routes: component.supported_generated_routes,
            stage_assignments: component.stage_assignments,
            generated_inventory: self.generated_inventory(component.generated_inventory),
        }
    }

    fn generated_inventory(&self, entry: GeneratedMetalInventoryEntry) -> MetalGeneratedInventory {
        MetalGeneratedInventory {
            manifest_module: "planner_manifest_v1_generated",
            manifest_lookup_fn: "STWO_METAL_PLANNER_COMPONENTS_V1.iter().find",
            manifest_version: 1,
            registration_key: entry.registration_key,
            abi_family: entry.abi_family,
            abi_symbols: entry.abi_symbols,
            build_modules: entry.build_modules,
            witness_hook: entry.witness_hook,
            specialization_keys: entry.specialization_keys,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        MetalArtifactLookupError, MetalArtifactSupportError, MetalGeneratedRouteKind,
        MetalOperationKind, MetalRegisteredBenchmarkOperation, STWO_METAL_ARTIFACT_REGISTRY_V1,
        STWO_METAL_ARTIFACT_SCHEMA_VERSION_V1,
    };
    use crate::backend::metal::planner::{MetalComponentCapability, MetalSupportTier};

    #[test]
    fn artifact_registry_reports_schema_mismatch_explicitly() {
        let error = STWO_METAL_ARTIFACT_REGISTRY_V1
            .artifact_supporting_generated_route(
                "fibonacci_example",
                STWO_METAL_ARTIFACT_SCHEMA_VERSION_V1 + 1,
                MetalGeneratedRouteKind::RegisteredProve,
            )
            .unwrap_err();

        assert_eq!(
            error,
            MetalArtifactSupportError::Lookup(MetalArtifactLookupError::SchemaMismatch {
                expected: STWO_METAL_ARTIFACT_SCHEMA_VERSION_V1 + 1,
                found: STWO_METAL_ARTIFACT_SCHEMA_VERSION_V1,
            })
        );
    }

    #[test]
    fn artifact_registry_materializes_component_artifact_for_known_component() {
        let artifact = STWO_METAL_ARTIFACT_REGISTRY_V1
            .artifact_supporting_generated_route(
                "fibonacci_example",
                STWO_METAL_ARTIFACT_SCHEMA_VERSION_V1,
                MetalGeneratedRouteKind::RegisteredProve,
            )
            .unwrap();

        assert_eq!(artifact.component_name, "fibonacci_example");
        assert_eq!(artifact.workload_family, "wide_fibonacci");
        assert_eq!(artifact.support_tier, MetalSupportTier::FriOnly);
        assert_eq!(
            artifact.required_prove_capabilities,
            &[
                MetalComponentCapability::FriBlake2sSubpath,
                MetalComponentCapability::WitnessMain,
                MetalComponentCapability::QuotientEval,
                MetalComponentCapability::PcsCommitment,
            ]
        );
        assert_eq!(
            artifact
                .as_plan_input(MetalOperationKind::Prove)
                .component_name,
            "fibonacci_example"
        );
        assert_eq!(
            artifact.supported_benchmark_operations,
            &[
                MetalRegisteredBenchmarkOperation::TraceGeneration,
                MetalRegisteredBenchmarkOperation::ProveVerify,
            ]
        );
        assert_eq!(artifact.stage_assignments.len(), 5);
        assert!(artifact
            .supported_generated_routes
            .contains(&MetalGeneratedRouteKind::RegisteredProve));
        assert_eq!(
            artifact.generated_inventory.registration_key,
            "fibonacci_example"
        );
        assert_eq!(artifact.generated_inventory.abi_family, "wide_fibonacci");
        assert_eq!(
            artifact.generated_inventory.abi_symbols,
            &[
                "metal.trace.wide_fibonacci.v1",
                "metal.quotient.wide_fibonacci.v1",
                "metal.fri.blake2s.v1",
            ]
        );
        assert_eq!(
            artifact.generated_inventory.manifest_lookup_fn,
            "STWO_METAL_PLANNER_COMPONENTS_V1.iter().find"
        );
        assert!(artifact.generated_inventory.build_modules.contains(&"fri"));
        assert_eq!(
            artifact.generated_inventory.witness_hook,
            Some("ingest_cpu_wide_fibonacci_witness")
        );
        assert_eq!(
            artifact.generated_inventory.specialization_keys,
            &["log_n_instances", "n_columns"]
        );
    }

    #[test]
    fn artifact_registry_rejects_unsupported_generated_route_explicitly() {
        let error = STWO_METAL_ARTIFACT_REGISTRY_V1
            .artifact_supporting_generated_route(
                "poseidon_example",
                STWO_METAL_ARTIFACT_SCHEMA_VERSION_V1,
                MetalGeneratedRouteKind::BenchmarkProveVerify,
            )
            .unwrap_err();

        assert_eq!(
            error,
            MetalArtifactSupportError::UnsupportedGeneratedRoute {
                component_name: "poseidon_example",
                route: MetalGeneratedRouteKind::BenchmarkProveVerify,
            }
        );
    }

    #[test]
    fn artifact_registry_exposes_generated_inventory_without_route_lookup() {
        let inventory = STWO_METAL_ARTIFACT_REGISTRY_V1
            .generated_inventory_by_name("xor_example", STWO_METAL_ARTIFACT_SCHEMA_VERSION_V1)
            .unwrap();

        assert_eq!(inventory.registration_key, "xor_example");
        assert_eq!(inventory.abi_family, "xor");
        assert_eq!(inventory.abi_symbols, &["metal.acceptance.simd_bridge.v1"]);
        assert_eq!(inventory.specialization_keys, &[] as &[&str]);
    }
}
