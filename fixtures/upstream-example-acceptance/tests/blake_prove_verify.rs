use stwo::core::channel::Blake2sChannel;
use stwo::core::pcs::PcsConfig;
use stwo::core::poly::circle::CanonicCoset;
use stwo::core::vcs_lifted::blake2_merkle::Blake2sMerkleChannel;
use stwo::prover::poly::circle::PolyOps;
use stwo::prover::{prove, CommitmentSchemeProver, ComponentProver};
use stwo_examples::blake::air::{build_blake_proving_setup, prove_blake, verify_blake};
use stwo_metal::{
    declare_exemplar_metal_workload_boundary, metal_runtime_support, MetalBackend,
    MetalExecutionIntent, MetalRuntimeSupport,
};
use stwo_metal_upstream_example_acceptance::{acceptance_bridge_catalog, simd_tree_to_metal};

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn vendored_upstream_blake_example_proves_and_verifies_via_metal_backend() {
    assert_eq!(
        metal_runtime_support(),
        MetalRuntimeSupport::Available,
        "Metal runtime must be available for the native Apple Silicon acceptance test"
    );

    let log_size = 6u32;
    let config = PcsConfig::default();
    let boundary = declare_exemplar_metal_workload_boundary(
        MetalExecutionIntent::PreferMetal,
        "blake_example",
    )
    .expect("blake workload boundary should be declared");
    let catalog = acceptance_bridge_catalog(&boundary)
        .expect("blake acceptance lane should stay Metal-capable");
    let setup = build_blake_proving_setup::<Blake2sMerkleChannel>(log_size, config);

    let twiddles = MetalBackend::precompute_twiddles(
        CanonicCoset::new(setup.max_trace_log_size() + 1 + config.fri_config.log_blowup_factor)
            .circle_domain()
            .half_coset,
    );
    let prover_channel = &mut Blake2sChannel::default();
    let mut commitment_scheme =
        CommitmentSchemeProver::<MetalBackend, Blake2sMerkleChannel>::new(config, &twiddles);
    commitment_scheme.set_store_polynomials_coefficients();

    let mut tree_builder = commitment_scheme.tree_builder();
    tree_builder.extend_evals(simd_tree_to_metal(setup.preprocessed_trace().to_vec()));
    tree_builder.commit(prover_channel);

    setup.mix_stmt0(prover_channel);
    let mut tree_builder = commitment_scheme.tree_builder();
    tree_builder.extend_evals(simd_tree_to_metal(setup.trace().to_vec()));
    tree_builder.commit(prover_channel);

    setup.replay_interaction_element_draw(prover_channel);
    setup.mix_stmt1(prover_channel);
    let mut tree_builder = commitment_scheme.tree_builder();
    tree_builder.extend_evals(simd_tree_to_metal(setup.interaction_trace().to_vec()));
    tree_builder.commit(prover_channel);

    let scheduler_component = catalog.framework(setup.scheduler_component());
    let round_components = setup
        .round_components()
        .iter()
        .map(|component| catalog.framework(component))
        .collect::<Vec<_>>();
    let xor12_component = catalog.framework(setup.xor12_component());
    let xor9_component = catalog.framework(setup.xor9_component());
    let xor8_component = catalog.framework(setup.xor8_component());
    let xor7_component = catalog.framework(setup.xor7_component());
    let xor4_component = catalog.framework(setup.xor4_component());

    let mut proving_component_refs =
        vec![&scheduler_component as &dyn ComponentProver<MetalBackend>];
    proving_component_refs.extend(
        round_components
            .iter()
            .map(|component| component as &dyn ComponentProver<MetalBackend>),
    );
    proving_component_refs.extend([
        &xor12_component as &dyn ComponentProver<MetalBackend>,
        &xor9_component as &dyn ComponentProver<MetalBackend>,
        &xor8_component as &dyn ComponentProver<MetalBackend>,
        &xor7_component as &dyn ComponentProver<MetalBackend>,
        &xor4_component as &dyn ComponentProver<MetalBackend>,
    ]);

    let metal_proof = prove::<MetalBackend, Blake2sMerkleChannel>(
        &proving_component_refs,
        prover_channel,
        commitment_scheme,
    )
    .expect("blake should prove through MetalBackend");

    verify_blake::<Blake2sMerkleChannel>(setup.into_proof(metal_proof.clone()))
        .expect("blake MetalBackend proof should verify through the stock verifier");

    let cpu_proof = prove_blake::<Blake2sMerkleChannel>(log_size, config);
    let cpu_commitments = cpu_proof.stark_proof().commitments.0.clone();
    verify_blake::<Blake2sMerkleChannel>(cpu_proof)
        .expect("upstream CPU blake proof should verify");

    assert_eq!(
        metal_proof.commitments.0, cpu_commitments,
        "the MetalBackend blake row should preserve the upstream proof commitments"
    );
}
