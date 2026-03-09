use stwo::core::vcs::blake2_hash::Blake2sHash;
#[cfg(not(feature = "vendored-upstream-bridge"))]
use stwo::core::vcs::blake2_merkle::Blake2sMerkleHasher;
#[cfg(feature = "vendored-upstream-bridge")]
use stwo::core::vcs_lifted::blake2_merkle::Blake2sMerkleHasher;
#[cfg(feature = "vendored-upstream-bridge")]
use stwo::prover::backend::Column;
use stwo::prover::backend::ColumnOps;
#[cfg(not(feature = "vendored-upstream-bridge"))]
use stwo::prover::vcs::ops::MerkleOps;
#[cfg(feature = "vendored-upstream-bridge")]
use stwo::prover::vcs_lifted::ops::MerkleOpsLifted;

use crate::backend::cuda::capability::{panic_for_unsupported_surface, CudaBackendSurface};
use crate::backend::cuda::{CudaBackend, UploadedDevicePointerVec, UploadedUint32Vec};
use crate::stwo_cuda::base_field_vec::BaseFieldVec;
use crate::stwo_cuda::bindings;
use crate::stwo_cuda::blake_2s_hash_vec::Blake2sHashVec;

impl ColumnOps<Blake2sHash> for CudaBackend {
    type Column = Blake2sHashVec;

    fn bit_reverse_column(_column: &mut Self::Column) {
        panic_for_unsupported_surface(
            CudaBackendSurface::Blake2sHashColumnBitReverse,
            "ColumnOps<Blake2sHash>::bit_reverse_column",
        )
    }
}

#[cfg(not(feature = "vendored-upstream-bridge"))]
impl MerkleOps<Blake2sMerkleHasher> for CudaBackend {
    fn commit_on_layer(
        log_size: u32,
        prev_layer: Option<&Blake2sHashVec>,
        columns: &[&BaseFieldVec],
    ) -> Blake2sHashVec {
        let size = 1 << log_size;
        let number_of_columns = columns.len();

        let result: Blake2sHashVec = Blake2sHashVec::new_uninitialized(size);
        unsafe {
            Self::commit_on_layer_using_gpu(
                size,
                number_of_columns,
                columns,
                prev_layer,
                result.device_ptr,
            );
        }

        result
    }
}

#[cfg(feature = "vendored-upstream-bridge")]
impl MerkleOpsLifted<Blake2sMerkleHasher> for CudaBackend {
    fn build_leaves(columns: &[&BaseFieldVec], lifting_log_size: u32) -> Blake2sHashVec {
        if columns.is_empty() {
            let hasher = Blake2sMerkleHasher::default();
            return Blake2sHashVec::from_vec(vec![hasher.finalize()]);
        }

        assert!(lifting_log_size < usize::BITS);
        assert!(columns[0].len() >= 2, "A column must be of length >= 2.");

        let mut previous_len = columns[0].len();
        let mut column_log_sizes = Vec::with_capacity(columns.len());
        for column in columns {
            let len = column.len();
            assert!(
                len.is_power_of_two(),
                "column lengths must be powers of two"
            );
            assert!(len >= 2, "A column must be of length >= 2.");
            assert!(
                previous_len <= len,
                "lifted Blake2s columns must be sorted increasingly by length"
            );
            let log_size = len.ilog2();
            assert!(
                log_size <= lifting_log_size,
                "lifting_log_size must be at least the largest column log size"
            );
            column_log_sizes.push(log_size);
            previous_len = len;
        }

        let size = 1usize << lifting_log_size;
        let result = Blake2sHashVec::new_uninitialized(size);
        unsafe {
            Self::commit_on_first_layer_lifted_using_gpu(
                columns,
                &column_log_sizes,
                lifting_log_size,
                result.device_ptr,
            );
        }
        result
    }

    fn build_next_layer(prev_layer: &Blake2sHashVec) -> Blake2sHashVec {
        let size = prev_layer.len() / 2;
        let result = Blake2sHashVec::new_uninitialized(size);

        unsafe {
            Self::commit_on_layer_using_gpu(size, 0, &[], Some(prev_layer), result.device_ptr);
        }

        result
    }
}

impl CudaBackend {
    unsafe fn commit_on_first_layer_lifted_using_gpu(
        columns: &[&BaseFieldVec],
        column_log_sizes: &[u32],
        lifting_log_size: u32,
        result_pointer: *const Blake2sHash,
    ) {
        let size = 1usize << lifting_log_size;
        let device_column_pointers_vector: Vec<*const u32> =
            columns.iter().map(|column| column.device_ptr).collect();
        let device_column_pointers =
            UploadedDevicePointerVec::upload(&device_column_pointers_vector);
        let uploaded_column_log_sizes = UploadedUint32Vec::upload(column_log_sizes);

        bindings::commit_on_first_layer_lifted(
            size,
            columns.len(),
            device_column_pointers.as_ptr(),
            uploaded_column_log_sizes.as_ptr(),
            lifting_log_size,
            result_pointer as *mut Blake2sHash,
        );
    }

