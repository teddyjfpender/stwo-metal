#![cfg(all(stwo_cuda_link, feature = "vendored-upstream-bridge"))]

use itertools::Itertools;
use stwo::core::circle::SECURE_FIELD_CIRCLE_GEN;
use stwo::core::fields::m31::BaseField;
use stwo::core::fields::qm31::SecureField;
use stwo::core::pcs::quotients::{ColumnSampleBatch, NumeratorData};
use stwo::core::poly::circle::CanonicCoset;
use stwo::prover::backend::simd::column::BaseColumn;
use stwo::prover::backend::{Column, CpuBackend};
use stwo::prover::poly::circle::CircleEvaluation;
use stwo::prover::poly::BitReversedOrder;
use stwo::prover::{AccumulatedNumerators, QuotientOps};
use stwo_metal::{BaseFieldVec, CudaBackend};

#[test]
fn test_accumulate_numerators_compared_with_cpu() {
    const LOG_SIZE: u32 = 5;
    let domain = CanonicCoset::new(LOG_SIZE).circle_domain();
    let e0: BaseColumn = (0..domain.size()).map(BaseField::from).collect();
    let e1: BaseColumn = (0..domain.size()).map(|i| BaseField::from(2 * i)).collect();

    let gpu_columns = vec![
        CircleEvaluation::<CudaBackend, BaseField, BitReversedOrder>::new(
            domain,
            BaseFieldVec::from_vec(e0.to_cpu()),
        ),
        CircleEvaluation::<CudaBackend, BaseField, BitReversedOrder>::new(
            domain,
            BaseFieldVec::from_vec(e1.to_cpu()),
        ),
    ];
    let cpu_columns = gpu_columns
        .iter()
        .map(|column| {
            CircleEvaluation::<CpuBackend, _, BitReversedOrder>::new(
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
                    sample_value: SecureField::from_u32_unchecked(7, 0, 0, 0),
                    random_coeff: SecureField::from_u32_unchecked(3, 0, 0, 0),
                },
                NumeratorData {
                    column_index: 1,
                    sample_value: SecureField::from_u32_unchecked(11, 0, 0, 0),
                    random_coeff: SecureField::from_u32_unchecked(5, 0, 0, 0),
                },
            ],
        },
        ColumnSampleBatch {
            point: SECURE_FIELD_CIRCLE_GEN.double(),
            cols_vals_randpows: vec![NumeratorData {
                column_index: 0,
                sample_value: SecureField::from_u32_unchecked(13, 0, 0, 0),
                random_coeff: SecureField::from_u32_unchecked(17, 0, 0, 0),
            }],
        },
    ];

    let mut gpu_accumulated: Vec<AccumulatedNumerators<CudaBackend>> = vec![];
    CudaBackend::accumulate_numerators(
        &gpu_columns.iter().collect_vec(),
        &sample_batches,
        &mut gpu_accumulated,
    );

    let mut cpu_accumulated: Vec<AccumulatedNumerators<CpuBackend>> = vec![];
    CpuBackend::accumulate_numerators(
        &cpu_columns.iter().collect_vec(),
        &sample_batches,
        &mut cpu_accumulated,
    );

    assert_eq!(gpu_accumulated.len(), cpu_accumulated.len());
    for (gpu_acc, cpu_acc) in gpu_accumulated.into_iter().zip(cpu_accumulated.into_iter()) {
        assert_eq!(gpu_acc.sample_point, cpu_acc.sample_point);
        assert_eq!(gpu_acc.first_linear_term_acc, cpu_acc.first_linear_term_acc);
        assert_eq!(
            gpu_acc.partial_numerators_acc.to_cpu().to_vec(),
            cpu_acc.partial_numerators_acc.to_vec()
        );
    }
}
