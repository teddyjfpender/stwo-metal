use std::simd::u32x16;

use itertools::{chain, multiunzip, Itertools};
use num_traits::Zero;
use serde::Serialize;
use stwo::core::air::Component;
use stwo::core::channel::{Channel, MerkleChannel};
use stwo::core::fields::m31::BaseField;
use stwo::core::fields::qm31::SecureField;
use stwo::core::pcs::{CommitmentSchemeVerifier, PcsConfig, TreeVec};
use stwo::core::poly::circle::CanonicCoset;
use stwo::core::proof::StarkProof;
use stwo::core::vcs_lifted::merkle_hasher::MerkleHasherLifted;
use stwo::core::verifier::{verify, VerificationError};
use stwo::core::ColumnVec;
use stwo::prover::backend::simd::m31::LOG_N_LANES;
use stwo::prover::backend::simd::SimdBackend;
use stwo::prover::backend::BackendForChannel;
use stwo::prover::poly::circle::{CircleEvaluation, PolyOps};
use stwo::prover::poly::BitReversedOrder;
use stwo::prover::{prove, CommitmentSchemeProver, ComponentProver};
use stwo_constraint_framework::preprocessed_columns::PreProcessedColumnId;
use stwo_constraint_framework::{TraceLocationAllocator, PREPROCESSED_TRACE_IDX};
use tracing::{span, Level};

use super::preprocessed_columns::XorTable;
use super::round::{blake_round_info, BlakeRoundComponent, BlakeRoundEval};
use super::scheduler::{BlakeSchedulerComponent, BlakeSchedulerEval};
use super::xor_table::{xor12, xor4, xor7, xor8, xor9};
use crate::blake::round::RoundElements;
use crate::blake::scheduler::{self, blake_scheduler_info, BlakeElements, BlakeInput};
use crate::blake::{round, xor_table, BlakeXorElements, XorAccums, N_ROUNDS, ROUND_LOG_SPLIT};

fn preprocessed_xor_columns() -> [PreProcessedColumnId; 15] {
    [
        XorTable::new(12, 4, 0).id(),
        XorTable::new(12, 4, 1).id(),
        XorTable::new(12, 4, 2).id(),
        XorTable::new(9, 2, 0).id(),
        XorTable::new(9, 2, 1).id(),
        XorTable::new(9, 2, 2).id(),
        XorTable::new(8, 2, 0).id(),
        XorTable::new(8, 2, 1).id(),
        XorTable::new(8, 2, 2).id(),
        XorTable::new(7, 2, 0).id(),
        XorTable::new(7, 2, 1).id(),
        XorTable::new(7, 2, 2).id(),
        XorTable::new(4, 0, 0).id(),
        XorTable::new(4, 0, 1).id(),
        XorTable::new(4, 0, 2).id(),
    ]
}

const fn preprocessed_xor_columns_log_sizes() -> [u32; 15] {
    [
        XorTable::new(12, 4, 0).column_bits(),
        XorTable::new(12, 4, 1).column_bits(),
        XorTable::new(12, 4, 2).column_bits(),
        XorTable::new(9, 2, 0).column_bits(),
        XorTable::new(9, 2, 1).column_bits(),
        XorTable::new(9, 2, 2).column_bits(),
        XorTable::new(8, 2, 0).column_bits(),
        XorTable::new(8, 2, 1).column_bits(),
        XorTable::new(8, 2, 2).column_bits(),
        XorTable::new(7, 2, 0).column_bits(),
        XorTable::new(7, 2, 1).column_bits(),
        XorTable::new(7, 2, 2).column_bits(),
        XorTable::new(4, 0, 0).column_bits(),
        XorTable::new(4, 0, 1).column_bits(),
        XorTable::new(4, 0, 2).column_bits(),
    ]
}

