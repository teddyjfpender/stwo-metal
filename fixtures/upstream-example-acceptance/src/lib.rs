//! Standalone acceptance harnesses for vendored upstream Stwo examples.

use std::borrow::Cow;

use stwo::core::air::accumulation::PointEvaluationAccumulator;
use stwo::core::air::{Component, Components};
use stwo::core::channel::Blake2sM31Channel;
use stwo::core::circle::CirclePoint;
use stwo::core::fields::m31::BaseField;
use stwo::core::fields::qm31::SecureField;
use stwo::core::pcs::{CommitmentSchemeVerifier, PcsConfig, TreeVec};
use stwo::core::poly::circle::CanonicCoset;
use stwo::core::proof::StarkProof;
use stwo::core::utils::bit_reverse;
use stwo::core::vcs_lifted::blake2_merkle::{Blake2sM31MerkleChannel, Blake2sM31MerkleHasher};
use stwo::core::verifier::{verify, VerificationError};
use stwo::core::ColumnVec;
use stwo::prover::backend::{BackendForChannel, Column, CpuBackend};
use stwo::prover::poly::circle::{CircleEvaluation, PolyOps};
use stwo::prover::poly::BitReversedOrder;
use stwo::prover::secure_column::SecureColumnByCoords;
use stwo::prover::{
    prove, CommitmentSchemeProver, ComponentProver, DomainEvaluationAccumulator, ProvingError,
    Trace,
};
use stwo_constraint_framework::{
    CpuDomainEvaluator, FrameworkComponent, FrameworkEval, PREPROCESSED_TRACE_IDX,
};
use stwo_metal::{MetalBackend, MetalBaseFieldVec};

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

/// Local acceptance-only adapter for vendored upstream framework components.
///
/// Inputs:
/// - an unchanged vendored `FrameworkComponent<E>`
///
/// Outputs:
/// - a `ComponentProver<MetalBackend>` wrapper suitable for the generic proving API used in this
///   acceptance crate
///
/// Invariants:
/// - workload logic remains upstream-owned
/// - the quotient-evaluation boundary is still an explicit CPU-domain bridge localized to this
///   acceptance layer
///
/// Failure modes:
/// - inherits upstream panics for malformed trace layouts or unsupported evaluation-domain
///   assumptions
#[derive(Clone, Copy)]
pub struct AcceptanceMetalFrameworkComponent<'a, E: FrameworkEval> {
    inner: &'a FrameworkComponent<E>,
}

pub fn bridge_framework_component_to_metal<E: FrameworkEval>(
    component: &FrameworkComponent<E>,
) -> AcceptanceMetalFrameworkComponent<'_, E> {
    AcceptanceMetalFrameworkComponent { inner: component }
}

/// Proves and verifies one single-trace upstream component through the stock
/// Blake2s CPU path.
///
/// Inputs:
/// - `trace`: one committed main trace already materialized as ordinary `CpuBackend` circle
///   evaluations.
/// - `component`: the unchanged upstream component whose trace layout matches `trace`.
/// - `max_trace_log_size`: the maximum trace log size needed for twiddle precomputation in this
///   single-trace flow.
///
/// Outputs:
/// - a verified `StarkProof` if the provided trace and component agree.
///
/// Invariants:
/// - the workload logic remains upstream-owned; this helper only drives the stock prover/verifier
///   around an already materialized CPU trace.
/// - the helper commits an empty preprocessed tree and one main trace tree.
///
/// Failure modes:
/// - returns `SingleTraceCpuBridgeError::Prove` if proving fails.
/// - returns `SingleTraceCpuBridgeError::Verify` if the generated proof does not verify.
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
///
/// Inputs:
/// - `trace_trees`: commitment trees in protocol order, including the preprocessed tree if present
/// - `proving_components`: the backend-specific proving view of the components
/// - `components`: the verifier-facing component view for the same workload
/// - `max_trace_log_size`: the largest trace log size in the workload
/// - `store_polynomials_coefficients`: whether the commitment scheme should retain coefficients,
///   matching the upstream workload path
///
/// Outputs:
/// - a verified `StarkProof` if the component set proves and verifies
///
/// Invariants:
/// - workload semantics remain owned by the vendored upstream example
/// - backend substitution is the only intended delta
///
/// Failure modes:
/// - returns `ComponentSetBackendError::Prove` if proving fails
/// - returns `ComponentSetBackendError::Verify` if verification fails
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
    eval: CircleEvaluation<stwo::prover::backend::simd::SimdBackend, BaseField, BitReversedOrder>,
) -> CircleEvaluation<MetalBackend, BaseField, BitReversedOrder> {
    CircleEvaluation::new(
        eval.domain,
        MetalBaseFieldVec::from_vec(eval.values.to_cpu()),
    )
}

pub fn simd_tree_to_metal(
    tree: ColumnVec<
        CircleEvaluation<stwo::prover::backend::simd::SimdBackend, BaseField, BitReversedOrder>,
    >,
) -> ColumnVec<CircleEvaluation<MetalBackend, BaseField, BitReversedOrder>> {
    tree.into_iter().map(simd_evaluation_to_metal).collect()
}

