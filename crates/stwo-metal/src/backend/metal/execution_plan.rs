use super::artifact::{
    MetalArtifactRegistry, MetalComponentArtifact, MetalGeneratedInventory,
    MetalGeneratedRouteKind, MetalRegisteredBenchmarkOperation, STWO_METAL_ARTIFACT_REGISTRY_V1,
    STWO_METAL_ARTIFACT_SCHEMA_VERSION_V1,
};
use super::generated_policy::{
    resolve_generated_route_policy, MetalGeneratedRoutePolicy, UnsupportedGeneratedComponentReason,
};
use super::planner::{
    plan_metal_operation, MetalComponentPlanInput, MetalExecutionIntent, MetalExecutionPlan,
    MetalOperationKind, MetalPlannerError, UnknownMetalComponent, UnsupportedGeneratedMetalRoute,
};
use super::workload_contract::MetalWorkloadStageAssignment;

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

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) struct RegisteredMetalLoweringInput {
    pub schema_version: u16,
    pub route: MetalGeneratedRouteKind,
    pub component_name: &'static str,
    pub workload_family: &'static str,
    pub abi_family: &'static str,
    pub abi_symbols: &'static [&'static str],
    pub build_modules: &'static [&'static str],
    pub witness_hook: Option<&'static str>,
    pub specialization_keys: &'static [&'static str],
}

#[derive(Copy, Clone, Debug)]
pub(crate) struct RegisteredMetalRuntimePlanInput {
    pub lowering: RegisteredMetalLoweringInput,
    pub operation: MetalOperationKind,
    pub plan_input: MetalComponentPlanInput<'static>,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) struct RegisteredMetalWorkloadBoundaryInput {
    pub lowering: RegisteredMetalLoweringInput,
    pub plan: MetalExecutionPlan,
    pub stage_assignments: &'static [MetalWorkloadStageAssignment],
    pub generated_inventory: MetalGeneratedInventory,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) struct RegisteredMetalBenchmarkDeclarationInput {
    pub workload_boundary: RegisteredMetalWorkloadBoundaryInput,
    pub lowering: RegisteredMetalLoweringInput,
    pub benchmark_route: MetalGeneratedRouteKind,
    pub supported_benchmark_operations: &'static [MetalRegisteredBenchmarkOperation],
}

impl RegisteredMetalComponent {
    pub fn lowering_input(self) -> RegisteredMetalLoweringInput {
        let inventory = self.generated_inventory();
        RegisteredMetalLoweringInput {
            schema_version: self.schema_version,
            route: self.route,
            component_name: self.component_name(),
            workload_family: self.artifact.workload_family,
            abi_family: inventory.abi_family,
            abi_symbols: inventory.abi_symbols,
            build_modules: inventory.build_modules,
            witness_hook: inventory.witness_hook,
            specialization_keys: inventory.specialization_keys,
        }
    }

    pub fn runtime_plan_input(
        self,
        operation: MetalOperationKind,
    ) -> RegisteredMetalRuntimePlanInput {
        RegisteredMetalRuntimePlanInput {
            lowering: self.lowering_input(),
            operation,
            plan_input: self.artifact.as_plan_input(operation),
        }
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

pub(crate) fn registered_runtime_plan_input<'a>(
    component_name: &'a str,
    route: MetalGeneratedRouteKind,
    operation: MetalOperationKind,
) -> Result<RegisteredMetalRuntimePlanInput, MetalPlannerError<'a>> {
    registered_generated_component(component_name, route)
        .map(|registration| registration.runtime_plan_input(operation))
}

pub(crate) fn registered_workload_boundary_input(
    intent: MetalExecutionIntent,
    component_name: &'static str,
) -> Result<RegisteredMetalWorkloadBoundaryInput, MetalPlannerError<'static>> {
    let boundary_registration =
        registered_generated_component(component_name, MetalGeneratedRouteKind::WorkloadBoundary)?;
    let runtime_input = registered_runtime_plan_input(
        component_name,
        MetalGeneratedRouteKind::RegisteredProve,
        MetalOperationKind::Prove,
    )?;
    let plan = plan_metal_operation(intent, runtime_input.operation, &[runtime_input.plan_input])
        .map_err(MetalPlannerError::Unsupported)?;

    Ok(RegisteredMetalWorkloadBoundaryInput {
        lowering: boundary_registration.lowering_input(),
        plan,
        stage_assignments: boundary_registration.artifact.stage_assignments,
        generated_inventory: boundary_registration.generated_inventory(),
    })
}

