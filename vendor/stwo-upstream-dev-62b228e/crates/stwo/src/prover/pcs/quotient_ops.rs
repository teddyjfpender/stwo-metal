use std::collections::BTreeMap;
use std::time::Instant;

use itertools::Itertools;
use num_traits::One;
use tracing::{span, Level};

use crate::core::circle::CirclePoint;
use crate::core::fields::m31::BaseField;
use crate::core::fields::qm31::SecureField;
use crate::core::pcs::quotients::{
    ColumnSampleBatch, PointSample,
};
use crate::core::pcs::TreeVec;
use crate::core::poly::circle::CanonicCoset;
use crate::prover::backend::ColumnOps;
use crate::prover::poly::circle::{CircleEvaluation, PolyOps, SecureEvaluation};
use crate::prover::poly::BitReversedOrder;
use crate::prover::secure_column::SecureColumnByCoords;
use crate::prover::AccumulationOps;

pub trait QuotientOps: PolyOps {
    /// Receives a non-empty set of columns of the *same* size, and populates the vector
    /// `accumulated_numerators_vec` with their FRI numerators accumulations, across
    /// `sample_batches`.
    ///
    /// For each sample batch in sample_batches, accumulates the numerators of the columns involved
    /// in this batch and pushes an `AccumulatedNumerators` object into
    /// `accumulated_numerators_vec`.
    fn accumulate_numerators(
        columns: &[&CircleEvaluation<Self, BaseField, BitReversedOrder>],
        sample_batches: &[ColumnSampleBatch],
        accumulated_numerators_vec: &mut Vec<AccumulatedNumerators<Self>>,
    );

    /// Given a vector of `AccumulatedNumerators`, the function iterates over the points of the
    /// largest domain of the accumulated numerators, and:
    /// * for each sample point, computes the denominator of the quotient for that (domain point,
    ///   sample point).
    /// * multiplies it by the accumulated numerator for that domain point and sample point.
    /// * sums across sample points.
    fn compute_quotients_and_combine(
        accs: Vec<AccumulatedNumerators<Self>>,
        lifting_log_size: u32,
    ) -> SecureEvaluation<Self, BitReversedOrder>;
}

/// Helper struct that keeps track of the accumulation of the numerators involved in the FRI
/// quotients.
pub struct AccumulatedNumerators<B: ColumnOps<BaseField>> {
    /// One of the sample points received by the pcs.
    pub sample_point: CirclePoint<SecureField>,
    /// Stores a circle evaluation of the form:
    ///     p -> ∑ α^{k_i} * (cᵢ * f̃ᵢ(p) - bᵢ)
    /// where
    /// * p ∈ canonic coset of log size = l (where l is the log size of the column).
    /// * i runs over some column indices of the trace.
    /// * α is the random coefficient for the accumulation.
    /// * k_i is the randomness exponent for column i and sample point `sample_point`.
    /// * f̃ᵢ is the lift of the trace poly fᵢ to log size l.
    /// * (bᵢ, cᵢ) are the `b` and `c` line coefficients for column i and sample point
    ///   `sample_point`.
    pub partial_numerators_acc: SecureColumnByCoords<B>,
    /// Stores an accumulation of the form
    ///      ∑ α^{k_i} * aᵢ
    /// where
    /// * i runs over some column indices of the trace.
    /// * α and k_i are as in the previous docstring for `partial_numerators_acc`.
    /// * aᵢ is the `a` line coefficient for column i and sample point `sample_point`.
    ///
    /// The index set of the summation is equal to the index set of the summation referred in the
    /// previous docstring for `partial_numerators_acc`.
    pub first_linear_term_acc: SecureField,
}

struct GroupedColumnSamples<'a, B: QuotientOps> {
    columns: Vec<&'a CircleEvaluation<B, BaseField, BitReversedOrder>>,
    samples_with_randomness: Vec<Vec<(PointSample, SecureField)>>,
}