#[derive(Serialize)]
pub struct BlakeStatement0 {
    log_size: u32,
}
impl BlakeStatement0 {
    fn log_sizes(&self) -> TreeVec<Vec<u32>> {
        let mut sizes = vec![];
        sizes.push(
            blake_scheduler_info()
                .mask_offsets
                .as_cols_ref()
                .map_cols(|_| self.log_size),
        );
        for l in ROUND_LOG_SPLIT {
            sizes.push(
                blake_round_info()
                    .mask_offsets
                    .as_cols_ref()
                    .map_cols(|_| self.log_size + l),
            );
        }
        sizes.push(xor_table::xor12::trace_sizes::<12, 4>());
        sizes.push(xor_table::xor9::trace_sizes::<9, 2>());
        sizes.push(xor_table::xor8::trace_sizes::<8, 2>());
        sizes.push(xor_table::xor7::trace_sizes::<7, 2>());
        sizes.push(xor_table::xor4::trace_sizes::<4, 0>());

        let mut log_sizes = TreeVec::concat_cols(sizes.into_iter());

        log_sizes[PREPROCESSED_TRACE_IDX] = preprocessed_xor_columns_log_sizes().into();

        log_sizes
    }
    fn mix_into(&self, channel: &mut impl Channel) {
        channel.mix_u64(self.log_size as u64);
    }
}

pub struct AllElements {
    blake_elements: BlakeElements,
    round_elements: RoundElements,
    xor_elements: BlakeXorElements,
}
impl AllElements {
    pub fn draw(channel: &mut impl Channel) -> Self {
        Self {
            blake_elements: BlakeElements::draw(channel),
            round_elements: RoundElements::draw(channel),
            xor_elements: BlakeXorElements::draw(channel),
        }
    }
}

pub struct BlakeStatement1 {
    scheduler_claimed_sum: SecureField,
    round_claimed_sums: Vec<SecureField>,
    xor12_claimed_sum: SecureField,
    xor9_claimed_sum: SecureField,
    xor8_claimed_sum: SecureField,
    xor7_claimed_sum: SecureField,
    xor4_claimed_sum: SecureField,
}
impl BlakeStatement1 {
    fn mix_into(&self, channel: &mut impl Channel) {
        channel.mix_felts(
            &chain![
                [
                    self.scheduler_claimed_sum,
                    self.xor12_claimed_sum,
                    self.xor9_claimed_sum,
                    self.xor8_claimed_sum,
                    self.xor7_claimed_sum,
                    self.xor4_claimed_sum
                ],
                self.round_claimed_sums.clone()
            ]
            .collect_vec(),
        )
    }
}

pub struct BlakeProof<H: MerkleHasherLifted> {
    stmt0: BlakeStatement0,
    stmt1: BlakeStatement1,
    stark_proof: StarkProof<H>,
}

impl<H: MerkleHasherLifted> BlakeProof<H> {
    pub fn stark_proof(&self) -> &StarkProof<H> {
        &self.stark_proof
    }
}

