use stwo::core::fields::m31::BaseField;
use stwo::core::fri::FriConfig;
use stwo::core::poly::circle::CircleDomain;
use stwo::core::vcs_lifted::blake2_merkle::Blake2sMerkleHasher;
use stwo::prover::backend::CpuBackend;
use stwo::prover::fri::FriDecommitResult;
use stwo::prover::poly::circle::SecureEvaluation;
use stwo::prover::poly::BitReversedOrder;

use super::artifact::{MetalGeneratedInventory, MetalGeneratedRouteKind};
use super::execution_plan::{registered_execution_binding, RegisteredMetalExecutionSeed};
use super::planner::{MetalExecutionIntent, MetalExecutionPlan, MetalPlannerError};
use super::subpath::MetalFriBlake2sSubpath;
use super::witness::{MetalWideFibonacciTrace, MetalWideFibonacciTraceError};
use super::workload_contract::{
    MetalWorkloadOwnership, MetalWorkloadStage, MetalWorkloadStageAssignment,
};
use crate::stwo_metal::secure_field_vec::SecureFieldVec;

#[derive(Clone, Debug)]
pub struct MetalWorkloadBoundary {
    workload_name: &'static str,
    generated_inventory: MetalGeneratedInventory,
    execution_seed: RegisteredMetalExecutionSeed,
}

impl MetalWorkloadBoundary {
    fn map_execution_seed_error(
        &self,
        error: super::execution_plan::RegisteredMetalExecutionSeedError,
    ) -> MetalWorkloadHandoffError<'static> {
        match error {
            super::execution_plan::RegisteredMetalExecutionSeedError::PlanNotMetalCapable {
                plan,
                ..
            } => MetalWorkloadHandoffError::PlanNotMetalCapable {
                workload_name: self.workload_name,
                plan,
            },
            super::execution_plan::RegisteredMetalExecutionSeedError::UnsupportedCpuOwnership {
                stage,
                ..
            } => MetalWorkloadHandoffError::UnsupportedCpuOwnership {
                workload_name: self.workload_name,
                stage,
            },
            super::execution_plan::RegisteredMetalExecutionSeedError::NonCanonicDomain { .. } => {
                MetalWorkloadHandoffError::NonCanonicDomain {
                    workload_name: self.workload_name,
                }
            }
            other => unreachable!(
                "workload staging should only delegate to evaluation-handoff seed checks, got {other:?}"
            ),
        }
    }

    pub fn workload_name(&self) -> &'static str {
        self.workload_name
    }

    pub fn plan(&self) -> MetalExecutionPlan {
        self.execution_seed.plan
    }

    pub fn stage_assignments(&self) -> &'static [MetalWorkloadStageAssignment] {
        self.execution_seed.stage_assignments
    }

    pub fn generated_inventory(&self) -> MetalGeneratedInventory {
        self.generated_inventory
    }

    pub fn stage_ownership(&self, stage: MetalWorkloadStage) -> Option<MetalWorkloadOwnership> {
        self.execution_seed.stage_ownership(stage)
    }

    pub fn ingest_cpu_fri_ready_evaluation(
        &self,
        evaluation: &SecureEvaluation<CpuBackend, BitReversedOrder>,
    ) -> Result<MetalFriReadyEvaluationInput, MetalWorkloadHandoffError<'static>> {
        self.execution_seed
            .allow_cpu_fri_ready_evaluation(evaluation.domain)
            .map_err(|error| self.map_execution_seed_error(error))?;

        Ok(MetalFriReadyEvaluationInput {
            workload_name: self.workload_name,
            domain: evaluation.domain,
            column: SecureFieldVec::from_vec(evaluation.values.to_vec()),
        })
    }

    pub fn ingest_cpu_quotient_evaluation(
        &self,
        quotient_evaluation: &SecureEvaluation<CpuBackend, BitReversedOrder>,
    ) -> Result<MetalCpuQuotientEvaluationInput, MetalWorkloadHandoffError<'static>> {
        self.execution_seed
            .allow_cpu_quotient_evaluation(quotient_evaluation.domain)
            .map_err(|error| self.map_execution_seed_error(error))?;

        Ok(MetalCpuQuotientEvaluationInput {
            workload_name: self.workload_name,
            quotient_evaluation: quotient_evaluation.clone(),
        })
    }

    pub fn ingest_cpu_wide_fibonacci_witness(
        &self,
        input_a: &[BaseField],
        input_b: &[BaseField],
        n_columns: u32,
    ) -> Result<MetalCpuWideFibonacciWitnessInput, MetalWorkloadHandoffError<'static>> {
        if self.workload_name != "fibonacci_example" {
            return Err(MetalWorkloadHandoffError::UnsupportedWitnessArtifact {
                workload_name: self.workload_name,
            });
        }
        if self.stage_ownership(MetalWorkloadStage::WitnessMain)
            != Some(MetalWorkloadOwnership::CpuOwned)
        {
            return Err(MetalWorkloadHandoffError::UnsupportedCpuOwnership {
                workload_name: self.workload_name,
                stage: MetalWorkloadStage::WitnessMain,
            });
        }
        if input_a.len() != input_b.len() {
            return Err(MetalWorkloadHandoffError::WitnessInputLengthMismatch {
                workload_name: self.workload_name,
                input_a_len: input_a.len(),
                input_b_len: input_b.len(),
            });
        }
        if !input_a.len().is_power_of_two() {
            return Err(MetalWorkloadHandoffError::WitnessInputLengthNotPowerOfTwo {
                workload_name: self.workload_name,
                input_len: input_a.len(),
            });
        }
        if n_columns < 2 {
            return Err(MetalWorkloadHandoffError::InvalidWitnessColumnCount {
                workload_name: self.workload_name,
                n_columns,
            });
        }

        Ok(MetalCpuWideFibonacciWitnessInput {
            workload_name: self.workload_name,
            log_n_instances: input_a.len().ilog2(),
            n_columns,
            input_a: input_a.to_vec(),
            input_b: input_b.to_vec(),
            execution_seed: self.execution_seed,
        })
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum MetalWorkloadHandoffError<'a> {
    PlanNotMetalCapable {
        workload_name: &'a str,
        plan: MetalExecutionPlan,
    },
    NonCanonicDomain {
        workload_name: &'a str,
    },
    WorkloadMismatch {
        expected_workload: &'a str,
        actual_workload: &'a str,
    },
    UnsupportedCpuOwnership {
        workload_name: &'a str,
        stage: MetalWorkloadStage,
    },
    UnsupportedWitnessArtifact {
        workload_name: &'a str,
    },
    WitnessInputLengthMismatch {
        workload_name: &'a str,
        input_a_len: usize,
        input_b_len: usize,
    },
    WitnessInputLengthNotPowerOfTwo {
        workload_name: &'a str,
        input_len: usize,
    },
    InvalidWitnessColumnCount {
        workload_name: &'a str,
        n_columns: u32,
    },
}

