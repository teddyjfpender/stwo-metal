#[cfg(not(feature = "vendored-upstream-bridge"))]
use interface::base_field_vec::BaseFieldVec;
#[cfg(not(feature = "vendored-upstream-bridge"))]
use interface::bindings;
#[cfg(not(feature = "vendored-upstream-bridge"))]
use interface::bindings::{CirclePointSecureField, CudaSecureField};
#[cfg(not(feature = "vendored-upstream-bridge"))]
use itertools::Itertools;
#[cfg(feature = "vendored-upstream-bridge")]
use itertools::Itertools;
use stwo::core::fields::m31::BaseField;
#[cfg(not(feature = "vendored-upstream-bridge"))]
use stwo::core::fields::qm31::SecureField;
#[cfg(feature = "vendored-upstream-bridge")]
use stwo::core::pcs::quotients::{quotient_constants, ColumnSampleBatch};
#[cfg(feature = "vendored-upstream-bridge")]
use stwo::core::poly::circle::CanonicCoset;
#[cfg(not(feature = "vendored-upstream-bridge"))]
use stwo::core::poly::circle::CircleDomain;
#[cfg(feature = "vendored-upstream-bridge")]
use stwo::prover::backend::Column;
use stwo::prover::poly::circle::CircleEvaluation;
#[cfg(not(feature = "vendored-upstream-bridge"))]
use stwo::prover::poly::circle::SecureEvaluation;
#[cfg(feature = "vendored-upstream-bridge")]
use stwo::prover::poly::circle::SecureEvaluation;
use stwo::prover::poly::BitReversedOrder;
use stwo::prover::secure_column::SecureColumnByCoords;
#[cfg(feature = "vendored-upstream-bridge")]
use stwo::prover::AccumulatedNumerators;
use stwo::prover::QuotientOps;

use crate::backend::cuda::CudaBackend;
#[cfg(feature = "vendored-upstream-bridge")]
use crate::backend::cuda::UploadedDevicePointerVec;
#[cfg(not(feature = "vendored-upstream-bridge"))]
use crate::stwo_cuda as interface;
#[cfg(feature = "vendored-upstream-bridge")]
use crate::stwo_cuda::bindings::{self, CirclePointSecureField, CudaSecureField};

#[cfg(not(feature = "vendored-upstream-bridge"))]
impl QuotientOps for CudaBackend {
    fn accumulate_quotients(
        domain: CircleDomain,
        columns: &[&CircleEvaluation<Self, BaseField, BitReversedOrder>],
        random_coeff: SecureField,
        sample_batches: &[ColumnSampleBatch],
        _log_blowup_factor: u32,
    ) -> SecureEvaluation<Self, BitReversedOrder> {
        // Handle empty sample_batches to avoid CUDA kernel launch with num_blocks=0
        if sample_batches.is_empty() {
            // Return zero evaluation when there are no samples
            return SecureEvaluation::new(
                domain,
                SecureColumnByCoords {
                    columns: [
                        BaseFieldVec::new_zeroes(domain.size()),
                        BaseFieldVec::new_zeroes(domain.size()),
                        BaseFieldVec::new_zeroes(domain.size()),
                        BaseFieldVec::new_zeroes(domain.size()),
                    ],
                },
            );
        }

        let domain_size = domain.size();
        let number_of_columns = columns.len();

        let result: SecureEvaluation<Self, BitReversedOrder> = SecureEvaluation::new(
            domain,
            SecureColumnByCoords {
                columns: [
                    BaseFieldVec::new_uninitialized(domain_size),
                    BaseFieldVec::new_uninitialized(domain_size),
                    BaseFieldVec::new_uninitialized(domain_size),
                    BaseFieldVec::new_uninitialized(domain_size),
                ],
            },
        );
        let device_column_pointers_vector = columns
            .iter()
            .map(|column| column.values.device_ptr)
            .collect_vec();
        unsafe {
            let half_coset_initial_index = domain.half_coset.initial_index;
            let half_coset_step_size = domain.half_coset.step_size;

            let device_column_pointers =
                UploadedDevicePointerVec::upload(&device_column_pointers_vector);

            let sample_points: Vec<CirclePointSecureField> = sample_batches
                .iter()
                .map(|column_sample_batch| column_sample_batch.point.into())
                .collect();

            let sample_column_indexes: Vec<u32> = sample_batches
                .iter()
                .flat_map(|column_sample_batch| {
                    column_sample_batch
                        .columns_and_values
                        .iter()
                        .map(|(column, _)| *column as u32)
                        .collect_vec()
                })
                .collect_vec();

            let sample_column_and_values_sizes: Vec<u32> = sample_batches
                .iter()
                .map(|column_sample_batch| column_sample_batch.columns_and_values.len() as u32)
                .collect_vec();

            let sample_column_values: Vec<CudaSecureField> = sample_batches
                .iter()
                .flat_map(|column_sample_batch| {
                    column_sample_batch
                        .columns_and_values
                        .iter()
                        .map(|(_, value)| (*value).into())
                        .collect_vec()
                })
                .collect_vec();

            let flattened_line_coeffs_size = sample_column_indexes.len() * 3;

            bindings::accumulate_quotients(
                half_coset_initial_index.0 as u32,
                half_coset_step_size.0 as u32,
                domain_size as u32,
                device_column_pointers.as_ptr(),
                number_of_columns,
                random_coeff.into(),
                sample_points.as_ptr() as *const u32,
                sample_column_indexes.as_ptr(),
                sample_column_indexes.len() as u32,
                sample_column_values.as_ptr(),
                sample_column_and_values_sizes.as_ptr(),
                sample_points.len() as u32,
                result.values.columns[0].device_ptr,
                result.values.columns[1].device_ptr,
                result.values.columns[2].device_ptr,
                result.values.columns[3].device_ptr,
                flattened_line_coeffs_size as u32,
            );
        }
        result
    }
}

