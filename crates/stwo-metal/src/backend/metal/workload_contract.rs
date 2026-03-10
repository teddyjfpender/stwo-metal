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