#[derive(Clone, Debug)]
pub struct MetalFriReadyEvaluationInput {
    workload_name: &'static str,
    domain: CircleDomain,
    column: SecureFieldVec,
}

impl MetalFriReadyEvaluationInput {
    pub fn workload_name(&self) -> &'static str {
        self.workload_name
    }

    pub fn domain(&self) -> CircleDomain {
        self.domain
    }

    pub fn column(&self) -> &SecureFieldVec {
        &self.column
    }
}

#[derive(Clone)]
pub struct MetalCpuQuotientEvaluationInput {
    workload_name: &'static str,
    quotient_evaluation: SecureEvaluation<CpuBackend, BitReversedOrder>,
}

impl MetalCpuQuotientEvaluationInput {
    pub fn workload_name(&self) -> &'static str {
        self.workload_name
    }

    pub fn quotient_evaluation(&self) -> &SecureEvaluation<CpuBackend, BitReversedOrder> {
        &self.quotient_evaluation
    }

    pub fn domain(&self) -> CircleDomain {
        self.quotient_evaluation.domain
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MetalCpuWideFibonacciWitnessInput {
    workload_name: &'static str,
    log_n_instances: u32,
    n_columns: u32,
    input_a: Vec<BaseField>,
    input_b: Vec<BaseField>,
    execution_seed: RegisteredMetalExecutionSeed,
}

impl MetalCpuWideFibonacciWitnessInput {
    pub fn workload_name(&self) -> &'static str {
        self.workload_name
    }

    pub fn log_n_instances(&self) -> u32 {
        self.log_n_instances
    }

    pub fn n_columns(&self) -> u32 {
        self.n_columns
    }

    pub fn input_a(&self) -> &[BaseField] {
        &self.input_a
    }

    pub fn input_b(&self) -> &[BaseField] {
        &self.input_b
    }

    pub fn generate_trace(&self) -> Result<MetalWideFibonacciTrace, MetalWideFibonacciTraceError> {
        self.execution_seed.generate_wide_fibonacci_trace(
            &self.input_a,
            &self.input_b,
            self.n_columns,
        )
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

    pub fn prove_input(
        &self,
        input: &MetalFriReadyEvaluationInput,
    ) -> Result<FriDecommitResult<Blake2sMerkleHasher>, MetalWorkloadHandoffError<'static>> {
        if input.workload_name() != self.boundary.workload_name() {
            return Err(MetalWorkloadHandoffError::WorkloadMismatch {
                expected_workload: self.boundary.workload_name(),
                actual_workload: input.workload_name(),
            });
        }

        Ok(self.prove(input.column(), input.domain()))
    }

    pub fn prove_from_cpu_evaluation(
        &self,
        evaluation: &SecureEvaluation<CpuBackend, BitReversedOrder>,
    ) -> Result<FriDecommitResult<Blake2sMerkleHasher>, MetalWorkloadHandoffError<'static>> {
        let input = self.boundary.ingest_cpu_fri_ready_evaluation(evaluation)?;
        self.prove_input(&input)
    }

    pub fn prove_from_cpu_quotient_evaluation(
        &self,
        quotient_evaluation: &SecureEvaluation<CpuBackend, BitReversedOrder>,
    ) -> Result<FriDecommitResult<Blake2sMerkleHasher>, MetalWorkloadHandoffError<'static>> {
        let input = self
            .boundary
            .ingest_cpu_quotient_evaluation(quotient_evaluation)?;
        self.prove_from_cpu_evaluation(input.quotient_evaluation())
    }
}