#[cfg(feature = "vendored-upstream-bridge")]
impl QuotientOps for CudaBackend {
    fn accumulate_numerators(
        columns: &[&CircleEvaluation<Self, BaseField, BitReversedOrder>],
        sample_batches: &[ColumnSampleBatch],
        accumulated_numerators_vec: &mut Vec<AccumulatedNumerators<Self>>,
    ) {
        if columns.is_empty() || sample_batches.is_empty() {
            return;
        }

        let size = columns[0].len();
        let quotient_constants = quotient_constants(sample_batches);
        let device_column_pointers_vector = columns
            .iter()
            .map(|column| column.values.device_ptr)
            .collect_vec();
        let device_column_pointers =
            UploadedDevicePointerVec::upload(&device_column_pointers_vector);

        for (batch, coeffs) in sample_batches.iter().zip(quotient_constants.line_coeffs) {
            if batch.cols_vals_randpows.is_empty() {
                accumulated_numerators_vec.push(AccumulatedNumerators {
                    sample_point: batch.point,
                    partial_numerators_acc: SecureColumnByCoords::zeros(size),
                    first_linear_term_acc: coeffs.into_iter().map(|(a, ..)| a).sum(),
                });
                continue;
            }

            let sample_column_indexes = batch
                .cols_vals_randpows
                .iter()
                .map(|data| data.column_index as u32)
                .collect_vec();
            let line_coeffs_b = coeffs
                .iter()
                .map(|(_, b, _)| CudaSecureField::from(*b))
                .collect_vec();
            let line_coeffs_c = coeffs
                .iter()
                .map(|(_, _, c)| CudaSecureField::from(*c))
                .collect_vec();
            let first_linear_term_acc = coeffs.iter().map(|(a, ..)| *a).sum();
            let partial_numerators_acc: SecureColumnByCoords<CudaBackend> =
                unsafe { SecureColumnByCoords::uninitialized(size) };

            unsafe {
                bindings::accumulate_partial_quotient_numerators(
                    size as u32,
                    device_column_pointers.as_ptr(),
                    sample_column_indexes.as_ptr(),
                    sample_column_indexes.len() as u32,
                    line_coeffs_b.as_ptr(),
                    line_coeffs_c.as_ptr(),
                    partial_numerators_acc.columns[0].device_ptr,
                    partial_numerators_acc.columns[1].device_ptr,
                    partial_numerators_acc.columns[2].device_ptr,
                    partial_numerators_acc.columns[3].device_ptr,
                );
            }

            accumulated_numerators_vec.push(AccumulatedNumerators {
                sample_point: batch.point,
                partial_numerators_acc,
                first_linear_term_acc,
            });
        }
    }

