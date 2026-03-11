use std::time::Instant;

use hashbrown::{HashMap, HashSet};
use itertools::Itertools;
#[cfg(feature = "parallel")]
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use tracing::{info, span, Level};

use crate::core::channel::{Channel, MerkleChannel};
use crate::core::circle::CirclePoint;
use crate::core::fields::m31::BaseField;
use crate::core::fields::qm31::SecureField;
use crate::core::pcs::quotients::{
    CommitmentSchemeProof, CommitmentSchemeProofAux, ExtendedCommitmentSchemeProof, PointSample,
};
use crate::core::pcs::utils::prepare_query_positions_for_tree;
use crate::core::pcs::{PcsConfig, TreeSubspan, TreeVec};
use crate::core::poly::circle::CanonicCoset;
use crate::core::utils::MaybeOwned;
use crate::core::vcs_lifted::merkle_hasher::MerkleHasherLifted;
use crate::core::vcs_lifted::verifier::ExtendedMerkleDecommitmentLifted;
use crate::core::ColumnVec;
use crate::prover::air::component_prover::{Poly, Trace, WeightsHashMap};
use crate::prover::backend::{BackendForChannel, Col};
use crate::prover::fri::{FriDecommitResult, FriProver};
use crate::prover::mempool::BaseColumnPool;
use crate::prover::pcs::quotient_ops::compute_fri_quotients;
use crate::prover::poly::circle::{CircleCoefficients, CircleEvaluation};
use crate::prover::poly::twiddles::TwiddleTree;
use crate::prover::poly::BitReversedOrder;
use crate::prover::vcs_lifted::prover::MerkleProverLifted;

pub mod quotient_ops;

struct BatchedEvalGroup<'a, B: BackendForChannel<MC>, MC: MerkleChannel> {
    coeffs: Vec<&'a CircleCoefficients<B>>,
    slots: Vec<(usize, usize, usize)>,
    _marker: std::marker::PhantomData<MC>,
}

impl<'a, B: BackendForChannel<MC>, MC: MerkleChannel> BatchedEvalGroup<'a, B, MC> {
    fn new() -> Self {
        Self {
            coeffs: Vec::new(),
            slots: Vec::new(),
            _marker: std::marker::PhantomData,
        }
    }
}

/// The prover side of a FRI polynomial commitment scheme. See [super].
pub struct CommitmentSchemeProver<'a, B: BackendForChannel<MC>, MC: MerkleChannel> {
    pub trees: TreeVec<MaybeOwned<'a, CommitmentTreeProver<B, MC>>>,
    pub config: PcsConfig,
    twiddles: &'a TwiddleTree<B>,
    pub store_polynomials_coefficients: bool,
    /// Pre-allocated base field column pool for polynomial evaluation during commit.
    pub base_column_pool: MaybeOwned<'a, BaseColumnPool<B>>,
}
impl<'a, B: BackendForChannel<MC>, MC: MerkleChannel> CommitmentSchemeProver<'a, B, MC> {
    /// Creates a new empty commitment scheme prover with the given configuration and twiddles. The
    /// commitment scheme does not store the polynomials coefficients by default.
    pub fn new(config: PcsConfig, twiddles: &'a TwiddleTree<B>) -> Self {
        CommitmentSchemeProver {
            trees: TreeVec::default(),
            config,
            twiddles,
            store_polynomials_coefficients: false,
            base_column_pool: MaybeOwned::Owned(BaseColumnPool::new()),
        }
    }

    pub fn with_memory_pool(
        config: PcsConfig,
        twiddles: &'a TwiddleTree<B>,
        base_column_pool: &'a BaseColumnPool<B>,
    ) -> Self {
        CommitmentSchemeProver {
            trees: TreeVec::default(),
            config,
            twiddles,
            store_polynomials_coefficients: false,
            base_column_pool: MaybeOwned::Borrowed(base_column_pool),
        }
    }

    /// Sets the commitment scheme to store the polynomials coefficients starting from the next
    /// commit.
    pub const fn set_store_polynomials_coefficients(&mut self) {
        self.store_polynomials_coefficients = true;
    }

    /// Evaluates the given polynomials, commits them into a Merkle tree, mixes the root into
    /// the channel, and appends the resulting tree to the scheme.
    fn commit(&mut self, polynomials: ColumnVec<CircleCoefficients<B>>, channel: &mut MC::C) {
        let _span = span!(Level::INFO, "Commitment").entered();
        let tree = CommitmentTreeProver::new(
            polynomials,
            self.config.fri_config.log_blowup_factor,
            self.twiddles,
            self.store_polynomials_coefficients,
            self.config.lifting_log_size,
            &self.base_column_pool,
        );
        MC::mix_root(channel, tree.commitment.root());
        self.trees.push(MaybeOwned::Owned(tree));
    }

