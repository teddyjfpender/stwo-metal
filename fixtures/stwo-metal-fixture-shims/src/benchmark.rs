use stwo::core::channel::Blake2sChannel;
use stwo::core::fields::qm31::{SecureField, SECURE_EXTENSION_DEGREE};
use stwo::core::pcs::utils::get_lifting_log_size;
use stwo::core::pcs::TreeVec;
use stwo::core::vcs_lifted::blake2_merkle::Blake2sMerkleChannel;
use stwo::prover::{CommitmentSchemeProver, ComponentProvers};
use stwo_metal::{
    MetalBackend, MetalBenchmarkLaneError, MetalExecutionPlan, MetalWideFibonacciBenchmarkBoundary,
    MetalWorkloadStage,
};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct WideFibonacciProveValuesLane {
    workload_name: &'static str,
}

impl WideFibonacciProveValuesLane {
    pub fn workload_name(self) -> &'static str {
        self.workload_name
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum WideFibonacciProveValuesLaneError {
    PlanNotMetalCapable {
        workload_name: &'static str,
        plan: MetalExecutionPlan,
    },
    UnsupportedWitnessOwnership {
        workload_name: &'static str,
        stage: MetalWorkloadStage,
    },
    UnsupportedFriOwnership {
        workload_name: &'static str,
        stage: MetalWorkloadStage,
    },
}

pub struct WideFibonacciProveValuesStaging {
    pub oods_point: stwo::core::circle::CirclePoint<SecureField>,
    pub max_log_degree_bound: u32,
    pub sample_points: TreeVec<Vec<Vec<stwo::core::circle::CirclePoint<SecureField>>>>,
}

pub fn registered_wide_fibonacci_prove_values_lane(
    benchmark_boundary: &MetalWideFibonacciBenchmarkBoundary,
) -> Result<WideFibonacciProveValuesLane, WideFibonacciProveValuesLaneError> {
    benchmark_boundary
        .validate_prove_values_lane()
        .map(|workload_name| WideFibonacciProveValuesLane { workload_name })
        .map_err(|error| match error {
            MetalBenchmarkLaneError::PlanNotMetalCapable {
                workload_name,
                plan,
            } => WideFibonacciProveValuesLaneError::PlanNotMetalCapable {
                workload_name,
                plan,
            },
            MetalBenchmarkLaneError::UnsupportedWitnessOwnership {
                workload_name,
                stage,
            } => WideFibonacciProveValuesLaneError::UnsupportedWitnessOwnership {
                workload_name,
                stage,
            },
            MetalBenchmarkLaneError::UnsupportedFriOwnership {
                workload_name,
                stage,
            } => WideFibonacciProveValuesLaneError::UnsupportedFriOwnership {
                workload_name,
                stage,
            },
        })
}

pub fn stage_wide_fibonacci_prove_values(
    lane: &WideFibonacciProveValuesLane,
    component_provers: &ComponentProvers<'_, MetalBackend>,
    channel: &mut Blake2sChannel,
    commitment_scheme: &CommitmentSchemeProver<'_, MetalBackend, Blake2sMerkleChannel>,
) -> WideFibonacciProveValuesStaging {
    let _ = lane.workload_name();
    let oods_point = stwo::core::circle::CirclePoint::<SecureField>::get_random_point(channel);
    let split_composition_log_size = commitment_scheme
        .trees
        .last()
        .unwrap()
        .commitment
        .layers
        .len() as u32
        - 1;
    let lifting_log_size =
        get_lifting_log_size(&commitment_scheme.config, split_composition_log_size);
    let max_log_degree_bound =
        lifting_log_size - commitment_scheme.config.fri_config.log_blowup_factor;

    let mut sample_points =
        component_provers
            .components()
            .mask_points(oods_point, max_log_degree_bound, false);
    sample_points.push(vec![vec![oods_point]; 2 * SECURE_EXTENSION_DEGREE]);

    WideFibonacciProveValuesStaging {
        oods_point,
        max_log_degree_bound,
        sample_points,
    }
}

#[cfg(test)]
mod tests {
    use stwo_metal::{MetalExecutionIntent, MetalExecutionPlan};

    use super::{registered_wide_fibonacci_prove_values_lane, WideFibonacciProveValuesLaneError};

    #[test]
    fn registered_benchmark_boundary_satisfies_prove_values_staging_contract() {
        let boundary = stwo_metal::declare_wide_fibonacci_benchmark_boundary(
            MetalExecutionIntent::PreferMetal,
            stwo_metal::WIDE_FIBONACCI_PROVE_LOG20_TARGET,
        )
        .unwrap();

        let lane = registered_wide_fibonacci_prove_values_lane(&boundary).unwrap();

        assert_eq!(lane.workload_name(), "fibonacci_example");
    }

    #[test]
    fn registered_lane_rejects_cpu_only_benchmark_plan() {
        let boundary = stwo_metal::declare_wide_fibonacci_benchmark_boundary(
            MetalExecutionIntent::ForceCpu,
            stwo_metal::WIDE_FIBONACCI_PROVE_LOG20_TARGET,
        )
        .unwrap();

        let error = registered_wide_fibonacci_prove_values_lane(&boundary).unwrap_err();

        assert_eq!(
            error,
            WideFibonacciProveValuesLaneError::PlanNotMetalCapable {
                workload_name: "fibonacci_example",
                plan: MetalExecutionPlan::CpuOnly,
            }
        );
    }
}
