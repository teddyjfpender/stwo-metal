#![cfg(all(feature = "prover", target_os = "macos", target_arch = "aarch64"))]

use hashbrown::HashMap;
use stwo::core::fields::m31::BaseField;
use stwo::core::fields::qm31::{SecureField, QM31};
use stwo::core::poly::circle::CanonicCoset;
use stwo::core::queries::Queries;
use stwo::core::vcs_lifted::blake2_merkle::Blake2sMerkleHasher;
use stwo::core::vcs_lifted::verifier::MerkleVerifierLifted;
use stwo::prover::backend::CpuBackend;
use stwo::prover::fri::FriOps;
use stwo::prover::line::LineEvaluation;
use stwo::prover::poly::circle::{PolyOps, SecureEvaluation};
use stwo::prover::poly::BitReversedOrder;
use stwo::prover::vcs_lifted::prover::MerkleProverLifted;
use stwo_metal::{
    metal_runtime_support, MetalLineEvaluation, MetalRuntimeSupport, MetalSecureFieldVec,
};

#[test]
fn metal_native_first_inner_layer_decommit_matches_cpu_reference() {
    assert_eq!(
        metal_runtime_support(),
        MetalRuntimeSupport::Available,
        "Metal runtime must be available for the native Apple Silicon parity test"
    );

    const LOG_SIZE: u32 = 10;
    let circle_domain = CanonicCoset::new(LOG_SIZE).circle_domain();
    let alpha = SecureField::from_u32_unchecked(3, 4, 5, 9);
    let values = (0..(1 << LOG_SIZE))
        .map(|i| {
            SecureField::from_u32_unchecked(
                6 * i as u32,
                6 * i as u32 + 1,
                6 * i as u32 + 2,
                6 * i as u32 + 3,
            )
        })
        .collect::<Vec<_>>();
    let queries = Queries::new(&[1, 2, 7, 8, 14, 15], LOG_SIZE - 1);
    let fold_step = 2;

    let src_cpu = SecureEvaluation::<CpuBackend, BitReversedOrder>::new(
        circle_domain,
        values.clone().into_iter().collect(),
    );
    let twiddles = CpuBackend::precompute_twiddles(circle_domain.half_coset);
    let mut expected_eval = LineEvaluation::<CpuBackend>::new_zero(
        stwo::core::poly::line::LineDomain::new(circle_domain.half_coset),
    );
    CpuBackend::fold_circle_into_line(&mut expected_eval, &src_cpu, alpha, &twiddles);
    let (expected_positions, expected_witness, expected_values) =
        cpu_expected_fri_query_material(&expected_eval, &queries, fold_step);
    let expected_merkle = MerkleProverLifted::<CpuBackend, Blake2sMerkleHasher>::commit(
        expected_eval.values.columns.iter().collect(),
        expected_eval.values.len().ilog2(),
    );
    let (_queried_values, expected_decommitment) = expected_merkle.decommit(
        &expected_positions,
        expected_eval.values.columns.iter().collect(),
    );

    let src_metal = MetalSecureFieldVec::from_vec(values);
    let line_eval = MetalLineEvaluation::from_first_circle_fold(&src_metal, circle_domain, alpha);
    let commitment = line_eval.commitment::<Blake2sMerkleHasher>();
    let actual = commitment.decommit_fri_layer(&line_eval, &queries, fold_step);

    assert_eq!(actual.decommitment_positions, expected_positions);
    assert_eq!(actual.proof.proof.fri_witness, expected_witness);
    assert_eq!(
        actual.proof.proof.decommitment.hash_witness,
        expected_decommitment.decommitment.hash_witness
    );
    assert_eq!(actual.proof.proof.commitment, expected_merkle.root());
    assert_eq!(actual.proof.aux.all_values[0], expected_values);
    assert_eq!(
        actual.proof.aux.decommitment.all_node_values,
        expected_decommitment.aux.all_node_values
    );

    let verifier = MerkleVerifierLifted::<Blake2sMerkleHasher>::new(
        actual.proof.proof.commitment,
        vec![line_eval.domain().log_size(); 4],
        Some(line_eval.domain().log_size()),
    );
    verifier
        .verify(
            &actual.decommitment_positions,
            queried_values_from_map(
                &actual.proof.aux.all_values[0],
                &actual.decommitment_positions,
            ),
            actual.proof.proof.decommitment.clone(),
        )
        .expect("native first-inner-layer decommitment should verify against its root");
}

fn cpu_expected_fri_query_material(
    evaluation: &LineEvaluation<CpuBackend>,
    queries: &Queries,
    fold_step: u32,
) -> (Vec<usize>, Vec<SecureField>, HashMap<usize, QM31>) {
    let mut decommitment_positions = Vec::new();
    let mut fri_witness = Vec::new();
    let mut value_map = HashMap::new();

    for subset_queries in queries
        .positions
        .chunk_by(|a, b| a >> fold_step == b >> fold_step)
    {
        let subset_start = (subset_queries[0] >> fold_step) << fold_step;
        let subset_positions = subset_start..subset_start + (1 << fold_step);
        let mut subset_queries_iter = subset_queries.iter().peekable();

        for position in subset_positions {
            decommitment_positions.push(position);
            let eval = evaluation.values.at(position);
            value_map.insert(position, eval);
            if subset_queries_iter.next_if_eq(&&position).is_none() {
                fri_witness.push(eval);
            }
        }
    }

    (decommitment_positions, fri_witness, value_map)
}

fn queried_values_from_map(
    value_map: &HashMap<usize, QM31>,
    positions: &[usize],
) -> Vec<Vec<BaseField>> {
    let mut columns = vec![Vec::with_capacity(positions.len()); 4];
    for position in positions {
        let value = value_map[position];
        for (column, limb) in columns.iter_mut().zip(value.to_m31_array()) {
            column.push(BaseField::from_u32_unchecked(limb.0));
        }
    }
    columns
}
