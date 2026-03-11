use stwo::core::channel::Blake2sChannel;
use stwo::core::fields::qm31::{SecureField, SECURE_EXTENSION_DEGREE};
use stwo::core::pcs::utils::get_lifting_log_size;
use stwo::core::pcs::TreeVec;
use stwo::core::vcs_lifted::blake2_merkle::Blake2sMerkleChannel;
use stwo::prover::{CommitmentSchemeProver, ComponentProver, ComponentProvers};
use stwo_metal::{
    MetalBackend, MetalExecutionPlan, MetalWideFibonacciBenchmarkBoundary, MetalWorkloadBoundary,
    MetalWorkloadOwnership, MetalWorkloadStage,
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

fn wide_fibonacci_prove_values_lane(
    boundary: &MetalWorkloadBoundary,
) -> Result<WideFibonacciProveValuesLane, WideFibonacciProveValuesLaneError> {
    let workload_name = boundary.workload_name();
    if !matches!(
        boundary.plan(),
        MetalExecutionPlan::MetalFriHybrid | MetalExecutionPlan::MetalFull
    ) {
        return Err(WideFibonacciProveValuesLaneError::PlanNotMetalCapable {
            workload_name,
            plan: boundary.plan(),
        });
    }
    if boundary.stage_ownership(MetalWorkloadStage::WitnessMain)
        != Some(MetalWorkloadOwnership::CpuOwned)
    {
        return Err(
            WideFibonacciProveValuesLaneError::UnsupportedWitnessOwnership {
                workload_name,
                stage: MetalWorkloadStage::WitnessMain,
            },
        );
    }
    if boundary.stage_ownership(MetalWorkloadStage::FriBlake2s)
        != Some(MetalWorkloadOwnership::MetalNative)
    {
        return Err(WideFibonacciProveValuesLaneError::UnsupportedFriOwnership {
            workload_name,
            stage: MetalWorkloadStage::FriBlake2s,
        });
    }

    Ok(WideFibonacciProveValuesLane { workload_name })
}

pub fn registered_wide_fibonacci_prove_values_lane(
    benchmark_boundary: &MetalWideFibonacciBenchmarkBoundary,
) -> Result<WideFibonacciProveValuesLane, WideFibonacciProveValuesLaneError> {
    wide_fibonacci_prove_values_lane(benchmark_boundary.workload_boundary())
}

pub fn stage_wide_fibonacci_prove_values(
    lane: &WideFibonacciProveValuesLane,
    components: &[&dyn ComponentProver<MetalBackend>],
    channel: &mut Blake2sChannel,
    commitment_scheme: &CommitmentSchemeProver<'_, MetalBackend, Blake2sMerkleChannel>,
) -> WideFibonacciProveValuesStaging {
    let _ = lane.workload_name();

    let component_provers = ComponentProvers {
        components: components.to_vec(),
        n_preprocessed_columns: commitment_scheme.trees
            [stwo::core::verifier::PREPROCESSED_TRACE_IDX]
            .polynomials
            .len(),
    };
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
    use stwo_metal::{
        declare_exemplar_metal_workload_boundary, MetalExecutionIntent, MetalExecutionPlan,
    };

    use super::{
        registered_wide_fibonacci_prove_values_lane, wide_fibonacci_prove_values_lane,
        WideFibonacciProveValuesLaneError,
    };

    #[test]
    fn wide_fibonacci_workload_boundary_satisfies_prove_values_staging_contract() {
        let boundary = declare_exemplar_metal_workload_boundary(
            MetalExecutionIntent::PreferMetal,
            "fibonacci_example",
        )
        .unwrap();
        let lane = wide_fibonacci_prove_values_lane(&boundary).unwrap();

        assert_eq!(lane.workload_name(), "fibonacci_example");
    }

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
    fn wide_fibonacci_lane_rejects_cpu_only_authority() {
        let boundary = declare_exemplar_metal_workload_boundary(
            MetalExecutionIntent::ForceCpu,
            "fibonacci_example",
        )
        .unwrap();

        let error = wide_fibonacci_prove_values_lane(&boundary).unwrap_err();

        assert_eq!(
            error,
            WideFibonacciProveValuesLaneError::PlanNotMetalCapable {
                workload_name: "fibonacci_example",
                plan: MetalExecutionPlan::CpuOnly,
            }
        );
    }
}
