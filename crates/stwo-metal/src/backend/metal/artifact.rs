use super::planner::{
    MetalComponentCapability, MetalComponentPlanInput, MetalOperationKind, MetalSupportTier,
};
use super::planner_manifest_v1_generated::{
    GeneratedMetalPlannerComponent, STWO_METAL_PLANNER_COMPONENTS_V1,
};

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
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct MetalComponentArtifact {
    pub schema_version: u16,
    pub producer: MetalProducerIdentity,
    pub component_name: &'static str,
    pub support_tier: MetalSupportTier,
    pub declared_capabilities: &'static [MetalComponentCapability],
    pub required_prove_capabilities: &'static [MetalComponentCapability],
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
    SchemaMismatch {
        expected: u16,
        found: u16,
    },
    UnknownComponent {
        component_name: &'a str,
    },
}

#[derive(Copy, Clone, Debug)]
pub struct MetalArtifactRegistry {
    schema_version: u16,
    producer: MetalProducerIdentity,
    generated_inventory: MetalGeneratedInventory,
    artifacts: &'static [GeneratedMetalPlannerComponent],
}

pub const STWO_METAL_ARTIFACT_REGISTRY_V1: MetalArtifactRegistry = MetalArtifactRegistry {
    schema_version: STWO_METAL_ARTIFACT_SCHEMA_VERSION_V1,
    producer: MetalProducerIdentity {
        producer_name: "stwo-metal",
        producer_version: "planner-manifest-v1",
        provenance: "crates/stwo-metal/src/backend/metal/planner_manifest_v1_generated.rs",
    },
    generated_inventory: MetalGeneratedInventory {
        manifest_module: "planner_manifest_v1_generated",
        manifest_lookup_fn: "planner_component_by_name",
        manifest_version: 1,
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

    pub fn artifact_for_prove<'a>(
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

    fn component_artifact(
        &self,
        component: &GeneratedMetalPlannerComponent,
    ) -> MetalComponentArtifact {
        MetalComponentArtifact {
            schema_version: self.schema_version,
            producer: self.producer,
            component_name: component.component_name,
            support_tier: component.support_tier,
            declared_capabilities: component.declared_capabilities,
            required_prove_capabilities: component.required_prove_capabilities,
            generated_inventory: self.generated_inventory,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        MetalArtifactLookupError, MetalOperationKind, STWO_METAL_ARTIFACT_REGISTRY_V1,
        STWO_METAL_ARTIFACT_SCHEMA_VERSION_V1,
    };
    use crate::backend::metal::planner::{MetalComponentCapability, MetalSupportTier};

    #[test]
    fn artifact_registry_reports_schema_mismatch_explicitly() {
        let error = STWO_METAL_ARTIFACT_REGISTRY_V1
            .artifact_for_prove("fibonacci_example", STWO_METAL_ARTIFACT_SCHEMA_VERSION_V1 + 1)
            .unwrap_err();

        assert_eq!(
            error,
            MetalArtifactLookupError::SchemaMismatch {
                expected: STWO_METAL_ARTIFACT_SCHEMA_VERSION_V1 + 1,
                found: STWO_METAL_ARTIFACT_SCHEMA_VERSION_V1,
            }
        );
    }

    #[test]
    fn artifact_registry_materializes_component_artifact_for_known_component() {
        let artifact = STWO_METAL_ARTIFACT_REGISTRY_V1
            .artifact_for_prove("fibonacci_example", STWO_METAL_ARTIFACT_SCHEMA_VERSION_V1)
            .unwrap();

        assert_eq!(artifact.component_name, "fibonacci_example");
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
            artifact.as_plan_input(MetalOperationKind::Prove).component_name,
            "fibonacci_example"
        );
    }
}