    /// Appends an externally constructed [`CommitmentTreeProver`] to the scheme and mixes its
    /// Merkle root into the channel. Accepts both owned and borrowed trees.
    pub fn commit_tree(
        &mut self,
        tree: MaybeOwned<'a, CommitmentTreeProver<B, MC>>,
        channel: &mut MC::C,
    ) {
        MC::mix_root(channel, tree.commitment.root());
        self.trees.push(tree);
    }

    pub fn tree_builder(&mut self) -> TreeBuilder<'_, 'a, B, MC> {
        TreeBuilder {
            tree_index: self.trees.len(),
            commitment_scheme: self,
            polys: Vec::default(),
        }
    }

    pub fn roots(&self) -> TreeVec<<MC::H as MerkleHasherLifted>::Hash> {
        self.trees.as_ref().map(|tree| tree.commitment.root())
    }

    pub fn polynomials(&self) -> TreeVec<ColumnVec<&Poly<B>>> {
        self.trees
            .as_ref()
            .map(|tree| tree.polynomials.iter().collect())
    }

    pub fn evaluations(
        &self,
    ) -> TreeVec<ColumnVec<&CircleEvaluation<B, BaseField, BitReversedOrder>>> {
        self.trees
            .as_ref()
            .map(|tree| tree.polynomials.iter().map(|poly| &poly.evals).collect())
    }

