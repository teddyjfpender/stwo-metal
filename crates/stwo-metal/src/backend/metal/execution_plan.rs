use stwo::core::fields::m31::BaseField;
use stwo::core::poly::circle::CircleDomain;

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
use super::witness::{
    generate_metal_wide_fibonacci_trace, MetalWideFibonacciTrace, MetalWideFibonacciTraceError,
    MetalWideFibonacciTraceRequest,
};
use super::workload_contract::{
    MetalWorkloadOwnership, MetalWorkloadStage, MetalWorkloadStageAssignment,
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
pub(crate) struct RegisteredMetalExecutionBinding {
    pub workload_boundary: RegisteredMetalWorkloadBoundaryInput,
    pub declaration_lowering: RegisteredMetalLoweringInput,
    pub declaration_route: MetalGeneratedRouteKind,
    pub supported_benchmark_operations: &'static [MetalRegisteredBenchmarkOperation],
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) struct RegisteredMetalExecutionSeed {
    pub component_name: &'static str,
    pub route: MetalGeneratedRouteKind,
    pub plan: MetalExecutionPlan,
    pub abi_symbols: &'static [&'static str],
    pub build_modules: &'static [&'static str],
    pub specialization_keys: &'static [&'static str],
    pub stage_assignments: &'static [MetalWorkloadStageAssignment],
    pub witness_hook: Option<&'static str>,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) enum RegisteredMetalExecutionSeedError {
    UnsupportedWitnessHook {
        component_name: &'static str,
        witness_hook: Option<&'static str>,
    },
    MissingSpecializationKey {
        component_name: &'static str,
        missing_key: &'static str,
    },
    PlanNotMetalCapable {
        component_name: &'static str,
        plan: MetalExecutionPlan,
    },
    UnsupportedCpuOwnership {
        component_name: &'static str,
        stage: MetalWorkloadStage,
    },
    NonCanonicDomain {
        component_name: &'static str,
    },
    WitnessInputLengthMismatch {
        component_name: &'static str,
        input_a_len: usize,
        input_b_len: usize,
    },
    WitnessInputLengthNotPowerOfTwo {
        component_name: &'static str,
        input_len: usize,
    },
    InvalidWitnessColumnCount {
        component_name: &'static str,
        n_columns: u32,
    },
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
            operation,
            plan_input: self.artifact.as_plan_input(operation),
        }
    }
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

pub(crate) fn registered_execution_binding(
    intent: MetalExecutionIntent,
    component_name: &'static str,
    declaration_route: MetalGeneratedRouteKind,
) -> Result<RegisteredMetalExecutionBinding, MetalPlannerError<'static>> {
    let declaration_registration =
        registered_generated_component(component_name, declaration_route)?;
    let workload_boundary = registered_workload_boundary_input(intent, component_name)?;

    assert_eq!(
        declaration_registration
            .generated_inventory()
            .registration_key,
        workload_boundary.generated_inventory.registration_key
    );
    assert_eq!(
        declaration_registration.generated_inventory().abi_family,
        workload_boundary.generated_inventory.abi_family
    );

    Ok(RegisteredMetalExecutionBinding {
        workload_boundary,
        declaration_lowering: declaration_registration.lowering_input(),
        declaration_route,
        supported_benchmark_operations: declaration_registration
            .artifact
            .supported_benchmark_operations,
    })
}

impl RegisteredMetalExecutionBinding {
    pub fn execution_seed(self) -> RegisteredMetalExecutionSeed {
        RegisteredMetalExecutionSeed {
            component_name: self.declaration_lowering.component_name,
            route: self.declaration_route,
            plan: self.workload_boundary.plan,
            abi_symbols: self.declaration_lowering.abi_symbols,
            build_modules: self.declaration_lowering.build_modules,
            specialization_keys: self.declaration_lowering.specialization_keys,
            stage_assignments: self.workload_boundary.stage_assignments,
            witness_hook: self.declaration_lowering.witness_hook,
        }
    }
}

impl RegisteredMetalExecutionSeed {
    pub(crate) fn stage_ownership(
        self,
        stage: MetalWorkloadStage,
    ) -> Option<MetalWorkloadOwnership> {
        self.stage_assignments
            .iter()
            .find(|assignment| assignment.stage == stage)
            .map(|assignment| assignment.ownership)
    }

