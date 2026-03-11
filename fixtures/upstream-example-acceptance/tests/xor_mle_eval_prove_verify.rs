use std::array;

use stwo::core::air::{Component, Components};
use stwo::core::channel::Blake2sChannel;
use stwo::core::fields::qm31::SecureField;
use stwo::core::pcs::{CommitmentSchemeVerifier, PcsConfig};
use stwo::core::poly::circle::CanonicCoset;
use stwo::core::vcs_lifted::blake2_merkle::Blake2sMerkleChannel;
use stwo::core::verifier::verify;
use stwo::prover::backend::simd::SimdBackend;
use stwo::prover::backend::Column;
use stwo::prover::lookups::mle::Mle;
use stwo::prover::poly::circle::PolyOps;
use stwo::prover::{prove, CommitmentSchemeProver, ComponentProver};
use stwo_constraint_framework::TraceLocationAllocator;
use stwo_examples::xor::gkr_lookups::accumulation::MIN_LOG_BLOWUP_FACTOR;
use stwo_examples::xor::gkr_lookups::mle_eval::mle_coeff_column::{
    build_trace as build_mle_coeff_trace, MleCoeffColumnComponent, MleCoeffColumnEval,
};
use stwo_examples::xor::gkr_lookups::mle_eval::{
    build_trace as build_mle_eval_trace, MleEvalProverComponent, MleEvalVerifierComponent,
};
use stwo_metal::{
    declare_exemplar_metal_workload_boundary, metal_runtime_support, MetalBackend,
    MetalExecutionIntent, MetalRuntimeSupport,
};
use stwo_metal_upstream_example_acceptance::{
    acceptance_metal_lane, simd_tree_to_metal, AcceptanceMetalBridgeCatalog,
};

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn vendored_upstream_xor_mle_eval_example_proves_and_verifies_via_metal_backend() {
    assert_eq!(
        metal_runtime_support(),
        MetalRuntimeSupport::Available,
        "Metal runtime must be available for the native Apple Silicon acceptance test"
    );

    const N_VARIABLES: usize = 8;
    const COEFFS_COL_TRACE: usize = 1;
    const MLE_EVAL_TRACE: usize = 2;
    const LOG_EXPAND: u32 = 1;
    let boundary =
        declare_exemplar_metal_workload_boundary(MetalExecutionIntent::PreferMetal, "xor_example")
            .expect("xor workload boundary should be declared");
    let lane = acceptance_metal_lane(boundary.workload_name(), boundary.execution_authority())
        .expect("xor acceptance lane should stay Metal-capable");
    let catalog = AcceptanceMetalBridgeCatalog::new(lane);

    let log_size = N_VARIABLES as u32;
    let size = 1 << log_size;
    let mle_coeffs = (0..size)
        .map(|index| {
            SecureField::from_u32_unchecked(
                index as u32,
                (index as u32).wrapping_mul(3),
                (index as u32).wrapping_mul(5),
                (index as u32).wrapping_mul(7),
            )
        })
        .collect();
    let mle = Mle::<SimdBackend, SecureField>::new(mle_coeffs);
    let eval_point: [SecureField; N_VARIABLES] = array::from_fn(|index| {
        SecureField::from_u32_unchecked(
            (index as u32) + 11,
            (index as u32) + 13,
            (index as u32) + 17,
            (index as u32) + 19,
        )
    });
    let claim = {
        fn eval(mle_evals: &[SecureField], p: &[SecureField]) -> SecureField {
            match p {
                [] => mle_evals[0],
                &[p_i, ref p @ ..] => {
                    let (lhs, rhs) = mle_evals.split_at(mle_evals.len() / 2);
                    let lhs_eval = eval(lhs, p);
                    let rhs_eval = eval(rhs, p);
                    p_i * (rhs_eval - lhs_eval) + lhs_eval
                }
            }
        }

        eval(
            &mle.clone()
                .into_evals()
                .to_cpu()
                .into_iter()
                .map(|value| value.into())
                .collect::<Vec<_>>(),
            &eval_point,
        )
    };

    let simd_twiddles = SimdBackend::precompute_twiddles(
        CanonicCoset::new(log_size + LOG_EXPAND + MIN_LOG_BLOWUP_FACTOR)
            .circle_domain()
            .half_coset,
    );
    let metal_twiddles = MetalBackend::precompute_twiddles(
        CanonicCoset::new(log_size + LOG_EXPAND + MIN_LOG_BLOWUP_FACTOR)
            .circle_domain()
            .half_coset,
    );
    let config = PcsConfig::default();
    let prover_channel = &mut Blake2sChannel::default();
    let mut commitment_scheme =
        CommitmentSchemeProver::<MetalBackend, Blake2sMerkleChannel>::new(config, &metal_twiddles);
    commitment_scheme.set_store_polynomials_coefficients();

    let mut tree_builder = commitment_scheme.tree_builder();
    tree_builder.extend_evals(vec![]);
    tree_builder.commit(prover_channel);

    let mut tree_builder = commitment_scheme.tree_builder();
    tree_builder.extend_evals(simd_tree_to_metal(build_mle_coeff_trace(&mle)));
    tree_builder.commit(prover_channel);

    let mut tree_builder = commitment_scheme.tree_builder();
    tree_builder.extend_evals(simd_tree_to_metal(build_mle_eval_trace(
        &mle,
        &eval_point,
        claim,
    )));
    tree_builder.commit(prover_channel);

    let trace_location_allocator = &mut TraceLocationAllocator::default();
    let mle_coeffs_col_component = MleCoeffColumnComponent::new(
        trace_location_allocator,
        MleCoeffColumnEval::new(COEFFS_COL_TRACE, mle.n_variables()),
        SecureField::from_u32_unchecked(0, 0, 0, 0),
    );
    let mle_eval_component = MleEvalProverComponent::generate(
        trace_location_allocator,
        &mle_coeffs_col_component,
        &eval_point,
        mle,
        claim,
        &simd_twiddles,
        MLE_EVAL_TRACE,
    );

    let metal_coeffs_component = catalog.framework(&mle_coeffs_col_component);
    let metal_eval_component = catalog.simd(&mle_eval_component);
    let proving_components: [&dyn ComponentProver<MetalBackend>; 2] =
        [&metal_coeffs_component, &metal_eval_component];

    let proof = prove::<MetalBackend, Blake2sMerkleChannel>(
        &proving_components,
        prover_channel,
        commitment_scheme,
    )
    .expect("xor MLE eval should prove through MetalBackend");

    let trace_location_allocator = &mut TraceLocationAllocator::default();
    let verifier_coeffs_component = MleCoeffColumnComponent::new(
        trace_location_allocator,
        MleCoeffColumnEval::new(COEFFS_COL_TRACE, N_VARIABLES),
        SecureField::from_u32_unchecked(0, 0, 0, 0),
    );
    let verifier_eval_component = MleEvalVerifierComponent::new(
        trace_location_allocator,
        &verifier_coeffs_component,
        &eval_point,
        claim,
        MLE_EVAL_TRACE,
    );
    let components = Components {
        components: vec![
            &verifier_coeffs_component as &dyn Component,
            &verifier_eval_component,
        ],
        n_preprocessed_columns: 0,
    };

    let log_sizes = components.column_log_sizes();
    let verifier_channel = &mut Blake2sChannel::default();
    let commitment_scheme =
        &mut CommitmentSchemeVerifier::<Blake2sMerkleChannel>::new(proof.config);
    commitment_scheme.commit(proof.commitments[0], &[], verifier_channel);
    commitment_scheme.commit(proof.commitments[1], &log_sizes[1], verifier_channel);
    commitment_scheme.commit(proof.commitments[2], &log_sizes[2], verifier_channel);
    verify(
        &components.components,
        verifier_channel,
        commitment_scheme,
        proof,
    )
    .expect("xor MLE eval MetalBackend proof should verify");
}