    unsafe fn commit_on_layer_using_gpu(
        size: usize,
        number_of_columns: usize,
        columns: &[&BaseFieldVec],
        prev_layer: Option<&Blake2sHashVec>,
        result_pointer: *const Blake2sHash,
    ) {
        let device_column_pointers_vector: Vec<*const u32> =
            columns.iter().map(|column| column.device_ptr).collect();

        let device_column_pointers =
            UploadedDevicePointerVec::upload(&device_column_pointers_vector);

        if let Some(previous_layer) = prev_layer {
            bindings::commit_on_layer_with_previous(
                size,
                number_of_columns,
                device_column_pointers.as_ptr(),
                previous_layer.device_ptr,
                result_pointer as *mut Blake2sHash,
            );
        } else {
            bindings::commit_on_first_layer(
                size,
                number_of_columns,
                device_column_pointers.as_ptr(),
                result_pointer as *mut Blake2sHash,
            );
        }
    }
}

#[cfg(all(test, stwo_cuda_link, not(feature = "vendored-upstream-bridge")))]
mod tests {
    use stwo::core::fields::m31::{BaseField, M31};
    use stwo::core::vcs::blake2_merkle::Blake2sMerkleHasher;
    use stwo::prover::backend::{Column, CpuBackend};
    use stwo::prover::vcs::ops::MerkleOps;

    use crate::backend::cuda::CudaBackend;
    use crate::stwo_cuda::base_field_vec::BaseFieldVec;
    use crate::stwo_cuda::blake_2s_hash_vec::Blake2sHashVec;

    #[test]
    fn test_commit_on_first_layer_with_many_columns_compared_with_cpu() {
        let log_size = 16;
        let size = 1 << log_size;

        let cpu_columns_vector: Vec<Vec<BaseField>> = columns_test_vector(100, size);
        let gpu_columns_vector: Vec<BaseFieldVec> = gpu_columns_from(&cpu_columns_vector);

        let expected_result = <CpuBackend as MerkleOps<Blake2sMerkleHasher>>::commit_on_layer(
            log_size,
            None,
            &cpu_columns_vector.iter().collect::<Vec<_>>(),
        );
        let result: Blake2sHashVec =
            <CudaBackend as MerkleOps<Blake2sMerkleHasher>>::commit_on_layer(
                log_size,
                None,
                &gpu_columns_vector.iter().collect::<Vec<_>>(),
            );

        assert_eq!(result.to_cpu(), expected_result);
    }

    #[test]
    fn test_commit_on_layer_with_previous_layer_compared_with_cpu() {
        let current_layer_log_size = 10;
        let current_layer_size = 1 << current_layer_log_size;
        let previous_layer_log_size = current_layer_log_size + 1;
        let previous_layer_size = 1 << previous_layer_log_size;

        // First layer

        let cpu_columns_vector: Vec<Vec<BaseField>> = columns_test_vector(35, previous_layer_size);
        let gpu_columns_vector: Vec<BaseFieldVec> = gpu_columns_from(&cpu_columns_vector);

        let cpu_previous_layer = <CpuBackend as MerkleOps<Blake2sMerkleHasher>>::commit_on_layer(
            previous_layer_log_size,
            None,
            &cpu_columns_vector.iter().collect::<Vec<_>>(),
        );
        let gpu_previous_layer: Blake2sHashVec =
            <CudaBackend as MerkleOps<Blake2sMerkleHasher>>::commit_on_layer(
                previous_layer_log_size,
                None,
                &gpu_columns_vector.iter().collect::<Vec<_>>(),
            );

        // Current layer

        let cpu_columns_vector: Vec<Vec<BaseField>> = columns_test_vector(16, current_layer_size);
        let gpu_columns_vector: Vec<BaseFieldVec> = gpu_columns_from(&cpu_columns_vector);

        let expected_result = <CpuBackend as MerkleOps<Blake2sMerkleHasher>>::commit_on_layer(
            current_layer_log_size,
            Some(&cpu_previous_layer),
            &cpu_columns_vector.iter().collect::<Vec<_>>(),
        );
        let result: Blake2sHashVec =
            <CudaBackend as MerkleOps<Blake2sMerkleHasher>>::commit_on_layer(
                current_layer_log_size,
                Some(&gpu_previous_layer),
                &gpu_columns_vector.iter().collect::<Vec<_>>(),
            );

        assert_eq!(result.to_cpu(), expected_result);
    }

