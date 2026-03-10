//! Standalone acceptance harnesses for vendored upstream Stwo examples.

use stwo::core::air::{Component, Components};
use stwo::core::channel::Blake2sM31Channel;
use stwo::core::fields::m31::BaseField;
use stwo::core::pcs::{CommitmentSchemeVerifier, PcsConfig, TreeVec};
use stwo::core::poly::circle::CanonicCoset;
use stwo::core::proof::StarkProof;
use stwo::core::vcs_lifted::blake2_merkle::{Blake2sM31MerkleChannel, Blake2sM31MerkleHasher};
use stwo::core::verifier::{verify, VerificationError};
use stwo::core::ColumnVec;
use stwo::prover::backend::simd::SimdBackend;
use stwo::prover::backend::{BackendForChannel, Column, CpuBackend};
use stwo::prover::poly::circle::{CircleEvaluation, PolyOps};
use stwo::prover::poly::BitReversedOrder;
use stwo::prover::{prove, CommitmentSchemeProver, ComponentProver, ProvingError};
use stwo_metal::{MetalBackend, MetalBaseFieldVec};
pub use stwo_metal_upstream_bridge::{
    acceptance_bridge_catalog, acceptance_registered_metal_lane, AcceptanceMetalBridgeCatalog,
    AcceptanceMetalFrameworkComponent, AcceptanceMetalLane, AcceptanceMetalLaneError,
    AcceptanceMetalSimdComponent,
};

#[derive(Debug)]
pub enum SingleTraceCpuBridgeError {
    Prove(ProvingError),
    Verify(VerificationError),
}

impl core::fmt::Display for SingleTraceCpuBridgeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Prove(error) => write!(f, "CPU bridge prove step failed: {error}"),
            Self::Verify(error) => write!(f, "CPU bridge verify step failed: {error}"),
        }
    }
}

impl std::error::Error for SingleTraceCpuBridgeError {}

#[derive(Debug)]
pub enum SingleTraceBackendError {
    Prove(ProvingError),
    Verify(VerificationError),
}

impl core::fmt::Display for SingleTraceBackendError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Prove(error) => write!(f, "backend prove step failed: {error}"),
            Self::Verify(error) => write!(f, "backend verify step failed: {error}"),
        }
    }
}

impl std::error::Error for SingleTraceBackendError {}

#[derive(Debug)]
pub enum ComponentSetBackendError {
    Prove(ProvingError),
    Verify(VerificationError),
}

impl core::fmt::Display for ComponentSetBackendError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Prove(error) => write!(f, "component-set backend prove step failed: {error}"),
            Self::Verify(error) => write!(f, "component-set backend verify step failed: {error}"),
        }
    }
}

impl std::error::Error for ComponentSetBackendError {}

/// Proves and verifies one single-trace upstream component through the stock
/// Blake2s CPU path.
pub fn prove_and_verify_single_trace_component_via_cpu_blake2s<C>(
    trace: Vec<CircleEvaluation<CpuBackend, BaseField, BitReversedOrder>>,
    component: &C,
    max_trace_log_size: u32,
) -> Result<StarkProof<Blake2sM31MerkleHasher>, SingleTraceCpuBridgeError>
where
    C: ComponentProver<CpuBackend> + Component,
{
    let config = PcsConfig::default();
    let twiddles = CpuBackend::precompute_twiddles(
        CanonicCoset::new(max_trace_log_size + 1 + config.fri_config.log_blowup_factor)
            .circle_domain()
            .half_coset,
    );

    let prover_channel = &mut Blake2sM31Channel::default();
    let mut commitment_scheme =
        CommitmentSchemeProver::<CpuBackend, Blake2sM31MerkleChannel>::new(config, &twiddles);

    let mut tree_builder = commitment_scheme.tree_builder();
    tree_builder.extend_evals(vec![]);
    tree_builder.commit(prover_channel);

    let mut tree_builder = commitment_scheme.tree_builder();
    tree_builder.extend_evals(trace);
    tree_builder.commit(prover_channel);

    let proof = prove::<CpuBackend, Blake2sM31MerkleChannel>(
        &[component],
        prover_channel,
        commitment_scheme,
    )
    .map_err(SingleTraceCpuBridgeError::Prove)?;

    let verifier_channel = &mut Blake2sM31Channel::default();
    let commitment_scheme = &mut CommitmentSchemeVerifier::<Blake2sM31MerkleChannel>::new(config);
    let sizes = component.trace_log_degree_bounds();
    commitment_scheme.commit(proof.commitments[0], &sizes[0], verifier_channel);
    commitment_scheme.commit(proof.commitments[1], &sizes[1], verifier_channel);
    verify(
        &[component],
        verifier_channel,
        commitment_scheme,
        proof.clone(),
    )
    .map_err(SingleTraceCpuBridgeError::Verify)?;

    Ok(proof)
}