    pub fn trace(&self) -> Trace<'_, B> {
        let polys = self.polynomials();
        Trace { polys }
    }

    pub fn build_weights_hash_map(
        &self,
        sampled_points: &TreeVec<ColumnVec<Vec<CirclePoint<SecureField>>>>,
        max_log_size: u32,
    ) -> WeightsHashMap<B>
    where
        Col<B, SecureField>: Send + Sync,
    {
        let weights_dashmap = WeightsHashMap::<B>::new();

        let mut unique_keys = HashSet::new();
        for (tree_polys, tree_points) in self.polynomials().0.iter().zip(sampled_points.0.iter()) {
            for (poly, points) in tree_polys.iter().zip(tree_points.iter()) {
                let log_size = poly.evals.domain.log_size();
                for &point in points {
                    unique_keys.insert((log_size, point.repeated_double(max_log_size - log_size)));
                }
            }
        }

        #[cfg(not(feature = "parallel"))]
        let computed_weights = unique_keys
            .into_iter()
            .map(|(log_size, point)| {
                (
                    (log_size, point),
                    CircleEvaluation::<B, BaseField, BitReversedOrder>::barycentric_weights(
                        CanonicCoset::new(log_size),
                        point,
                    ),
                )
            })
            .collect_vec();

        #[cfg(feature = "parallel")]
        let computed_weights = unique_keys
            .into_iter()
            .collect::<Vec<_>>()
            .into_par_iter()
            .map(|(log_size, point)| {
                (
                    (log_size, point),
                    CircleEvaluation::<B, BaseField, BitReversedOrder>::barycentric_weights(
                        CanonicCoset::new(log_size),
                        point,
                    ),
                )
            })
            .collect::<Vec<_>>();

        for (key, weights) in computed_weights {
            weights_dashmap.insert(key, weights);
        }

        weights_dashmap
    }

    pub fn prove_values(
        mut self,
        sampled_points: TreeVec<ColumnVec<Vec<CirclePoint<SecureField>>>>,
        channel: &mut MC::C,
    ) -> ExtendedCommitmentSchemeProof<MC::H> {
        let debug_prove_values = std::env::var_os("STWO_CUDA_DEBUG_PROVE_VALUES").is_some();
        let profile_prove_values = std::env::var_os("STWO_METAL_PROFILE_PROVE_VALUES").is_some();
        let debug_phase = |phase: &str| {
            if debug_prove_values {
                eprintln!("prove_values_phase={phase}");
            }
        };
        let emit_timing = |phase: &str, elapsed_ms: f64| {
            if profile_prove_values {
                eprintln!("prove_values_timing phase={phase} ms={elapsed_ms}");
            }
        };

        debug_phase("entry");
        // Evaluate polynomials on open points.
        let span = span!(
            Level::INFO,
            "Evaluate columns out of domain",
            class = "EvaluateOutOfDomain"
        )
        .entered();

        let lifting_log_size = self.trees.last().unwrap().commitment.layers.len() as u32 - 1;
        let weights_hash_map = if self.store_polynomials_coefficients {
            None
        } else {
            Some(self.build_weights_hash_map(&sampled_points, lifting_log_size))
        };

        let samples_start = Instant::now();
        let samples: TreeVec<Vec<Vec<PointSample>>> = if self.store_polynomials_coefficients {
            let polynomials = self.polynomials();
            let mut sample_trees = sampled_points
                .0
                .iter()
                .map(|tree_points| {
                    tree_points
                        .iter()
                        .map(|points| {
                            points
                                .iter()
                                .map(|&point| PointSample {
                                    point,
                                    value: SecureField::default(),
                                })
                                .collect_vec()
                        })
                        .collect_vec()
                })
                .collect_vec();

            // Group requests by the folded query point and coefficient size so the backend can
            // evaluate same-size batches directly without an extra regroup/scatter pass.
            let mut groups: HashMap<
                (CirclePoint<SecureField>, u32),
                BatchedEvalGroup<'_, B, MC>,
            > = HashMap::new();
            for (tree_index, (tree_polys, tree_points)) in polynomials
                .0
                .iter()
                .zip(sampled_points.0.iter())
                .enumerate()
            {
                for (column_index, (poly, points)) in
                    tree_polys.iter().zip(tree_points.iter()).enumerate()
                {
                    let coeffs = poly.coeffs.as_ref().expect(
                        "coefficients should exist when store_polynomials_coefficients is enabled",
                    );
                    let repeated_double = lifting_log_size - poly.evals.domain.log_size();
                    let coeffs_log_size = coeffs.log_size();
                    for (point_index, &point) in points.iter().enumerate() {
                        let group = groups
                            .entry((point.repeated_double(repeated_double), coeffs_log_size))
                            .or_insert_with(BatchedEvalGroup::new);
                        group.coeffs.push(coeffs);
                        group.slots.push((tree_index, column_index, point_index));
                    }
                }
            }
            debug_phase("request_groups_built");

            debug_phase("batched_eval_start");
            for ((folded_point, _), group) in groups {
                let values = B::batch_eval_at_point(&group.coeffs, folded_point);
                for ((tree_index, column_index, point_index), value) in
                    group.slots.into_iter().zip(values.into_iter())
                {
                    sample_trees[tree_index][column_index][point_index].value = value;
                }
            }
            debug_phase("batched_eval_done");

            TreeVec(sample_trees)
        } else {
            // Lambda that evaluates a polynomial on a collection of circle points and returns a
            // vector of point samples.
            let eval_at_points = |(poly, points): (&Poly<B>, &Vec<CirclePoint<SecureField>>)| {
                points
                    .iter()
                    .map(|&point| PointSample {
                        point,
                        value: poly.eval_at_point(
                            point.repeated_double(lifting_log_size - poly.evals.domain.log_size()),
                            weights_hash_map.as_ref(),
                        ),
                    })
                    .collect_vec()
            };

            #[cfg(not(feature = "parallel"))]
            let samples: TreeVec<Vec<Vec<PointSample>>> = self
                .polynomials()
                .zip_cols(&sampled_points)
                .map_cols(eval_at_points);
            #[cfg(feature = "parallel")]
            let samples: TreeVec<Vec<Vec<PointSample>>> = self
                .polynomials()
                .zip_cols(&sampled_points)
                .par_map_cols(eval_at_points);
            samples
        };
        emit_timing("samples", samples_start.elapsed().as_secs_f64() * 1000.0);

        span.exit();
        debug_phase("samples_ready");
        let sampled_values_start = Instant::now();
        let total_sample_count = samples
            .0
            .iter()
            .map(|tree| tree.iter().map(|column| column.len()).sum::<usize>())
            .sum();
        let mut flattened_sampled_values = Vec::with_capacity(total_sample_count);
        let sampled_values = TreeVec(
            samples
                .0
                .iter()
                .map(|tree| {
                    tree.iter()
                        .map(|column| {
                            column
                                .iter()
                                .map(|sample| {
                                    flattened_sampled_values.push(sample.value);
                                    sample.value
                                })
                                .collect_vec()
                        })
                        .collect_vec()
                })
                .collect_vec(),
        );
        channel.mix_felts(&flattened_sampled_values);
        emit_timing(
            "sampled_values_mix",
            sampled_values_start.elapsed().as_secs_f64() * 1000.0,
        );
        debug_phase("sampled_values_mixed");

        let columns = self.evaluations();
        print_column_size_histogram::<B, MC>(&columns);
        // Compute oods quotients for boundary constraints on the sampled points.
        let quotients_start = Instant::now();
        let quotients = compute_fri_quotients(
            &columns,
            &samples,
            channel.draw_secure_felt(),
            lifting_log_size,
            self.config.fri_config.log_blowup_factor,
        );
        emit_timing(
            "quotients",
            quotients_start.elapsed().as_secs_f64() * 1000.0,
        );
        debug_phase("quotients_ready");

        // Run FRI commitment phase on the oods quotients.
        let fri_commit_start = Instant::now();
        let fri_prover =
            FriProver::<B, MC>::commit(channel, self.config.fri_config, &quotients, self.twiddles);
        emit_timing(
            "fri_commit",
            fri_commit_start.elapsed().as_secs_f64() * 1000.0,
        );
        debug_phase("fri_commit_ready");

        // Proof of work.
        let span1 = span!(Level::INFO, "Grind", class = "Queries POW").entered();
        let pow_start = Instant::now();
        let proof_of_work = B::grind(channel, self.config.pow_bits);
        span1.exit();
        channel.mix_u64(proof_of_work);
        emit_timing("proof_of_work", pow_start.elapsed().as_secs_f64() * 1000.0);
        debug_phase("proof_of_work_ready");

        // FRI decommitment phase.
        let fri_decommit_start = Instant::now();
        let FriDecommitResult {
            fri_proof,
            query_positions,
            unsorted_query_locations,
        } = fri_prover.decommit(channel);
        emit_timing(
            "fri_decommit",
            fri_decommit_start.elapsed().as_secs_f64() * 1000.0,
        );
        debug_phase("fri_decommit_ready");
        // Build the query position tree.
        let query_tree_start = Instant::now();
        let mut prepared_tree_queries_by_log_size = HashMap::<u32, Vec<usize>>::new();
        let query_positions_tree = TreeVec::new(
            self.trees
                .iter()
                .map(|tree| {
                    let tree_log_size = tree.commitment.layers.len() as u32 - 1;
                    prepared_tree_queries_by_log_size
                        .entry(tree_log_size)
                        .or_insert_with(|| {
                            prepare_query_positions_for_tree(
                                &query_positions,
                                lifting_log_size,
                                tree_log_size,
                            )
                        })
                        .clone()
                })
                .collect::<Vec<_>>(),
        );
        emit_timing(
            "query_position_tree",
            query_tree_start.elapsed().as_secs_f64() * 1000.0,
        );
        let commitments = self.roots();
        let mut queried_values = Vec::with_capacity(self.trees.len());
        let mut decommitments = Vec::with_capacity(self.trees.len());
        let mut aux = Vec::with_capacity(self.trees.len());
        let tree_decommit_start = Instant::now();
        for (tree_index, (tree, query_positions)) in self
            .trees
            .as_ref()
            .zip_eq(query_positions_tree.as_ref())
            .0
            .into_iter()
            .enumerate()
        {
            if debug_prove_values {
                eprintln!("prove_values_phase=tree_decommit_start:{tree_index}");
            }
            let (values, decommit_result) = tree.decommit(query_positions);
            queried_values.push(values);
            decommitments.push(decommit_result.decommitment);
            aux.push(decommit_result.aux);
            if debug_prove_values {
                eprintln!("prove_values_phase=tree_decommit_done:{tree_index}");
            }
        }
        emit_timing(
            "tree_decommit",
            tree_decommit_start.elapsed().as_secs_f64() * 1000.0,
        );
        debug_phase("tree_decommit_ready");

        // Return evaluation buffers to the memory pool for reuse (owned trees only).
        for tree in &mut self.trees.0 {
            if let MaybeOwned::Owned(tree) = tree {
                for poly in tree.polynomials.drain(..) {
                    let log_size = poly.evals.domain.log_size();
                    self.base_column_pool.give_back(log_size, poly.evals.values);
                }
            }
        }

        let proof = ExtendedCommitmentSchemeProof {
            proof: CommitmentSchemeProof {
                commitments,
                sampled_values,
                decommitments: TreeVec(decommitments),
                queried_values: TreeVec(queried_values),
                proof_of_work,
                fri_proof: fri_proof.proof,
                config: self.config,
            },
            aux: CommitmentSchemeProofAux {
                unsorted_query_locations,
                trace_decommitment: TreeVec(aux),
                fri: fri_proof.aux,
            },
        };
        debug_phase("return_ready");
        proof
    }
}

