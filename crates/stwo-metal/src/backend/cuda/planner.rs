use super::planner_manifest_v1_generated::planner_input_for_prove;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum CudaExecutionIntent {
    ForceCpu,
    PreferCuda,
    RequireCuda,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum CudaOperationKind {
    Prove,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum CudaComponentCapability {
    ConstraintEval,
    WitnessMain,
    WitnessInteraction,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum CudaSupportTier {
    ConstraintEvalOnly,
    ConstraintAndInteraction,
    FullWitness,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum CudaExecutionPlan {
    CpuOnly,
    CudaFull,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct UnsupportedCudaPlan<'a> {
    pub component_name: &'a str,
    pub missing_capability: CudaComponentCapability,
    pub intent: CudaExecutionIntent,
    pub operation: CudaOperationKind,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct UnknownCudaComponent<'a> {
    pub component_name: &'a str,
    pub operation: CudaOperationKind,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum CudaPlannerError<'a> {
    UnknownComponent(UnknownCudaComponent<'a>),
    Unsupported(UnsupportedCudaPlan<'a>),
}

#[derive(Copy, Clone, Debug)]
pub struct CudaComponentPlanInput<'a> {
    pub component_name: &'a str,
    pub support_tier: CudaSupportTier,
    pub declared_capabilities: &'a [CudaComponentCapability],
    pub required_capabilities: &'a [CudaComponentCapability],
}

impl<'a> CudaComponentPlanInput<'a> {
    fn first_missing_capability(self) -> Option<CudaComponentCapability> {
        self.required_capabilities
            .iter()
            .copied()
            .find(|required| !self.declared_capabilities.contains(required))
    }
}

pub fn plan_cuda_operation<'a>(
    intent: CudaExecutionIntent,
    operation: CudaOperationKind,
    components: &[CudaComponentPlanInput<'a>],
) -> Result<CudaExecutionPlan, UnsupportedCudaPlan<'a>> {
    if matches!(intent, CudaExecutionIntent::ForceCpu) {
        return Ok(CudaExecutionPlan::CpuOnly);
    }

    let missing = components.iter().find_map(|component| {
        component
            .first_missing_capability()
            .map(|missing_capability| UnsupportedCudaPlan {
                component_name: component.component_name,
                missing_capability,
                intent,
                operation,
            })
    });

    match (intent, missing) {
        (_, None) => Ok(CudaExecutionPlan::CudaFull),
        (CudaExecutionIntent::PreferCuda, Some(_)) => Ok(CudaExecutionPlan::CpuOnly),
        (CudaExecutionIntent::RequireCuda, Some(problem)) => Err(problem),
        (CudaExecutionIntent::ForceCpu, _) => Ok(CudaExecutionPlan::CpuOnly),
    }
}

pub fn plan_exemplar_prove_by_name<'a>(
    intent: CudaExecutionIntent,
    component_names: &'a [&'a str],
) -> Result<CudaExecutionPlan, CudaPlannerError<'a>> {
    let inputs = component_names
        .iter()
        .map(|component_name| {
            planner_input_for_prove(component_name).ok_or(CudaPlannerError::UnknownComponent(
                UnknownCudaComponent {
                    component_name,
                    operation: CudaOperationKind::Prove,
                },
            ))
        })
        .collect::<Result<Vec<_>, _>>()?;

    plan_cuda_operation(intent, CudaOperationKind::Prove, &inputs)
        .map_err(CudaPlannerError::Unsupported)
}

#[cfg(test)]
mod tests {
    use super::{
        plan_cuda_operation, CudaComponentCapability, CudaComponentPlanInput, CudaExecutionIntent,
        CudaExecutionPlan, CudaOperationKind, CudaSupportTier, UnsupportedCudaPlan,
    };

    const CONSTRAINT_ONLY: &[CudaComponentCapability] = &[CudaComponentCapability::ConstraintEval];
    const FULL_WITNESS: &[CudaComponentCapability] = &[
        CudaComponentCapability::ConstraintEval,
        CudaComponentCapability::WitnessMain,
        CudaComponentCapability::WitnessInteraction,
    ];

    #[test]
    fn force_cpu_always_chooses_cpu_plan() {
        let components = [CudaComponentPlanInput {
            component_name: "poseidon_example",
            support_tier: CudaSupportTier::ConstraintEvalOnly,
            declared_capabilities: CONSTRAINT_ONLY,
            required_capabilities: FULL_WITNESS,
        }];

        let plan = plan_cuda_operation(
            CudaExecutionIntent::ForceCpu,
            CudaOperationKind::Prove,
            &components,
        )
        .unwrap();

        assert_eq!(plan, CudaExecutionPlan::CpuOnly);
    }

    #[test]
    fn prefer_cuda_uses_cuda_when_all_required_capabilities_exist() {
        let components = [CudaComponentPlanInput {
            component_name: "poseidon_example",
            support_tier: CudaSupportTier::FullWitness,
            declared_capabilities: FULL_WITNESS,
            required_capabilities: FULL_WITNESS,
        }];

        let plan = plan_cuda_operation(
            CudaExecutionIntent::PreferCuda,
            CudaOperationKind::Prove,
            &components,
        )
        .unwrap();

        assert_eq!(plan, CudaExecutionPlan::CudaFull);
    }

    #[test]
    fn prefer_cuda_falls_back_to_cpu_when_a_required_capability_is_missing() {
        let components = [CudaComponentPlanInput {
            component_name: "poseidon_example",
            support_tier: CudaSupportTier::ConstraintEvalOnly,
            declared_capabilities: CONSTRAINT_ONLY,
            required_capabilities: FULL_WITNESS,
        }];

        let plan = plan_cuda_operation(
            CudaExecutionIntent::PreferCuda,
            CudaOperationKind::Prove,
            &components,
        )
        .unwrap();

        assert_eq!(plan, CudaExecutionPlan::CpuOnly);
    }

    #[test]
    fn require_cuda_fails_explicitly_when_a_required_capability_is_missing() {
        let components = [CudaComponentPlanInput {
            component_name: "poseidon_example",
            support_tier: CudaSupportTier::ConstraintEvalOnly,
            declared_capabilities: CONSTRAINT_ONLY,
            required_capabilities: FULL_WITNESS,
        }];

        let error = plan_cuda_operation(
            CudaExecutionIntent::RequireCuda,
            CudaOperationKind::Prove,
            &components,
        )
        .unwrap_err();

        assert_eq!(
            error,
            UnsupportedCudaPlan {
                component_name: "poseidon_example",
                missing_capability: CudaComponentCapability::WitnessMain,
                intent: CudaExecutionIntent::RequireCuda,
                operation: CudaOperationKind::Prove,
            }
        );
    }

    #[test]
    fn capabilities_drive_planning_even_when_tier_is_full_witness() {
        let components = [CudaComponentPlanInput {
            component_name: "wide_fibonacci_example",
            support_tier: CudaSupportTier::FullWitness,
            declared_capabilities: CONSTRAINT_ONLY,
            required_capabilities: &[
                CudaComponentCapability::ConstraintEval,
                CudaComponentCapability::WitnessMain,
            ],
        }];

        let error = plan_cuda_operation(
            CudaExecutionIntent::RequireCuda,
            CudaOperationKind::Prove,
            &components,
        )
        .unwrap_err();

        assert_eq!(
            error.missing_capability,
            CudaComponentCapability::WitnessMain
        );
    }
}
