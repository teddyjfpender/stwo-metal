use stwo::core::channel::Blake2sChannel;
use stwo::core::fields::qm31::{SecureField, SECURE_EXTENSION_DEGREE};
use stwo::core::pcs::utils::get_lifting_log_size;
use stwo::core::pcs::TreeVec;
use stwo::core::vcs_lifted::blake2_merkle::Blake2sMerkleChannel;
use stwo::prover::{CommitmentSchemeProver, ComponentProver, ComponentProvers};
use stwo_metal::{
    MetalBackend, MetalExecutionPlan, MetalWideFibonacciBenchmarkBoundary, MetalWorkloadOwnership,
    MetalWorkloadStage,
};

pub struct WideFibonacciProveValuesStaging {
    pub oods_point: stwo::core::circle::CirclePoint<SecureField>,
    pub max_log_degree_bound: u32,
    pub sample_points: TreeVec<Vec<Vec<stwo::core::circle::CirclePoint<SecureField>>>>,
}

fn assert_wide_fibonacci_prove_values_boundary(boundary: &MetalWideFibonacciBenchmarkBoundary) {
    let workload_boundary = boundary.workload_boundary();

    assert!(
        matches!(
            workload_boundary.plan(),
            MetalExecutionPlan::MetalFriHybrid | MetalExecutionPlan::MetalFull
        ),
        "wide-fibonacci prove-values staging requires a Metal-capable plan"
    );
    assert_eq!(
        workload_boundary.stage_ownership(MetalWorkloadStage::WitnessMain),
        Some(MetalWorkloadOwnership::CpuOwned),
        "wide-fibonacci benchmark witness staging must remain explicitly CPU-owned"
    );
    assert_eq!(
        workload_boundary.stage_ownership(MetalWorkloadStage::FriBlake2s),
        Some(MetalWorkloadOwnership::MetalNative),
        "wide-fibonacci prove-values staging requires the FRI/Blake2s stage on the Metal lane"
    );
}

pub fn stage_wide_fibonacci_prove_values(
    benchmark_boundary: &MetalWideFibonacciBenchmarkBoundary,
    components: &[&dyn ComponentProver<MetalBackend>],
    channel: &mut Blake2sChannel,
    commitment_scheme: &CommitmentSchemeProver<'_, MetalBackend, Blake2sMerkleChannel>,
) -> WideFibonacciProveValuesStaging {
    assert_wide_fibonacci_prove_values_boundary(benchmark_boundary);

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
        declare_wide_fibonacci_benchmark_boundary, MetalBenchmarkOperation,
        MetalBenchmarkReferencePlatform, MetalBenchmarkTarget, MetalExecutionIntent,
    };

    use super::assert_wide_fibonacci_prove_values_boundary;

    #[test]
    fn wide_fibonacci_boundary_satisfies_prove_values_staging_contract() {
        let target = MetalBenchmarkTarget {
            benchmark_id: "wide_fibonacci_prove_verify_contract_test_v1",
            workload_name: "fibonacci_example",
            family: "wide_fibonacci",
            operation: MetalBenchmarkOperation::ProveVerify,
            log_n_instances: 6,
            n_columns: 8,
            reference_platform: MetalBenchmarkReferencePlatform::Rtx4090Cuda,
            reference_elapsed_ms: 90.0,
        };
        let boundary =
            declare_wide_fibonacci_benchmark_boundary(MetalExecutionIntent::PreferMetal, target)
                .unwrap();

        assert_wide_fibonacci_prove_values_boundary(&boundary);
    }
}
