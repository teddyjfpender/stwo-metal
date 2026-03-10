use stwo::core::channel::Blake2sChannel;
use stwo::core::fields::m31::M31;
use stwo::core::pcs::PcsConfig;
use stwo::core::poly::circle::CanonicCoset;
use stwo::core::vcs_lifted::blake2_merkle::Blake2sMerkleChannel;
use stwo::prover::poly::circle::PolyOps;
use stwo::prover::{prove, CommitmentSchemeProver};
use stwo_constraint_framework::TraceLocationAllocator;
use stwo_examples::state_machine::components::{
    State, StateMachineComponents, StateMachineOp0Component, StateMachineOp1Component,
    StateTransitionEval,
};
use stwo_examples::state_machine::gen::{gen_interaction_trace, gen_trace};
use stwo_examples::state_machine::{prove_state_machine, verify_state_machine};
use stwo_metal::{
    declare_exemplar_metal_workload_boundary, metal_runtime_support, MetalBackend,
    MetalExecutionIntent, MetalRuntimeSupport,
};
use stwo_metal_upstream_example_acceptance::{
    acceptance_bridge_catalog, simd_tree_to_metal,
};

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn vendored_upstream_state_machine_proves_and_verifies_via_metal_backend() {
    assert_eq!(
        metal_runtime_support(),
        MetalRuntimeSupport::Available,
        "Metal runtime must be available for the native Apple Silicon acceptance test"
    );

    let log_n_rows = 8u32;
    let config = PcsConfig::default();
    let boundary = declare_exemplar_metal_workload_boundary(
        MetalExecutionIntent::PreferMetal,
        "state_machine_example",
    )
    .expect("state machine workload boundary should be declared");
    let catalog = acceptance_bridge_catalog(&boundary)
        .expect("state machine acceptance lane should stay Metal-capable");
    let initial_state: State = [M31::from_u32_unchecked(0), M31::from_u32_unchecked(0)];
    let mut intermediate_state = initial_state;
    intermediate_state[0] += M31::from_u32_unchecked(1 << log_n_rows);

    let metal_twiddles = MetalBackend::precompute_twiddles(
        CanonicCoset::new(log_n_rows + config.fri_config.log_blowup_factor + 1)
            .circle_domain()
            .half_coset,
    );
    let prover_channel = &mut Blake2sChannel::default();
    config.mix_into(prover_channel);
    let mut commitment_scheme =
        CommitmentSchemeProver::<MetalBackend, Blake2sMerkleChannel>::new(config, &metal_twiddles);
    commitment_scheme.set_store_polynomials_coefficients();

    let trace_op0 = gen_trace(log_n_rows, initial_state, 0);
    let trace_op1 = gen_trace(log_n_rows - 1, intermediate_state, 1);

    let tree_builder = commitment_scheme.tree_builder();
    tree_builder.commit(prover_channel);

    let stmt0 = stwo_examples::state_machine::components::StateMachineStatement0 {
        n: log_n_rows,
        m: log_n_rows - 1,
    };
    stmt0.mix_into(prover_channel);

    let mut tree_builder = commitment_scheme.tree_builder();
    tree_builder.extend_evals(
        [
            simd_tree_to_metal(trace_op0.clone()),
            simd_tree_to_metal(trace_op1.clone()),
        ]
        .into_iter()
        .flatten()
        .collect(),
    );
    tree_builder.commit(prover_channel);

    let lookup_elements =
        stwo_examples::state_machine::components::StateMachineElements::draw(prover_channel);
    let (interaction_trace_op0, claimed_sum_op0) =
        gen_interaction_trace(&trace_op0, 0, &lookup_elements);
    let (interaction_trace_op1, claimed_sum_op1) =
        gen_interaction_trace(&trace_op1, 1, &lookup_elements);

    let stmt1 = stwo_examples::state_machine::components::StateMachineStatement1 {
        x_axis_claimed_sum: claimed_sum_op0,
        y_axis_claimed_sum: claimed_sum_op1,
    };
    stmt1.mix_into(prover_channel);

    let mut tree_builder = commitment_scheme.tree_builder();
    tree_builder.extend_evals(
        [
            simd_tree_to_metal(interaction_trace_op0.clone()),
            simd_tree_to_metal(interaction_trace_op1.clone()),
        ]
        .into_iter()
        .flatten()
        .collect(),
    );
    tree_builder.commit(prover_channel);

    let mut allocator = TraceLocationAllocator::default();
    let component0 = StateMachineOp0Component::new(
        &mut allocator,
        StateTransitionEval {
            log_n_rows,
            lookup_elements: lookup_elements.clone(),
            claimed_sum: claimed_sum_op0,
        },
        claimed_sum_op0,
    );
    let component1 = StateMachineOp1Component::new(
        &mut allocator,
        StateTransitionEval {
            log_n_rows: log_n_rows - 1,
            lookup_elements,
            claimed_sum: claimed_sum_op1,
        },
        claimed_sum_op1,
    );

    let proving_component0 = catalog.framework(&component0);
    let proving_component1 = catalog.framework(&component1);
    let metal_proof = prove::<MetalBackend, Blake2sMerkleChannel>(
        &[&proving_component0, &proving_component1],
        prover_channel,
        commitment_scheme,
    )
    .expect("state machine should prove through MetalBackend");

    let components = StateMachineComponents {
        component0,
        component1,
    };
    let proof = stwo_examples::state_machine::components::StateMachineProof {
        public_input: [
            initial_state,
            [
                M31::from_u32_unchecked(1 << log_n_rows),
                M31::from_u32_unchecked(1 << (log_n_rows - 1)),
            ],
        ],
        stmt0,
        stmt1,
        stark_proof: metal_proof.clone(),
    };
    verify_state_machine(&mut Blake2sChannel::default(), components, proof)
        .expect("state machine MetalBackend proof should verify through the stock verifier");

    let (cpu_components, cpu_proof, _) = prove_state_machine(
        log_n_rows,
        initial_state,
        config,
        &mut Blake2sChannel::default(),
        false,
    );
    let cpu_commitments = cpu_proof.stark_proof.commitments.0.clone();
    verify_state_machine(&mut Blake2sChannel::default(), cpu_components, cpu_proof)
        .expect("upstream CPU state machine proof should verify");

    assert_eq!(
        metal_proof.commitments.0, cpu_commitments,
        "the MetalBackend state-machine row should preserve the upstream proof commitments"
    );
}
