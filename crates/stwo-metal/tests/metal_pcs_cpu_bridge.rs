use itertools::Itertools;
use stwo::core::circle::SECURE_FIELD_CIRCLE_GEN;
use stwo::core::fields::m31::BaseField;
use stwo::core::fields::qm31::SecureField;
use stwo::core::pcs::quotients::{ColumnSampleBatch, NumeratorData};
use stwo::core::poly::circle::CanonicCoset;
use stwo::prover::backend::{Column, CpuBackend};
use stwo::prover::poly::circle::CircleEvaluation;
use stwo::prover::poly::BitReversedOrder;
use stwo::prover::secure_column::SecureColumnByCoords;
use stwo::prover::{AccumulatedNumerators, AccumulationOps, QuotientOps};
use stwo_metal::{metal_runtime_support, MetalBackend, MetalBaseFieldVec, MetalRuntimeSupport};

fn require_metal_runtime() {
    assert_eq!(
        metal_runtime_support(),
        MetalRuntimeSupport::Available,
        "Metal runtime must be available for the native Apple Silicon PCS bridge tests"
    );
}

fn sf(a: u32, b: u32, c: u32, d: u32) -> SecureField {
    SecureField::from_u32_unchecked(a, b, c, d)
}

fn metal_secure_column(values: &[SecureField]) -> SecureColumnByCoords<MetalBackend> {
    let mut coords: [Vec<BaseField>; 4] = std::array::from_fn(|_| Vec::with_capacity(values.len()));
    for value in values {
        for (coord_column, coord) in coords.iter_mut().zip(value.to_m31_array()) {
            coord_column.push(coord);
        }
    }

    SecureColumnByCoords {
        columns: coords.map(MetalBaseFieldVec::from_vec),
    }
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn metal_accumulation_cpu_bridge_matches_cpu() {
    require_metal_runtime();

    let mut metal_left = metal_secure_column(&[
        sf(1, 2, 3, 4),
        sf(5, 6, 7, 8),
        sf(9, 10, 11, 12),
        sf(13, 14, 15, 16),
    ]);
    let metal_right = metal_secure_column(&[
        sf(2, 3, 4, 5),
        sf(6, 7, 8, 9),
        sf(10, 11, 12, 13),
        sf(14, 15, 16, 17),
    ]);

    let mut cpu_left = metal_left.to_cpu();
    let cpu_right = metal_right.to_cpu();

    MetalBackend::accumulate(&mut metal_left, &metal_right);
    CpuBackend::accumulate(&mut cpu_left, &cpu_right);

    assert_eq!(metal_left.to_cpu().to_vec(), cpu_left.to_vec());
    assert_eq!(
        MetalBackend::generate_secure_powers(sf(3, 0, 0, 0), 6),
        CpuBackend::generate_secure_powers(sf(3, 0, 0, 0), 6)
    );

    let metal_lifted = MetalBackend::lift_and_accumulate(vec![
        metal_secure_column(&[
            sf(1, 0, 0, 0),
            sf(2, 0, 0, 0),
            sf(3, 0, 0, 0),
            sf(4, 0, 0, 0),
        ]),
        metal_secure_column(&[
            sf(5, 0, 0, 0),
            sf(6, 0, 0, 0),
            sf(7, 0, 0, 0),
            sf(8, 0, 0, 0),
            sf(9, 0, 0, 0),
            sf(10, 0, 0, 0),
            sf(11, 0, 0, 0),
            sf(12, 0, 0, 0),
        ]),
    ])
    .expect("lift-and-accumulate should produce a result");
    let cpu_lifted = CpuBackend::lift_and_accumulate(vec![
        metal_secure_column(&[
            sf(1, 0, 0, 0),
            sf(2, 0, 0, 0),
            sf(3, 0, 0, 0),
            sf(4, 0, 0, 0),
        ])
        .to_cpu(),
        metal_secure_column(&[
            sf(5, 0, 0, 0),
            sf(6, 0, 0, 0),
            sf(7, 0, 0, 0),
            sf(8, 0, 0, 0),
            sf(9, 0, 0, 0),
            sf(10, 0, 0, 0),
            sf(11, 0, 0, 0),
            sf(12, 0, 0, 0),
        ])
        .to_cpu(),
    ])
    .expect("cpu lift-and-accumulate should produce a result");
    assert_eq!(metal_lifted.to_cpu().to_vec(), cpu_lifted.to_vec());
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn metal_quotient_cpu_bridge_accumulate_numerators_matches_cpu() {
    require_metal_runtime();

    const LOG_SIZE: u32 = 5;
    let domain = CanonicCoset::new(LOG_SIZE).circle_domain();
    let e0: Vec<BaseField> = (0..domain.size()).map(BaseField::from).collect();
    let e1: Vec<BaseField> = (0..domain.size()).map(|i| BaseField::from(2 * i)).collect();

    let metal_columns = vec![
        CircleEvaluation::<MetalBackend, BaseField, BitReversedOrder>::new(
            domain,
            MetalBaseFieldVec::from_vec(e0),
        ),
        CircleEvaluation::<MetalBackend, BaseField, BitReversedOrder>::new(
            domain,
            MetalBaseFieldVec::from_vec(e1),
        ),
    ];
    let cpu_columns = metal_columns
        .iter()
        .map(|column| {
            CircleEvaluation::<CpuBackend, BaseField, BitReversedOrder>::new(
                column.domain,
                column.values.to_cpu(),
            )
        })
        .collect_vec();

    let sample_batches = vec![
        ColumnSampleBatch {
            point: SECURE_FIELD_CIRCLE_GEN,
            cols_vals_randpows: vec![
                NumeratorData {
                    column_index: 0,
                    sample_value: sf(7, 0, 0, 0),
                    random_coeff: sf(3, 0, 0, 0),
                },
                NumeratorData {
                    column_index: 1,
                    sample_value: sf(11, 0, 0, 0),
                    random_coeff: sf(5, 0, 0, 0),
                },
            ],
        },
        ColumnSampleBatch {
            point: SECURE_FIELD_CIRCLE_GEN.double(),
            cols_vals_randpows: vec![NumeratorData {
                column_index: 0,
                sample_value: sf(13, 0, 0, 0),
                random_coeff: sf(17, 0, 0, 0),
            }],
        },
    ];

    let mut metal_accumulated: Vec<AccumulatedNumerators<MetalBackend>> = vec![];
    MetalBackend::accumulate_numerators(
        &metal_columns.iter().collect_vec(),
        &sample_batches,
        &mut metal_accumulated,
    );

    let mut cpu_accumulated: Vec<AccumulatedNumerators<CpuBackend>> = vec![];
    CpuBackend::accumulate_numerators(
        &cpu_columns.iter().collect_vec(),
        &sample_batches,
        &mut cpu_accumulated,
    );

    assert_eq!(metal_accumulated.len(), cpu_accumulated.len());
    for (metal_acc, cpu_acc) in metal_accumulated
        .into_iter()
        .zip(cpu_accumulated.into_iter())
    {
        assert_eq!(metal_acc.sample_point, cpu_acc.sample_point);
        assert_eq!(
            metal_acc.first_linear_term_acc,
            cpu_acc.first_linear_term_acc
        );
        assert_eq!(
            metal_acc.partial_numerators_acc.to_cpu().to_vec(),
            cpu_acc.partial_numerators_acc.to_vec()
        );
    }
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn metal_quotient_cpu_bridge_compute_and_combine_matches_cpu() {
    require_metal_runtime();

    let lifting_log_size = 3;
    let metal_accs = vec![
        AccumulatedNumerators {
            sample_point: SECURE_FIELD_CIRCLE_GEN,
            partial_numerators_acc: metal_secure_column(&[
                sf(1, 2, 3, 4),
                sf(2, 3, 4, 5),
                sf(3, 4, 5, 6),
                sf(4, 5, 6, 7),
                sf(5, 6, 7, 8),
                sf(6, 7, 8, 9),
                sf(7, 8, 9, 10),
                sf(8, 9, 10, 11),
            ]),
            first_linear_term_acc: sf(9, 1, 2, 3),
        },
        AccumulatedNumerators {
            sample_point: SECURE_FIELD_CIRCLE_GEN.double(),
            partial_numerators_acc: metal_secure_column(&[
                sf(12, 13, 14, 15),
                sf(13, 14, 15, 16),
                sf(14, 15, 16, 17),
                sf(15, 16, 17, 18),
            ]),
            first_linear_term_acc: sf(4, 5, 6, 7),
        },
    ];
    let cpu_accs = vec![
        AccumulatedNumerators {
            sample_point: SECURE_FIELD_CIRCLE_GEN,
            partial_numerators_acc: [
                sf(1, 2, 3, 4),
                sf(2, 3, 4, 5),
                sf(3, 4, 5, 6),
                sf(4, 5, 6, 7),
                sf(5, 6, 7, 8),
                sf(6, 7, 8, 9),
                sf(7, 8, 9, 10),
                sf(8, 9, 10, 11),
            ]
            .into_iter()
            .collect(),
            first_linear_term_acc: sf(9, 1, 2, 3),
        },
        AccumulatedNumerators {
            sample_point: SECURE_FIELD_CIRCLE_GEN.double(),
            partial_numerators_acc: [
                sf(12, 13, 14, 15),
                sf(13, 14, 15, 16),
                sf(14, 15, 16, 17),
                sf(15, 16, 17, 18),
            ]
            .into_iter()
            .collect(),
            first_linear_term_acc: sf(4, 5, 6, 7),
        },
    ];

    let metal_eval = MetalBackend::compute_quotients_and_combine(metal_accs, lifting_log_size);
    let cpu_eval = CpuBackend::compute_quotients_and_combine(cpu_accs, lifting_log_size);

    assert_eq!(metal_eval.domain, cpu_eval.domain);
    assert_eq!(
        metal_eval.values.to_cpu().to_vec(),
        cpu_eval.values.to_vec()
    );
}