pub struct BlakeComponents {
    scheduler_component: BlakeSchedulerComponent,
    round_components: Vec<BlakeRoundComponent>,
    xor12: xor12::XorTableComponent<12, 4>,
    xor9: xor9::XorTableComponent<9, 2>,
    xor8: xor8::XorTableComponent<8, 2>,
    xor7: xor7::XorTableComponent<7, 2>,
    xor4: xor4::XorTableComponent<4, 0>,
}
impl BlakeComponents {
    fn new(stmt0: &BlakeStatement0, all_elements: &AllElements, stmt1: &BlakeStatement1) -> Self {
        let tree_span_provider =
            &mut TraceLocationAllocator::new_with_preprocessed_columns(&preprocessed_xor_columns());

        Self {
            scheduler_component: BlakeSchedulerComponent::new(
                tree_span_provider,
                BlakeSchedulerEval {
                    log_size: stmt0.log_size,
                    blake_lookup_elements: all_elements.blake_elements.clone(),
                    round_lookup_elements: all_elements.round_elements.clone(),
                    claimed_sum: stmt1.scheduler_claimed_sum,
                },
                stmt1.scheduler_claimed_sum,
            ),
            round_components: ROUND_LOG_SPLIT
                .iter()
                .zip(stmt1.round_claimed_sums.clone())
                .map(|(l, claimed_sum)| {
                    BlakeRoundComponent::new(
                        tree_span_provider,
                        BlakeRoundEval {
                            log_size: stmt0.log_size + l,
                            xor_lookup_elements: all_elements.xor_elements.clone(),
                            round_lookup_elements: all_elements.round_elements.clone(),
                            claimed_sum,
                        },
                        claimed_sum,
                    )
                })
                .collect(),
            xor12: xor12::XorTableComponent::new(
                tree_span_provider,
                xor12::XorTableEval {
                    lookup_elements: all_elements.xor_elements.xor12.clone(),
                    claimed_sum: stmt1.xor12_claimed_sum,
                },
                stmt1.xor12_claimed_sum,
            ),
            xor9: xor9::XorTableComponent::new(
                tree_span_provider,
                xor9::XorTableEval {
                    lookup_elements: all_elements.xor_elements.xor9.clone(),
                    claimed_sum: stmt1.xor9_claimed_sum,
                },
                stmt1.xor9_claimed_sum,
            ),
            xor8: xor8::XorTableComponent::new(
                tree_span_provider,
                xor8::XorTableEval {
                    lookup_elements: all_elements.xor_elements.xor8.clone(),
                    claimed_sum: stmt1.xor8_claimed_sum,
                },
                stmt1.xor8_claimed_sum,
            ),
            xor7: xor7::XorTableComponent::new(
                tree_span_provider,
                xor7::XorTableEval {
                    lookup_elements: all_elements.xor_elements.xor7.clone(),
                    claimed_sum: stmt1.xor7_claimed_sum,
                },
                stmt1.xor7_claimed_sum,
            ),
            xor4: xor4::XorTableComponent::new(
                tree_span_provider,
                xor4::XorTableEval {
                    lookup_elements: all_elements.xor_elements.xor4.clone(),
                    claimed_sum: stmt1.xor4_claimed_sum,
                },
                stmt1.xor4_claimed_sum,
            ),
        }
    }
    fn components(&self) -> Vec<&dyn Component> {
        chain![
            [&self.scheduler_component as &dyn Component],
            self.round_components.iter().map(|c| c as &dyn Component),
            [
                &self.xor12 as &dyn Component,
                &self.xor9 as &dyn Component,
                &self.xor8 as &dyn Component,
                &self.xor7 as &dyn Component,
                &self.xor4 as &dyn Component,
            ]
        ]
        .collect()
    }

    fn component_provers(&self) -> Vec<&dyn ComponentProver<SimdBackend>> {
        chain![
            [&self.scheduler_component as &dyn ComponentProver<SimdBackend>],
            self.round_components
                .iter()
                .map(|c| c as &dyn ComponentProver<SimdBackend>),
            [
                &self.xor12 as &dyn ComponentProver<SimdBackend>,
                &self.xor9 as &dyn ComponentProver<SimdBackend>,
                &self.xor8 as &dyn ComponentProver<SimdBackend>,
                &self.xor7 as &dyn ComponentProver<SimdBackend>,
                &self.xor4 as &dyn ComponentProver<SimdBackend>,
            ]
        ]
        .collect()
    }
}

/// Backend-substitution seam for the vendored upstream Blake example.
///
/// Inputs:
/// - `log_size`: the scheduler trace log size used by the unchanged upstream workload.
/// - `config`: PCS configuration used to derive the same channel flow as `prove_blake`.
///
/// Outputs:
/// - the committed trace trees, statements, and component set required to substitute the backend
///   prove step without changing the workload logic.
///
/// Invariants:
/// - trace generation, interaction generation, and statement construction remain upstream-owned.
/// - the returned traces are ordered exactly as `prove_blake` commits them.
///
/// Failure modes:
/// - panics under the same malformed log-size assumptions as `prove_blake`.
pub struct BlakeProvingSetup {
    stmt0: BlakeStatement0,
    stmt1: BlakeStatement1,
    preprocessed_trace: ColumnVec<CircleEvaluation<SimdBackend, BaseField, BitReversedOrder>>,
    trace: ColumnVec<CircleEvaluation<SimdBackend, BaseField, BitReversedOrder>>,
    interaction_trace: ColumnVec<CircleEvaluation<SimdBackend, BaseField, BitReversedOrder>>,
    components: BlakeComponents,
    log_max_rows: u32,
}

impl BlakeProvingSetup {
    pub fn max_trace_log_size(&self) -> u32 {
        self.log_max_rows
    }

    pub fn preprocessed_trace(
        &self,
    ) -> &[CircleEvaluation<SimdBackend, BaseField, BitReversedOrder>] {
        &self.preprocessed_trace
    }

    pub fn trace(&self) -> &[CircleEvaluation<SimdBackend, BaseField, BitReversedOrder>] {
        &self.trace
    }