/// Proves and verifies one single-trace upstream component through the provided
/// backend and the stock Blake2s verifier path.
pub fn prove_and_verify_single_trace_component_via_backend_blake2s<B, C>(
    trace: Vec<CircleEvaluation<B, BaseField, BitReversedOrder>>,
    component: &C,
    max_trace_log_size: u32,
) -> Result<StarkProof<Blake2sM31MerkleHasher>, SingleTraceBackendError>
where
    B: BackendForChannel<Blake2sM31MerkleChannel>,
    C: ComponentProver<B> + Component,
{
    let config = PcsConfig::default();
    let twiddles = B::precompute_twiddles(
        CanonicCoset::new(max_trace_log_size + 1 + config.fri_config.log_blowup_factor)
            .circle_domain()
            .half_coset,
    );

    let prover_channel = &mut Blake2sM31Channel::default();
    let mut commitment_scheme =
        CommitmentSchemeProver::<B, Blake2sM31MerkleChannel>::new(config, &twiddles);

    let mut tree_builder = commitment_scheme.tree_builder();
    tree_builder.extend_evals(vec![]);
    tree_builder.commit(prover_channel);

    let mut tree_builder = commitment_scheme.tree_builder();
    tree_builder.extend_evals(trace);
    tree_builder.commit(prover_channel);

    let proof =
        prove::<B, Blake2sM31MerkleChannel>(&[component], prover_channel, commitment_scheme)
            .map_err(SingleTraceBackendError::Prove)?;

    let verifier_channel = &mut Blake2sM31Channel::default();
    let commitment_scheme = &mut CommitmentSchemeVerifier::<Blake2sM31MerkleChannel>::new(config);
    let sizes = component.trace_log_degree_bounds();
    commitment_scheme.commit(proof.commitments[0], &sizes[0], verifier_channel);
    commitment_scheme.commit(proof.commitments[1], &sizes[1], verifier_channel);
    verify(
        &[component],
        verifier_channel,
        commitment_scheme,
        proof.clone(),
    )
    .map_err(SingleTraceBackendError::Verify)?;

    Ok(proof)
}

/// Proves and verifies a full multi-tree component set through the provided
/// backend and the stock Blake2s verifier path.
pub fn prove_and_verify_component_set_via_backend_blake2s<B>(
    trace_trees: TreeVec<ColumnVec<CircleEvaluation<B, BaseField, BitReversedOrder>>>,
    proving_components: &[&dyn ComponentProver<B>],
    components: &[&dyn Component],
    max_trace_log_size: u32,
    store_polynomials_coefficients: bool,
) -> Result<StarkProof<Blake2sM31MerkleHasher>, ComponentSetBackendError>
where
    B: BackendForChannel<Blake2sM31MerkleChannel>,
{
    let config = PcsConfig::default();
    let n_preprocessed_columns = trace_trees
        .first()
        .map(std::vec::Vec::len)
        .unwrap_or_default();
    let twiddles = B::precompute_twiddles(
        CanonicCoset::new(max_trace_log_size + 1 + config.fri_config.log_blowup_factor)
            .circle_domain()
            .half_coset,
    );

    let prover_channel = &mut Blake2sM31Channel::default();
    let mut commitment_scheme =
        CommitmentSchemeProver::<B, Blake2sM31MerkleChannel>::new(config, &twiddles);
    if store_polynomials_coefficients {
        commitment_scheme.set_store_polynomials_coefficients();
    }

    for tree in trace_trees.0 {
        let mut tree_builder = commitment_scheme.tree_builder();
        tree_builder.extend_evals(tree);
        tree_builder.commit(prover_channel);
    }

    let proof =
        prove::<B, Blake2sM31MerkleChannel>(proving_components, prover_channel, commitment_scheme)
            .map_err(ComponentSetBackendError::Prove)?;

    let verifier_channel = &mut Blake2sM31Channel::default();
    let commitment_scheme = &mut CommitmentSchemeVerifier::<Blake2sM31MerkleChannel>::new(config);
    let sizes = Components {
        components: components.to_vec(),
        n_preprocessed_columns,
    }
    .column_log_sizes();
    for (commitment, size) in proof.commitments.iter().zip(sizes.iter()) {
        commitment_scheme.commit(*commitment, size, verifier_channel);
    }
    verify(
        components,
        verifier_channel,
        commitment_scheme,
        proof.clone(),
    )
    .map_err(ComponentSetBackendError::Verify)?;

    Ok(proof)
}

pub fn simd_evaluation_to_metal(
    eval: CircleEvaluation<SimdBackend, BaseField, BitReversedOrder>,
) -> CircleEvaluation<MetalBackend, BaseField, BitReversedOrder> {
    CircleEvaluation::new(
        eval.domain,
        MetalBaseFieldVec::from_vec(eval.values.to_cpu()),
    )
}

pub fn simd_tree_to_metal(
    tree: ColumnVec<CircleEvaluation<SimdBackend, BaseField, BitReversedOrder>>,
) -> ColumnVec<CircleEvaluation<MetalBackend, BaseField, BitReversedOrder>> {
    tree.into_iter().map(simd_evaluation_to_metal).collect()
}