pub(crate) fn registered_benchmark_declaration_input(
    intent: MetalExecutionIntent,
    component_name: &'static str,
    route: MetalGeneratedRouteKind,
) -> Result<RegisteredMetalBenchmarkDeclarationInput, MetalPlannerError<'static>> {
    let benchmark_registration = registered_generated_component(component_name, route)?;
    let workload_boundary = registered_workload_boundary_input(intent, component_name)?;

    assert_eq!(
        benchmark_registration
            .generated_inventory()
            .registration_key,
        workload_boundary.generated_inventory.registration_key
    );
    assert_eq!(
        benchmark_registration.generated_inventory().abi_family,
        workload_boundary.generated_inventory.abi_family
    );

    Ok(RegisteredMetalBenchmarkDeclarationInput {
        workload_boundary,
        lowering: benchmark_registration.lowering_input(),
        benchmark_route: route,
        supported_benchmark_operations: benchmark_registration
            .artifact
            .supported_benchmark_operations,
    })
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
    let runtime_input = registered_runtime_plan_input(
        component_name,
        MetalGeneratedRouteKind::RegisteredProve,
        MetalOperationKind::Prove,
    )?;
    let plan = plan_metal_operation(intent, runtime_input.operation, &[runtime_input.plan_input])
        .map_err(MetalPlannerError::Unsupported)?;

    Ok(RegisteredMetalExecutionPlan {
        schema_version: STWO_METAL_ARTIFACT_SCHEMA_VERSION_V1,
        operation: runtime_input.operation,
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
            .map(|registration| registration.runtime_plan_input(operation).plan_input)
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
        registered_benchmark_declaration_input, registered_generated_artifact,
        registered_generated_component, registered_runtime_plan_input,
        registered_workload_boundary_input,
    };
    use crate::backend::metal::artifact::{
        MetalGeneratedRouteKind, MetalRegisteredBenchmarkOperation,
    };
    use crate::backend::metal::planner::{
        MetalExecutionIntent, MetalExecutionPlan, MetalOperationKind, MetalPlannerError,
        UnknownMetalComponent, UnsupportedGeneratedMetalRoute,
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
        let lowering_input = registration.lowering_input();
        assert_eq!(lowering_input.component_name, "fibonacci_example");
        assert_eq!(lowering_input.workload_family, "wide_fibonacci");
        assert_eq!(lowering_input.abi_family, "wide_fibonacci");
        assert_eq!(
            lowering_input.build_modules,
            &[
                "planner_manifest_v1_generated",
                "witness",
                "quotient",
                "fri",
                "benchmark"
            ]
        );
        assert_eq!(
            lowering_input.witness_hook,
            Some("ingest_cpu_wide_fibonacci_witness")
        );
    }

    #[test]
    fn registered_runtime_plan_input_consumes_lowering_input() {
        let runtime_input = registered_runtime_plan_input(
            "fibonacci_example",
            MetalGeneratedRouteKind::RegisteredProve,
            MetalOperationKind::Prove,
        )
        .unwrap();

        assert_eq!(runtime_input.lowering.component_name, "fibonacci_example");
        assert_eq!(runtime_input.lowering.abi_family, "wide_fibonacci");
        assert_eq!(runtime_input.operation, MetalOperationKind::Prove);
        assert_eq!(runtime_input.plan_input.component_name, "fibonacci_example");
        assert_eq!(
            runtime_input.lowering.specialization_keys,
            &["log_n_instances", "n_columns"]
        );
    }

    #[test]
    fn registered_workload_boundary_input_consumes_lowering_and_runtime_inputs() {
        let boundary_input = registered_workload_boundary_input(
            MetalExecutionIntent::PreferMetal,
            "fibonacci_example",
        )
        .unwrap();

        assert_eq!(boundary_input.lowering.component_name, "fibonacci_example");
        assert_eq!(
            boundary_input.lowering.route,
            MetalGeneratedRouteKind::WorkloadBoundary
        );
        assert_eq!(boundary_input.plan, MetalExecutionPlan::MetalFriHybrid);
        assert_eq!(
            boundary_input.generated_inventory.registration_key,
            "fibonacci_example"
        );
        assert_eq!(
            boundary_input.generated_inventory.specialization_keys,
            &["log_n_instances", "n_columns"]
        );
        assert_eq!(boundary_input.stage_assignments.len(), 5);
    }

    #[test]
    fn registered_benchmark_declaration_input_reuses_workload_boundary_input() {
        let benchmark_input = registered_benchmark_declaration_input(
            MetalExecutionIntent::PreferMetal,
            "fibonacci_example",
            MetalGeneratedRouteKind::BenchmarkProveVerify,
        )
        .unwrap();

        assert_eq!(
            benchmark_input
                .workload_boundary
                .generated_inventory
                .registration_key,
            "fibonacci_example"
        );
        assert_eq!(
            benchmark_input
                .workload_boundary
                .generated_inventory
                .abi_family,
            "wide_fibonacci"
        );
        assert_eq!(benchmark_input.lowering.component_name, "fibonacci_example");
        assert_eq!(benchmark_input.lowering.workload_family, "wide_fibonacci");
        assert_eq!(
            benchmark_input.benchmark_route,
            MetalGeneratedRouteKind::BenchmarkProveVerify
        );
        assert!(benchmark_input
            .supported_benchmark_operations
            .contains(&MetalRegisteredBenchmarkOperation::ProveVerify));
        assert_eq!(
            benchmark_input.workload_boundary.plan,
            MetalExecutionPlan::MetalFriHybrid
        );
    }
}
