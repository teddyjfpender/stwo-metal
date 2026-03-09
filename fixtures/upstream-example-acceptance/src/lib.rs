//! Standalone acceptance harnesses for vendored upstream Stwo examples.

use stwo::core::air::Component;
use stwo::core::channel::Blake2sM31Channel;
use stwo::core::fields::m31::BaseField;
use stwo::core::pcs::{CommitmentSchemeVerifier, PcsConfig};
use stwo::core::poly::circle::CanonicCoset;
use stwo::core::proof::StarkProof;
use stwo::core::vcs_lifted::blake2_merkle::{Blake2sM31MerkleChannel, Blake2sM31MerkleHasher};
use stwo::core::verifier::{verify, VerificationError};
use stwo::prover::backend::CpuBackend;
use stwo::prover::poly::circle::{CircleEvaluation, PolyOps};
use stwo::prover::poly::BitReversedOrder;
use stwo::prover::{prove, CommitmentSchemeProver, ComponentProver, ProvingError};

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

/// Proves and verifies one single-trace upstream component through the stock
/// Blake2s CPU path.
///
/// Inputs:
/// - `trace`: one committed main trace already materialized as ordinary
///   `CpuBackend` circle evaluations.
/// - `component`: the unchanged upstream component whose trace layout matches
///   `trace`.
/// - `max_trace_log_size`: the maximum trace log size needed for twiddle
///   precomputation in this single-trace flow.
///
/// Outputs:
/// - a verified `StarkProof` if the provided trace and component agree.
///
/// Invariants:
/// - the workload logic remains upstream-owned; this helper only drives the
///   stock prover/verifier around an already materialized CPU trace.
/// - the helper commits an empty preprocessed tree and one main trace tree.
///
/// Failure modes:
/// - returns `SingleTraceCpuBridgeError::Prove` if proving fails.
/// - returns `SingleTraceCpuBridgeError::Verify` if the generated proof does
///   not verify.
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
    verify(&[component], verifier_channel, commitment_scheme, proof.clone())
        .map_err(SingleTraceCpuBridgeError::Verify)?;

    Ok(proof)
}