    fn compute_quotients_and_combine(
        accs: Vec<AccumulatedNumerators<Self>>,
        lifting_log_size: u32,
    ) -> SecureEvaluation<Self, BitReversedOrder> {
        let domain = CanonicCoset::new(lifting_log_size).circle_domain();
        let domain_size = 1usize << lifting_log_size;

        if accs.is_empty() {
            return SecureEvaluation::new(domain, SecureColumnByCoords::zeros(domain_size));
        }

        let sample_points = accs
            .iter()
            .map(|acc| CirclePointSecureField::from(acc.sample_point))
            .collect_vec();
        let first_linear_term_accs = accs
            .iter()
            .map(|acc| CudaSecureField::from(acc.first_linear_term_acc))
            .collect_vec();
        let partial_numerator_log_sizes = accs
            .iter()
            .map(|acc| {
                let log_size = acc.partial_numerators_acc.len().ilog2();
                assert!(
                    log_size <= lifting_log_size,
                    "partial numerator log size {log_size} exceeds lifting log size {lifting_log_size}"
                );
                log_size
            })
            .collect_vec();
        let partial_numerator_column_ptrs: [Vec<*const u32>; 4] = std::array::from_fn(|coord| {
            accs.iter()
                .map(|acc| acc.partial_numerators_acc.columns[coord].device_ptr)
                .collect_vec()
        });
        let uploaded_partial_numerator_columns = partial_numerator_column_ptrs
            .each_ref()
            .map(|host_ptrs| UploadedDevicePointerVec::upload(host_ptrs.as_slice()));

        let quotients: SecureColumnByCoords<CudaBackend> =
            unsafe { SecureColumnByCoords::uninitialized(domain_size) };

        unsafe {
            bindings::combine_quotients_from_numerators(
                domain.half_coset.initial_index.0 as u32,
                domain.half_coset.step_size.0 as u32,
                domain_size as u32,
                lifting_log_size,
                sample_points.as_ptr(),
                sample_points.len() as u32,
                first_linear_term_accs.as_ptr(),
                partial_numerator_log_sizes.as_ptr(),
                uploaded_partial_numerator_columns[0].as_ptr(),
                uploaded_partial_numerator_columns[1].as_ptr(),
                uploaded_partial_numerator_columns[2].as_ptr(),
                uploaded_partial_numerator_columns[3].as_ptr(),
                quotients.columns[0].device_ptr,
                quotients.columns[1].device_ptr,
                quotients.columns[2].device_ptr,
                quotients.columns[3].device_ptr,
            );
        }

        SecureEvaluation::new(domain, quotients)
    }
}

#[cfg(all(test, stwo_cuda_link, not(feature = "vendored-upstream-bridge")))]
mod tests {
    use itertools::Itertools;
    use num_traits::Zero;
    use stwo::core::circle::SECURE_FIELD_CIRCLE_GEN;
    use stwo::core::fields::m31::{BaseField, M31};
    use stwo::core::fields::qm31::QM31;
    use stwo::core::pcs::quotients::ColumnSampleBatch;
    use stwo::core::poly::circle::CanonicCoset;
    use stwo::prover::backend::simd::column::BaseColumn;
    use stwo::prover::backend::{Column, CpuBackend};
    use stwo::prover::poly::circle::CircleEvaluation;
    use stwo::prover::poly::BitReversedOrder;
    use stwo::prover::QuotientOps;