    pub fn interaction_trace(
        &self,
    ) -> &[CircleEvaluation<SimdBackend, BaseField, BitReversedOrder>] {
        &self.interaction_trace
    }

    pub fn scheduler_component(&self) -> &BlakeSchedulerComponent {
        &self.components.scheduler_component
    }

    pub fn round_components(&self) -> &[BlakeRoundComponent] {
        &self.components.round_components
    }

    pub fn xor12_component(&self) -> &xor12::XorTableComponent<12, 4> {
        &self.components.xor12
    }

    pub fn xor9_component(&self) -> &xor9::XorTableComponent<9, 2> {
        &self.components.xor9
    }

    pub fn xor8_component(&self) -> &xor8::XorTableComponent<8, 2> {
        &self.components.xor8
    }

    pub fn xor7_component(&self) -> &xor7::XorTableComponent<7, 2> {
        &self.components.xor7
    }

    pub fn xor4_component(&self) -> &xor4::XorTableComponent<4, 0> {
        &self.components.xor4
    }

    pub fn mix_stmt0(&self, channel: &mut impl Channel) {
        self.stmt0.mix_into(channel);
    }

    pub fn replay_interaction_element_draw(&self, channel: &mut impl Channel) {
        let _ = AllElements::draw(channel);
    }

    pub fn mix_stmt1(&self, channel: &mut impl Channel) {
        self.stmt1.mix_into(channel);
    }

    pub fn components(&self) -> Vec<&dyn Component> {
        self.components.components()
    }

    pub fn component_provers(&self) -> Vec<&dyn ComponentProver<SimdBackend>> {
        self.components.component_provers()
    }

    pub fn into_proof<H: MerkleHasherLifted>(self, stark_proof: StarkProof<H>) -> BlakeProof<H> {
        BlakeProof {
            stmt0: self.stmt0,
            stmt1: self.stmt1,
            stark_proof,
        }
    }
}