fn group_columns_and_samples_by_log_size<'a, B: QuotientOps>(
    columns: &'a TreeVec<Vec<&'a CircleEvaluation<B, BaseField, BitReversedOrder>>>,
    samples: &'a TreeVec<Vec<Vec<PointSample>>>,
    lifting_log_size: u32,
    random_coeff: SecureField,
) -> BTreeMap<u32, GroupedColumnSamples<'a, B>> {
    let mut random_pows = (0..).scan(SecureField::one(), |acc, _| {
        let curr = *acc;
        *acc *= random_coeff;
        Some(curr)
    });
    let lifting_domain_generator = CanonicCoset::new(lifting_log_size).step();
    let mut grouped_by_log_size = BTreeMap::<u32, GroupedColumnSamples<'a, B>>::new();

    for (columns_per_tree, samples_per_tree) in columns.iter().zip(samples.iter()) {
        for (column, samples_per_col) in columns_per_tree.iter().zip(samples_per_tree.iter()) {
            let log_size = column.domain.log_size();
            let group = grouped_by_log_size
                .entry(log_size)
                .or_insert_with(|| GroupedColumnSamples {
                    columns: Vec::new(),
                    samples_with_randomness: Vec::new(),
                });
            group.columns.push(*column);

            if samples_per_col.is_empty() {
                group.samples_with_randomness.push(Vec::new());
                continue;
            }

            let mut new_samples = Vec::with_capacity(samples_per_col.len() + 1);
            // Keep the same periodicity and random-coefficient order as the original
            // tree-shaped staging helper.
            if let [_prev_point_sample, point_sample] = &samples_per_col[..] {
                let period_generator = lifting_domain_generator.repeated_double(log_size);
                new_samples.push((
                    PointSample {
                        point: point_sample.point + period_generator.into_ef(),
                        value: point_sample.value,
                    },
                    random_pows.next().unwrap(),
                ));
            }
            for sample in samples_per_col.iter() {
                new_samples.push((sample.clone(), random_pows.next().unwrap()));
            }
            group.samples_with_randomness.push(new_samples);
        }
    }

    grouped_by_log_size
}

pub fn compute_fri_quotients<B: QuotientOps + AccumulationOps>(
    columns: &TreeVec<Vec<&CircleEvaluation<B, BaseField, BitReversedOrder>>>,
    samples: &TreeVec<Vec<Vec<PointSample>>>,
    random_coeff: SecureField,
    lifting_log_size: u32,
    _log_blowup_factor: u32,
) -> SecureEvaluation<B, BitReversedOrder> {
    let _span = span!(Level::INFO, "Compute FRI quotients", class = "FRIQuotients").entered();
    let profile_quotients = std::env::var_os("STWO_METAL_PROFILE_QUOTIENTS").is_some();
    let emit_timing = |phase: &str, elapsed_ms: f64| {
        if profile_quotients {
            eprintln!("quotients_timing phase={phase} ms={elapsed_ms}");
        }
    };
    let mut accumulated_numerators_vec: Vec<AccumulatedNumerators<B>> = vec![];
    let samples_with_randomness_start = Instant::now();
    let grouped_by_log_size = group_columns_and_samples_by_log_size(
        columns,
        samples,
        lifting_log_size,
        random_coeff,
    );
    emit_timing(
        "samples_with_randomness",
        samples_with_randomness_start.elapsed().as_secs_f64() * 1000.0,
    );

    // Populate `accumulated_numerators_vec`, per (log_size, sample_point). After this iteration,
    // `accumulated_numerators_vec` will have length equal to
    //
    //   ∑_k (# of distinct sample points per log size k).
    //
    let accumulate_numerators_start = Instant::now();
    for (_, grouped) in grouped_by_log_size {
        let sample_refs = grouped.samples_with_randomness.iter().collect_vec();
        let sample_batches = ColumnSampleBatch::new_vec(&sample_refs);
        B::accumulate_numerators(
            &grouped.columns,
            &sample_batches,
            &mut accumulated_numerators_vec,
        );
    }
    emit_timing(
        "accumulate_numerators",
        accumulate_numerators_start.elapsed().as_secs_f64() * 1000.0,
    );

    // Group and accumulate the numerators per sample point: the accumulations (of different
    // lengths) get lifted and accumulated to a single vector. After this step, there is a single
    // accumulation per sample point.
    let lift_and_accumulate_start = Instant::now();
    let mut accumulations_by_sample_point: BTreeMap<
        (SecureField, SecureField),
        (CirclePoint<SecureField>, Vec<AccumulatedNumerators<B>>),
    > = BTreeMap::new();
    for accumulation in accumulated_numerators_vec {
        let sample_point = accumulation.sample_point;
        accumulations_by_sample_point
            .entry((sample_point.x, sample_point.y))
            .or_insert_with(|| (sample_point, Vec::new()))
            .1
            .push(accumulation);
    }
    let accumulations_per_sample_point = accumulations_by_sample_point
        .into_values()
        .map(|(sample_point, accumulations_per_log_size)| {
            // Accumulate the `a` coefficients.
            let first_linear_term_acc: SecureField = accumulations_per_log_size
                .iter()
                .map(|x| x.first_linear_term_acc)
                .sum();
            // Lift and accumulate the partial numerators vectors.
            // `partial_numerators_acc` is already sorted increasingly by size as required by
            // `B::lift_and_accumulate`.
            let partial_numerators_acc = accumulations_per_log_size
                .into_iter()
                .map(|x| x.partial_numerators_acc)
                .collect_vec();
            let res = B::lift_and_accumulate(partial_numerators_acc).unwrap();

            AccumulatedNumerators {
                sample_point,
                partial_numerators_acc: res,
                first_linear_term_acc,
            }
        })
        .collect_vec();
    emit_timing(
        "lift_and_accumulate",
        lift_and_accumulate_start.elapsed().as_secs_f64() * 1000.0,
    );

    let combine_start = Instant::now();
    let quotients =
        B::compute_quotients_and_combine(accumulations_per_sample_point, lifting_log_size);
    emit_timing(
        "compute_quotients_and_combine",
        combine_start.elapsed().as_secs_f64() * 1000.0,
    );
    quotients
}

