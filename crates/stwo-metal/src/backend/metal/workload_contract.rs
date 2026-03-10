use super::planner::MetalExecutionPlan;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum MetalWorkloadStage {
    WitnessMain,
    WitnessInteraction,
    QuotientEval,
    PcsCommitment,
    FriBlake2s,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum MetalWorkloadOwnership {
    MetalNative,
    CpuOwned,
    NotApplicable,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct MetalWorkloadStageAssignment {
    pub stage: MetalWorkloadStage,
    pub ownership: MetalWorkloadOwnership,
    pub detail: &'static str,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct MetalExecutionAuthority {
    plan: MetalExecutionPlan,
    stage_assignments: &'static [MetalWorkloadStageAssignment],
}

impl MetalExecutionAuthority {
    pub(crate) fn new(
        plan: MetalExecutionPlan,
        stage_assignments: &'static [MetalWorkloadStageAssignment],
    ) -> Self {
        Self {
            plan,
            stage_assignments,
        }
    }

    pub fn plan(self) -> MetalExecutionPlan {
        self.plan
    }

    pub fn stage_assignments(self) -> &'static [MetalWorkloadStageAssignment] {
        self.stage_assignments
    }

    pub fn stage_ownership(self, stage: MetalWorkloadStage) -> Option<MetalWorkloadOwnership> {
        self.stage_assignments
            .iter()
            .find(|assignment| assignment.stage == stage)
            .map(|assignment| assignment.ownership)
    }
}