/// Helper struct for aggregating polynomials and evaluations for a commitment tree.
pub struct TreeBuilder<'a, 'b, B: BackendForChannel<MC>, MC: MerkleChannel> {
    tree_index: usize,
    commitment_scheme: &'a mut CommitmentSchemeProver<'b, B, MC>,
    polys: ColumnVec<CircleCoefficients<B>>,
}
impl<B: BackendForChannel<MC>, MC: MerkleChannel> TreeBuilder<'_, '_, B, MC> {
    pub fn extend_evals(
        &mut self,
        columns: Vec<CircleEvaluation<B, BaseField, BitReversedOrder>>,
    ) -> TreeSubspan {
        let span = span!(Level::INFO, "Interpolation for commitment").entered();
        let polys = B::interpolate_columns(columns, self.commitment_scheme.twiddles);
        span.exit();

        self.extend_polys(polys)
    }

    pub fn extend_polys(
        &mut self,
        columns: impl IntoIterator<Item = CircleCoefficients<B>>,
    ) -> TreeSubspan {
        let col_start = self.polys.len();
        self.polys.extend(columns);
        let col_end = self.polys.len();
        TreeSubspan {
            tree_index: self.tree_index,
            col_start,
            col_end,
        }
    }

    pub fn commit(self, channel: &mut MC::C) {
        let _span = span!(Level::INFO, "Commitment").entered();
        self.commitment_scheme.commit(self.polys, channel);
    }
}

