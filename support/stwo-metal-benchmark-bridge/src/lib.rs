use stwo::core::channel::Blake2sChannel;
use stwo::core::fields::qm31::{SecureField, SECURE_EXTENSION_DEGREE};
use stwo::core::pcs::utils::get_lifting_log_size;
use stwo::core::pcs::TreeVec;
use stwo::core::vcs_lifted::blake2_merkle::Blake2sMerkleChannel;
use stwo::prover::{CommitmentSchemeProver, ComponentProver, ComponentProvers};
use stwo_metal::{
    MetalBackend, MetalExecutionAuthority, MetalExecutionPlan, MetalWorkloadOwnership,
    MetalWorkloadStage,
};

pub struct WideFibonacciProveValuesStaging {
    pub oods_point: stwo::core::circle::CirclePoint<SecureField>,
    pub max_log_degree_bound: u32,
    pub sample_points: TreeVec<Vec<Vec<stwo::core::circle::CirclePoint<SecureField>>>>,
}

fn assert_wide_fibonacci_prove_values_boundary(execution_authority: &MetalExecutionAuthority) {
    assert!(
        matches!(
            execution_authority.plan(),
            MetalExecutionPlan::MetalFriHybrid | MetalExecutionPlan::MetalFull
        ),
        "wide-fibonacci prove-values staging requires a Metal-capable plan"
    );
    assert_eq!(
        execution_authority.stage_ownership(MetalWorkloadStage::WitnessMain),
        Some(MetalWorkloadOwnership::CpuOwned),
        "wide-fibonacci benchmark witness staging must remain explicitly CPU-owned"
    );
    assert_eq!(
        execution_authority.stage_ownership(MetalWorkloadStage::FriBlake2s),
        Some(MetalWorkloadOwnership::MetalNative),
        "wide-fibonacci prove-values staging requires the FRI/Blake2s stage on the Metal lane"
    );
}

pub fn stage_wide_fibonacci_prove_values(
    execution_authority: &MetalExecutionAuthority,
    components: &[&dyn ComponentProver<MetalBackend>],
    channel: &mut Blake2sChannel,
    commitment_scheme: &CommitmentSchemeProver<'_, MetalBackend, Blake2sMerkleChannel>,
) -> WideFibonacciProveValuesStaging {
    assert_wide_fibonacci_prove_values_boundary(execution_authority);

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
    use stwo_metal::{declare_exemplar_metal_workload_boundary, MetalExecutionIntent};

    use super::assert_wide_fibonacci_prove_values_boundary;

    #[test]
    fn wide_fibonacci_workload_boundary_satisfies_prove_values_staging_contract() {
        let boundary = declare_exemplar_metal_workload_boundary(
            MetalExecutionIntent::PreferMetal,
            "fibonacci_example",
        )
        .unwrap();
        let execution_authority = boundary.execution_authority();

        assert_wide_fibonacci_prove_values_boundary(&execution_authority);
    }
}
