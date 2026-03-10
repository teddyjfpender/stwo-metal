use super::artifact::{
    MetalArtifactLookupError, MetalArtifactRegistry, STWO_METAL_ARTIFACT_REGISTRY_V1,
    STWO_METAL_ARTIFACT_SCHEMA_VERSION_V1,
};
use super::planner::{
    plan_metal_operation, MetalExecutionIntent, MetalExecutionPlan, MetalOperationKind,
    MetalPlannerError, UnknownMetalComponent,
};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) struct RegisteredMetalExecutionPlan<'a> {
    pub schema_version: u16,
    pub operation: MetalOperationKind,
    pub components: &'a [&'a str],
    pub plan: MetalExecutionPlan,
}

pub(crate) fn plan_registered_metal_prove<'a>(
    intent: MetalExecutionIntent,
    component_names: &'a [&'a str],
) -> Result<RegisteredMetalExecutionPlan<'a>, MetalPlannerError<'a>> {
    plan_registered_metal_operation(
        &STWO_METAL_ARTIFACT_REGISTRY_V1,
        STWO_METAL_ARTIFACT_SCHEMA_VERSION_V1,
        intent,
        MetalOperationKind::Prove,
        component_names,
    )
}

pub(crate) fn plan_registered_metal_prove_static(
    intent: MetalExecutionIntent,
    component_name: &'static str,
) -> Result<RegisteredMetalExecutionPlan<'static>, MetalPlannerError<'static>> {
    let component_names = match component_name {
        "fibonacci_example" => &["fibonacci_example"][..],
        "poseidon_example" => &["poseidon_example"][..],
        _ => {
            return Err(MetalPlannerError::UnknownComponent(UnknownMetalComponent {
                component_name,
                operation: MetalOperationKind::Prove,
            }))
        }
    };

    plan_registered_metal_prove(intent, component_names)
}

pub(crate) fn plan_registered_metal_operation<'a>(
    registry: &MetalArtifactRegistry,
    expected_schema_version: u16,
    intent: MetalExecutionIntent,
    operation: MetalOperationKind,
    component_names: &'a [&'a str],
) -> Result<RegisteredMetalExecutionPlan<'a>, MetalPlannerError<'a>> {
    let inputs = component_names
        .iter()
        .map(|component_name| {
            registry
                .artifact_for_prove(component_name, expected_schema_version)
                .map(|artifact| artifact.as_plan_input(operation))
                .map_err(|error| match error {
                    MetalArtifactLookupError::SchemaMismatch { expected: _, found: _ } => {
                        MetalPlannerError::UnknownComponent(UnknownMetalComponent {
                            component_name,
                            operation,
                        })
                    }
                    MetalArtifactLookupError::UnknownComponent { component_name } => {
                        MetalPlannerError::UnknownComponent(UnknownMetalComponent {
                            component_name,
                            operation,
                        })
                    }
                })
        })
        .collect::<Result<Vec<_>, _>>()?;

    let plan = plan_metal_operation(intent, operation, &inputs).map_err(MetalPlannerError::Unsupported)?;

    Ok(RegisteredMetalExecutionPlan {
        schema_version: registry.schema_version(),
        operation,
        components: component_names,
        plan,
    })
}

#[cfg(test)]
mod tests {
    use super::{plan_registered_metal_prove, plan_registered_metal_prove_static};
    use crate::backend::metal::planner::{
        MetalExecutionIntent, MetalExecutionPlan, MetalPlannerError, UnknownMetalComponent,
    };

    #[test]
    fn registered_planner_chooses_hybrid_for_known_fibonacci_component() {
        let plan =
            plan_registered_metal_prove(MetalExecutionIntent::PreferMetal, &["fibonacci_example"])
                .unwrap();

        assert_eq!(plan.plan, MetalExecutionPlan::MetalFriHybrid);
    }

    #[test]
    fn registered_planner_fails_closed_for_unknown_components() {
        let error =
            plan_registered_metal_prove(MetalExecutionIntent::PreferMetal, &["missing_example"])
                .unwrap_err();

        assert_eq!(
            error,
            MetalPlannerError::UnknownComponent(UnknownMetalComponent {
                component_name: "missing_example",
                operation: crate::backend::metal::planner::MetalOperationKind::Prove,
            })
        );
    }

    #[test]
    fn static_registered_planner_supports_declared_components() {
        let plan =
            plan_registered_metal_prove_static(MetalExecutionIntent::PreferMetal, "poseidon_example")
                .unwrap();

        assert_eq!(plan.plan, MetalExecutionPlan::MetalFriHybrid);
    }
}