/// Prover data for a single commitment tree in a commitment scheme. The commitment scheme allows to
/// commit on a set of polynomials at a time. This corresponds to such a set.
pub struct CommitmentTreeProver<B: BackendForChannel<MC>, MC: MerkleChannel> {
    pub polynomials: ColumnVec<Poly<B>>,
    pub commitment: MerkleProverLifted<B, MC::H>,
}

impl<B: BackendForChannel<MC>, MC: MerkleChannel> CommitmentTreeProver<B, MC> {
    pub fn new(
        polynomials: ColumnVec<CircleCoefficients<B>>,
        log_blowup_factor: u32,
        twiddles: &TwiddleTree<B>,
        store_polynomials_coefficients: bool,
        lifting_log_size: Option<u32>,
        base_column_pool: &BaseColumnPool<B>,
    ) -> Self {
        let span = span!(Level::INFO, "Extension").entered();
        let polynomials = B::evaluate_polynomials(
            polynomials,
            log_blowup_factor,
            twiddles,
            store_polynomials_coefficients,
            base_column_pool,
        );
        span.exit();

        let _span = span!(Level::INFO, "Merkle").entered();
        let max_log_domain_size = polynomials
            .iter()
            .map(|poly| poly.evals.domain.log_size())
            .max()
            .unwrap_or_default();
        let lifting_log_size = lifting_log_size.unwrap_or(max_log_domain_size);
        let tree = MerkleProverLifted::commit(
            polynomials
                .iter()
                .map(|poly: &Poly<B>| &poly.evals.values)
                .collect(),
            lifting_log_size,
        );

        CommitmentTreeProver {
            polynomials,
            commitment: tree,
        }
    }

    /// Decommits the merkle tree on the given query positions.
    /// Returns the values at the queried positions and the decommitment.
    /// The queries are given as a mapping from the log size of the layer size to the queried
    /// positions on each column of that size.
    fn decommit(
        &self,
        queries: &[usize],
    ) -> (
        ColumnVec<Vec<BaseField>>,
        ExtendedMerkleDecommitmentLifted<MC::H>,
    ) {
        let eval_vec = self
            .polynomials
            .iter()
            .map(|poly| &poly.evals.values)
            .collect_vec();
        self.commitment.decommit(queries, eval_vec)
    }
}

fn print_column_size_histogram<B: BackendForChannel<MC>, MC: MerkleChannel>(
    columns_per_tree: &TreeVec<ColumnVec<&CircleEvaluation<B, BaseField, BitReversedOrder>>>,
) {
    let mut log_size_histogram = HashMap::new();
    for columns in columns_per_tree.iter() {
        for column in columns {
            *log_size_histogram
                .entry(column.domain.log_size())
                .or_insert(0) += 1;
        }
    }
    for (log_size, count) in log_size_histogram {
        info!("Log size {log_size}: {count}");
    }
}