    pub fn allow_cpu_fri_ready_evaluation(
        self,
        domain: CircleDomain,
    ) -> Result<(), RegisteredMetalExecutionSeedError> {
        if !matches!(
            self.plan,
            MetalExecutionPlan::MetalFriHybrid | MetalExecutionPlan::MetalFull
        ) {
            return Err(RegisteredMetalExecutionSeedError::PlanNotMetalCapable {
                component_name: self.component_name,
                plan: self.plan,
            });
        }
        if !domain.is_canonic() {
            return Err(RegisteredMetalExecutionSeedError::NonCanonicDomain {
                component_name: self.component_name,
            });
        }

        Ok(())
    }

    pub fn allow_cpu_quotient_evaluation(
        self,
        domain: CircleDomain,
    ) -> Result<(), RegisteredMetalExecutionSeedError> {
        if self.stage_ownership(MetalWorkloadStage::QuotientEval)
            != Some(MetalWorkloadOwnership::CpuOwned)
        {
            return Err(RegisteredMetalExecutionSeedError::UnsupportedCpuOwnership {
                component_name: self.component_name,
                stage: MetalWorkloadStage::QuotientEval,
            });
        }

        self.allow_cpu_fri_ready_evaluation(domain)
    }

    pub(crate) fn supports_hybrid_fri_lane(self) -> bool {
        matches!(
            self.plan,
            MetalExecutionPlan::MetalFriHybrid | MetalExecutionPlan::MetalFull
        ) && self.stage_ownership(MetalWorkloadStage::FriBlake2s)
            == Some(MetalWorkloadOwnership::MetalNative)
    }

    pub(crate) fn allow_cpu_wide_fibonacci_witness(
        self,
    ) -> Result<(), RegisteredMetalExecutionSeedError> {
        if self.stage_ownership(MetalWorkloadStage::WitnessMain)
            != Some(MetalWorkloadOwnership::CpuOwned)
        {
            return Err(RegisteredMetalExecutionSeedError::UnsupportedCpuOwnership {
                component_name: self.component_name,
                stage: MetalWorkloadStage::WitnessMain,
            });
        }
        if self.component_name != "fibonacci_example"
            || self.witness_hook != Some("ingest_cpu_wide_fibonacci_witness")
        {
            return Err(RegisteredMetalExecutionSeedError::UnsupportedWitnessHook {
                component_name: self.component_name,
                witness_hook: self.witness_hook,
            });
        }

        Ok(())
    }

    pub(crate) fn validate_wide_fibonacci_witness_shape(
        self,
        input_a_len: usize,
        input_b_len: usize,
        n_columns: u32,
    ) -> Result<u32, RegisteredMetalExecutionSeedError> {
        self.allow_cpu_wide_fibonacci_witness()?;

        if input_a_len != input_b_len {
            return Err(
                RegisteredMetalExecutionSeedError::WitnessInputLengthMismatch {
                    component_name: self.component_name,
                    input_a_len,
                    input_b_len,
                },
            );
        }
        if !input_a_len.is_power_of_two() {
            return Err(
                RegisteredMetalExecutionSeedError::WitnessInputLengthNotPowerOfTwo {
                    component_name: self.component_name,
                    input_len: input_a_len,
                },
            );
        }
        if n_columns < 2 {
            return Err(
                RegisteredMetalExecutionSeedError::InvalidWitnessColumnCount {
                    component_name: self.component_name,
                    n_columns,
                },
            );
        }

        Ok(input_a_len.ilog2())
    }

    pub fn wide_fibonacci_trace_request<'a>(
        self,
        input_a: &'a [BaseField],
        input_b: &'a [BaseField],
        n_columns: u32,
    ) -> Result<MetalWideFibonacciTraceRequest<'a>, RegisteredMetalExecutionSeedError> {
        self.validate_wide_fibonacci_witness_shape(input_a.len(), input_b.len(), n_columns)?;
        for missing_key in ["log_n_instances", "n_columns"] {
            if !self.specialization_keys.contains(&missing_key) {
                return Err(
                    RegisteredMetalExecutionSeedError::MissingSpecializationKey {
                        component_name: self.component_name,
                        missing_key,
                    },
                );
            }
        }

        Ok(MetalWideFibonacciTraceRequest {
            input_a,
            input_b,
            n_columns,
        })
    }

    pub fn generate_wide_fibonacci_trace(
        self,
        input_a: &[BaseField],
        input_b: &[BaseField],
        n_columns: u32,
    ) -> Result<MetalWideFibonacciTrace, MetalWideFibonacciTraceError> {
        let request = self
            .wide_fibonacci_trace_request(input_a, input_b, n_columns)
            .map_err(|error| MetalWideFibonacciTraceError::Runtime {
                message: format!(
                    "wide-fibonacci execution seed rejected trace generation: {error:?}"
                ),
            })?;
        generate_metal_wide_fibonacci_trace(request)
    }
}