pub fn declare_exemplar_metal_workload_boundary(
    intent: MetalExecutionIntent,
    workload_name: &'static str,
) -> Result<MetalWorkloadBoundary, MetalPlannerError<'static>> {
    let binding = registered_execution_binding(
        intent,
        workload_name,
        MetalGeneratedRouteKind::WorkloadBoundary,
    )?;
    let boundary_input = binding.workload_boundary;

    Ok(MetalWorkloadBoundary {
        workload_name,
        generated_inventory: boundary_input.generated_inventory,
        execution_seed: binding.execution_seed(),
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
    use stwo::core::fields::m31::BaseField;
    use stwo::core::fields::qm31::SecureField;
    use stwo::core::poly::circle::CanonicCoset;
    use stwo::prover::backend::cpu::CpuCirclePoly;
    use stwo::prover::backend::CpuBackend;
    use stwo::prover::poly::circle::SecureEvaluation;
    use stwo::prover::poly::BitReversedOrder;

    use super::{
        declare_exemplar_metal_workload_boundary, MetalExecutionIntent, MetalExecutionPlan,
        MetalWorkloadHandoffError, MetalWorkloadOwnership, MetalWorkloadStage,
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
        assert_eq!(
            boundary.generated_inventory().registration_key,
            "fibonacci_example"
        );
        assert_eq!(boundary.generated_inventory().abi_family, "wide_fibonacci");
        assert_eq!(
            boundary.generated_inventory().specialization_keys,
            &["log_n_instances", "n_columns"]
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

    #[test]
    fn boundary_ingests_cpu_fri_ready_evaluation() {
        const LOG_SIZE: u32 = 5;

        let domain = CanonicCoset::new(LOG_SIZE).circle_domain();
        let values = CpuCirclePoly::new(
            (1..=(1 << 4))
                .map(|i| BaseField::from_u32_unchecked(i))
                .collect(),
        )
        .evaluate(domain)
        .values
        .into_iter()
        .map(SecureField::from)
        .collect::<Vec<_>>();
        let evaluation = SecureEvaluation::<CpuBackend, BitReversedOrder>::new(
            domain,
            values.clone().into_iter().collect(),
        );
        let boundary = declare_exemplar_metal_workload_boundary(
            MetalExecutionIntent::PreferMetal,
            "fibonacci_example",
        )
        .unwrap();

        let input = boundary
            .ingest_cpu_fri_ready_evaluation(&evaluation)
            .unwrap();

        assert_eq!(input.workload_name(), "fibonacci_example");
        assert_eq!(input.domain(), domain);
        assert_eq!(input.column().to_vec(), values);
    }

    #[test]
    fn boundary_ingests_cpu_quotient_evaluation() {
        const LOG_SIZE: u32 = 5;

        let domain = CanonicCoset::new(LOG_SIZE).circle_domain();
        let values = CpuCirclePoly::new(
            (1..=(1 << 4))
                .map(|i| BaseField::from_u32_unchecked(i))
                .collect(),
        )
        .evaluate(domain)
        .values
        .into_iter()
        .map(SecureField::from)
        .collect::<Vec<_>>();
        let evaluation = SecureEvaluation::<CpuBackend, BitReversedOrder>::new(
            domain,
            values.clone().into_iter().collect(),
        );
        let boundary = declare_exemplar_metal_workload_boundary(
            MetalExecutionIntent::PreferMetal,
            "fibonacci_example",
        )
        .unwrap();

        let input = boundary
            .ingest_cpu_quotient_evaluation(&evaluation)
            .unwrap();

        assert_eq!(input.workload_name(), "fibonacci_example");
        assert_eq!(input.domain(), domain);
        assert_eq!(input.quotient_evaluation().values.to_vec(), values);
    }

    #[test]
    fn cpu_only_boundary_rejects_fri_ready_handoff() {
        const LOG_SIZE: u32 = 5;

        let domain = CanonicCoset::new(LOG_SIZE).circle_domain();
        let values = CpuCirclePoly::new(
            (1..=(1 << 4))
                .map(|i| BaseField::from_u32_unchecked(i))
                .collect(),
        )
        .evaluate(domain)
        .values
        .into_iter()
        .map(SecureField::from)
        .collect::<Vec<_>>();
        let evaluation = SecureEvaluation::<CpuBackend, BitReversedOrder>::new(
            domain,
            values.into_iter().collect(),
        );
        let boundary = declare_exemplar_metal_workload_boundary(
            MetalExecutionIntent::ForceCpu,
            "fibonacci_example",
        )
        .unwrap();

        let error = boundary
            .ingest_cpu_fri_ready_evaluation(&evaluation)
            .unwrap_err();

        assert_eq!(
            error,
            MetalWorkloadHandoffError::PlanNotMetalCapable {
                workload_name: "fibonacci_example",
                plan: MetalExecutionPlan::CpuOnly,
            }
        );
    }
}