    fn gpu_columns_from(columns: &Vec<Vec<BaseField>>) -> Vec<BaseFieldVec> {
        columns
            .clone()
            .into_iter()
            .map(|vector| BaseFieldVec::from_vec(vector))
            .collect()
    }

    fn columns_test_vector(
        number_of_columns: usize,
        size_of_columns: usize,
    ) -> Vec<Vec<BaseField>> {
        (0..number_of_columns)
            .map(|index_of_column| {
                (0..size_of_columns)
                    .map(|index_in_column| M31::from(index_in_column * index_of_column))
                    .collect()
            })
            .collect()
    }

    #[test]
    fn test_commit_on_first_layer_log24() {
        // Test at log_size=24 to check for size-related issues
        let log_size = 24u32;
        let size = 1 << log_size;

        // Use fewer columns to save memory - just 4 columns
        let cpu_columns_vector: Vec<Vec<BaseField>> = columns_test_vector(4, size);
        let gpu_columns_vector: Vec<BaseFieldVec> = gpu_columns_from(&cpu_columns_vector);

        let expected_result = <CpuBackend as MerkleOps<Blake2sMerkleHasher>>::commit_on_layer(
            log_size,
            None,
            &cpu_columns_vector.iter().collect::<Vec<_>>(),
        );
        let result: Blake2sHashVec =
            <CudaBackend as MerkleOps<Blake2sMerkleHasher>>::commit_on_layer(
                log_size,
                None,
                &gpu_columns_vector.iter().collect::<Vec<_>>(),
            );

        // Compare first and last hashes
        let cpu_hashes = expected_result.clone();
        let gpu_hashes = result.to_cpu();

        assert_eq!(gpu_hashes.len(), size);
        assert_eq!(cpu_hashes.len(), size);

        // Check first 100 hashes
        assert_eq!(
            gpu_hashes[..100],
            cpu_hashes[..100],
            "First 100 hashes mismatch"
        );
        // Check last 100 hashes
        assert_eq!(
            gpu_hashes[size - 100..],
            cpu_hashes[size - 100..],
            "Last 100 hashes mismatch"
        );
        // Full equality
        assert_eq!(gpu_hashes, cpu_hashes);
    }
}

#[cfg(all(test, stwo_cuda_link, feature = "vendored-upstream-bridge"))]
mod lifted_tests {
    use stwo::core::fields::m31::{BaseField, M31};
    use stwo::core::vcs_lifted::blake2_merkle::Blake2sMerkleHasher;
    use stwo::prover::backend::{Column, CpuBackend};
    use stwo::prover::vcs_lifted::ops::MerkleOpsLifted;

    use crate::backend::cuda::CudaBackend;
    use crate::stwo_cuda::base_field_vec::BaseFieldVec;
    use crate::stwo_cuda::blake_2s_hash_vec::Blake2sHashVec;

    #[test]
    fn test_build_leaves_with_mixed_column_sizes_compared_with_cpu() {
        let cpu_columns = vec![
            base_field_column(4, 1),
            base_field_column(4, 3),
            base_field_column(8, 5),
            base_field_column(8, 7),
            base_field_column(16, 11),
        ];
        let gpu_columns = gpu_columns_from(&cpu_columns);
        let expected = <CpuBackend as MerkleOpsLifted<Blake2sMerkleHasher>>::build_leaves(
            &cpu_columns.iter().collect::<Vec<_>>(),
            4,
        );
        let result: Blake2sHashVec =
            <CudaBackend as MerkleOpsLifted<Blake2sMerkleHasher>>::build_leaves(
                &gpu_columns.iter().collect::<Vec<_>>(),
                4,
            );

        assert_eq!(result.to_cpu(), expected);
    }

    #[test]
    fn test_build_leaves_with_additional_lifting_compared_with_cpu() {
        let cpu_columns = vec![
            base_field_column(4, 13),
            base_field_column(8, 17),
            base_field_column(8, 19),
        ];
        let gpu_columns = gpu_columns_from(&cpu_columns);
        let expected = <CpuBackend as MerkleOpsLifted<Blake2sMerkleHasher>>::build_leaves(
            &cpu_columns.iter().collect::<Vec<_>>(),
            5,
        );
        let result: Blake2sHashVec =
            <CudaBackend as MerkleOpsLifted<Blake2sMerkleHasher>>::build_leaves(
                &gpu_columns.iter().collect::<Vec<_>>(),
                5,
            );

        assert_eq!(result.to_cpu(), expected);
    }

    fn gpu_columns_from(columns: &[Vec<BaseField>]) -> Vec<BaseFieldVec> {
        columns
            .iter()
            .cloned()
            .map(BaseFieldVec::from_vec)
            .collect()
    }

    fn base_field_column(size: usize, multiplier: u32) -> Vec<BaseField> {
        (0..size)
            .map(|index| M31::from(index as u32 * multiplier + multiplier))
            .collect()
    }
}