pub fn build_blake_proving_setup<MC: MerkleChannel>(
    log_size: u32,
    config: PcsConfig,
) -> BlakeProvingSetup
where
    SimdBackend: BackendForChannel<MC>,
{
    assert!(log_size >= LOG_N_LANES);
    assert_eq!(
        ROUND_LOG_SPLIT.map(|x| 1 << x).into_iter().sum::<u32>() as usize,
        N_ROUNDS
    );

    const XOR_TABLE_MAX_LOG_SIZE: u32 = 16;
    let log_max_rows =
        (log_size + *ROUND_LOG_SPLIT.iter().max().unwrap()).max(XOR_TABLE_MAX_LOG_SIZE);

    let blake_inputs = (0..(1 << (log_size - LOG_N_LANES)))
        .map(|i| {
            let v = [u32x16::from_array(std::array::from_fn(|j| (i + 2 * j) as u32)); 16];
            let m = [u32x16::from_array(std::array::from_fn(|j| (i + 2 * j + 1) as u32)); 16];
            BlakeInput { v, m }
        })
        .collect_vec();

    let twiddles = SimdBackend::precompute_twiddles(
        CanonicCoset::new(log_max_rows + 1 + config.fri_config.log_blowup_factor)
            .circle_domain()
            .half_coset,
    );
    let channel = &mut MC::C::default();
    let mut commitment_scheme = CommitmentSchemeProver::new(config, &twiddles);

    let preprocessed_trace = chain![
        XorTable::new(12, 4, 0).generate_constant_trace(),
        XorTable::new(9, 2, 0).generate_constant_trace(),
        XorTable::new(8, 2, 0).generate_constant_trace(),
        XorTable::new(7, 2, 0).generate_constant_trace(),
        XorTable::new(4, 0, 0).generate_constant_trace(),
    ]
    .collect_vec();
    let mut tree_builder = commitment_scheme.tree_builder();
    tree_builder.extend_evals(preprocessed_trace.clone());
    tree_builder.commit(channel);

    let (scheduler_trace, scheduler_lookup_data, round_inputs) =
        scheduler::gen_trace(log_size, &blake_inputs);

    let mut xor_accums = XorAccums::default();
    let mut rest = &round_inputs[..];
    let (round_traces, round_lookup_data): (Vec<_>, Vec<_>) =
        multiunzip(ROUND_LOG_SPLIT.map(|l| {
            let (cur_inputs, r) = rest.split_at(1 << (log_size - LOG_N_LANES + l));
            rest = r;
            round::generate_trace(log_size + l, cur_inputs, &mut xor_accums)
        }));

    let (xor_trace12, xor_lookup_data12) = xor_table::xor12::generate_trace(xor_accums.xor12);
    let (xor_trace9, xor_lookup_data9) = xor_table::xor9::generate_trace(xor_accums.xor9);
    let (xor_trace8, xor_lookup_data8) = xor_table::xor8::generate_trace(xor_accums.xor8);
    let (xor_trace7, xor_lookup_data7) = xor_table::xor7::generate_trace(xor_accums.xor7);
    let (xor_trace4, xor_lookup_data4) = xor_table::xor4::generate_trace(xor_accums.xor4);

    let stmt0 = BlakeStatement0 { log_size };
    stmt0.mix_into(channel);

    let trace = chain![
        scheduler_trace,
        round_traces.into_iter().flatten(),
        xor_trace12,
        xor_trace9,
        xor_trace8,
        xor_trace7,
        xor_trace4,
    ]
    .collect_vec();
    let mut tree_builder = commitment_scheme.tree_builder();
    tree_builder.extend_evals(trace.clone());
    tree_builder.commit(channel);

    let all_elements = AllElements::draw(channel);

    let (scheduler_trace, scheduler_claimed_sum) = scheduler::gen_interaction_trace(
        log_size,
        scheduler_lookup_data,
        &all_elements.round_elements,
        &all_elements.blake_elements,
    );

    let (round_traces, round_claimed_sums): (Vec<_>, Vec<_>) = multiunzip(
        ROUND_LOG_SPLIT
            .iter()
            .zip(round_lookup_data)
            .map(|(l, lookup_data)| {
                round::generate_interaction_trace(
                    log_size + l,
                    lookup_data,
                    &all_elements.xor_elements,
                    &all_elements.round_elements,
                )
            }),
    );

    let (xor_trace12, xor12_claimed_sum) = xor_table::xor12::generate_interaction_trace(
        xor_lookup_data12,
        &all_elements.xor_elements.xor12,
    );
    let (xor_trace9, xor9_claimed_sum) = xor_table::xor9::generate_interaction_trace(
        xor_lookup_data9,
        &all_elements.xor_elements.xor9,
    );
    let (xor_trace8, xor8_claimed_sum) = xor_table::xor8::generate_interaction_trace(
        xor_lookup_data8,
        &all_elements.xor_elements.xor8,
    );
    let (xor_trace7, xor7_claimed_sum) = xor_table::xor7::generate_interaction_trace(
        xor_lookup_data7,
        &all_elements.xor_elements.xor7,
    );
    let (xor_trace4, xor4_claimed_sum) = xor_table::xor4::generate_interaction_trace(
        xor_lookup_data4,
        &all_elements.xor_elements.xor4,
    );

    let interaction_trace = chain![
        scheduler_trace,
        round_traces.into_iter().flatten(),
        xor_trace12,
        xor_trace9,
        xor_trace8,
        xor_trace7,
        xor_trace4,
    ]
    .collect_vec();

    let stmt1 = BlakeStatement1 {
        scheduler_claimed_sum,
        round_claimed_sums,
        xor12_claimed_sum,
        xor9_claimed_sum,
        xor8_claimed_sum,
        xor7_claimed_sum,
        xor4_claimed_sum,
    };

    let components = BlakeComponents::new(&stmt0, &all_elements, &stmt1);

    BlakeProvingSetup {
        stmt0,
        stmt1,
        preprocessed_trace,
        trace,
        interaction_trace,
        components,
        log_max_rows,
    }
}

