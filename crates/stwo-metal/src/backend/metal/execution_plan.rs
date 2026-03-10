use super::artifact::{
    MetalArtifactLookupError, MetalArtifactRegistry, MetalArtifactSupportError,
    MetalGeneratedRouteKind, STWO_METAL_ARTIFACT_REGISTRY_V1,
    STWO_METAL_ARTIFACT_SCHEMA_VERSION_V1,
};
use super::planner::{
    plan_metal_operation, MetalExecutionIntent, MetalExecutionPlan, MetalOperationKind,
    MetalPlannerError, UnknownMetalComponent, UnsupportedGeneratedMetalRoute,
};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) struct RegisteredMetalExecutionPlan {
    pub schema_version: u16,
    pub operation: MetalOperationKind,
    pub plan: MetalExecutionPlan,
}

pub(crate) fn plan_registered_metal_prove<'a>(
    intent: MetalExecutionIntent,
    component_names: &'a [&'a str],
) -> Result<RegisteredMetalExecutionPlan, MetalPlannerError<'a>> {
    plan_registered_metal_operation(
        &STWO_METAL_ARTIFACT_REGISTRY_V1,
        STWO_METAL_ARTIFACT_SCHEMA_VERSION_V1,
        intent,
        MetalOperationKind::Prove,
        component_names,
    )
}

pub(crate) fn plan_registered_metal_component_prove(
    intent: MetalExecutionIntent,
    component_name: &'static str,
) -> Result<RegisteredMetalExecutionPlan, MetalPlannerError<'static>> {
    let artifact = STWO_METAL_ARTIFACT_REGISTRY_V1
        .artifact_supporting_generated_route(
            component_name,
            STWO_METAL_ARTIFACT_SCHEMA_VERSION_V1,
            MetalGeneratedRouteKind::RegisteredProve,
        )
        .map_err(|error| match error {
            MetalArtifactSupportError::Lookup(MetalArtifactLookupError::SchemaMismatch {
                expected: _,
                found: _,
            }) => {
                MetalPlannerError::UnknownComponent(UnknownMetalComponent {
                    component_name,
                    operation: MetalOperationKind::Prove,
                })
            }
            MetalArtifactSupportError::Lookup(MetalArtifactLookupError::UnknownComponent {
                component_name,
            }) => {
                MetalPlannerError::UnknownComponent(UnknownMetalComponent {
                    component_name,
                    operation: MetalOperationKind::Prove,
                })
            }
            MetalArtifactSupportError::UnsupportedGeneratedRoute { component_name, route } => {
                MetalPlannerError::UnsupportedGeneratedRoute(UnsupportedGeneratedMetalRoute {
                    component_name,
                    route,
                })
            }
        })?;
    let input = artifact.as_plan_input(MetalOperationKind::Prove);
    let plan = plan_metal_operation(intent, MetalOperationKind::Prove, &[input])
        .map_err(MetalPlannerError::Unsupported)?;

    Ok(RegisteredMetalExecutionPlan {
        schema_version: STWO_METAL_ARTIFACT_SCHEMA_VERSION_V1,
        operation: MetalOperationKind::Prove,
        plan,
    })
}

pub(crate) fn plan_registered_metal_operation<'a>(
    registry: &MetalArtifactRegistry,
    expected_schema_version: u16,
    intent: MetalExecutionIntent,
    operation: MetalOperationKind,
    component_names: &'a [&'a str],
) -> Result<RegisteredMetalExecutionPlan, MetalPlannerError<'a>> {
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
        plan,
    })
}

#[cfg(test)]
mod tests {
    use super::{plan_registered_metal_component_prove, plan_registered_metal_prove};
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
    fn component_registered_planner_supports_declared_components() {
        let plan = plan_registered_metal_component_prove(
            MetalExecutionIntent::PreferMetal,
            "poseidon_example",
        )
        .unwrap();

        assert_eq!(plan.plan, MetalExecutionPlan::MetalFriHybrid);
    }
}
