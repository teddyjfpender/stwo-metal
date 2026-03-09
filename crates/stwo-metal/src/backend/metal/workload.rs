use stwo::core::fri::FriConfig;
use stwo::core::poly::circle::CircleDomain;
use stwo::core::vcs_lifted::blake2_merkle::Blake2sMerkleHasher;
use stwo::prover::fri::FriDecommitResult;

use super::planner::{
    plan_metal_operation, MetalExecutionIntent, MetalExecutionPlan, MetalOperationKind,
    MetalPlannerError,
};
use super::planner_manifest_v1_generated::planner_input_for_prove;
use super::subpath::MetalFriBlake2sSubpath;
use crate::stwo_metal::secure_field_vec::SecureFieldVec;

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

#[derive(Clone, Debug)]
pub struct MetalWorkloadBoundary {
    workload_name: &'static str,
    plan: MetalExecutionPlan,
    stage_assignments: &'static [MetalWorkloadStageAssignment],
}

impl MetalWorkloadBoundary {
    pub fn workload_name(&self) -> &'static str {
        self.workload_name
    }

    pub fn plan(&self) -> MetalExecutionPlan {
        self.plan
    }

    pub fn stage_assignments(&self) -> &'static [MetalWorkloadStageAssignment] {
        self.stage_assignments
    }

    pub fn stage_ownership(&self, stage: MetalWorkloadStage) -> Option<MetalWorkloadOwnership> {
        self.stage_assignments
            .iter()
            .find(|assignment| assignment.stage == stage)
            .map(|assignment| assignment.ownership)
    }
}

#[derive(Clone, Debug)]
pub struct MetalHybridFriWorkload {
    boundary: MetalWorkloadBoundary,
    fri_subpath: MetalFriBlake2sSubpath,
}

impl MetalHybridFriWorkload {
    pub fn boundary(&self) -> &MetalWorkloadBoundary {
        &self.boundary
    }

    pub fn fri_subpath(&self) -> &MetalFriBlake2sSubpath {
        &self.fri_subpath
    }

    pub fn prove(
        &self,
        column: &SecureFieldVec,
        domain: CircleDomain,
    ) -> FriDecommitResult<Blake2sMerkleHasher> {
        self.fri_subpath.prove(column, domain)
    }
}

const FIBONACCI_EXAMPLE_STAGE_ASSIGNMENTS: &[MetalWorkloadStageAssignment] = &[
    MetalWorkloadStageAssignment {
        stage: MetalWorkloadStage::WitnessMain,
        ownership: MetalWorkloadOwnership::CpuOwned,
        detail: "The main trace witness path remains CPU-owned for the declared Fibonacci workload boundary.",
    },
    MetalWorkloadStageAssignment {
        stage: MetalWorkloadStage::WitnessInteraction,
        ownership: MetalWorkloadOwnership::NotApplicable,
        detail: "The declared Fibonacci workload boundary has no interaction trace stage.",
    },
    MetalWorkloadStageAssignment {
        stage: MetalWorkloadStage::QuotientEval,
        ownership: MetalWorkloadOwnership::CpuOwned,
        detail: "Constraint quotient evaluation remains outside the declared Metal workload boundary.",
    },
    MetalWorkloadStageAssignment {
        stage: MetalWorkloadStage::PcsCommitment,
        ownership: MetalWorkloadOwnership::CpuOwned,
        detail: "PCS commitment ownership remains CPU-side for the declared Fibonacci workload boundary.",
    },
    MetalWorkloadStageAssignment {
        stage: MetalWorkloadStage::FriBlake2s,
        ownership: MetalWorkloadOwnership::MetalNative,
        detail: "The bounded Blake2s FRI sub-path is owned by the native Metal lane for the declared Fibonacci workload boundary.",
    },
];

const POSEIDON_EXAMPLE_STAGE_ASSIGNMENTS: &[MetalWorkloadStageAssignment] = &[
    MetalWorkloadStageAssignment {
        stage: MetalWorkloadStage::WitnessMain,
        ownership: MetalWorkloadOwnership::CpuOwned,
        detail: "The main Poseidon witness path remains CPU-owned for the declared workload boundary.",
    },
    MetalWorkloadStageAssignment {
        stage: MetalWorkloadStage::WitnessInteraction,
        ownership: MetalWorkloadOwnership::CpuOwned,
        detail: "The interaction witness path remains CPU-owned for the declared Poseidon workload boundary.",
    },
    MetalWorkloadStageAssignment {
        stage: MetalWorkloadStage::QuotientEval,
        ownership: MetalWorkloadOwnership::CpuOwned,
        detail: "Constraint quotient evaluation remains outside the declared Metal workload boundary.",
    },
    MetalWorkloadStageAssignment {
        stage: MetalWorkloadStage::PcsCommitment,
        ownership: MetalWorkloadOwnership::CpuOwned,
        detail: "PCS commitment ownership remains CPU-side for the declared Poseidon workload boundary.",
    },
    MetalWorkloadStageAssignment {
        stage: MetalWorkloadStage::FriBlake2s,
        ownership: MetalWorkloadOwnership::MetalNative,
        detail: "The bounded Blake2s FRI sub-path is owned by the native Metal lane for the declared Poseidon workload boundary.",
    },
];

