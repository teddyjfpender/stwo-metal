use super::artifact::{
    MetalArtifactRegistry, MetalComponentArtifact, MetalGeneratedInventory,
    MetalGeneratedRouteKind, STWO_METAL_ARTIFACT_REGISTRY_V1,
    STWO_METAL_ARTIFACT_SCHEMA_VERSION_V1,
};
use super::generated_policy::{
    resolve_generated_route_policy, MetalGeneratedRoutePolicy, UnsupportedGeneratedComponentReason,
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

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) struct RegisteredMetalComponent {
    pub schema_version: u16,
    pub route: MetalGeneratedRouteKind,
    pub artifact: MetalComponentArtifact,
}

impl RegisteredMetalComponent {
    pub fn component_name(self) -> &'static str {
        self.artifact.component_name
    }

    pub fn generated_inventory(self) -> MetalGeneratedInventory {
        self.artifact.generated_inventory
    }
}

pub(crate) fn registered_generated_artifact<'a>(
    component_name: &'a str,
    route: MetalGeneratedRouteKind,
) -> Result<MetalComponentArtifact, MetalPlannerError<'a>> {
    registered_generated_component_in_registry(
        &STWO_METAL_ARTIFACT_REGISTRY_V1,
        STWO_METAL_ARTIFACT_SCHEMA_VERSION_V1,
        component_name,
        route,
    )
    .map(|registration| registration.artifact)
}

pub(crate) fn registered_generated_component<'a>(
    component_name: &'a str,
    route: MetalGeneratedRouteKind,
) -> Result<RegisteredMetalComponent, MetalPlannerError<'a>> {
    registered_generated_component_in_registry(
        &STWO_METAL_ARTIFACT_REGISTRY_V1,
        STWO_METAL_ARTIFACT_SCHEMA_VERSION_V1,
        component_name,
        route,
    )
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
    let registration =
        registered_generated_component(component_name, MetalGeneratedRouteKind::RegisteredProve)?;
    let input = registration
        .artifact
        .as_plan_input(MetalOperationKind::Prove);
    let plan = plan_metal_operation(intent, MetalOperationKind::Prove, &[input])
        .map_err(MetalPlannerError::Unsupported)?;

    Ok(RegisteredMetalExecutionPlan {
        schema_version: STWO_METAL_ARTIFACT_SCHEMA_VERSION_V1,
        operation: MetalOperationKind::Prove,
        plan,
    })
}

pub(crate) fn registered_generated_component_in_registry<'a>(
    registry: &MetalArtifactRegistry,
    expected_schema_version: u16,
    component_name: &'a str,
    route: MetalGeneratedRouteKind,
) -> Result<RegisteredMetalComponent, MetalPlannerError<'a>> {
    match resolve_generated_route_policy(registry, expected_schema_version, component_name, route) {
        MetalGeneratedRoutePolicy::UseRegisteredArtifact(artifact) => {
            Ok(RegisteredMetalComponent {
                schema_version: registry.schema_version(),
                route,
                artifact,
            })
        }
        MetalGeneratedRoutePolicy::Reject(problem) => match problem.reason {
            UnsupportedGeneratedComponentReason::ArtifactNotRegistered
            | UnsupportedGeneratedComponentReason::SchemaMismatch { .. } => {
                Err(MetalPlannerError::UnknownComponent(UnknownMetalComponent {
                    component_name,
                    operation: MetalOperationKind::Prove,
                }))
            }
            UnsupportedGeneratedComponentReason::UnsupportedRoute(route) => Err(
                MetalPlannerError::UnsupportedGeneratedRoute(UnsupportedGeneratedMetalRoute {
                    component_name,
                    route,
                }),
            ),
        },
    }
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
            registered_generated_component_in_registry(
                registry,
                expected_schema_version,
                component_name,
                MetalGeneratedRouteKind::RegisteredProve,
            )
            .map(|registration| registration.artifact.as_plan_input(operation))
        })
        .collect::<Result<Vec<_>, _>>()?;

    let plan =
        plan_metal_operation(intent, operation, &inputs).map_err(MetalPlannerError::Unsupported)?;

    Ok(RegisteredMetalExecutionPlan {
        schema_version: registry.schema_version(),
        operation,
        plan,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        plan_registered_metal_component_prove, plan_registered_metal_prove,
        registered_generated_artifact, registered_generated_component,
    };
    use crate::backend::metal::artifact::MetalGeneratedRouteKind;
    use crate::backend::metal::planner::{
        MetalExecutionIntent, MetalExecutionPlan, MetalPlannerError, UnknownMetalComponent,
        UnsupportedGeneratedMetalRoute,
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

    #[test]
    fn registered_generated_artifact_fails_closed_for_unsupported_route() {
        let error = registered_generated_artifact(
            "poseidon_example",
            MetalGeneratedRouteKind::BenchmarkProveVerify,
        )
        .unwrap_err();

        assert_eq!(
            error,
            MetalPlannerError::UnsupportedGeneratedRoute(UnsupportedGeneratedMetalRoute {
                component_name: "poseidon_example",
                route: MetalGeneratedRouteKind::BenchmarkProveVerify,
            })
        );
    }

    #[test]
    fn registered_generated_component_exposes_inventory_for_lowering() {
        let registration = registered_generated_component(
            "fibonacci_example",
            MetalGeneratedRouteKind::WorkloadBoundary,
        )
        .unwrap();

        assert_eq!(registration.component_name(), "fibonacci_example");
        assert_eq!(
            registration.route,
            MetalGeneratedRouteKind::WorkloadBoundary
        );
        assert_eq!(
            registration.generated_inventory().abi_symbols,
            &[
                "metal.trace.wide_fibonacci.v1",
                "metal.quotient.wide_fibonacci.v1",
                "metal.fri.blake2s.v1",
            ]
        );
        assert_eq!(
            registration.generated_inventory().specialization_keys,
            &["log_n_instances", "n_columns"]
        );
    }
}