#[allow(unused)]
pub fn prove_blake<MC: MerkleChannel>(log_size: u32, config: PcsConfig) -> (BlakeProof<MC::H>)
where
    SimdBackend: BackendForChannel<MC>,
{
    let setup = build_blake_proving_setup::<MC>(log_size, config);
    let span = span!(Level::INFO, "Precompute twiddles").entered();
    let twiddles = SimdBackend::precompute_twiddles(
        CanonicCoset::new(setup.max_trace_log_size() + 1 + config.fri_config.log_blowup_factor)
            .circle_domain()
            .half_coset,
    );
    span.exit();

    // Setup protocol.
    let channel = &mut MC::C::default();
    let mut commitment_scheme = CommitmentSchemeProver::new(config, &twiddles);
    let mut tree_builder = commitment_scheme.tree_builder();
    tree_builder.extend_evals(setup.preprocessed_trace().to_vec());
    tree_builder.commit(channel);
    let mut tree_builder = commitment_scheme.tree_builder();
    setup.stmt0.mix_into(channel);
    tree_builder.extend_evals(setup.trace().to_vec());
    tree_builder.commit(channel);
    let mut tree_builder = commitment_scheme.tree_builder();
    tree_builder.extend_evals(setup.interaction_trace().to_vec());
    setup.stmt1.mix_into(channel);
    tree_builder.commit(channel);

    assert_eq!(
        commitment_scheme
            .polynomials()
            .as_cols_ref()
            .map_cols(|c| c.evals.domain.log_size() - config.fri_config.log_blowup_factor)
            .0,
        setup.stmt0.log_sizes().0
    );

    // Prove constraints.
    let stark_proof = prove(&setup.components.component_provers(), channel, commitment_scheme).unwrap();

    setup.into_proof(stark_proof)
}

#[allow(unused)]
pub fn verify_blake<MC: MerkleChannel>(
    BlakeProof {
        stmt0,
        stmt1,
        stark_proof,
    }: BlakeProof<MC::H>,
) -> Result<(), VerificationError> {
    // TODO(alonf): Consider mixing the config into the channel.
    let channel = &mut MC::C::default();
    const REQUIRED_SECURITY_BITS: u32 = 5;
    assert!(stark_proof.config.security_bits() >= REQUIRED_SECURITY_BITS);
    let commitment_scheme = &mut CommitmentSchemeVerifier::<MC>::new(stark_proof.config);

    let log_sizes = stmt0.log_sizes();

    // Preprocessed trace.
    commitment_scheme.commit(stark_proof.commitments[0], &log_sizes[0], channel);

    // Trace.
    stmt0.mix_into(channel);
    commitment_scheme.commit(stark_proof.commitments[1], &log_sizes[1], channel);

    // Draw interaction elements.
    let all_elements = AllElements::draw(channel);

    // Interaction trace.
    stmt1.mix_into(channel);
    commitment_scheme.commit(stark_proof.commitments[2], &log_sizes[2], channel);

    let components = BlakeComponents::new(&stmt0, &all_elements, &stmt1);

    // Check that all sums are correct.
    let claimed_sum = stmt1.scheduler_claimed_sum
        + stmt1.round_claimed_sums.iter().sum::<SecureField>()
        + stmt1.xor12_claimed_sum
        + stmt1.xor9_claimed_sum
        + stmt1.xor8_claimed_sum
        + stmt1.xor7_claimed_sum
        + stmt1.xor4_claimed_sum;

    // TODO(shahars): Add inputs to sum, and constraint them.
    assert_eq!(claimed_sum, SecureField::zero());

    verify(
        &components.components(),
        channel,
        commitment_scheme,
        stark_proof,
    )
}

#[cfg(test)]
mod tests {
    use std::env;

    use stwo::core::pcs::PcsConfig;
    use stwo::core::vcs_lifted::blake2_merkle::Blake2sMerkleChannel;

    use crate::blake::air::{prove_blake, verify_blake};

    // Note: this test is slow. Only run in release.
    #[cfg_attr(not(feature = "slow-tests"), ignore)]
    #[test_log::test]
    fn test_simd_blake_prove() {
        // Note: To see time measurement, run test with
        //   LOG_N_INSTANCES=16 RUST_LOG_SPAN_EVENTS=enter,close RUST_LOG=info RUSTFLAGS="
        //   -C target-cpu=native -C target-feature=+avx512f" cargo test --release
        //   test_simd_blake_prove -- --nocapture --ignored

        // Get from environment variable:
        let log_n_instances = env::var("LOG_N_INSTANCES")
            .unwrap_or_else(|_| "6".to_string())
            .parse::<u32>()
            .unwrap();
        let config = PcsConfig::default();

        // Prove.
        let proof = prove_blake::<Blake2sMerkleChannel>(log_n_instances, config);

        // Verify.
        verify_blake::<Blake2sMerkleChannel>(proof).unwrap();
    }
}