    use crate::backend::cuda::CudaBackend;
    use crate::stwo_cuda::base_field_vec::BaseFieldVec;
    #[test]
    fn test_accumulate_quotients_compared_with_cpu() {
        const LOG_SIZE: u32 = 5;
        const LOG_BLOWUP_FACTOR: u32 = 1;
        let small_domain = CanonicCoset::new(LOG_SIZE).circle_domain();
        let domain = CanonicCoset::new(LOG_SIZE + LOG_BLOWUP_FACTOR).circle_domain();
        let e0: BaseColumn = (0..small_domain.size()).map(BaseField::from).collect();
        let e1: BaseColumn = (0..small_domain.size())
            .map(|i| BaseField::from(2 * i))
            .collect();
        let polys = vec![
            CircleEvaluation::<CudaBackend, BaseField, BitReversedOrder>::new(
                small_domain,
                BaseFieldVec::from_vec(e0.to_cpu()),
            )
            .interpolate(),
            CircleEvaluation::<CudaBackend, BaseField, BitReversedOrder>::new(
                small_domain,
                BaseFieldVec::from_vec(e1.to_cpu()),
            )
            .interpolate(),
        ];
        let columns = vec![polys[0].evaluate(domain), polys[1].evaluate(domain)];
        let random_coeff = QM31::from_m31(M31::from(1), M31::from(2), M31::from(3), M31::from(4));
        let a = polys[0].eval_at_point(SECURE_FIELD_CIRCLE_GEN);
        let b = polys[1].eval_at_point(SECURE_FIELD_CIRCLE_GEN);
        let samples = vec![
            ColumnSampleBatch {
                point: SECURE_FIELD_CIRCLE_GEN,
                columns_and_values: vec![(0, a), (1, b)],
            },
            ColumnSampleBatch {
                point: SECURE_FIELD_CIRCLE_GEN,
                columns_and_values: vec![(0, a), (1, b)],
            },
            ColumnSampleBatch {
                point: SECURE_FIELD_CIRCLE_GEN,
                columns_and_values: vec![(0, a), (1, b)],
            },
        ];
        let cpu_columns = columns
            .iter()
            .map(|c| {
                CircleEvaluation::<CpuBackend, _, BitReversedOrder>::new(
                    c.domain,
                    c.values.to_cpu(),
                )
            })
            .collect_vec();

        let cpu_result = CpuBackend::accumulate_quotients(
            domain,
            &cpu_columns.iter().collect_vec(),
            random_coeff,
            &samples,
            LOG_BLOWUP_FACTOR,
        )
        .values
        .to_vec();

        let gpu_result = CudaBackend::accumulate_quotients(
            domain,
            &columns.iter().collect_vec(),
            random_coeff,
            &samples,
            LOG_BLOWUP_FACTOR,
        )
        .values
        .to_cpu()
        .to_vec();

        assert_eq!(gpu_result, cpu_result);
    }

    #[test]
    fn test_accumulate_quotients_compared_with_cpu_expend() {
        let log_size: u32 = 6;
        let log_blowup_factor: u32 = 1;
        let num_columns: usize = 4;

        let small_domain = CanonicCoset::new(log_size).circle_domain();
        let domain = CanonicCoset::new(log_size + log_blowup_factor).circle_domain();

        let base_columns: Vec<BaseColumn> = (0..num_columns)
            .map(|i| {
                (0..small_domain.size())
                    .map(|j| BaseField::from((i + 1) * j))
                    .collect()
            })
            .collect();

        let polys: Vec<_> = base_columns
            .iter()
            .map(|col| {
                CircleEvaluation::<CudaBackend, BaseField, BitReversedOrder>::new(
                    small_domain,
                    BaseFieldVec::from_vec(col.to_cpu()),
                )
                .interpolate()
            })
            .collect();

        let columns: Vec<_> = polys.iter().map(|poly| poly.evaluate(domain)).collect();

        let random_coeff = QM31::from_m31(
            M31::from(1208161154),
            M31::from(1460422684),
            M31::from(150901284),
            M31::from(373213585),
        );

        let samples: Vec<ColumnSampleBatch> = (0..num_columns)
            .map(|i| {
                let point = SECURE_FIELD_CIRCLE_GEN;
                let value = polys[i].eval_at_point(point);
                ColumnSampleBatch {
                    point,
                    columns_and_values: vec![(i, value)],
                }
            })
            .collect();

        let cpu_columns: Vec<_> = columns
            .iter()
            .map(|c| {
                CircleEvaluation::<CpuBackend, _, BitReversedOrder>::new(
                    c.domain,
                    c.values.to_cpu(),
                )
            })
            .collect();

        let cpu_result = CpuBackend::accumulate_quotients(
            domain,
            &cpu_columns.iter().collect_vec(),
            random_coeff,
            &samples,
            log_blowup_factor,
        )
        .values
        .to_vec();

        let gpu_result = CudaBackend::accumulate_quotients(
            domain,
            &columns.iter().collect_vec(),
            random_coeff,
            &samples,
            log_blowup_factor,
        )
        .values
        .to_cpu()
        .to_vec();

        assert_eq!(gpu_result, cpu_result);

        let log_size: u32 = 4;
        let log_blowup_factor: u32 = 1;
        let num_columns: usize = 325;

        let small_domain = CanonicCoset::new(log_size).circle_domain();
        let domain = CanonicCoset::new(log_size + log_blowup_factor).circle_domain();

        let base_columns: Vec<BaseColumn> = (0..num_columns)
            .map(|i| {
                (0..small_domain.size())
                    .map(|j| {
                        if (317..320).contains(&i) {
                            BaseField::zero()
                        } else if (321..324).contains(&i) {
                            BaseField::zero()
                        } else {
                            BaseField::from((i + 1) * j)
                        }
                    })
                    .collect()
            })
            .collect();

        let polys: Vec<_> = base_columns
            .iter()
            .map(|col| {
                CircleEvaluation::<CudaBackend, BaseField, BitReversedOrder>::new(
                    small_domain,
                    BaseFieldVec::from_vec(col.to_cpu()),
                )
                .interpolate()
            })
            .collect();

        let columns: Vec<_> = polys.iter().map(|poly| poly.evaluate(domain)).collect();

        let random_coeff = QM31::from_m31(
            M31::from(1208161154),
            M31::from(1460422684),
            M31::from(150901284),
            M31::from(373213585),
        );

        let samples: Vec<ColumnSampleBatch> = (0..num_columns)
            .map(|i| {
                let point = SECURE_FIELD_CIRCLE_GEN;
                let value = polys[i].eval_at_point(point);
                ColumnSampleBatch {
                    point,
                    columns_and_values: vec![(i, value)],
                }
            })
            .collect();

        let cpu_columns: Vec<_> = columns
            .iter()
            .map(|c| {
                CircleEvaluation::<CpuBackend, _, BitReversedOrder>::new(
                    c.domain,
                    c.values.to_cpu(),
                )
            })
            .collect();

        let cpu_result = CpuBackend::accumulate_quotients(
            domain,
            &cpu_columns.iter().collect_vec(),
            random_coeff,
            &samples,
            log_blowup_factor,
        )
        .values
        .to_vec();

        let gpu_result = CudaBackend::accumulate_quotients(
            domain,
            &columns.iter().collect_vec(),
            random_coeff,
            &samples,
            log_blowup_factor,
        )
        .values
        .to_cpu()
        .to_vec();

        assert_eq!(gpu_result, cpu_result);
    }