#[cfg(test)]
mod tests {
    use itertools::Itertools;
    use num_traits::Zero;
    use rand::rngs::SmallRng;
    use rand::{Rng, SeedableRng};

    use crate::core::channel::Blake2sChannel;
    use crate::core::circle::SECURE_FIELD_CIRCLE_GEN;
    use crate::core::fields::m31::M31;
    use crate::core::pcs::quotients::PointSample;
    use crate::core::pcs::{CommitmentSchemeVerifier, PcsConfig, TreeVec};
    use crate::core::poly::circle::CanonicCoset;
    use crate::core::vcs_lifted::blake2_merkle::Blake2sMerkleChannel;
    use crate::core::verifier::VerificationError;
    use crate::prover::backend::cpu::{CpuCircleEvaluation, CpuCirclePoly};
    use crate::prover::backend::simd::SimdBackend;
    use crate::prover::backend::{Backend, BackendForChannel, CpuBackend};
    use crate::prover::pcs::quotient_ops::compute_fri_quotients;
    use crate::prover::poly::circle::CircleCoefficients;
    use crate::prover::{CommitmentSchemeProver, SecureField};

    #[test]
    fn test_quotients_are_low_degree() {
        let mut rng = SmallRng::seed_from_u64(0);
        const LOG_SIZE: u32 = 3;
        const LOG_BLOWUP_FACTOR: u32 = 4;

        let polynomial = CpuCirclePoly::new((0..1 << LOG_SIZE).map(M31::from).collect());
        let eval_domain = CanonicCoset::new(LOG_SIZE + LOG_BLOWUP_FACTOR).circle_domain();
        let eval = polynomial.evaluate(eval_domain);

        let sample_points = [
            SECURE_FIELD_CIRCLE_GEN.mul(rng.gen::<u128>()),
            SECURE_FIELD_CIRCLE_GEN.mul(rng.gen::<u128>()),
        ];
        let samples = sample_points
            .into_iter()
            .map(|x| PointSample {
                point: x,
                value: polynomial.eval_at_point(x),
            })
            .collect_vec();
        let rand_coeff =
            SecureField::from_m31_array(std::array::from_fn(|_| M31::from(rng.gen::<u32>())));
        let quot_eval = compute_fri_quotients(
            &TreeVec(vec![vec![&eval]]),
            &TreeVec(vec![vec![samples]]),
            rand_coeff,
            LOG_SIZE + LOG_BLOWUP_FACTOR,
            LOG_BLOWUP_FACTOR,
        );
        let mut coeffs = quot_eval
            .values
            .columns
            .iter()
            .map(|c| CpuCircleEvaluation::new(eval_domain, c.clone()).interpolate())
            .collect_vec();
        let zeros = coeffs[0].coeffs.split_off((1 << LOG_SIZE) - 1);

        assert!(zeros.iter().all(|c| c.is_zero()));
    }

