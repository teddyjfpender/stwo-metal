use ark_std::Zero;
use stwo::core::fields::qm31::SecureField;
use stwo::core::fri::FriConfig;
use stwo::core::poly::circle::CircleDomain;
use stwo::core::poly::line::LinePoly;
use stwo::core::queries::Queries;
use stwo::core::vcs_lifted::merkle_hasher::MerkleHasherLifted;
use stwo::prover::line::LineEvaluation;

use super::handoff::materialize_line_evaluation_via_cpu_bridge;
use super::sequence::MetalFriInnerLayerSequence;
use crate::stwo_metal::secure_field_vec::SecureFieldVec;

#[derive(Clone, Debug)]
pub struct MetalFriCommitmentSlice<H: MerkleHasherLifted> {
    inner_sequence: MetalFriInnerLayerSequence<H>,
    last_layer_poly: LinePoly,
}

impl<H: MerkleHasherLifted> MetalFriCommitmentSlice<H> {
    pub fn from_first_circle_fold(
        src: &SecureFieldVec,
        domain: CircleDomain,
        config: FriConfig,
        first_layer_alpha: SecureField,
        inner_layer_alphas: &[SecureField],
    ) -> Self {
        let inner_sequence = MetalFriInnerLayerSequence::<H>::from_first_circle_fold(
            src,
            domain,
            config,
            first_layer_alpha,
            inner_layer_alphas,
        );
        let last_layer_poly = interpolate_last_layer(config, inner_sequence.last_evaluation());
        Self {
            inner_sequence,
            last_layer_poly,
        }
    }

    pub fn inner_sequence(&self) -> &MetalFriInnerLayerSequence<H> {
        &self.inner_sequence
    }

    pub fn last_layer_poly(&self) -> &LinePoly {
        &self.last_layer_poly
    }

    pub fn decommit_on_queries(
        &self,
        queries: &Queries,
    ) -> Vec<stwo::core::fri::ExtendedFriLayerProof<H>> {
        self.inner_sequence.decommit_on_queries(queries)
    }
}

pub(super) fn interpolate_last_layer(
    config: FriConfig,
    last_evaluation: &super::line::MetalLineEvaluation,
) -> LinePoly {
    assert_eq!(
        last_evaluation.len(),
        config.last_layer_domain_size(),
        "last-layer interpolation requires the configured last-layer domain size"
    );

    let cpu_eval: LineEvaluation<stwo::prover::backend::CpuBackend> =
        materialize_line_evaluation_via_cpu_bridge(
            last_evaluation.domain(),
            last_evaluation.values(),
        );
    let mut coeffs = cpu_eval.interpolate().into_ordered_coefficients();

    let last_layer_degree_bound = 1 << config.log_last_layer_degree_bound;
    let zeros = coeffs.split_off(last_layer_degree_bound);
    assert!(
        zeros.iter().all(SecureField::is_zero),
        "invalid last-layer degree for the configured FRI slice"
    );

    LinePoly::from_ordered_coefficients(coeffs)
}
