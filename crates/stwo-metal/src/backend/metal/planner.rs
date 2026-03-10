use super::execution_plan::plan_registered_metal_prove;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum MetalExecutionIntent {
    ForceCpu,
    PreferMetal,
    RequireMetal,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum MetalOperationKind {
    Prove,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum MetalComponentCapability {
    FriBlake2sSubpath,
    QuotientEval,
    WitnessMain,
    WitnessInteraction,
    PcsCommitment,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum MetalSupportTier {
    FriOnly,
    HybridFriMetal,
    FullWorkload,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum MetalExecutionPlan {
    CpuOnly,
    MetalFriHybrid,
    MetalFull,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct UnsupportedMetalPlan<'a> {
    pub component_name: &'a str,
    pub missing_capability: MetalComponentCapability,
    pub intent: MetalExecutionIntent,
    pub operation: MetalOperationKind,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct UnknownMetalComponent<'a> {
    pub component_name: &'a str,
    pub operation: MetalOperationKind,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct UnsupportedGeneratedMetalRoute<'a> {
    pub component_name: &'a str,
    pub route: super::artifact::MetalGeneratedRouteKind,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum MetalPlannerError<'a> {
    UnknownComponent(UnknownMetalComponent<'a>),
    Unsupported(UnsupportedMetalPlan<'a>),
    UnsupportedGeneratedRoute(UnsupportedGeneratedMetalRoute<'a>),
}

#[derive(Copy, Clone, Debug)]
pub struct MetalComponentPlanInput<'a> {
    pub component_name: &'a str,
    pub support_tier: MetalSupportTier,
    pub declared_capabilities: &'a [MetalComponentCapability],
    pub required_capabilities: &'a [MetalComponentCapability],
}

impl<'a> MetalComponentPlanInput<'a> {
    fn first_missing_capability(self) -> Option<MetalComponentCapability> {
        self.required_capabilities
            .iter()
            .copied()
            .find(|required| !self.declared_capabilities.contains(required))
    }
}

pub fn plan_metal_operation<'a>(
    intent: MetalExecutionIntent,
    operation: MetalOperationKind,
    components: &[MetalComponentPlanInput<'a>],
) -> Result<MetalExecutionPlan, UnsupportedMetalPlan<'a>> {
    if matches!(intent, MetalExecutionIntent::ForceCpu) {
        return Ok(MetalExecutionPlan::CpuOnly);
    }

    let missing = components.iter().find_map(|component| {
        component
            .first_missing_capability()
            .map(|missing_capability| UnsupportedMetalPlan {
                component_name: component.component_name,
                missing_capability,
                intent,
                operation,
            })
    });

    match (intent, missing) {
        (_, None) => Ok(MetalExecutionPlan::MetalFull),
        (MetalExecutionIntent::PreferMetal, Some(_))
            if components.iter().all(|component| {
                component
                    .declared_capabilities
                    .contains(&MetalComponentCapability::FriBlake2sSubpath)
            }) =>
        {
            Ok(MetalExecutionPlan::MetalFriHybrid)
        }
        (MetalExecutionIntent::PreferMetal, Some(_)) => Ok(MetalExecutionPlan::CpuOnly),
        (MetalExecutionIntent::RequireMetal, Some(problem)) => Err(problem),
        (MetalExecutionIntent::ForceCpu, _) => Ok(MetalExecutionPlan::CpuOnly),
    }
}

pub fn plan_exemplar_metal_prove_by_name<'a>(
    intent: MetalExecutionIntent,
    component_names: &'a [&'a str],
) -> Result<MetalExecutionPlan, MetalPlannerError<'a>> {
    plan_registered_metal_prove(intent, component_names).map(|plan| plan.plan)
}

#[cfg(test)]
mod tests {
    use super::{
        plan_exemplar_metal_prove_by_name, plan_metal_operation, MetalComponentCapability,
        MetalComponentPlanInput, MetalExecutionIntent, MetalExecutionPlan, MetalOperationKind,
        MetalSupportTier, UnsupportedMetalPlan,
    };

    const FRI_ONLY: &[MetalComponentCapability] = &[MetalComponentCapability::FriBlake2sSubpath];
    const FIBONACCI_REQUIRED: &[MetalComponentCapability] = &[
        MetalComponentCapability::FriBlake2sSubpath,
        MetalComponentCapability::WitnessMain,
        MetalComponentCapability::QuotientEval,
        MetalComponentCapability::PcsCommitment,
    ];

    #[test]
    fn force_cpu_always_chooses_cpu_plan() {
        let components = [MetalComponentPlanInput {
            component_name: "fibonacci_example",
            support_tier: MetalSupportTier::FriOnly,
            declared_capabilities: FRI_ONLY,
            required_capabilities: FIBONACCI_REQUIRED,
        }];

        let plan = plan_metal_operation(
            MetalExecutionIntent::ForceCpu,
            MetalOperationKind::Prove,
            &components,
        )
        .unwrap();

        assert_eq!(plan, MetalExecutionPlan::CpuOnly);
    }

    #[test]
    fn prefer_metal_chooses_hybrid_when_fri_subpath_is_available() {
        let components = [MetalComponentPlanInput {
            component_name: "fibonacci_example",
            support_tier: MetalSupportTier::FriOnly,
            declared_capabilities: FRI_ONLY,
            required_capabilities: FIBONACCI_REQUIRED,
        }];

        let plan = plan_metal_operation(
            MetalExecutionIntent::PreferMetal,
            MetalOperationKind::Prove,
            &components,
        )
        .unwrap();

        assert_eq!(plan, MetalExecutionPlan::MetalFriHybrid);
    }

    #[test]
    fn require_metal_fails_when_non_fri_capabilities_are_missing() {
        let components = [MetalComponentPlanInput {
            component_name: "fibonacci_example",
            support_tier: MetalSupportTier::FriOnly,
            declared_capabilities: FRI_ONLY,
            required_capabilities: FIBONACCI_REQUIRED,
        }];

        let error = plan_metal_operation(
            MetalExecutionIntent::RequireMetal,
            MetalOperationKind::Prove,
            &components,
        )
        .unwrap_err();

        assert_eq!(
            error,
            UnsupportedMetalPlan {
                component_name: "fibonacci_example",
                missing_capability: MetalComponentCapability::WitnessMain,
                intent: MetalExecutionIntent::RequireMetal,
                operation: MetalOperationKind::Prove,
            }
        );
    }

    #[test]
    fn exemplar_planner_reports_hybrid_fri_for_fibonacci() {
        let plan = plan_exemplar_metal_prove_by_name(
            MetalExecutionIntent::PreferMetal,
            &["fibonacci_example"],
        )
        .unwrap();

        assert_eq!(plan, MetalExecutionPlan::MetalFriHybrid);
    }
}
