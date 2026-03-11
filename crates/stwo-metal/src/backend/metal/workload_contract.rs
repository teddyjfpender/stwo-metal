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

pub(crate) fn stage_ownership(
    stage_assignments: &'static [MetalWorkloadStageAssignment],
    stage: MetalWorkloadStage,
) -> Option<MetalWorkloadOwnership> {
    stage_assignments
        .iter()
        .find(|assignment| assignment.stage == stage)
        .map(|assignment| assignment.ownership)
}