    /// Generates a vector of random polynomials, such that the last one is of degree
    /// `LIFTING_LOG_SIZE - 1` and all the previous ones are of degree < `LIFTING_LOG_SIZE - 1`.
    fn prepare_polys<B: Backend, const N_COLS: usize, const LIFTING_LOG_SIZE: u32>(
    ) -> Vec<CircleCoefficients<B>> {
        let mut rng = SmallRng::seed_from_u64(0);
        let mut polys: Vec<CircleCoefficients<B>> = (0..N_COLS - 1)
            .map(|_| {
                CircleCoefficients::new(
                    (0..1 << rng.gen_range(4..LIFTING_LOG_SIZE - 1))
                        .map(M31::from)
                        .collect(),
                )
            })
            .collect();
        polys.push(CircleCoefficients::new(
            (0..1 << LIFTING_LOG_SIZE).map(M31::from).collect(),
        ));
        polys
    }

    fn prove_and_verify_pcs<
        B: BackendForChannel<Blake2sMerkleChannel>,
        const STORE_COEFFS: bool,
    >() -> Result<(), VerificationError> {
        const N_COLS: usize = 10;
        const LIFTING_LOG_SIZE: u32 = 8;

        // Setup the prover side of the pcs.
        let mut channel = Blake2sChannel::default();
        let config = PcsConfig::default();
        let twiddles = B::precompute_twiddles(
            CanonicCoset::new(LIFTING_LOG_SIZE + config.fri_config.log_blowup_factor).half_coset(),
        );
        let mut commitment_scheme =
            CommitmentSchemeProver::<B, Blake2sMerkleChannel>::new(config, &twiddles);
        if STORE_COEFFS {
            commitment_scheme.set_store_polynomials_coefficients();
        }
        let polys = prepare_polys::<B, N_COLS, LIFTING_LOG_SIZE>();
        let sizes = polys.iter().map(|poly| poly.log_size()).collect_vec();

        let mut tree_builder = commitment_scheme.tree_builder();
        tree_builder.extend_polys(polys);
        tree_builder.commit(&mut channel);

        let mut rng = SmallRng::seed_from_u64(0);
        let mask_structure = (0..N_COLS).map(|_| rng.gen_range(1..=2)).collect_vec();
        let samples = [
            SECURE_FIELD_CIRCLE_GEN.mul(rng.gen::<u128>()),
            SECURE_FIELD_CIRCLE_GEN.mul(rng.gen::<u128>()),
        ];
        let sampled_points = vec![(0..N_COLS)
            .zip(mask_structure.iter())
            .map(|(_, i)| samples.into_iter().take(*i).collect_vec())
            .collect_vec()];

        let proof = commitment_scheme.prove_values(TreeVec(sampled_points.clone()), &mut channel);

        // Verifier side of the pcs.
        let mut channel = Blake2sChannel::default();
        let mut verifier = CommitmentSchemeVerifier::<Blake2sMerkleChannel>::new(config);
        verifier.commit(proof.proof.commitments[0], &sizes, &mut channel);
        verifier.verify_values(TreeVec(sampled_points), proof.proof, &mut channel)
    }

    #[test]
    fn test_pcs_prove_and_verify_cpu() {
        assert!(prove_and_verify_pcs::<CpuBackend, true>().is_ok());
    }
    #[test]
    fn test_pcs_prove_and_verify_simd() {
        assert!(prove_and_verify_pcs::<SimdBackend, true>().is_ok());
    }
    #[test]
    fn test_pcs_prove_and_verify_simd_with_barycentric() {
        assert!(prove_and_verify_pcs::<SimdBackend, false>().is_ok());
    }
}