    #[test]
    fn test_accumulate_quotients_log24() {
        // Test accumulate_quotients at log_size=24 to check for size-related issues
        const LOG_SIZE: u32 = 24;
        const LOG_BLOWUP_FACTOR: u32 = 1;
        let small_domain = CanonicCoset::new(LOG_SIZE).circle_domain();
        let domain = CanonicCoset::new(LOG_SIZE + LOG_BLOWUP_FACTOR).circle_domain();

        // Create a simple column with pattern values
        let e0: Vec<BaseField> = (0..small_domain.size())
            .map(|i| BaseField::from(i as u32))
            .collect();

        let poly = CircleEvaluation::<CudaBackend, BaseField, BitReversedOrder>::new(
            small_domain,
            BaseFieldVec::from_vec(e0.clone()),
        )
        .interpolate();

        let column = poly.evaluate(domain);
        let columns = vec![column];

        let random_coeff = QM31::from_m31(M31::from(1), M31::from(2), M31::from(3), M31::from(4));
        let sample_value = poly.eval_at_point(SECURE_FIELD_CIRCLE_GEN);
        let samples = vec![ColumnSampleBatch {
            point: SECURE_FIELD_CIRCLE_GEN,
            columns_and_values: vec![(0, sample_value)],
        }];

        // CPU reference
        let cpu_column = CircleEvaluation::<CpuBackend, _, BitReversedOrder>::new(
            domain,
            columns[0].values.to_cpu(),
        );
        let cpu_columns = vec![cpu_column];

        let cpu_result = CpuBackend::accumulate_quotients(
            domain,
            &cpu_columns.iter().collect_vec(),
            random_coeff,
            &samples,
            LOG_BLOWUP_FACTOR,
        )
        .values
        .to_vec();

        let gpu_result = CudaBackend::accumulate_quotients(
            domain,
            &columns.iter().collect_vec(),
            random_coeff,
            &samples,
            LOG_BLOWUP_FACTOR,
        )
        .values
        .to_cpu()
        .to_vec();

        // Check first 1000 elements for quick verification
        assert_eq!(
            gpu_result[..1000],
            cpu_result[..1000],
            "First 1000 elements mismatch"
        );
        // Check last 1000 elements
        let len = gpu_result.len();
        assert_eq!(
            gpu_result[len - 1000..],
            cpu_result[len - 1000..],
            "Last 1000 elements mismatch"
        );
        // Full equality check
        assert_eq!(gpu_result, cpu_result);
    }
}