fn stage_assignments_for_workload(
    workload_name: &'static str,
) -> Result<&'static [MetalWorkloadStageAssignment], MetalPlannerError<'static>> {
    match workload_name {
        "fibonacci_example" => Ok(FIBONACCI_EXAMPLE_STAGE_ASSIGNMENTS),
        "poseidon_example" => Ok(POSEIDON_EXAMPLE_STAGE_ASSIGNMENTS),
        _ => Err(MetalPlannerError::UnknownComponent(
            super::planner::UnknownMetalComponent {
                component_name: workload_name,
                operation: super::planner::MetalOperationKind::Prove,
            },
        )),
    }
}

pub fn declare_exemplar_metal_workload_boundary(
    intent: MetalExecutionIntent,
    workload_name: &'static str,
) -> Result<MetalWorkloadBoundary, MetalPlannerError<'static>> {
    let input = planner_input_for_prove(workload_name).ok_or(
        MetalPlannerError::UnknownComponent(super::planner::UnknownMetalComponent {
            component_name: workload_name,
            operation: MetalOperationKind::Prove,
        }),
    )?;
    let plan = plan_metal_operation(intent, MetalOperationKind::Prove, &[input])
        .map_err(MetalPlannerError::Unsupported)?;
    let stage_assignments = stage_assignments_for_workload(workload_name)?;

    Ok(MetalWorkloadBoundary {
        workload_name,
        plan,
        stage_assignments,
    })
}

pub fn declare_exemplar_hybrid_fri_workload(
    intent: MetalExecutionIntent,
    workload_name: &'static str,
    config: FriConfig,
) -> Result<MetalHybridFriWorkload, MetalPlannerError<'static>> {
    let boundary = declare_exemplar_metal_workload_boundary(intent, workload_name)?;

    assert!(
        matches!(
            boundary.plan(),
            MetalExecutionPlan::MetalFriHybrid | MetalExecutionPlan::MetalFull
        ),
        "hybrid FRI workload declaration requires a Metal-capable workload boundary"
    );
    assert_eq!(
        boundary.stage_ownership(MetalWorkloadStage::FriBlake2s),
        Some(MetalWorkloadOwnership::MetalNative),
        "declared hybrid FRI workload must keep the FRI stage on the Metal lane"
    );

    Ok(MetalHybridFriWorkload {
        boundary,
        fri_subpath: MetalFriBlake2sSubpath::new(config),
    })
}

#[cfg(test)]
mod tests {
    use super::{
        declare_exemplar_metal_workload_boundary, MetalExecutionIntent, MetalExecutionPlan,
        MetalWorkloadOwnership, MetalWorkloadStage,
    };

    #[test]
    fn fibonacci_boundary_declares_hybrid_fri_and_cpu_owned_gaps() {
        let boundary = declare_exemplar_metal_workload_boundary(
            MetalExecutionIntent::PreferMetal,
            "fibonacci_example",
        )
        .unwrap();

        assert_eq!(boundary.plan(), MetalExecutionPlan::MetalFriHybrid);
        assert_eq!(
            boundary.stage_ownership(MetalWorkloadStage::WitnessMain),
            Some(MetalWorkloadOwnership::CpuOwned)
        );
        assert_eq!(
            boundary.stage_ownership(MetalWorkloadStage::FriBlake2s),
            Some(MetalWorkloadOwnership::MetalNative)
        );
    }

    #[test]
    fn poseidon_boundary_declares_cpu_owned_interaction_trace() {
        let boundary = declare_exemplar_metal_workload_boundary(
            MetalExecutionIntent::PreferMetal,
            "poseidon_example",
        )
        .unwrap();

        assert_eq!(
            boundary.stage_ownership(MetalWorkloadStage::WitnessInteraction),
            Some(MetalWorkloadOwnership::CpuOwned)
        );
    }
}
