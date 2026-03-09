#![cfg(all(stwo_cuda_link, feature = "vendored-upstream-bridge"))]

use stwo::core::circle::SECURE_FIELD_CIRCLE_GEN;
use stwo::core::fields::m31::BaseField;
use stwo::core::fields::qm31::SecureField;
use stwo::prover::backend::CpuBackend;
use stwo::prover::secure_column::SecureColumnByCoords;
use stwo::prover::{AccumulatedNumerators, QuotientOps};
use stwo_metal::{BaseFieldVec, CudaBackend};

fn sf(a: u32, b: u32, c: u32, d: u32) -> SecureField {
    SecureField::from_u32_unchecked(a, b, c, d)
}

fn gpu_secure_column(values: &[SecureField]) -> SecureColumnByCoords<CudaBackend> {
    let mut coords: [Vec<BaseField>; 4] = std::array::from_fn(|_| Vec::with_capacity(values.len()));
    for value in values {
        for (coord_column, coord) in coords.iter_mut().zip(value.to_m31_array()) {
            coord_column.push(coord);
        }
    }

    SecureColumnByCoords {
        columns: coords.map(BaseFieldVec::from_vec),
    }
}

#[test]
fn test_compute_quotients_and_combine_compared_with_cpu() {
    let lifting_log_size = 3;
    let gpu_accs = vec![
        AccumulatedNumerators {
            sample_point: SECURE_FIELD_CIRCLE_GEN,
            partial_numerators_acc: gpu_secure_column(&[
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
            partial_numerators_acc: gpu_secure_column(&[
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

    let gpu_eval = CudaBackend::compute_quotients_and_combine(gpu_accs, lifting_log_size);
    let cpu_eval = CpuBackend::compute_quotients_and_combine(cpu_accs, lifting_log_size);

    assert_eq!(gpu_eval.domain, cpu_eval.domain);
    assert_eq!(gpu_eval.values.to_cpu().to_vec(), cpu_eval.values.to_vec());
}