impl<E: FrameworkEval> Component for AcceptanceMetalFrameworkComponent<'_, E> {
    fn n_constraints(&self) -> usize {
        self.inner.n_constraints()
    }

    fn max_constraint_log_degree_bound(&self) -> u32 {
        self.inner.max_constraint_log_degree_bound()
    }

    fn trace_log_degree_bounds(&self) -> TreeVec<ColumnVec<u32>> {
        self.inner.trace_log_degree_bounds()
    }

    fn mask_points(
        &self,
        point: CirclePoint<SecureField>,
        max_log_degree_bound: u32,
    ) -> TreeVec<ColumnVec<Vec<CirclePoint<SecureField>>>> {
        self.inner.mask_points(point, max_log_degree_bound)
    }

    fn preprocessed_column_indices(&self) -> ColumnVec<usize> {
        self.inner.preprocessed_column_indices().to_vec()
    }

    fn evaluate_constraint_quotients_at_point(
        &self,
        point: CirclePoint<SecureField>,
        mask: &TreeVec<ColumnVec<Vec<SecureField>>>,
        evaluation_accumulator: &mut PointEvaluationAccumulator,
        max_log_degree_bound: u32,
    ) {
        self.inner.evaluate_constraint_quotients_at_point(
            point,
            mask,
            evaluation_accumulator,
            max_log_degree_bound,
        );
    }
}

impl<E: FrameworkEval + Sync> ComponentProver<MetalBackend>
    for AcceptanceMetalFrameworkComponent<'_, E>
{
    fn evaluate_constraint_quotients_on_domain(
        &self,
        trace: &Trace<'_, MetalBackend>,
        evaluation_accumulator: &mut DomainEvaluationAccumulator<MetalBackend>,
    ) {
        let n_constraints = self.inner.n_constraints();
        if n_constraints == 0 {
            return;
        }

        if self.inner.is_disabled() {
            evaluation_accumulator.skip_coeffs(n_constraints);
            return;
        }

        let eval_domain =
            CanonicCoset::new(self.inner.max_constraint_log_degree_bound()).circle_domain();
        let trace_domain = CanonicCoset::new(self.inner.log_size());

        let mut component_polys = trace.polys.sub_tree(self.inner.trace_locations());
        component_polys[PREPROCESSED_TRACE_IDX] = self
            .inner
            .preprocessed_column_indices()
            .iter()
            .map(|idx| &trace.polys[PREPROCESSED_TRACE_IDX][*idx])
            .collect();

        let need_to_extend = component_polys
            .iter()
            .flatten()
            .any(|poly| poly.evals.domain.log_size() != eval_domain.log_size());
        let trace: TreeVec<
            Vec<Cow<'_, CircleEvaluation<CpuBackend, BaseField, BitReversedOrder>>>,
        > = if need_to_extend {
            let twiddles = MetalBackend::precompute_twiddles(eval_domain.half_coset);
            component_polys.as_cols_ref().map_cols(|poly| {
                Cow::Owned(metal_eval_to_cpu(
                    poly.get_evaluation_on_domain(eval_domain, &twiddles),
                ))
            })
        } else {
            component_polys.map_cols(|poly| Cow::Owned(metal_eval_to_cpu(poly.evals.clone())))
        };

        let log_expand = eval_domain.log_size() - trace_domain.log_size();
        let mut denom_inv = (0..1 << log_expand)
            .map(|index| {
                stwo::core::constraints::coset_vanishing(
                    trace_domain.coset(),
                    eval_domain.at(index),
                )
                .inverse()
            })
            .collect::<Vec<_>>();
        bit_reverse(&mut denom_inv);

        let [mut accum] = evaluation_accumulator.columns([(eval_domain.log_size(), n_constraints)]);
        accum.random_coeff_powers.reverse();

        let trace_cols = trace.as_cols_ref().map_cols(|eval| eval.as_ref());
        *accum.col = metal_secure_column_from_cpu(accumulate_pointwise_cpu(
            self.inner,
            trace_cols,
            eval_domain.log_size(),
            trace_domain.log_size(),
            denom_inv,
            &accum.random_coeff_powers,
            &accum.col.to_cpu(),
        ));
    }
}

fn metal_eval_to_cpu(
    eval: CircleEvaluation<MetalBackend, BaseField, BitReversedOrder>,
) -> CircleEvaluation<CpuBackend, BaseField, BitReversedOrder> {
    let coeffs_domain = eval.domain;
    CircleEvaluation::new(coeffs_domain, eval.values.to_cpu())
}

fn metal_secure_column_from_cpu(
    column: SecureColumnByCoords<CpuBackend>,
) -> SecureColumnByCoords<MetalBackend> {
    SecureColumnByCoords {
        columns: column.columns.map(MetalBaseFieldVec::from_vec),
    }
}

fn accumulate_pointwise_cpu<E: FrameworkEval>(
    component: &FrameworkComponent<E>,
    trace_cols: TreeVec<Vec<&CircleEvaluation<CpuBackend, BaseField, BitReversedOrder>>>,
    eval_log_size: u32,
    trace_log_size: u32,
    denom_inv: Vec<BaseField>,
    random_coeff_powers: &[SecureField],
    accum: &SecureColumnByCoords<CpuBackend>,
) -> SecureColumnByCoords<CpuBackend> {
    let mut result = SecureColumnByCoords::zeros(1 << eval_log_size);
    for row in 0..(1 << eval_log_size) {
        let eval = CpuDomainEvaluator::new(
            &trace_cols,
            row,
            random_coeff_powers,
            trace_log_size,
            eval_log_size,
            component.log_size(),
            component.claimed_sum(),
        );
        let row_res = component.evaluate(eval).row_res;
        let row_denom_inv = denom_inv[row >> trace_log_size];
        result.set(row, accum.at(row) + row_res * row_denom_inv);
    }
    result
}