pub(crate) fn registered_execution_seed(
    intent: MetalExecutionIntent,
    component_name: &'static str,
    declaration_route: MetalGeneratedRouteKind,
) -> Result<RegisteredMetalExecutionSeed, MetalPlannerError<'static>> {
    registered_execution_binding(intent, component_name, declaration_route)
        .map(RegisteredMetalExecutionBinding::execution_seed)
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
    use stwo::core::fields::m31::BaseField;
    use stwo::core::poly::circle::CanonicCoset;

    use super::{
        plan_registered_metal_prove, registered_execution_binding, registered_execution_seed,
        registered_generated_component, registered_runtime_plan_input,
        registered_workload_boundary_input, RegisteredMetalExecutionSeedError,
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
    fn single_component_registered_planner_supports_declared_components() {
        let plan =
            plan_registered_metal_prove(MetalExecutionIntent::PreferMetal, &["poseidon_example"])
                .unwrap();

        assert_eq!(plan.plan, MetalExecutionPlan::MetalFriHybrid);
    }

    #[test]
    fn registered_generated_component_fails_closed_for_unsupported_route() {
        let error = registered_generated_component(
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
        let registration = registered_generated_component(
            "fibonacci_example",
            MetalGeneratedRouteKind::RegisteredProve,
        )
        .unwrap();
        let runtime_input = registered_runtime_plan_input(
            "fibonacci_example",
            MetalGeneratedRouteKind::RegisteredProve,
            MetalOperationKind::Prove,
        )
        .unwrap();

        assert_eq!(runtime_input.operation, MetalOperationKind::Prove);
        assert_eq!(runtime_input.plan_input.component_name, "fibonacci_example");
        assert_eq!(
            registration.lowering_input().specialization_keys,
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
    fn registered_execution_binding_reuses_workload_boundary_input_for_benchmarks() {
        let binding = registered_execution_binding(
            MetalExecutionIntent::PreferMetal,
            "fibonacci_example",
            MetalGeneratedRouteKind::BenchmarkProveVerify,
        )
        .unwrap();

        assert_eq!(
            binding
                .workload_boundary
                .generated_inventory
                .registration_key,
            "fibonacci_example"
        );
        assert_eq!(
            binding.workload_boundary.generated_inventory.abi_family,
            "wide_fibonacci"
        );
        assert_eq!(
            binding.declaration_lowering.component_name,
            "fibonacci_example"
        );
        assert_eq!(
            binding.declaration_lowering.workload_family,
            "wide_fibonacci"
        );
        assert_eq!(
            binding.declaration_route,
            MetalGeneratedRouteKind::BenchmarkProveVerify
        );
        assert!(binding
            .supported_benchmark_operations
            .contains(&MetalRegisteredBenchmarkOperation::ProveVerify));
        assert_eq!(
            binding.workload_boundary.plan,
            MetalExecutionPlan::MetalFriHybrid
        );
    }

    #[test]
    fn registered_execution_binding_packages_shared_boundary_inputs() {
        let binding = registered_execution_binding(
            MetalExecutionIntent::PreferMetal,
            "fibonacci_example",
            MetalGeneratedRouteKind::BenchmarkTraceGeneration,
        )
        .unwrap();

        assert_eq!(
            binding.workload_boundary.plan,
            MetalExecutionPlan::MetalFriHybrid
        );
        assert_eq!(
            binding.declaration_lowering.component_name,
            "fibonacci_example"
        );
        assert_eq!(
            binding.declaration_route,
            MetalGeneratedRouteKind::BenchmarkTraceGeneration
        );
        assert!(binding
            .supported_benchmark_operations
            .contains(&MetalRegisteredBenchmarkOperation::TraceGeneration));
    }

    #[test]
    fn registered_execution_seed_flattens_binding_for_scheduling() {
        let seed = registered_execution_seed(
            MetalExecutionIntent::PreferMetal,
            "fibonacci_example",
            MetalGeneratedRouteKind::BenchmarkTraceGeneration,
        )
        .unwrap();

        assert_eq!(seed.component_name, "fibonacci_example");
        assert_eq!(
            seed.route,
            MetalGeneratedRouteKind::BenchmarkTraceGeneration
        );
        assert_eq!(seed.plan, MetalExecutionPlan::MetalFriHybrid);
        assert!(seed
            .abi_symbols
            .iter()
            .all(|symbol| symbol.starts_with("metal.")));
        assert!(seed.build_modules.contains(&"benchmark"));
        assert_eq!(seed.specialization_keys, &["log_n_instances", "n_columns"]);
        assert_eq!(seed.witness_hook, Some("ingest_cpu_wide_fibonacci_witness"));
    }

    #[test]
    fn registered_execution_seed_builds_wide_fibonacci_trace_request() {
        let seed = registered_execution_seed(
            MetalExecutionIntent::PreferMetal,
            "fibonacci_example",
            MetalGeneratedRouteKind::BenchmarkTraceGeneration,
        )
        .unwrap();
        let input_a = [
            BaseField::from_u32_unchecked(1),
            BaseField::from_u32_unchecked(2),
        ];
        let input_b = [
            BaseField::from_u32_unchecked(3),
            BaseField::from_u32_unchecked(4),
        ];

        let request = seed
            .wide_fibonacci_trace_request(&input_a, &input_b, 8)
            .unwrap();

        assert_eq!(request.input_a, &input_a);
        assert_eq!(request.input_b, &input_b);
        assert_eq!(request.n_columns, 8);
    }

    #[test]
    fn registered_execution_seed_validates_wide_fibonacci_witness_shape() {
        let seed = registered_execution_seed(
            MetalExecutionIntent::PreferMetal,
            "fibonacci_example",
            MetalGeneratedRouteKind::BenchmarkTraceGeneration,
        )
        .unwrap();

        let log_n_instances = seed
            .validate_wide_fibonacci_witness_shape(1 << 6, 1 << 6, 8)
            .unwrap();

        assert_eq!(log_n_instances, 6);
    }

    #[test]
    fn registered_execution_seed_rejects_wide_fibonacci_length_mismatch() {
        let seed = registered_execution_seed(
            MetalExecutionIntent::PreferMetal,
            "fibonacci_example",
            MetalGeneratedRouteKind::BenchmarkTraceGeneration,
        )
        .unwrap();

        let error = seed
            .validate_wide_fibonacci_witness_shape(1 << 6, (1 << 6) - 1, 8)
            .unwrap_err();

        assert_eq!(
            error,
            RegisteredMetalExecutionSeedError::WitnessInputLengthMismatch {
                component_name: "fibonacci_example",
                input_a_len: 1 << 6,
                input_b_len: (1 << 6) - 1,
            }
        );
    }

    #[test]
    fn registered_execution_seed_rejects_wrong_witness_hook() {
        let seed = registered_execution_seed(
            MetalExecutionIntent::PreferMetal,
            "blake_example",
            MetalGeneratedRouteKind::WorkloadBoundary,
        )
        .unwrap();
        let input_a = [BaseField::from_u32_unchecked(1)];
        let input_b = [BaseField::from_u32_unchecked(2)];

        let error = seed
            .wide_fibonacci_trace_request(&input_a, &input_b, 8)
            .unwrap_err();

        assert_eq!(
            error,
            RegisteredMetalExecutionSeedError::UnsupportedWitnessHook {
                component_name: "blake_example",
                witness_hook: None,
            }
        );
    }

    #[test]
    fn registered_execution_seed_allows_canonic_cpu_handoffs_for_hybrid_fibonacci() {
        let seed = registered_execution_seed(
            MetalExecutionIntent::PreferMetal,
            "fibonacci_example",
            MetalGeneratedRouteKind::WorkloadBoundary,
        )
        .unwrap();
        let domain = CanonicCoset::new(5).circle_domain();

        assert_eq!(seed.allow_cpu_fri_ready_evaluation(domain), Ok(()));
        assert_eq!(seed.allow_cpu_quotient_evaluation(domain), Ok(()));
        assert_eq!(seed.allow_cpu_wide_fibonacci_witness(), Ok(()));
        assert!(seed.supports_hybrid_fri_lane());
    }

    #[test]
    fn registered_execution_seed_rejects_cpu_handoffs_for_cpu_only_plan() {
        let seed = registered_execution_seed(
            MetalExecutionIntent::ForceCpu,
            "fibonacci_example",
            MetalGeneratedRouteKind::WorkloadBoundary,
        )
        .unwrap();
        let domain = CanonicCoset::new(5).circle_domain();

        assert_eq!(
            seed.allow_cpu_fri_ready_evaluation(domain),
            Err(RegisteredMetalExecutionSeedError::PlanNotMetalCapable {
                component_name: "fibonacci_example",
                plan: MetalExecutionPlan::CpuOnly,
            })
        );
        assert!(!seed.supports_hybrid_fri_lane());
    }
}
