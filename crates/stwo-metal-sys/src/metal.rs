use core::ffi::c_void;
use std::ffi::CStr;
use std::ptr::NonNull;
use std::sync::OnceLock;

include!(concat!(env!("OUT_DIR"), "/metal_autogen.rs"));

const ERROR_BUFFER_LEN: usize = 512;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum MetalRuntimeSupport {
    Available,
    DisabledByConfiguration,
    UnsupportedTarget,
    InitializationFailed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MetalError {
    message: String,
}

impl MetalError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl core::fmt::Display for MetalError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for MetalError {}

#[derive(Debug)]
struct RuntimeHandle {
    raw: NonNull<c_void>,
}

unsafe impl Send for RuntimeHandle {}
unsafe impl Sync for RuntimeHandle {}

impl Drop for RuntimeHandle {
    fn drop(&mut self) {
        unsafe { ffi::runtime_destroy(self.raw.as_ptr()) };
    }
}

#[derive(Debug)]
pub struct U32Buffer {
    raw: NonNull<c_void>,
    len: usize,
}

unsafe impl Send for U32Buffer {}
unsafe impl Sync for U32Buffer {}

impl U32Buffer {
    /// Returns an opaque process-local identifier suitable for internal caching.
    ///
    /// This does not promise pointer stability across processes or serialization.
    pub fn identity(&self) -> usize {
        self.raw.as_ptr() as usize
    }

    pub fn from_slice(values: &[u32]) -> Result<Self, MetalError> {
        let runtime = shared_runtime()?;
        let raw = unsafe {
            ffi::buffer_from_host(
                runtime.raw.as_ptr(),
                values.as_ptr(),
                values.len(),
                error_buffer_mut_ptr,
            )
        }?;
        Ok(Self {
            raw,
            len: values.len(),
        })
    }

    pub fn zeroed(len: usize) -> Result<Self, MetalError> {
        let runtime = shared_runtime()?;
        let raw =
            unsafe { ffi::buffer_alloc_zeroed(runtime.raw.as_ptr(), len, error_buffer_mut_ptr) }?;
        Ok(Self { raw, len })
    }

    pub fn uninitialized(len: usize) -> Result<Self, MetalError> {
        let runtime = shared_runtime()?;
        let raw = unsafe {
            ffi::buffer_alloc_uninitialized(runtime.raw.as_ptr(), len, error_buffer_mut_ptr)
        }?;
        Ok(Self { raw, len })
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn get(&self, index: usize) -> u32 {
        assert!(
            index < self.len,
            "buffer index {index} out of bounds for len {}",
            self.len
        );
        unsafe { ffi::buffer_get(self.raw.as_ptr(), index) }
    }

    pub fn set(&mut self, index: usize, value: u32) {
        assert!(
            index < self.len,
            "buffer index {index} out of bounds for len {}",
            self.len
        );
        unsafe { ffi::buffer_set(self.raw.as_ptr(), index, value) };
    }

    pub fn copy_from(&mut self, other: &Self) -> Result<(), MetalError> {
        assert!(
            self.len >= other.len,
            "destination buffer len {} is smaller than source len {}",
            self.len,
            other.len
        );
        unsafe {
            ffi::buffer_copy(
                other.raw.as_ptr(),
                self.raw.as_ptr(),
                other.len,
                0,
                error_buffer_mut_ptr,
            )
        }
    }

    pub fn copy_from_offset(&mut self, other: &Self, offset: usize) -> Result<(), MetalError> {
        assert!(
            offset + other.len <= self.len,
            "destination buffer len {} cannot fit source len {} at offset {}",
            self.len,
            other.len,
            offset
        );
        unsafe {
            ffi::buffer_copy(
                other.raw.as_ptr(),
                self.raw.as_ptr(),
                other.len,
                offset,
                error_buffer_mut_ptr,
            )
        }
    }

    pub fn copy_range_from(
        &mut self,
        other: &Self,
        src_offset: usize,
        len: usize,
        dst_offset: usize,
    ) -> Result<(), MetalError> {
        assert!(
            src_offset + len <= other.len,
            "source buffer len {} cannot provide range {}..{}",
            other.len,
            src_offset,
            src_offset + len
        );
        assert!(
            dst_offset + len <= self.len,
            "destination buffer len {} cannot fit range len {} at offset {}",
            self.len,
            len,
            dst_offset
        );
        unsafe {
            ffi::buffer_copy_range(
                other.raw.as_ptr(),
                self.raw.as_ptr(),
                src_offset,
                len,
                dst_offset,
                error_buffer_mut_ptr,
            )
        }
    }

    pub fn clone_range(&self, start: usize, len: usize) -> Result<Self, MetalError> {
        let mut cloned = Self::uninitialized(len)?;
        cloned.copy_range_from(self, start, len, 0)?;
        Ok(cloned)
    }

    pub fn to_vec(&self) -> Result<Vec<u32>, MetalError> {
        let runtime = shared_runtime()?;
        let mut values = vec![0u32; self.len];
        unsafe {
            ffi::buffer_read(
                runtime.raw.as_ptr(),
                self.raw.as_ptr(),
                values.as_mut_ptr(),
                self.len,
                error_buffer_mut_ptr,
            )?;
        }
        Ok(values)
    }

    pub unsafe fn host_ptr(&self) -> *const u32 {
        ffi::buffer_host_ptr(self.raw.as_ptr())
    }

    pub fn read_indices(&self, indices: &[usize]) -> Result<Vec<u32>, MetalError> {
        let runtime = shared_runtime()?;
        assert!(
            self.len <= u32::MAX as usize,
            "indexed Metal buffer reads require len to fit in u32"
        );
        let index_values = indices
            .iter()
            .map(|&index| {
                assert!(
                    index < self.len,
                    "buffer index {index} out of bounds for len {}",
                    self.len
                );
                index as u32
            })
            .collect::<Vec<_>>();
        let mut values = vec![0u32; indices.len()];
        unsafe {
            ffi::buffer_read_indices(
                runtime.raw.as_ptr(),
                self.raw.as_ptr(),
                index_values.as_ptr(),
                index_values.len(),
                values.as_mut_ptr(),
                error_buffer_mut_ptr,
            )?;
        }
        Ok(values)
    }

    pub fn bit_reverse(&mut self) -> Result<(), MetalError> {
        assert!(
            self.len.is_power_of_two(),
            "bit reverse requires a power-of-two buffer"
        );
        let runtime = shared_runtime()?;
        unsafe {
            ffi::bit_reverse_u32(
                runtime.raw.as_ptr(),
                self.raw.as_ptr(),
                self.len.ilog2(),
                error_buffer_mut_ptr,
            )
        }
    }

    pub fn bit_reverse_u32x4(&mut self, element_len: usize) -> Result<(), MetalError> {
        assert!(
            element_len.is_power_of_two(),
            "bit reverse requires a power-of-two element length"
        );
        assert_eq!(
            self.len,
            element_len * 4,
            "u32x4 bit reverse requires exactly four limbs per element"
        );
        let runtime = shared_runtime()?;
        unsafe {
            ffi::bit_reverse_u32x4(
                runtime.raw.as_ptr(),
                self.raw.as_ptr(),
                element_len.ilog2(),
                error_buffer_mut_ptr,
            )
        }
    }

    pub fn invert_m31_in_place(&mut self) -> Result<(), MetalError> {
        let runtime = shared_runtime()?;
        unsafe {
            ffi::invert_m31_values_u32(
                runtime.raw.as_ptr(),
                self.raw.as_ptr(),
                self.len,
                error_buffer_mut_ptr,
            )
        }
    }

    pub fn write_twiddle_level(
        &mut self,
        offset: usize,
        initial_xy: [u32; 2],
        step_xy: [u32; 2],
        level_log_size: u32,
    ) -> Result<(), MetalError> {
        assert!(
            level_log_size > 0,
            "twiddle precompute requires a non-zero level_log_size"
        );
        let level_len = 1usize << (level_log_size - 1);
        assert!(
            offset + level_len <= self.len,
            "twiddle level offset {} with level length {} exceeds buffer len {}",
            offset,
            level_len,
            self.len
        );
        let runtime = shared_runtime()?;
        unsafe {
            ffi::precompute_twiddle_level_u32(
                runtime.raw.as_ptr(),
                self.raw.as_ptr(),
                offset,
                initial_xy,
                step_xy,
                level_log_size,
                error_buffer_mut_ptr,
            )
        }
    }

    pub fn rfft_evaluate_in_place(&mut self, twiddles: &Self) -> Result<(), MetalError> {
        assert!(
            self.len.is_power_of_two(),
            "RFFT evaluate requires a power-of-two value buffer"
        );
        assert_eq!(
            twiddles.len,
            self.len / 2,
            "RFFT evaluate requires a twiddle slice with one half-coset tree tail"
        );
        let runtime = shared_runtime()?;
        unsafe {
            ffi::rfft_evaluate_u32(
                runtime.raw.as_ptr(),
                self.raw.as_ptr(),
                twiddles.raw.as_ptr(),
                self.len.ilog2(),
                error_buffer_mut_ptr,
            )
        }
    }

    pub fn rfft_evaluate_subbuffer_in_place(
        &mut self,
        value_offset: usize,
        values_len: usize,
        twiddles: &Self,
    ) -> Result<(), MetalError> {
        assert!(
            values_len.is_power_of_two(),
            "RFFT subbuffer evaluate requires a power-of-two value buffer"
        );
        assert!(
            value_offset + values_len <= self.len,
            "RFFT subbuffer evaluate range must stay within the backing buffer"
        );
        assert_eq!(
            twiddles.len,
            values_len / 2,
            "RFFT subbuffer evaluate requires a twiddle slice with one half-coset tree tail"
        );
        let runtime = shared_runtime()?;
        unsafe {
            ffi::rfft_evaluate_subbuffer_u32(
                runtime.raw.as_ptr(),
                self.raw.as_ptr(),
                value_offset,
                values_len.ilog2(),
                twiddles.raw.as_ptr(),
                error_buffer_mut_ptr,
            )
        }
    }

    pub fn ifft_interpolate_in_place(
        &mut self,
        inverse_twiddles: &Self,
        scale_factor: u32,
    ) -> Result<(), MetalError> {
        assert!(
            self.len.is_power_of_two(),
            "IFFT interpolate requires a power-of-two value buffer"
        );
        assert_eq!(
            inverse_twiddles.len,
            self.len / 2,
            "IFFT interpolate requires an inverse-twiddle slice with one half-coset tree tail"
        );
        let runtime = shared_runtime()?;
        unsafe {
            ffi::ifft_interpolate_u32(
                runtime.raw.as_ptr(),
                self.raw.as_ptr(),
                inverse_twiddles.raw.as_ptr(),
                self.len.ilog2(),
                scale_factor,
                error_buffer_mut_ptr,
            )
        }
    }

    pub fn ifft_line_interpolate_in_place(
        &mut self,
        inverse_line_twiddles: &Self,
        scale_factor: u32,
    ) -> Result<(), MetalError> {
        assert!(
            self.len.is_power_of_two(),
            "Line IFFT interpolate requires a power-of-two value buffer"
        );
        assert_eq!(
            inverse_line_twiddles.len,
            self.len.saturating_sub(1),
            "Line IFFT interpolate requires stage twiddles of total length len(values)-1"
        );
        let runtime = shared_runtime()?;
        unsafe {
            ffi::ifft_line_interpolate_u32(
                runtime.raw.as_ptr(),
                self.raw.as_ptr(),
                inverse_line_twiddles.raw.as_ptr(),
                self.len.ilog2(),
                scale_factor,
                error_buffer_mut_ptr,
            )
        }
    }

    pub fn batch_eval_at_point_base_field(
        &self,
        factors: &Self,
        coeffs_log_len: u32,
        n_polys: usize,
    ) -> Result<Self, MetalError> {
        let coeffs_size = 1usize << coeffs_log_len;
        assert_eq!(
            self.len,
            coeffs_size * n_polys,
            "batched point evaluation requires a flattened coefficient buffer with coeffs_size * n_polys base-field elements"
        );
        assert_eq!(
            factors.len,
            (coeffs_log_len as usize) * 4,
            "batched point evaluation requires one qm31 folding factor per coefficient level"
        );
        let runtime = shared_runtime()?;
        let dst = Self::uninitialized(n_polys * 4)?;
        unsafe {
            ffi::batch_eval_at_point_base_field_u32(
                runtime.raw.as_ptr(),
                self.raw.as_ptr(),
                factors.raw.as_ptr(),
                dst.raw.as_ptr(),
                coeffs_log_len,
                n_polys
                    .try_into()
                    .expect("batched point evaluation polynomial count should fit in u32"),
                error_buffer_mut_ptr,
            )?;
        }
        Ok(dst)
    }

    pub fn batch_eval_first_pass_base_field(
        &self,
        factors: &Self,
        coeffs_log_len: u32,
        n_polys: usize,
    ) -> Result<Self, MetalError> {
        let coeffs_size = 1usize << coeffs_log_len;
        let blocks_per_poly = if coeffs_log_len > 9 {
            coeffs_size >> 9
        } else {
            1
        };
        assert_eq!(
            self.len,
            coeffs_size * n_polys,
            "batched point-evaluation first pass requires a flattened coefficient buffer with coeffs_size * n_polys base-field elements"
        );
        assert_eq!(
            factors.len,
            (coeffs_log_len as usize) * 4,
            "batched point-evaluation first pass requires one qm31 folding factor per coefficient level"
        );
        let runtime = shared_runtime()?;
        let dst = Self::uninitialized(blocks_per_poly * n_polys * 4)?;
        unsafe {
            ffi::batch_eval_first_pass_base_field_u32(
                runtime.raw.as_ptr(),
                self.raw.as_ptr(),
                factors.raw.as_ptr(),
                dst.raw.as_ptr(),
                coeffs_log_len,
                n_polys.try_into().expect(
                    "batched point-evaluation first-pass polynomial count should fit in u32",
                ),
                error_buffer_mut_ptr,
            )?;
        }
        Ok(dst)
    }

    pub fn fix_first_variable_base_field(
        &self,
        assignment_limbs: [u32; 4],
    ) -> Result<Self, MetalError> {
        assert!(
            self.len.is_power_of_two() && self.len >= 2,
            "base-field MLE fix-first-variable requires a power-of-two buffer with at least two evaluations"
        );
        let runtime = shared_runtime()?;
        let dst = Self::uninitialized((self.len / 2) * 4)?;
        unsafe {
            ffi::fix_first_variable_base_field_u32(
                runtime.raw.as_ptr(),
                self.raw.as_ptr(),
                dst.raw.as_ptr(),
                self.len.ilog2(),
                assignment_limbs,
                error_buffer_mut_ptr,
            )?;
        }
        Ok(dst)
    }

    pub fn fix_first_variable_secure_field(
        &self,
        assignment_limbs: [u32; 4],
    ) -> Result<Self, MetalError> {
        assert!(
            self.len.is_multiple_of(4),
            "secure-field MLE fix-first-variable requires four limbs per evaluation"
        );
        let element_len = self.len / 4;
        assert!(
            element_len.is_power_of_two() && element_len >= 2,
            "secure-field MLE fix-first-variable requires a power-of-two element count with at least two evaluations"
        );
        let runtime = shared_runtime()?;
        let dst = Self::uninitialized((element_len / 2) * 4)?;
        unsafe {
            ffi::fix_first_variable_secure_field_u32x4(
                runtime.raw.as_ptr(),
                self.raw.as_ptr(),
                dst.raw.as_ptr(),
                element_len.ilog2(),
                assignment_limbs,
                error_buffer_mut_ptr,
            )?;
        }
        Ok(dst)
    }

    pub fn gkr_gen_eq_evals_from_factors(
        factors: &Self,
        y_size: usize,
        v_limbs: [u32; 4],
    ) -> Result<Self, MetalError> {
        let eval_count = 1usize
            .checked_shl(y_size as u32)
            .expect("GKR eq-eval size should fit in usize");
        assert_eq!(
            factors.len,
            y_size * 2 * 4,
            "GKR eq-eval generation requires two qm31 factors per input coordinate"
        );
        let runtime = shared_runtime()?;
        let dst = Self::uninitialized(eval_count * 4)?;
        unsafe {
            ffi::gkr_gen_eq_evals_u32x4(
                runtime.raw.as_ptr(),
                factors.raw.as_ptr(),
                dst.raw.as_ptr(),
                y_size
                    .try_into()
                    .expect("GKR eq-eval y-size should fit in u32"),
                v_limbs,
                error_buffer_mut_ptr,
            )?;
        }
        Ok(dst)
    }

    pub fn gkr_next_grand_product_layer(&self) -> Result<Self, MetalError> {
        assert!(
            self.len.is_multiple_of(8),
            "GKR next grand-product layer requires an even number of secure-field evaluations"
        );
        let input_len = self.len / 4;
        assert!(
            input_len.is_power_of_two() && input_len >= 2,
            "GKR next grand-product layer requires a power-of-two logical input length with at least two evaluations"
        );
        let runtime = shared_runtime()?;
        let dst = Self::uninitialized((input_len / 2) * 4)?;
        unsafe {
            ffi::gkr_next_grand_product_layer_u32x4(
                runtime.raw.as_ptr(),
                self.raw.as_ptr(),
                dst.raw.as_ptr(),
                input_len.ilog2(),
                error_buffer_mut_ptr,
            )?;
        }
        Ok(dst)
    }

    pub fn gkr_next_logup_generic_layer(
        numerators: &Self,
        denominators: &Self,
    ) -> Result<(Self, Self), MetalError> {
        assert_eq!(
            numerators.len, denominators.len,
            "GKR next generic layer requires matching numerator and denominator lengths"
        );
        assert!(
            numerators.len.is_multiple_of(8),
            "GKR next generic layer requires an even number of secure-field evaluations"
        );
        let input_len = numerators.len / 4;
        assert!(
            input_len.is_power_of_two() && input_len >= 2,
            "GKR next generic layer requires a power-of-two logical input length with at least two evaluations"
        );
        let runtime = shared_runtime()?;
        let next_numerators = Self::uninitialized((input_len / 2) * 4)?;
        let next_denominators = Self::uninitialized((input_len / 2) * 4)?;
        unsafe {
            ffi::gkr_next_logup_generic_layer_u32x4(
                runtime.raw.as_ptr(),
                numerators.raw.as_ptr(),
                denominators.raw.as_ptr(),
                next_numerators.raw.as_ptr(),
                next_denominators.raw.as_ptr(),
                input_len.ilog2(),
                error_buffer_mut_ptr,
            )?;
        }
        Ok((next_numerators, next_denominators))
    }

    pub fn gkr_next_logup_multiplicities_layer(
        numerators: &Self,
        denominators: &Self,
    ) -> Result<(Self, Self), MetalError> {
        assert!(
            numerators.len * 4 == denominators.len,
            "GKR next multiplicities layer requires one base-field numerator per secure-field denominator element"
        );
        assert!(
            numerators.len.is_power_of_two() && numerators.len >= 2,
            "GKR next multiplicities layer requires a power-of-two logical input length with at least two evaluations"
        );
        let runtime = shared_runtime()?;
        let next_numerators = Self::uninitialized((numerators.len / 2) * 4)?;
        let next_denominators = Self::uninitialized((numerators.len / 2) * 4)?;
        unsafe {
            ffi::gkr_next_logup_multiplicities_layer_u32(
                runtime.raw.as_ptr(),
                numerators.raw.as_ptr(),
                denominators.raw.as_ptr(),
                next_numerators.raw.as_ptr(),
                next_denominators.raw.as_ptr(),
                numerators.len.ilog2(),
                error_buffer_mut_ptr,
            )?;
        }
        Ok((next_numerators, next_denominators))
    }

    pub fn gkr_next_logup_singles_layer(denominators: &Self) -> Result<(Self, Self), MetalError> {
        assert!(
            denominators.len.is_multiple_of(8),
            "GKR next singles layer requires an even number of secure-field evaluations"
        );
        let input_len = denominators.len / 4;
        assert!(
            input_len.is_power_of_two() && input_len >= 2,
            "GKR next singles layer requires a power-of-two logical input length with at least two evaluations"
        );
        let runtime = shared_runtime()?;
        let next_numerators = Self::uninitialized((input_len / 2) * 4)?;
        let next_denominators = Self::uninitialized((input_len / 2) * 4)?;
        unsafe {
            ffi::gkr_next_logup_singles_layer_u32x4(
                runtime.raw.as_ptr(),
                denominators.raw.as_ptr(),
                next_numerators.raw.as_ptr(),
                next_denominators.raw.as_ptr(),
                input_len.ilog2(),
                error_buffer_mut_ptr,
            )?;
        }
        Ok((next_numerators, next_denominators))
    }

    pub fn gkr_sum_grand_product(
        eq_evals: &Self,
        input_layer: &Self,
    ) -> Result<([u32; 4], [u32; 4]), MetalError> {
        assert_eq!(
            input_layer.len,
            eq_evals.len * 4,
            "GKR grand-product sum requires four secure-field evaluations per eq-eval term"
        );
        let n_terms = eq_evals.len / 4;
        let runtime = shared_runtime()?;
        unsafe {
            ffi::gkr_sum_grand_product_u32x4(
                runtime.raw.as_ptr(),
                eq_evals.raw.as_ptr(),
                input_layer.raw.as_ptr(),
                n_terms
                    .try_into()
                    .expect("GKR grand-product term count should fit in u32"),
                error_buffer_mut_ptr,
            )
        }
    }

    pub fn gkr_sum_logup_generic(
        eq_evals: &Self,
        numerators: &Self,
        denominators: &Self,
        lambda_limbs: [u32; 4],
    ) -> Result<([u32; 4], [u32; 4]), MetalError> {
        assert_eq!(
            numerators.len, denominators.len,
            "GKR generic sum requires matching numerator and denominator lengths"
        );
        assert_eq!(
            numerators.len,
            eq_evals.len * 4,
            "GKR generic sum requires four secure-field numerator evaluations per eq-eval term"
        );
        let n_terms = eq_evals.len / 4;
        let runtime = shared_runtime()?;
        unsafe {
            ffi::gkr_sum_logup_generic_u32x4(
                runtime.raw.as_ptr(),
                eq_evals.raw.as_ptr(),
                numerators.raw.as_ptr(),
                denominators.raw.as_ptr(),
                n_terms
                    .try_into()
                    .expect("GKR generic term count should fit in u32"),
                lambda_limbs,
                error_buffer_mut_ptr,
            )
        }
    }

    pub fn gkr_sum_logup_multiplicities(
        eq_evals: &Self,
        numerators: &Self,
        denominators: &Self,
        lambda_limbs: [u32; 4],
    ) -> Result<([u32; 4], [u32; 4]), MetalError> {
        assert_eq!(
            denominators.len,
            eq_evals.len * 4,
            "GKR multiplicities sum requires four denominator evaluations per eq-eval term"
        );
        assert_eq!(
            numerators.len, eq_evals.len,
            "GKR multiplicities sum requires four base-field numerators per eq-eval term"
        );
        let n_terms = eq_evals.len / 4;
        let runtime = shared_runtime()?;
        unsafe {
            ffi::gkr_sum_logup_multiplicities_u32(
                runtime.raw.as_ptr(),
                eq_evals.raw.as_ptr(),
                numerators.raw.as_ptr(),
                denominators.raw.as_ptr(),
                n_terms
                    .try_into()
                    .expect("GKR multiplicities term count should fit in u32"),
                lambda_limbs,
                error_buffer_mut_ptr,
            )
        }
    }

    pub fn gkr_sum_logup_singles(
        eq_evals: &Self,
        denominators: &Self,
        lambda_limbs: [u32; 4],
    ) -> Result<([u32; 4], [u32; 4]), MetalError> {
        assert_eq!(
            denominators.len,
            eq_evals.len * 4,
            "GKR singles sum requires four denominator evaluations per eq-eval term"
        );
        let n_terms = eq_evals.len / 4;
        let runtime = shared_runtime()?;
        unsafe {
            ffi::gkr_sum_logup_singles_u32x4(
                runtime.raw.as_ptr(),
                eq_evals.raw.as_ptr(),
                denominators.raw.as_ptr(),
                n_terms
                    .try_into()
                    .expect("GKR singles term count should fit in u32"),
                lambda_limbs,
                error_buffer_mut_ptr,
            )
        }
    }

    pub fn inclusive_prefix_sum_bit_rev_circle_domain_in_place(
        &mut self,
    ) -> Result<(), MetalError> {
        assert!(
            self.len.is_power_of_two(),
            "prefix sum requires a power-of-two base-field buffer"
        );
        let runtime = shared_runtime()?;
        unsafe {
            ffi::inclusive_prefix_sum_bit_rev_circle_domain_u32(
                runtime.raw.as_ptr(),
                self.raw.as_ptr(),
                self.len.ilog2(),
                error_buffer_mut_ptr,
            )
        }
    }

    pub fn permute_coset_to_circle_domain_bit_reversed(&self) -> Result<Self, MetalError> {
        assert!(
            self.len.is_power_of_two(),
            "coset permutation requires a power-of-two buffer"
        );
        let runtime = shared_runtime()?;
        let dst = Self::uninitialized(self.len)?;
        unsafe {
            ffi::permute_coset_to_circle_domain_bit_reversed_u32(
                runtime.raw.as_ptr(),
                self.raw.as_ptr(),
                dst.raw.as_ptr(),
                self.len.ilog2(),
                error_buffer_mut_ptr,
            )?;
        }
        Ok(dst)
    }

    pub fn pack_secure_column_coords(coord_columns: [&Self; 4]) -> Result<Self, MetalError> {
        let [coord_0, coord_1, coord_2, coord_3] = coord_columns;
        let len = coord_0.len;
        assert_eq!(
            coord_1.len, len,
            "secure-column packing requires equal coordinate lengths"
        );
        assert_eq!(
            coord_2.len, len,
            "secure-column packing requires equal coordinate lengths"
        );
        assert_eq!(
            coord_3.len, len,
            "secure-column packing requires equal coordinate lengths"
        );
        let runtime = shared_runtime()?;
        let dst = Self::uninitialized(len * 4)?;
        unsafe {
            ffi::pack_secure_column_coords_u32x4(
                runtime.raw.as_ptr(),
                coord_0.raw.as_ptr(),
                coord_1.raw.as_ptr(),
                coord_2.raw.as_ptr(),
                coord_3.raw.as_ptr(),
                dst.raw.as_ptr(),
                len.try_into()
                    .expect("secure-column packing logical length should fit in u32"),
                error_buffer_mut_ptr,
            )?;
        }
        Ok(dst)
    }

    pub fn unpack_secure_column_coords(&self) -> Result<[Self; 4], MetalError> {
        assert!(
            self.len.is_multiple_of(4),
            "secure-column unpacking requires four limbs per element"
        );
        let len = self.len / 4;
        let runtime = shared_runtime()?;
        let coord_0 = Self::uninitialized(len)?;
        let coord_1 = Self::uninitialized(len)?;
        let coord_2 = Self::uninitialized(len)?;
        let coord_3 = Self::uninitialized(len)?;
        unsafe {
            ffi::unpack_secure_column_coords_u32x4(
                runtime.raw.as_ptr(),
                self.raw.as_ptr(),
                coord_0.raw.as_ptr(),
                coord_1.raw.as_ptr(),
                coord_2.raw.as_ptr(),
                coord_3.raw.as_ptr(),
                len.try_into()
                    .expect("secure-column unpacking logical length should fit in u32"),
                error_buffer_mut_ptr,
            )?;
        }
        Ok([coord_0, coord_1, coord_2, coord_3])
    }

    pub fn accumulate_secure_columns_coords(
        lhs_columns: [&Self; 4],
        rhs_columns: [&Self; 4],
    ) -> Result<[Self; 4], MetalError> {
        let [lhs_0, lhs_1, lhs_2, lhs_3] = lhs_columns;
        let [rhs_0, rhs_1, rhs_2, rhs_3] = rhs_columns;
        let element_len = lhs_0.len;
        assert_eq!(lhs_1.len, element_len);
        assert_eq!(lhs_2.len, element_len);
        assert_eq!(lhs_3.len, element_len);
        assert_eq!(rhs_0.len, element_len);
        assert_eq!(rhs_1.len, element_len);
        assert_eq!(rhs_2.len, element_len);
        assert_eq!(rhs_3.len, element_len);

        let runtime = shared_runtime()?;
        let dst_0 = Self::uninitialized(element_len)?;
        let dst_1 = Self::uninitialized(element_len)?;
        let dst_2 = Self::uninitialized(element_len)?;
        let dst_3 = Self::uninitialized(element_len)?;
        unsafe {
            ffi::accumulate_secure_columns_coords_u32x4(
                runtime.raw.as_ptr(),
                lhs_0.raw.as_ptr(),
                lhs_1.raw.as_ptr(),
                lhs_2.raw.as_ptr(),
                lhs_3.raw.as_ptr(),
                rhs_0.raw.as_ptr(),
                rhs_1.raw.as_ptr(),
                rhs_2.raw.as_ptr(),
                rhs_3.raw.as_ptr(),
                dst_0.raw.as_ptr(),
                dst_1.raw.as_ptr(),
                dst_2.raw.as_ptr(),
                dst_3.raw.as_ptr(),
                element_len
                    .try_into()
                    .expect("secure-column accumulation length should fit in u32"),
                error_buffer_mut_ptr,
            )?;
        }
        Ok([dst_0, dst_1, dst_2, dst_3])
    }

    pub fn lift_accumulate_secure_columns_coords(
        lifted_columns: [&Self; 4],
        current_columns: [&Self; 4],
    ) -> Result<[Self; 4], MetalError> {
        let [lifted_0, lifted_1, lifted_2, lifted_3] = lifted_columns;
        let [current_0, current_1, current_2, current_3] = current_columns;
        let current_len = current_0.len;
        assert_eq!(current_1.len, current_len);
        assert_eq!(current_2.len, current_len);
        assert_eq!(current_3.len, current_len);
        assert!(current_len.is_power_of_two() && current_len >= 2);
        let lifted_len = lifted_0.len;
        assert_eq!(lifted_1.len, lifted_len);
        assert_eq!(lifted_2.len, lifted_len);
        assert_eq!(lifted_3.len, lifted_len);
        assert!(lifted_len.is_power_of_two() && lifted_len >= 2);
        assert!(
            current_len >= lifted_len,
            "lift-and-accumulate requires current length >= lifted length"
        );
        let log_ratio = current_len.ilog2() - lifted_len.ilog2();

        let runtime = shared_runtime()?;
        let dst_0 = Self::uninitialized(current_len)?;
        let dst_1 = Self::uninitialized(current_len)?;
        let dst_2 = Self::uninitialized(current_len)?;
        let dst_3 = Self::uninitialized(current_len)?;
        unsafe {
            ffi::lift_accumulate_secure_columns_coords_u32x4(
                runtime.raw.as_ptr(),
                lifted_0.raw.as_ptr(),
                lifted_1.raw.as_ptr(),
                lifted_2.raw.as_ptr(),
                lifted_3.raw.as_ptr(),
                current_0.raw.as_ptr(),
                current_1.raw.as_ptr(),
                current_2.raw.as_ptr(),
                current_3.raw.as_ptr(),
                dst_0.raw.as_ptr(),
                dst_1.raw.as_ptr(),
                dst_2.raw.as_ptr(),
                dst_3.raw.as_ptr(),
                current_len.ilog2(),
                log_ratio,
                error_buffer_mut_ptr,
            )?;
        }
        Ok([dst_0, dst_1, dst_2, dst_3])
    }

    pub fn fri_fold_circle_into_line_first_layer_u32x4(
        &self,
        inverse_y_factors: &Self,
        alpha_limbs: [u32; 4],
    ) -> Result<Self, MetalError> {
        assert!(
            self.len.is_multiple_of(8),
            "fri first-layer fold requires an even number of secure-field elements"
        );
        let element_len = self.len / 4;
        let output_len = element_len / 2;
        assert_eq!(
            inverse_y_factors.len, output_len,
            "fri first-layer fold requires one inverse-y factor per output element"
        );
        let runtime = shared_runtime()?;
        let dst = Self::uninitialized(output_len * 4)?;
        unsafe {
            ffi::fri_fold_circle_into_line_first_layer_u32x4(
                runtime.raw.as_ptr(),
                self.raw.as_ptr(),
                dst.raw.as_ptr(),
                inverse_y_factors.raw.as_ptr(),
                element_len.ilog2(),
                alpha_limbs,
                error_buffer_mut_ptr,
            )?;
        }
        Ok(dst)
    }

    pub fn fri_fold_circle_into_line_accumulate_from_coords_u32x4(
        src_columns: [&Self; 4],
        dst_columns: [&mut Self; 4],
        inverse_y_factors: &Self,
        alpha_limbs: [u32; 4],
        alpha_sq_limbs: [u32; 4],
    ) -> Result<(), MetalError> {
        let [src_0, src_1, src_2, src_3] = src_columns;
        let element_len = src_0.len;
        assert_eq!(src_1.len, element_len);
        assert_eq!(src_2.len, element_len);
        assert_eq!(src_3.len, element_len);
        assert!(
            element_len.is_multiple_of(2),
            "fri first-layer accumulation requires an even number of secure-field elements"
        );
        let output_len = element_len / 2;
        assert_eq!(
            inverse_y_factors.len, output_len,
            "fri first-layer accumulation requires one inverse-y factor per output element"
        );
        let [dst_0, dst_1, dst_2, dst_3] = dst_columns;
        assert_eq!(
            dst_0.len, output_len,
            "fri first-layer accumulation requires destination coordinate buffers sized to the output length"
        );
        assert_eq!(dst_1.len, output_len);
        assert_eq!(dst_2.len, output_len);
        assert_eq!(dst_3.len, output_len);
        let runtime = shared_runtime()?;
        unsafe {
            ffi::fri_fold_circle_into_line_accumulate_coords_u32x4(
                runtime.raw.as_ptr(),
                src_0.raw.as_ptr(),
                src_1.raw.as_ptr(),
                src_2.raw.as_ptr(),
                src_3.raw.as_ptr(),
                dst_0.raw.as_ptr(),
                dst_1.raw.as_ptr(),
                dst_2.raw.as_ptr(),
                dst_3.raw.as_ptr(),
                inverse_y_factors.raw.as_ptr(),
                element_len.ilog2(),
                alpha_limbs,
                alpha_sq_limbs,
                error_buffer_mut_ptr,
            )
        }
    }

    pub fn fri_fold_circle_into_line_first_layer_from_coords_u32x4(
        src_columns: [&Self; 4],
        inverse_y_factors: &Self,
        alpha_limbs: [u32; 4],
    ) -> Result<[Self; 4], MetalError> {
        let [src_0, src_1, src_2, src_3] = src_columns;
        let element_len = src_0.len;
        assert_eq!(src_1.len, element_len);
        assert_eq!(src_2.len, element_len);
        assert_eq!(src_3.len, element_len);
        assert!(
            element_len.is_multiple_of(2),
            "fri first-layer coordinate fold requires an even number of secure-field elements"
        );
        let output_len = element_len / 2;
        assert_eq!(
            inverse_y_factors.len, output_len,
            "fri first-layer coordinate fold requires one inverse-y factor per output element"
        );
        let runtime = shared_runtime()?;
        let dst_0 = Self::uninitialized(output_len)?;
        let dst_1 = Self::uninitialized(output_len)?;
        let dst_2 = Self::uninitialized(output_len)?;
        let dst_3 = Self::uninitialized(output_len)?;
        unsafe {
            ffi::fri_fold_circle_into_line_first_layer_coords_u32x4(
                runtime.raw.as_ptr(),
                src_0.raw.as_ptr(),
                src_1.raw.as_ptr(),
                src_2.raw.as_ptr(),
                src_3.raw.as_ptr(),
                dst_0.raw.as_ptr(),
                dst_1.raw.as_ptr(),
                dst_2.raw.as_ptr(),
                dst_3.raw.as_ptr(),
                inverse_y_factors.raw.as_ptr(),
                element_len.ilog2(),
                alpha_limbs,
                error_buffer_mut_ptr,
            )?;
        }
        Ok([dst_0, dst_1, dst_2, dst_3])
    }

    pub fn fri_fold_line_step_from_coords_u32x4(
        src_columns: [&Self; 4],
        inverse_x_factors: &Self,
        alpha_limbs: [u32; 4],
    ) -> Result<[Self; 4], MetalError> {
        let [src_0, src_1, src_2, src_3] = src_columns;
        let element_len = src_0.len;
        assert_eq!(src_1.len, element_len);
        assert_eq!(src_2.len, element_len);
        assert_eq!(src_3.len, element_len);
        assert!(
            element_len.is_multiple_of(2),
            "fri line-fold step requires an even number of secure-field elements"
        );
        let output_len = element_len / 2;
        assert_eq!(
            inverse_x_factors.len, output_len,
            "fri line-fold step requires one inverse-x factor per output element"
        );
        let runtime = shared_runtime()?;
        let dst_0 = Self::uninitialized(output_len)?;
        let dst_1 = Self::uninitialized(output_len)?;
        let dst_2 = Self::uninitialized(output_len)?;
        let dst_3 = Self::uninitialized(output_len)?;
        unsafe {
            ffi::fri_fold_line_step_coords_u32x4(
                runtime.raw.as_ptr(),
                src_0.raw.as_ptr(),
                src_1.raw.as_ptr(),
                src_2.raw.as_ptr(),
                src_3.raw.as_ptr(),
                dst_0.raw.as_ptr(),
                dst_1.raw.as_ptr(),
                dst_2.raw.as_ptr(),
                dst_3.raw.as_ptr(),
                inverse_x_factors.raw.as_ptr(),
                element_len.ilog2(),
                alpha_limbs,
                error_buffer_mut_ptr,
            )?;
        }
        Ok([dst_0, dst_1, dst_2, dst_3])
    }

    pub fn fri_fold_line_step_u32x4(
        &self,
        inverse_x_factors: &Self,
        alpha_limbs: [u32; 4],
    ) -> Result<Self, MetalError> {
        assert!(
            self.len.is_multiple_of(8),
            "fri line-fold step requires an even number of secure-field elements"
        );
        let element_len = self.len / 4;
        let output_len = element_len / 2;
        assert_eq!(
            inverse_x_factors.len, output_len,
            "fri line-fold step requires one inverse-x factor per output element"
        );
        let runtime = shared_runtime()?;
        let dst = Self::uninitialized(output_len * 4)?;
        unsafe {
            ffi::fri_fold_line_step_u32x4(
                runtime.raw.as_ptr(),
                self.raw.as_ptr(),
                dst.raw.as_ptr(),
                inverse_x_factors.raw.as_ptr(),
                element_len.ilog2(),
                alpha_limbs,
                error_buffer_mut_ptr,
            )?;
        }
        Ok(dst)
    }

    pub fn generate_wide_fibonacci_trace(
        input_a: &Self,
        input_b: &Self,
        n_columns: u32,
    ) -> Result<Self, MetalError> {
        assert!(
            input_a.len.is_power_of_two(),
            "wide-fibonacci trace generation requires a power-of-two input length"
        );
        assert_eq!(
            input_a.len, input_b.len,
            "wide-fibonacci trace generation requires equal input lengths"
        );
        assert!(
            n_columns >= 2,
            "wide-fibonacci trace generation requires at least two columns"
        );
        let output_len = input_a
            .len
            .checked_mul(n_columns as usize)
            .expect("wide-fibonacci trace output length should fit in usize");
        let runtime = shared_runtime()?;
        let dst = Self::uninitialized(output_len)?;
        unsafe {
            ffi::generate_wide_fibonacci_trace_u32(
                runtime.raw.as_ptr(),
                input_a.raw.as_ptr(),
                input_b.raw.as_ptr(),
                dst.raw.as_ptr(),
                input_a.len.ilog2(),
                n_columns,
                error_buffer_mut_ptr,
            )?;
        }
        Ok(dst)
    }

    pub fn accumulate_wide_fibonacci_quotients(
        trace_evaluations: &Self,
        random_coeff_powers: &Self,
        denominator_inverses: &Self,
        domain_log_size: u32,
        eval_domain_log_size: u32,
        n_constraints: u32,
    ) -> Result<Self, MetalError> {
        assert!(
            eval_domain_log_size >= domain_log_size,
            "wide-fibonacci quotient accumulation requires eval_domain_log_size >= domain_log_size"
        );
        assert!(
            n_constraints >= 1,
            "wide-fibonacci quotient accumulation requires at least one constraint"
        );
        let eval_domain_size = 1usize << eval_domain_log_size;
        let expected_trace_len = eval_domain_size
            .checked_mul(n_constraints as usize + 2)
            .expect("wide-fibonacci quotient trace length should fit in usize");
        assert_eq!(
            trace_evaluations.len, expected_trace_len,
            "wide-fibonacci quotient accumulation expects a contiguous column-major trace buffer"
        );
        assert_eq!(
            random_coeff_powers.len,
            n_constraints as usize * 4,
            "wide-fibonacci quotient accumulation expects one qm31 coefficient per constraint"
        );
        assert_eq!(
            denominator_inverses.len,
            1usize << (eval_domain_log_size - domain_log_size),
            "wide-fibonacci quotient accumulation expects one denominator inverse per evaluation-domain coset"
        );
        let runtime = shared_runtime()?;
        let dst = Self::uninitialized(eval_domain_size * 4)?;
        unsafe {
            ffi::accumulate_wide_fibonacci_quotients_u32x4(
                runtime.raw.as_ptr(),
                trace_evaluations.raw.as_ptr(),
                random_coeff_powers.raw.as_ptr(),
                denominator_inverses.raw.as_ptr(),
                dst.raw.as_ptr(),
                domain_log_size,
                eval_domain_log_size,
                n_constraints,
                error_buffer_mut_ptr,
            )?;
        }
        Ok(dst)
    }

    pub fn accumulate_partial_numerators(
        columns: &Self,
        column_indices: &Self,
        b_coeffs: &Self,
        c_coeffs: &Self,
        row_count: usize,
    ) -> Result<Self, MetalError> {
        assert!(
            row_count > 0,
            "partial numerator accumulation requires a non-zero row count"
        );
        assert_eq!(
            columns.len % row_count,
            0,
            "partial numerator accumulation expects flattened base columns with an integral number of rows"
        );
        assert_eq!(
            column_indices.len * 4,
            b_coeffs.len,
            "partial numerator accumulation expects one qm31 b coefficient per column index"
        );
        assert_eq!(
            column_indices.len * 4,
            c_coeffs.len,
            "partial numerator accumulation expects one qm31 c coefficient per column index"
        );
        let runtime = shared_runtime()?;
        let dst = Self::uninitialized(row_count * 4)?;
        unsafe {
            ffi::accumulate_partial_numerators_u32x4(
                runtime.raw.as_ptr(),
                columns.raw.as_ptr(),
                column_indices.raw.as_ptr(),
                b_coeffs.raw.as_ptr(),
                c_coeffs.raw.as_ptr(),
                dst.raw.as_ptr(),
                row_count
                    .try_into()
                    .expect("partial numerator accumulation row count should fit in u32"),
                column_indices
                    .len
                    .try_into()
                    .expect("partial numerator accumulation term count should fit in u32"),
                error_buffer_mut_ptr,
            )?;
        }
        Ok(dst)
    }

    pub fn accumulate_partial_numerators_batched(
        columns: &Self,
        column_indices: &Self,
        b_coeffs: &Self,
        c_coeffs: &Self,
        term_offsets: &Self,
        term_counts: &Self,
        row_count: usize,
    ) -> Result<Self, MetalError> {
        assert!(
            row_count > 0,
            "batched partial numerator accumulation requires a non-zero row count"
        );
        assert_eq!(
            columns.len % row_count,
            0,
            "batched partial numerator accumulation expects flattened base columns with an integral number of rows"
        );
        assert_eq!(
            column_indices.len * 4,
            b_coeffs.len,
            "batched partial numerator accumulation expects one qm31 b coefficient per column index"
        );
        assert_eq!(
            column_indices.len * 4,
            c_coeffs.len,
            "batched partial numerator accumulation expects one qm31 c coefficient per column index"
        );
        assert_eq!(
            term_offsets.len, term_counts.len,
            "batched partial numerator accumulation expects one term offset per batch"
        );
        let runtime = shared_runtime()?;
        let dst = Self::uninitialized(row_count * term_counts.len * 4)?;
        unsafe {
            ffi::accumulate_partial_numerators_batched_u32x4(
                runtime.raw.as_ptr(),
                columns.raw.as_ptr(),
                column_indices.raw.as_ptr(),
                b_coeffs.raw.as_ptr(),
                c_coeffs.raw.as_ptr(),
                term_offsets.raw.as_ptr(),
                term_counts.raw.as_ptr(),
                dst.raw.as_ptr(),
                row_count
                    .try_into()
                    .expect("batched partial numerator accumulation row count should fit in u32"),
                term_counts
                    .len
                    .try_into()
                    .expect("batched partial numerator accumulation batch count should fit in u32"),
                error_buffer_mut_ptr,
            )?;
        }
        Ok(dst)
    }

    pub fn compute_quotients_and_combine(
        partial_coord_columns: [&Self; 4],
        sample_points: &Self,
        first_linear_terms: &Self,
        partial_log_sizes: &Self,
        partial_offsets: &Self,
        domain_x: &Self,
        domain_y: &Self,
        lifting_log_size: u32,
    ) -> Result<Self, MetalError> {
        let [partial_coord_0, partial_coord_1, partial_coord_2, partial_coord_3] =
            partial_coord_columns;
        let row_count = 1usize << lifting_log_size;
        assert_eq!(
            partial_coord_0.len, partial_coord_1.len,
            "quotient combination requires equal flattened partial numerator coordinate lengths"
        );
        assert_eq!(partial_coord_0.len, partial_coord_2.len);
        assert_eq!(partial_coord_0.len, partial_coord_3.len);
        assert_eq!(
            sample_points.len % 8,
            0,
            "quotient combination expects eight sample-point limbs per accumulation"
        );
        let n_accumulations = sample_points.len / 8;
        assert_eq!(
            first_linear_terms.len,
            n_accumulations * 4,
            "quotient combination expects one qm31 first-linear term per accumulation"
        );
        assert_eq!(
            partial_log_sizes.len, n_accumulations,
            "quotient combination expects one partial log-size per accumulation"
        );
        assert_eq!(
            partial_offsets.len, n_accumulations,
            "quotient combination expects one partial offset per accumulation"
        );
        assert_eq!(
            domain_x.len, row_count,
            "quotient combination expects one domain x-coordinate per lifting-domain row"
        );
        assert_eq!(
            domain_y.len, row_count,
            "quotient combination expects one domain y-coordinate per lifting-domain row"
        );
        let runtime = shared_runtime()?;
        let dst = Self::uninitialized(row_count * 4)?;
        unsafe {
            ffi::compute_quotients_and_combine_u32x4(
                runtime.raw.as_ptr(),
                partial_coord_0.raw.as_ptr(),
                partial_coord_1.raw.as_ptr(),
                partial_coord_2.raw.as_ptr(),
                partial_coord_3.raw.as_ptr(),
                sample_points.raw.as_ptr(),
                first_linear_terms.raw.as_ptr(),
                partial_log_sizes.raw.as_ptr(),
                partial_offsets.raw.as_ptr(),
                domain_x.raw.as_ptr(),
                domain_y.raw.as_ptr(),
                dst.raw.as_ptr(),
                lifting_log_size,
                n_accumulations
                    .try_into()
                    .expect("quotient combination accumulation count should fit in u32"),
                error_buffer_mut_ptr,
            )?;
        }
        Ok(dst)
    }

    pub fn compute_quotients_and_combine_packed(
        partials: &Self,
        sample_points: &Self,
        first_linear_terms: &Self,
        partial_log_sizes: &Self,
        partial_offsets: &Self,
        domain_x: &Self,
        domain_y: &Self,
        lifting_log_size: u32,
    ) -> Result<Self, MetalError> {
        let row_count = 1usize << lifting_log_size;
        assert!(
            partials.len.is_multiple_of(4),
            "packed quotient combination expects qm31-packed partial numerators"
        );
        assert_eq!(
            sample_points.len % 8,
            0,
            "packed quotient combination expects eight sample-point limbs per accumulation"
        );
        let n_accumulations = sample_points.len / 8;
        assert_eq!(
            first_linear_terms.len,
            n_accumulations * 4,
            "packed quotient combination expects one qm31 first-linear term per accumulation"
        );
        assert_eq!(partial_log_sizes.len, n_accumulations);
        assert_eq!(partial_offsets.len, n_accumulations);
        assert_eq!(domain_x.len, row_count);
        assert_eq!(domain_y.len, row_count);
        let runtime = shared_runtime()?;
        let dst = Self::uninitialized(row_count * 4)?;
        unsafe {
            ffi::compute_quotients_and_combine_packed_u32x4(
                runtime.raw.as_ptr(),
                partials.raw.as_ptr(),
                sample_points.raw.as_ptr(),
                first_linear_terms.raw.as_ptr(),
                partial_log_sizes.raw.as_ptr(),
                partial_offsets.raw.as_ptr(),
                domain_x.raw.as_ptr(),
                domain_y.raw.as_ptr(),
                dst.raw.as_ptr(),
                lifting_log_size,
                n_accumulations
                    .try_into()
                    .expect("packed quotient combination accumulation count should fit in u32"),
                error_buffer_mut_ptr,
            )?;
        }
        Ok(dst)
    }

    pub fn blake2s_build_leaves_lifted(
        flat_columns: &Self,
        column_offsets: &Self,
        column_log_sizes: &Self,
        lifting_log_size: u32,
    ) -> Result<Self, MetalError> {
        let row_count = 1usize << lifting_log_size;
        assert_eq!(
            column_offsets.len, column_log_sizes.len,
            "lifted Blake2s leaves require one offset per column log-size"
        );
        let runtime = shared_runtime()?;
        let dst = Self::uninitialized(row_count * 8)?;
        unsafe {
            ffi::blake2s_build_leaves_lifted_u32(
                runtime.raw.as_ptr(),
                flat_columns.raw.as_ptr(),
                column_offsets.raw.as_ptr(),
                column_log_sizes.raw.as_ptr(),
                dst.raw.as_ptr(),
                column_offsets
                    .len
                    .try_into()
                    .expect("lifted Blake2s leaf column count should fit in u32"),
                lifting_log_size,
                error_buffer_mut_ptr,
            )?;
        }
        Ok(dst)
    }

    pub fn blake2s_build_leaves_lifted_wide(
        columns: &[&Self],
        column_log_sizes: &[u32],
        lifting_log_size: u32,
    ) -> Result<Self, MetalError> {
        assert!(
            !columns.is_empty(),
            "wide lifted Blake2s leaves require at least one source column"
        );
        assert_eq!(
            columns.len(),
            column_log_sizes.len(),
            "wide lifted Blake2s leaves require one log size per source column"
        );
        let row_count = 1usize << lifting_log_size;
        let runtime = shared_runtime()?;
        let state = Self::zeroed(row_count * 8)?;
        let dst = Self::uninitialized(row_count * 8)?;
        let mut processed_bytes_before = 0u32;

        for (chunk_index, (column_chunk, log_size_chunk)) in columns
            .chunks(16)
            .zip(column_log_sizes.chunks(16))
            .enumerate()
        {
            let mut column_ptrs = [std::ptr::null_mut(); 16];
            for (slot, column) in column_chunk.iter().enumerate() {
                column_ptrs[slot] = column.raw.as_ptr();
            }
            let is_first_chunk = chunk_index == 0;
            let is_final_chunk = chunk_index + 1 == columns.len().div_ceil(16);
            unsafe {
                ffi::blake2s_build_leaves_lifted_wide_chunk_u32(
                    runtime.raw.as_ptr(),
                    column_ptrs.as_ptr(),
                    state.raw.as_ptr(),
                    dst.raw.as_ptr(),
                    log_size_chunk.as_ptr(),
                    column_chunk
                        .len()
                        .try_into()
                        .expect("wide lifted Blake2s chunk column count should fit in u32"),
                    lifting_log_size,
                    processed_bytes_before,
                    is_first_chunk,
                    is_final_chunk,
                    error_buffer_mut_ptr,
                )?;
            }
            processed_bytes_before += u32::try_from(column_chunk.len() * 4)
                .expect("wide lifted Blake2s processed byte count should fit in u32");
        }

        Ok(dst)
    }

    pub fn blake2s_build_next_layer(prev_layer: &Self) -> Result<Self, MetalError> {
        assert!(
            prev_layer.len.is_multiple_of(16),
            "packed Blake2s next-layer hashing expects pairs of eight-word child hashes"
        );
        let next_hash_count = prev_layer.len / 16;
        let runtime = shared_runtime()?;
        let dst = Self::uninitialized(next_hash_count * 8)?;
        unsafe {
            ffi::blake2s_build_next_layer_u32(
                runtime.raw.as_ptr(),
                prev_layer.raw.as_ptr(),
                dst.raw.as_ptr(),
                next_hash_count
                    .try_into()
                    .expect("Blake2s next-layer parent count should fit in u32"),
                error_buffer_mut_ptr,
            )?;
        }
        Ok(dst)
    }

    pub fn blake2s_build_merkle_layers_from_leaves(
        leaf_layer: Self,
    ) -> Result<Vec<Self>, MetalError> {
        assert!(
            leaf_layer.len.is_multiple_of(8),
            "packed Blake2s leaves should contain eight words per hash"
        );
        let mut current_hash_count = leaf_layer.len / 8;
        assert!(
            current_hash_count.is_power_of_two(),
            "packed Blake2s leaves should contain a power-of-two hash count"
        );

        if current_hash_count == 0 {
            return Ok(vec![leaf_layer]);
        }

        let leaf_log_size = current_hash_count.ilog2();
        let runtime = shared_runtime()?;
        let mut upper_layers = Vec::with_capacity(leaf_log_size as usize);
        while current_hash_count > 1 {
            current_hash_count /= 2;
            upper_layers.push(Self::uninitialized(current_hash_count * 8)?);
        }

        let layer_ptrs = upper_layers
            .iter_mut()
            .map(|layer| layer.raw.as_ptr())
            .collect::<Vec<_>>();
        unsafe {
            ffi::blake2s_build_merkle_layers_u32(
                runtime.raw.as_ptr(),
                leaf_layer.raw.as_ptr(),
                layer_ptrs.as_ptr(),
                leaf_log_size,
                error_buffer_mut_ptr,
            )?;
        }

        let mut layers = Vec::with_capacity(upper_layers.len() + 1);
        layers.push(leaf_layer);
        layers.extend(upper_layers);
        Ok(layers)
    }
}

impl Clone for U32Buffer {
    fn clone(&self) -> Self {
        let mut cloned =
            Self::uninitialized(self.len).expect("clone should allocate a Metal buffer");
        cloned
            .copy_from(self)
            .expect("clone should copy source Metal buffer");
        cloned
    }
}

impl Drop for U32Buffer {
    fn drop(&mut self) {
        unsafe { ffi::buffer_destroy(self.raw.as_ptr()) };
    }
}

pub fn metal_runtime_support() -> MetalRuntimeSupport {
    if env!("STWO_METAL_BUILD_MODE") == "no-metal" {
        return MetalRuntimeSupport::DisabledByConfiguration;
    }
    if !cfg!(target_os = "macos") {
        return MetalRuntimeSupport::UnsupportedTarget;
    }
    if shared_runtime().is_ok() {
        MetalRuntimeSupport::Available
    } else {
        MetalRuntimeSupport::InitializationFailed
    }
}

pub fn metal_runtime_error() -> Option<String> {
    shared_runtime().err().map(|error| error.message.clone())
}

fn shared_runtime() -> Result<&'static RuntimeHandle, MetalError> {
    static RUNTIME: OnceLock<Result<RuntimeHandle, MetalError>> = OnceLock::new();
    RUNTIME
        .get_or_init(RuntimeHandle::initialize)
        .as_ref()
        .map_err(Clone::clone)
}

impl RuntimeHandle {
    fn initialize() -> Result<Self, MetalError> {
        if env!("STWO_METAL_BUILD_MODE") == "no-metal" {
            return Err(MetalError::new(
                "Metal runtime is disabled by STWO_METAL_MODE=no-metal.",
            ));
        }
        if !cfg!(target_os = "macos") {
            return Err(MetalError::new("Metal runtime requires a macOS host."));
        }
        if STWO_METAL_KERNEL_LIBRARY.is_empty() {
            return Err(MetalError::new(
                "Metal runtime was requested but no embedded `.metallib` was produced.",
            ));
        }

        unsafe {
            ffi::runtime_create(
                STWO_METAL_KERNEL_LIBRARY.as_ptr(),
                STWO_METAL_KERNEL_LIBRARY.len(),
                error_buffer_mut_ptr,
            )
            .map(|raw| Self { raw })
        }
    }
}

fn error_buffer_mut_ptr(buffer: &mut [i8; ERROR_BUFFER_LEN]) -> *mut i8 {
    buffer.as_mut_ptr()
}

fn decode_error_buffer(buffer: &[i8; ERROR_BUFFER_LEN]) -> String {
    unsafe { CStr::from_ptr(buffer.as_ptr()) }
        .to_string_lossy()
        .into_owned()
}

#[cfg(stwo_metal_link)]
mod ffi {
    use super::{c_void, decode_error_buffer, MetalError, NonNull, ERROR_BUFFER_LEN};

    unsafe extern "C" {
        fn stwo_metal_runtime_create(
            metallib_bytes: *const u8,
            metallib_len: usize,
            error_message: *mut i8,
            error_message_len: usize,
        ) -> *mut c_void;
        fn stwo_metal_runtime_destroy(runtime: *mut c_void);
        fn stwo_metal_u32_buffer_from_host(
            runtime: *mut c_void,
            host_ptr: *const u32,
            len: usize,
            error_message: *mut i8,
            error_message_len: usize,
        ) -> *mut c_void;
        fn stwo_metal_u32_buffer_alloc_zeroed(
            runtime: *mut c_void,
            len: usize,
            error_message: *mut i8,
            error_message_len: usize,
        ) -> *mut c_void;
        fn stwo_metal_u32_buffer_alloc_uninitialized(
            runtime: *mut c_void,
            len: usize,
            error_message: *mut i8,
            error_message_len: usize,
        ) -> *mut c_void;
        fn stwo_metal_u32_buffer_destroy(buffer: *mut c_void);
        fn stwo_metal_u32_buffer_read(
            runtime: *mut c_void,
            buffer: *mut c_void,
            host_ptr: *mut u32,
            len: usize,
            error_message: *mut i8,
            error_message_len: usize,
        ) -> bool;
        fn stwo_metal_u32_buffer_host_ptr(buffer: *mut c_void) -> *const u32;
        fn stwo_metal_u32_buffer_get(buffer: *mut c_void, index: usize) -> u32;
        fn stwo_metal_u32_buffer_set(buffer: *mut c_void, index: usize, value: u32);
        fn stwo_metal_u32_buffer_copy(
            src: *mut c_void,
            dst: *mut c_void,
            len: usize,
            dst_offset: usize,
            error_message: *mut i8,
            error_message_len: usize,
        ) -> bool;
        fn stwo_metal_u32_buffer_copy_range(
            src: *mut c_void,
            dst: *mut c_void,
            src_offset: usize,
            len: usize,
            dst_offset: usize,
            error_message: *mut i8,
            error_message_len: usize,
        ) -> bool;
        fn stwo_metal_bit_reverse_u32(
            runtime: *mut c_void,
            buffer: *mut c_void,
            log_len: u32,
            error_message: *mut i8,
            error_message_len: usize,
        ) -> bool;
        fn stwo_metal_bit_reverse_u32x4(
            runtime: *mut c_void,
            buffer: *mut c_void,
            log_len: u32,
            error_message: *mut i8,
            error_message_len: usize,
        ) -> bool;
        fn stwo_metal_invert_m31_values_u32(
            runtime: *mut c_void,
            buffer: *mut c_void,
            len: u32,
            error_message: *mut i8,
            error_message_len: usize,
        ) -> bool;
        fn stwo_metal_precompute_twiddle_level_u32(
            runtime: *mut c_void,
            dst: *mut c_void,
            offset: u32,
            initial_x: u32,
            initial_y: u32,
            step_x: u32,
            step_y: u32,
            level_log_size: u32,
            error_message: *mut i8,
            error_message_len: usize,
        ) -> bool;
        fn stwo_metal_rfft_evaluate_u32(
            runtime: *mut c_void,
            values: *mut c_void,
            twiddles: *mut c_void,
            values_log_len: u32,
            error_message: *mut i8,
            error_message_len: usize,
        ) -> bool;
        fn stwo_metal_rfft_evaluate_subbuffer_u32(
            runtime: *mut c_void,
            values: *mut c_void,
            value_offset: usize,
            values_log_len: u32,
            twiddles: *mut c_void,
            error_message: *mut i8,
            error_message_len: usize,
        ) -> bool;
        fn stwo_metal_ifft_interpolate_u32(
            runtime: *mut c_void,
            values: *mut c_void,
            inverse_twiddles: *mut c_void,
            values_log_len: u32,
            scale_factor: u32,
            error_message: *mut i8,
            error_message_len: usize,
        ) -> bool;
        fn stwo_metal_ifft_line_interpolate_u32(
            runtime: *mut c_void,
            values: *mut c_void,
            inverse_line_twiddles: *mut c_void,
            values_log_len: u32,
            scale_factor: u32,
            error_message: *mut i8,
            error_message_len: usize,
        ) -> bool;
        fn stwo_metal_batch_eval_at_point_base_field_u32(
            runtime: *mut c_void,
            flat_coeffs: *mut c_void,
            factors: *mut c_void,
            dst: *mut c_void,
            coeffs_log_len: u32,
            n_polys: u32,
            error_message: *mut i8,
            error_message_len: usize,
        ) -> bool;
        fn stwo_metal_batch_eval_first_pass_base_field_u32(
            runtime: *mut c_void,
            flat_coeffs: *mut c_void,
            factors: *mut c_void,
            dst: *mut c_void,
            coeffs_log_len: u32,
            n_polys: u32,
            error_message: *mut i8,
            error_message_len: usize,
        ) -> bool;
        fn stwo_metal_fix_first_variable_base_field_u32(
            runtime: *mut c_void,
            src: *mut c_void,
            dst: *mut c_void,
            src_log_len: u32,
            assignment_limbs: *const u32,
            error_message: *mut i8,
            error_message_len: usize,
        ) -> bool;
        fn stwo_metal_fix_first_variable_secure_field_u32x4(
            runtime: *mut c_void,
            src: *mut c_void,
            dst: *mut c_void,
            src_log_len: u32,
            assignment_limbs: *const u32,
            error_message: *mut i8,
            error_message_len: usize,
        ) -> bool;
        fn stwo_metal_gkr_gen_eq_evals_u32x4(
            runtime: *mut c_void,
            factors: *mut c_void,
            dst: *mut c_void,
            y_size: u32,
            v_limbs: *const u32,
            error_message: *mut i8,
            error_message_len: usize,
        ) -> bool;
        fn stwo_metal_gkr_next_grand_product_layer_u32x4(
            runtime: *mut c_void,
            src: *mut c_void,
            dst: *mut c_void,
            input_log_len: u32,
            error_message: *mut i8,
            error_message_len: usize,
        ) -> bool;
        fn stwo_metal_gkr_next_logup_generic_layer_u32x4(
            runtime: *mut c_void,
            numerators: *mut c_void,
            denominators: *mut c_void,
            next_numerators: *mut c_void,
            next_denominators: *mut c_void,
            input_log_len: u32,
            error_message: *mut i8,
            error_message_len: usize,
        ) -> bool;
        fn stwo_metal_gkr_next_logup_multiplicities_layer_u32(
            runtime: *mut c_void,
            numerators: *mut c_void,
            denominators: *mut c_void,
            next_numerators: *mut c_void,
            next_denominators: *mut c_void,
            input_log_len: u32,
            error_message: *mut i8,
            error_message_len: usize,
        ) -> bool;
        fn stwo_metal_gkr_next_logup_singles_layer_u32x4(
            runtime: *mut c_void,
            denominators: *mut c_void,
            next_numerators: *mut c_void,
            next_denominators: *mut c_void,
            input_log_len: u32,
            error_message: *mut i8,
            error_message_len: usize,
        ) -> bool;
        fn stwo_metal_gkr_sum_grand_product_u32x4(
            runtime: *mut c_void,
            eq_evals: *mut c_void,
            input_layer: *mut c_void,
            n_terms: u32,
            eval_at_0_limbs: *mut u32,
            eval_at_2_limbs: *mut u32,
            error_message: *mut i8,
            error_message_len: usize,
        ) -> bool;
        fn stwo_metal_gkr_sum_logup_generic_u32x4(
            runtime: *mut c_void,
            eq_evals: *mut c_void,
            numerators: *mut c_void,
            denominators: *mut c_void,
            n_terms: u32,
            lambda_limbs: *const u32,
            eval_at_0_limbs: *mut u32,
            eval_at_2_limbs: *mut u32,
            error_message: *mut i8,
            error_message_len: usize,
        ) -> bool;
        fn stwo_metal_gkr_sum_logup_multiplicities_u32(
            runtime: *mut c_void,
            eq_evals: *mut c_void,
            numerators: *mut c_void,
            denominators: *mut c_void,
            n_terms: u32,
            lambda_limbs: *const u32,
            eval_at_0_limbs: *mut u32,
            eval_at_2_limbs: *mut u32,
            error_message: *mut i8,
            error_message_len: usize,
        ) -> bool;
        fn stwo_metal_gkr_sum_logup_singles_u32x4(
            runtime: *mut c_void,
            eq_evals: *mut c_void,
            denominators: *mut c_void,
            n_terms: u32,
            lambda_limbs: *const u32,
            eval_at_0_limbs: *mut u32,
            eval_at_2_limbs: *mut u32,
            error_message: *mut i8,
            error_message_len: usize,
        ) -> bool;
        fn stwo_metal_inclusive_prefix_sum_bit_rev_circle_domain_u32(
            runtime: *mut c_void,
            buffer: *mut c_void,
            log_len: u32,
            error_message: *mut i8,
            error_message_len: usize,
        ) -> bool;
        fn stwo_metal_permute_coset_to_circle_domain_bit_reversed_u32(
            runtime: *mut c_void,
            src: *mut c_void,
            dst: *mut c_void,
            log_len: u32,
            error_message: *mut i8,
            error_message_len: usize,
        ) -> bool;
        fn stwo_metal_pack_secure_column_coords_u32x4(
            runtime: *mut c_void,
            coord_0: *mut c_void,
            coord_1: *mut c_void,
            coord_2: *mut c_void,
            coord_3: *mut c_void,
            dst: *mut c_void,
            element_len: u32,
            error_message: *mut i8,
            error_message_len: usize,
        ) -> bool;
        fn stwo_metal_unpack_secure_column_coords_u32x4(
            runtime: *mut c_void,
            src: *mut c_void,
            coord_0: *mut c_void,
            coord_1: *mut c_void,
            coord_2: *mut c_void,
            coord_3: *mut c_void,
            element_len: u32,
            error_message: *mut i8,
            error_message_len: usize,
        ) -> bool;
        fn stwo_metal_accumulate_secure_columns_coords_u32x4(
            runtime: *mut c_void,
            lhs_0: *mut c_void,
            lhs_1: *mut c_void,
            lhs_2: *mut c_void,
            lhs_3: *mut c_void,
            rhs_0: *mut c_void,
            rhs_1: *mut c_void,
            rhs_2: *mut c_void,
            rhs_3: *mut c_void,
            dst_0: *mut c_void,
            dst_1: *mut c_void,
            dst_2: *mut c_void,
            dst_3: *mut c_void,
            element_len: u32,
            error_message: *mut i8,
            error_message_len: usize,
        ) -> bool;
        fn stwo_metal_lift_accumulate_secure_columns_coords_u32x4(
            runtime: *mut c_void,
            lifted_0: *mut c_void,
            lifted_1: *mut c_void,
            lifted_2: *mut c_void,
            lifted_3: *mut c_void,
            current_0: *mut c_void,
            current_1: *mut c_void,
            current_2: *mut c_void,
            current_3: *mut c_void,
            dst_0: *mut c_void,
            dst_1: *mut c_void,
            dst_2: *mut c_void,
            dst_3: *mut c_void,
            current_log_size: u32,
            log_ratio: u32,
            error_message: *mut i8,
            error_message_len: usize,
        ) -> bool;
        fn stwo_metal_fri_fold_circle_into_line_first_layer_u32x4(
            runtime: *mut c_void,
            src: *mut c_void,
            dst: *mut c_void,
            inverse_y_factors: *mut c_void,
            src_log_len: u32,
            alpha_limbs: *const u32,
            error_message: *mut i8,
            error_message_len: usize,
        ) -> bool;
        fn stwo_metal_fri_fold_circle_into_line_accumulate_coords_u32x4(
            runtime: *mut c_void,
            src_0: *mut c_void,
            src_1: *mut c_void,
            src_2: *mut c_void,
            src_3: *mut c_void,
            dst_0: *mut c_void,
            dst_1: *mut c_void,
            dst_2: *mut c_void,
            dst_3: *mut c_void,
            inverse_y_factors: *mut c_void,
            src_log_len: u32,
            alpha_limbs: *const u32,
            alpha_sq_limbs: *const u32,
            error_message: *mut i8,
            error_message_len: usize,
        ) -> bool;
        fn stwo_metal_fri_fold_circle_into_line_first_layer_coords_u32x4(
            runtime: *mut c_void,
            src_0: *mut c_void,
            src_1: *mut c_void,
            src_2: *mut c_void,
            src_3: *mut c_void,
            dst_0: *mut c_void,
            dst_1: *mut c_void,
            dst_2: *mut c_void,
            dst_3: *mut c_void,
            inverse_y_factors: *mut c_void,
            src_log_len: u32,
            alpha_limbs: *const u32,
            error_message: *mut i8,
            error_message_len: usize,
        ) -> bool;
        fn stwo_metal_fri_fold_line_step_coords_u32x4(
            runtime: *mut c_void,
            src_0: *mut c_void,
            src_1: *mut c_void,
            src_2: *mut c_void,
            src_3: *mut c_void,
            dst_0: *mut c_void,
            dst_1: *mut c_void,
            dst_2: *mut c_void,
            dst_3: *mut c_void,
            inverse_x_factors: *mut c_void,
            src_log_len: u32,
            alpha_limbs: *const u32,
            error_message: *mut i8,
            error_message_len: usize,
        ) -> bool;
        fn stwo_metal_fri_fold_line_step_u32x4(
            runtime: *mut c_void,
            src: *mut c_void,
            dst: *mut c_void,
            inverse_x_factors: *mut c_void,
            src_log_len: u32,
            alpha_limbs: *const u32,
            error_message: *mut i8,
            error_message_len: usize,
        ) -> bool;
        fn stwo_metal_generate_wide_fibonacci_trace_u32(
            runtime: *mut c_void,
            input_a: *mut c_void,
            input_b: *mut c_void,
            trace: *mut c_void,
            input_log_len: u32,
            n_columns: u32,
            error_message: *mut i8,
            error_message_len: usize,
        ) -> bool;
        fn stwo_metal_accumulate_wide_fibonacci_quotients_u32x4(
            runtime: *mut c_void,
            trace_evaluations: *mut c_void,
            random_coeff_powers: *mut c_void,
            denominator_inverses: *mut c_void,
            dst: *mut c_void,
            domain_log_size: u32,
            eval_domain_log_size: u32,
            n_constraints: u32,
            error_message: *mut i8,
            error_message_len: usize,
        ) -> bool;
        fn stwo_metal_accumulate_partial_numerators_u32x4(
            runtime: *mut c_void,
            columns: *mut c_void,
            column_indices: *mut c_void,
            b_coeffs: *mut c_void,
            c_coeffs: *mut c_void,
            dst: *mut c_void,
            row_count: u32,
            n_terms: u32,
            error_message: *mut i8,
            error_message_len: usize,
        ) -> bool;
        fn stwo_metal_accumulate_partial_numerators_batched_u32x4(
            runtime: *mut c_void,
            columns: *mut c_void,
            column_indices: *mut c_void,
            b_coeffs: *mut c_void,
            c_coeffs: *mut c_void,
            term_offsets: *mut c_void,
            term_counts: *mut c_void,
            dst: *mut c_void,
            row_count: u32,
            n_batches: u32,
            error_message: *mut i8,
            error_message_len: usize,
        ) -> bool;
        fn stwo_metal_compute_quotients_and_combine_u32x4(
            runtime: *mut c_void,
            partial_coord_0: *mut c_void,
            partial_coord_1: *mut c_void,
            partial_coord_2: *mut c_void,
            partial_coord_3: *mut c_void,
            sample_points: *mut c_void,
            first_linear_terms: *mut c_void,
            partial_log_sizes: *mut c_void,
            partial_offsets: *mut c_void,
            domain_x: *mut c_void,
            domain_y: *mut c_void,
            dst: *mut c_void,
            lifting_log_size: u32,
            n_accumulations: u32,
            error_message: *mut i8,
            error_message_len: usize,
        ) -> bool;
        fn stwo_metal_compute_quotients_and_combine_packed_u32x4(
            runtime: *mut c_void,
            partials: *mut c_void,
            sample_points: *mut c_void,
            first_linear_terms: *mut c_void,
            partial_log_sizes: *mut c_void,
            partial_offsets: *mut c_void,
            domain_x: *mut c_void,
            domain_y: *mut c_void,
            dst: *mut c_void,
            lifting_log_size: u32,
            n_accumulations: u32,
            error_message: *mut i8,
            error_message_len: usize,
        ) -> bool;
        fn stwo_metal_blake2s_build_leaves_lifted_u32(
            runtime: *mut c_void,
            flat_columns: *mut c_void,
            column_offsets: *mut c_void,
            column_log_sizes: *mut c_void,
            dst: *mut c_void,
            n_columns: u32,
            lifting_log_size: u32,
            error_message: *mut i8,
            error_message_len: usize,
        ) -> bool;
        fn stwo_metal_blake2s_build_leaves_lifted_wide_chunk_u32(
            runtime: *mut c_void,
            column_buffers: *const *mut c_void,
            state: *mut c_void,
            dst: *mut c_void,
            column_log_sizes: *const u32,
            n_columns: u32,
            lifting_log_size: u32,
            processed_bytes_before: u32,
            is_first_chunk: u32,
            is_final_chunk: u32,
            error_message: *mut i8,
            error_message_len: usize,
        ) -> bool;
        fn stwo_metal_blake2s_build_next_layer_u32(
            runtime: *mut c_void,
            prev_layer: *mut c_void,
            dst: *mut c_void,
            next_len: u32,
            error_message: *mut i8,
            error_message_len: usize,
        ) -> bool;
        fn stwo_metal_blake2s_build_merkle_layers_u32(
            runtime: *mut c_void,
            leaf_layer: *mut c_void,
            layer_ptrs: *const *mut c_void,
            leaf_log_size: u32,
            error_message: *mut i8,
            error_message_len: usize,
        ) -> bool;
        fn stwo_metal_u32_buffer_read_indices(
            runtime: *mut c_void,
            buffer: *mut c_void,
            indices: *const u32,
            indices_len: usize,
            host_ptr: *mut u32,
            error_message: *mut i8,
            error_message_len: usize,
        ) -> bool;
    }

    pub unsafe fn runtime_create(
        metallib_bytes: *const u8,
        metallib_len: usize,
        error_ptr: fn(&mut [i8; ERROR_BUFFER_LEN]) -> *mut i8,
    ) -> Result<NonNull<c_void>, MetalError> {
        let mut error = [0i8; ERROR_BUFFER_LEN];
        let raw = stwo_metal_runtime_create(
            metallib_bytes,
            metallib_len,
            error_ptr(&mut error),
            error.len(),
        );
        NonNull::new(raw).ok_or_else(|| MetalError::new(decode_error_buffer(&error)))
    }

    pub unsafe fn runtime_destroy(runtime: *mut c_void) {
        stwo_metal_runtime_destroy(runtime);
    }

    pub unsafe fn buffer_from_host(
        runtime: *mut c_void,
        host_ptr: *const u32,
        len: usize,
        error_ptr: fn(&mut [i8; ERROR_BUFFER_LEN]) -> *mut i8,
    ) -> Result<NonNull<c_void>, MetalError> {
        let mut error = [0i8; ERROR_BUFFER_LEN];
        let raw = stwo_metal_u32_buffer_from_host(
            runtime,
            host_ptr,
            len,
            error_ptr(&mut error),
            error.len(),
        );
        NonNull::new(raw).ok_or_else(|| MetalError::new(decode_error_buffer(&error)))
    }

    pub unsafe fn buffer_alloc_zeroed(
        runtime: *mut c_void,
        len: usize,
        error_ptr: fn(&mut [i8; ERROR_BUFFER_LEN]) -> *mut i8,
    ) -> Result<NonNull<c_void>, MetalError> {
        let mut error = [0i8; ERROR_BUFFER_LEN];
        let raw =
            stwo_metal_u32_buffer_alloc_zeroed(runtime, len, error_ptr(&mut error), error.len());
        NonNull::new(raw).ok_or_else(|| MetalError::new(decode_error_buffer(&error)))
    }

    pub unsafe fn buffer_alloc_uninitialized(
        runtime: *mut c_void,
        len: usize,
        error_ptr: fn(&mut [i8; ERROR_BUFFER_LEN]) -> *mut i8,
    ) -> Result<NonNull<c_void>, MetalError> {
        let mut error = [0i8; ERROR_BUFFER_LEN];
        let raw = stwo_metal_u32_buffer_alloc_uninitialized(
            runtime,
            len,
            error_ptr(&mut error),
            error.len(),
        );
        NonNull::new(raw).ok_or_else(|| MetalError::new(decode_error_buffer(&error)))
    }

    pub unsafe fn buffer_destroy(buffer: *mut c_void) {
        stwo_metal_u32_buffer_destroy(buffer);
    }

    pub unsafe fn buffer_read(
        runtime: *mut c_void,
        buffer: *mut c_void,
        host_ptr: *mut u32,
        len: usize,
        error_ptr: fn(&mut [i8; ERROR_BUFFER_LEN]) -> *mut i8,
    ) -> Result<(), MetalError> {
        let mut error = [0i8; ERROR_BUFFER_LEN];
        if stwo_metal_u32_buffer_read(
            runtime,
            buffer,
            host_ptr,
            len,
            error_ptr(&mut error),
            error.len(),
        ) {
            Ok(())
        } else {
            Err(MetalError::new(decode_error_buffer(&error)))
        }
    }

    pub unsafe fn buffer_host_ptr(buffer: *mut c_void) -> *const u32 {
        stwo_metal_u32_buffer_host_ptr(buffer)
    }

    pub unsafe fn buffer_get(buffer: *mut c_void, index: usize) -> u32 {
        stwo_metal_u32_buffer_get(buffer, index)
    }

    pub unsafe fn buffer_set(buffer: *mut c_void, index: usize, value: u32) {
        stwo_metal_u32_buffer_set(buffer, index, value);
    }

    pub unsafe fn buffer_copy(
        src: *mut c_void,
        dst: *mut c_void,
        len: usize,
        dst_offset: usize,
        error_ptr: fn(&mut [i8; ERROR_BUFFER_LEN]) -> *mut i8,
    ) -> Result<(), MetalError> {
        let mut error = [0i8; ERROR_BUFFER_LEN];
        if stwo_metal_u32_buffer_copy(
            src,
            dst,
            len,
            dst_offset,
            error_ptr(&mut error),
            error.len(),
        ) {
            Ok(())
        } else {
            Err(MetalError::new(decode_error_buffer(&error)))
        }
    }

    pub unsafe fn buffer_copy_range(
        src: *mut c_void,
        dst: *mut c_void,
        src_offset: usize,
        len: usize,
        dst_offset: usize,
        error_ptr: fn(&mut [i8; ERROR_BUFFER_LEN]) -> *mut i8,
    ) -> Result<(), MetalError> {
        let mut error = [0i8; ERROR_BUFFER_LEN];
        if stwo_metal_u32_buffer_copy_range(
            src,
            dst,
            src_offset,
            len,
            dst_offset,
            error_ptr(&mut error),
            error.len(),
        ) {
            Ok(())
        } else {
            Err(MetalError::new(decode_error_buffer(&error)))
        }
    }

    pub unsafe fn bit_reverse_u32(
        runtime: *mut c_void,
        buffer: *mut c_void,
        log_len: u32,
        error_ptr: fn(&mut [i8; ERROR_BUFFER_LEN]) -> *mut i8,
    ) -> Result<(), MetalError> {
        let mut error = [0i8; ERROR_BUFFER_LEN];
        if stwo_metal_bit_reverse_u32(runtime, buffer, log_len, error_ptr(&mut error), error.len())
        {
            Ok(())
        } else {
            Err(MetalError::new(decode_error_buffer(&error)))
        }
    }

    pub unsafe fn bit_reverse_u32x4(
        runtime: *mut c_void,
        buffer: *mut c_void,
        log_len: u32,
        error_ptr: fn(&mut [i8; ERROR_BUFFER_LEN]) -> *mut i8,
    ) -> Result<(), MetalError> {
        let mut error = [0i8; ERROR_BUFFER_LEN];
        if stwo_metal_bit_reverse_u32x4(
            runtime,
            buffer,
            log_len,
            error_ptr(&mut error),
            error.len(),
        ) {
            Ok(())
        } else {
            Err(MetalError::new(decode_error_buffer(&error)))
        }
    }

    pub unsafe fn invert_m31_values_u32(
        runtime: *mut c_void,
        buffer: *mut c_void,
        len: usize,
        error_ptr: fn(&mut [i8; ERROR_BUFFER_LEN]) -> *mut i8,
    ) -> Result<(), MetalError> {
        let mut error = [0i8; ERROR_BUFFER_LEN];
        if stwo_metal_invert_m31_values_u32(
            runtime,
            buffer,
            len.try_into()
                .expect("Metal inversion length should fit in u32"),
            error_ptr(&mut error),
            error.len(),
        ) {
            Ok(())
        } else {
            Err(MetalError::new(decode_error_buffer(&error)))
        }
    }

    pub unsafe fn precompute_twiddle_level_u32(
        runtime: *mut c_void,
        dst: *mut c_void,
        offset: usize,
        initial_xy: [u32; 2],
        step_xy: [u32; 2],
        level_log_size: u32,
        error_ptr: fn(&mut [i8; ERROR_BUFFER_LEN]) -> *mut i8,
    ) -> Result<(), MetalError> {
        let mut error = [0i8; ERROR_BUFFER_LEN];
        if stwo_metal_precompute_twiddle_level_u32(
            runtime,
            dst,
            offset
                .try_into()
                .expect("Metal twiddle offset should fit in u32"),
            initial_xy[0],
            initial_xy[1],
            step_xy[0],
            step_xy[1],
            level_log_size,
            error_ptr(&mut error),
            error.len(),
        ) {
            Ok(())
        } else {
            Err(MetalError::new(decode_error_buffer(&error)))
        }
    }

    pub unsafe fn rfft_evaluate_u32(
        runtime: *mut c_void,
        values: *mut c_void,
        twiddles: *mut c_void,
        values_log_len: u32,
        error_ptr: fn(&mut [i8; ERROR_BUFFER_LEN]) -> *mut i8,
    ) -> Result<(), MetalError> {
        let mut error = [0i8; ERROR_BUFFER_LEN];
        if stwo_metal_rfft_evaluate_u32(
            runtime,
            values,
            twiddles,
            values_log_len,
            error_ptr(&mut error),
            error.len(),
        ) {
            Ok(())
        } else {
            Err(MetalError::new(decode_error_buffer(&error)))
        }
    }

    pub unsafe fn rfft_evaluate_subbuffer_u32(
        runtime: *mut c_void,
        values: *mut c_void,
        value_offset: usize,
        values_log_len: u32,
        twiddles: *mut c_void,
        error_ptr: fn(&mut [i8; ERROR_BUFFER_LEN]) -> *mut i8,
    ) -> Result<(), MetalError> {
        let mut error = [0i8; ERROR_BUFFER_LEN];
        if stwo_metal_rfft_evaluate_subbuffer_u32(
            runtime,
            values,
            value_offset,
            values_log_len,
            twiddles,
            error_ptr(&mut error),
            error.len(),
        ) {
            Ok(())
        } else {
            Err(MetalError::new(decode_error_buffer(&error)))
        }
    }

    pub unsafe fn ifft_interpolate_u32(
        runtime: *mut c_void,
        values: *mut c_void,
        inverse_twiddles: *mut c_void,
        values_log_len: u32,
        scale_factor: u32,
        error_ptr: fn(&mut [i8; ERROR_BUFFER_LEN]) -> *mut i8,
    ) -> Result<(), MetalError> {
        let mut error = [0i8; ERROR_BUFFER_LEN];
        if stwo_metal_ifft_interpolate_u32(
            runtime,
            values,
            inverse_twiddles,
            values_log_len,
            scale_factor,
            error_ptr(&mut error),
            error.len(),
        ) {
            Ok(())
        } else {
            Err(MetalError::new(decode_error_buffer(&error)))
        }
    }

    pub unsafe fn ifft_line_interpolate_u32(
        runtime: *mut c_void,
        values: *mut c_void,
        inverse_line_twiddles: *mut c_void,
        values_log_len: u32,
        scale_factor: u32,
        error_ptr: fn(&mut [i8; ERROR_BUFFER_LEN]) -> *mut i8,
    ) -> Result<(), MetalError> {
        let mut error = [0i8; ERROR_BUFFER_LEN];
        if stwo_metal_ifft_line_interpolate_u32(
            runtime,
            values,
            inverse_line_twiddles,
            values_log_len,
            scale_factor,
            error_ptr(&mut error),
            error.len(),
        ) {
            Ok(())
        } else {
            Err(MetalError::new(decode_error_buffer(&error)))
        }
    }

    pub unsafe fn batch_eval_at_point_base_field_u32(
        runtime: *mut c_void,
        flat_coeffs: *mut c_void,
        factors: *mut c_void,
        dst: *mut c_void,
        coeffs_log_len: u32,
        n_polys: u32,
        error_ptr: fn(&mut [i8; ERROR_BUFFER_LEN]) -> *mut i8,
    ) -> Result<(), MetalError> {
        let mut error = [0i8; ERROR_BUFFER_LEN];
        if stwo_metal_batch_eval_at_point_base_field_u32(
            runtime,
            flat_coeffs,
            factors,
            dst,
            coeffs_log_len,
            n_polys,
            error_ptr(&mut error),
            error.len(),
        ) {
            Ok(())
        } else {
            Err(MetalError::new(decode_error_buffer(&error)))
        }
    }

    pub unsafe fn batch_eval_first_pass_base_field_u32(
        runtime: *mut c_void,
        flat_coeffs: *mut c_void,
        factors: *mut c_void,
        dst: *mut c_void,
        coeffs_log_len: u32,
        n_polys: u32,
        error_ptr: fn(&mut [i8; ERROR_BUFFER_LEN]) -> *mut i8,
    ) -> Result<(), MetalError> {
        let mut error = [0i8; ERROR_BUFFER_LEN];
        if stwo_metal_batch_eval_first_pass_base_field_u32(
            runtime,
            flat_coeffs,
            factors,
            dst,
            coeffs_log_len,
            n_polys,
            error_ptr(&mut error),
            error.len(),
        ) {
            Ok(())
        } else {
            Err(MetalError::new(decode_error_buffer(&error)))
        }
    }

    pub unsafe fn permute_coset_to_circle_domain_bit_reversed_u32(
        runtime: *mut c_void,
        src: *mut c_void,
        dst: *mut c_void,
        log_len: u32,
        error_ptr: fn(&mut [i8; ERROR_BUFFER_LEN]) -> *mut i8,
    ) -> Result<(), MetalError> {
        let mut error = [0i8; ERROR_BUFFER_LEN];
        if stwo_metal_permute_coset_to_circle_domain_bit_reversed_u32(
            runtime,
            src,
            dst,
            log_len,
            error_ptr(&mut error),
            error.len(),
        ) {
            Ok(())
        } else {
            Err(MetalError::new(decode_error_buffer(&error)))
        }
    }

    pub unsafe fn pack_secure_column_coords_u32x4(
        runtime: *mut c_void,
        coord_0: *mut c_void,
        coord_1: *mut c_void,
        coord_2: *mut c_void,
        coord_3: *mut c_void,
        dst: *mut c_void,
        element_len: u32,
        error_ptr: fn(&mut [i8; ERROR_BUFFER_LEN]) -> *mut i8,
    ) -> Result<(), MetalError> {
        let mut error = [0i8; ERROR_BUFFER_LEN];
        if stwo_metal_pack_secure_column_coords_u32x4(
            runtime,
            coord_0,
            coord_1,
            coord_2,
            coord_3,
            dst,
            element_len,
            error_ptr(&mut error),
            error.len(),
        ) {
            Ok(())
        } else {
            Err(MetalError::new(decode_error_buffer(&error)))
        }
    }

    pub unsafe fn unpack_secure_column_coords_u32x4(
        runtime: *mut c_void,
        src: *mut c_void,
        coord_0: *mut c_void,
        coord_1: *mut c_void,
        coord_2: *mut c_void,
        coord_3: *mut c_void,
        element_len: u32,
        error_ptr: fn(&mut [i8; ERROR_BUFFER_LEN]) -> *mut i8,
    ) -> Result<(), MetalError> {
        let mut error = [0i8; ERROR_BUFFER_LEN];
        if stwo_metal_unpack_secure_column_coords_u32x4(
            runtime,
            src,
            coord_0,
            coord_1,
            coord_2,
            coord_3,
            element_len,
            error_ptr(&mut error),
            error.len(),
        ) {
            Ok(())
        } else {
            Err(MetalError::new(decode_error_buffer(&error)))
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub unsafe fn accumulate_secure_columns_coords_u32x4(
        runtime: *mut c_void,
        lhs_0: *mut c_void,
        lhs_1: *mut c_void,
        lhs_2: *mut c_void,
        lhs_3: *mut c_void,
        rhs_0: *mut c_void,
        rhs_1: *mut c_void,
        rhs_2: *mut c_void,
        rhs_3: *mut c_void,
        dst_0: *mut c_void,
        dst_1: *mut c_void,
        dst_2: *mut c_void,
        dst_3: *mut c_void,
        element_len: u32,
        error_ptr: fn(&mut [i8; ERROR_BUFFER_LEN]) -> *mut i8,
    ) -> Result<(), MetalError> {
        let mut error = [0i8; ERROR_BUFFER_LEN];
        if stwo_metal_accumulate_secure_columns_coords_u32x4(
            runtime,
            lhs_0,
            lhs_1,
            lhs_2,
            lhs_3,
            rhs_0,
            rhs_1,
            rhs_2,
            rhs_3,
            dst_0,
            dst_1,
            dst_2,
            dst_3,
            element_len,
            error_ptr(&mut error),
            error.len(),
        ) {
            Ok(())
        } else {
            Err(MetalError::new(decode_error_buffer(&error)))
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub unsafe fn lift_accumulate_secure_columns_coords_u32x4(
        runtime: *mut c_void,
        lifted_0: *mut c_void,
        lifted_1: *mut c_void,
        lifted_2: *mut c_void,
        lifted_3: *mut c_void,
        current_0: *mut c_void,
        current_1: *mut c_void,
        current_2: *mut c_void,
        current_3: *mut c_void,
        dst_0: *mut c_void,
        dst_1: *mut c_void,
        dst_2: *mut c_void,
        dst_3: *mut c_void,
        current_log_size: u32,
        log_ratio: u32,
        error_ptr: fn(&mut [i8; ERROR_BUFFER_LEN]) -> *mut i8,
    ) -> Result<(), MetalError> {
        let mut error = [0i8; ERROR_BUFFER_LEN];
        if stwo_metal_lift_accumulate_secure_columns_coords_u32x4(
            runtime,
            lifted_0,
            lifted_1,
            lifted_2,
            lifted_3,
            current_0,
            current_1,
            current_2,
            current_3,
            dst_0,
            dst_1,
            dst_2,
            dst_3,
            current_log_size,
            log_ratio,
            error_ptr(&mut error),
            error.len(),
        ) {
            Ok(())
        } else {
            Err(MetalError::new(decode_error_buffer(&error)))
        }
    }

    pub unsafe fn fix_first_variable_base_field_u32(
        runtime: *mut c_void,
        src: *mut c_void,
        dst: *mut c_void,
        src_log_len: u32,
        assignment_limbs: [u32; 4],
        error_ptr: fn(&mut [i8; ERROR_BUFFER_LEN]) -> *mut i8,
    ) -> Result<(), MetalError> {
        let mut error = [0i8; ERROR_BUFFER_LEN];
        if stwo_metal_fix_first_variable_base_field_u32(
            runtime,
            src,
            dst,
            src_log_len,
            assignment_limbs.as_ptr(),
            error_ptr(&mut error),
            error.len(),
        ) {
            Ok(())
        } else {
            Err(MetalError::new(decode_error_buffer(&error)))
        }
    }

    pub unsafe fn fix_first_variable_secure_field_u32x4(
        runtime: *mut c_void,
        src: *mut c_void,
        dst: *mut c_void,
        src_log_len: u32,
        assignment_limbs: [u32; 4],
        error_ptr: fn(&mut [i8; ERROR_BUFFER_LEN]) -> *mut i8,
    ) -> Result<(), MetalError> {
        let mut error = [0i8; ERROR_BUFFER_LEN];
        if stwo_metal_fix_first_variable_secure_field_u32x4(
            runtime,
            src,
            dst,
            src_log_len,
            assignment_limbs.as_ptr(),
            error_ptr(&mut error),
            error.len(),
        ) {
            Ok(())
        } else {
            Err(MetalError::new(decode_error_buffer(&error)))
        }
    }

    pub unsafe fn gkr_gen_eq_evals_u32x4(
        runtime: *mut c_void,
        factors: *mut c_void,
        dst: *mut c_void,
        y_size: u32,
        v_limbs: [u32; 4],
        error_ptr: fn(&mut [i8; ERROR_BUFFER_LEN]) -> *mut i8,
    ) -> Result<(), MetalError> {
        let mut error = [0i8; ERROR_BUFFER_LEN];
        if stwo_metal_gkr_gen_eq_evals_u32x4(
            runtime,
            factors,
            dst,
            y_size,
            v_limbs.as_ptr(),
            error_ptr(&mut error),
            error.len(),
        ) {
            Ok(())
        } else {
            Err(MetalError::new(decode_error_buffer(&error)))
        }
    }

    pub unsafe fn gkr_next_grand_product_layer_u32x4(
        runtime: *mut c_void,
        src: *mut c_void,
        dst: *mut c_void,
        input_log_len: u32,
        error_ptr: fn(&mut [i8; ERROR_BUFFER_LEN]) -> *mut i8,
    ) -> Result<(), MetalError> {
        let mut error = [0i8; ERROR_BUFFER_LEN];
        if stwo_metal_gkr_next_grand_product_layer_u32x4(
            runtime,
            src,
            dst,
            input_log_len,
            error_ptr(&mut error),
            error.len(),
        ) {
            Ok(())
        } else {
            Err(MetalError::new(decode_error_buffer(&error)))
        }
    }

    pub unsafe fn gkr_next_logup_generic_layer_u32x4(
        runtime: *mut c_void,
        numerators: *mut c_void,
        denominators: *mut c_void,
        next_numerators: *mut c_void,
        next_denominators: *mut c_void,
        input_log_len: u32,
        error_ptr: fn(&mut [i8; ERROR_BUFFER_LEN]) -> *mut i8,
    ) -> Result<(), MetalError> {
        let mut error = [0i8; ERROR_BUFFER_LEN];
        if stwo_metal_gkr_next_logup_generic_layer_u32x4(
            runtime,
            numerators,
            denominators,
            next_numerators,
            next_denominators,
            input_log_len,
            error_ptr(&mut error),
            error.len(),
        ) {
            Ok(())
        } else {
            Err(MetalError::new(decode_error_buffer(&error)))
        }
    }

    pub unsafe fn gkr_next_logup_multiplicities_layer_u32(
        runtime: *mut c_void,
        numerators: *mut c_void,
        denominators: *mut c_void,
        next_numerators: *mut c_void,
        next_denominators: *mut c_void,
        input_log_len: u32,
        error_ptr: fn(&mut [i8; ERROR_BUFFER_LEN]) -> *mut i8,
    ) -> Result<(), MetalError> {
        let mut error = [0i8; ERROR_BUFFER_LEN];
        if stwo_metal_gkr_next_logup_multiplicities_layer_u32(
            runtime,
            numerators,
            denominators,
            next_numerators,
            next_denominators,
            input_log_len,
            error_ptr(&mut error),
            error.len(),
        ) {
            Ok(())
        } else {
            Err(MetalError::new(decode_error_buffer(&error)))
        }
    }

    pub unsafe fn gkr_next_logup_singles_layer_u32x4(
        runtime: *mut c_void,
        denominators: *mut c_void,
        next_numerators: *mut c_void,
        next_denominators: *mut c_void,
        input_log_len: u32,
        error_ptr: fn(&mut [i8; ERROR_BUFFER_LEN]) -> *mut i8,
    ) -> Result<(), MetalError> {
        let mut error = [0i8; ERROR_BUFFER_LEN];
        if stwo_metal_gkr_next_logup_singles_layer_u32x4(
            runtime,
            denominators,
            next_numerators,
            next_denominators,
            input_log_len,
            error_ptr(&mut error),
            error.len(),
        ) {
            Ok(())
        } else {
            Err(MetalError::new(decode_error_buffer(&error)))
        }
    }

    pub unsafe fn gkr_sum_grand_product_u32x4(
        runtime: *mut c_void,
        eq_evals: *mut c_void,
        input_layer: *mut c_void,
        n_terms: u32,
        error_ptr: fn(&mut [i8; ERROR_BUFFER_LEN]) -> *mut i8,
    ) -> Result<([u32; 4], [u32; 4]), MetalError> {
        let mut error = [0i8; ERROR_BUFFER_LEN];
        let mut eval_at_0 = [0u32; 4];
        let mut eval_at_2 = [0u32; 4];
        if stwo_metal_gkr_sum_grand_product_u32x4(
            runtime,
            eq_evals,
            input_layer,
            n_terms,
            eval_at_0.as_mut_ptr(),
            eval_at_2.as_mut_ptr(),
            error_ptr(&mut error),
            error.len(),
        ) {
            Ok((eval_at_0, eval_at_2))
        } else {
            Err(MetalError::new(decode_error_buffer(&error)))
        }
    }

    pub unsafe fn gkr_sum_logup_generic_u32x4(
        runtime: *mut c_void,
        eq_evals: *mut c_void,
        numerators: *mut c_void,
        denominators: *mut c_void,
        n_terms: u32,
        lambda_limbs: [u32; 4],
        error_ptr: fn(&mut [i8; ERROR_BUFFER_LEN]) -> *mut i8,
    ) -> Result<([u32; 4], [u32; 4]), MetalError> {
        let mut error = [0i8; ERROR_BUFFER_LEN];
        let mut eval_at_0 = [0u32; 4];
        let mut eval_at_2 = [0u32; 4];
        if stwo_metal_gkr_sum_logup_generic_u32x4(
            runtime,
            eq_evals,
            numerators,
            denominators,
            n_terms,
            lambda_limbs.as_ptr(),
            eval_at_0.as_mut_ptr(),
            eval_at_2.as_mut_ptr(),
            error_ptr(&mut error),
            error.len(),
        ) {
            Ok((eval_at_0, eval_at_2))
        } else {
            Err(MetalError::new(decode_error_buffer(&error)))
        }
    }

    pub unsafe fn gkr_sum_logup_multiplicities_u32(
        runtime: *mut c_void,
        eq_evals: *mut c_void,
        numerators: *mut c_void,
        denominators: *mut c_void,
        n_terms: u32,
        lambda_limbs: [u32; 4],
        error_ptr: fn(&mut [i8; ERROR_BUFFER_LEN]) -> *mut i8,
    ) -> Result<([u32; 4], [u32; 4]), MetalError> {
        let mut error = [0i8; ERROR_BUFFER_LEN];
        let mut eval_at_0 = [0u32; 4];
        let mut eval_at_2 = [0u32; 4];
        if stwo_metal_gkr_sum_logup_multiplicities_u32(
            runtime,
            eq_evals,
            numerators,
            denominators,
            n_terms,
            lambda_limbs.as_ptr(),
            eval_at_0.as_mut_ptr(),
            eval_at_2.as_mut_ptr(),
            error_ptr(&mut error),
            error.len(),
        ) {
            Ok((eval_at_0, eval_at_2))
        } else {
            Err(MetalError::new(decode_error_buffer(&error)))
        }
    }

    pub unsafe fn gkr_sum_logup_singles_u32x4(
        runtime: *mut c_void,
        eq_evals: *mut c_void,
        denominators: *mut c_void,
        n_terms: u32,
        lambda_limbs: [u32; 4],
        error_ptr: fn(&mut [i8; ERROR_BUFFER_LEN]) -> *mut i8,
    ) -> Result<([u32; 4], [u32; 4]), MetalError> {
        let mut error = [0i8; ERROR_BUFFER_LEN];
        let mut eval_at_0 = [0u32; 4];
        let mut eval_at_2 = [0u32; 4];
        if stwo_metal_gkr_sum_logup_singles_u32x4(
            runtime,
            eq_evals,
            denominators,
            n_terms,
            lambda_limbs.as_ptr(),
            eval_at_0.as_mut_ptr(),
            eval_at_2.as_mut_ptr(),
            error_ptr(&mut error),
            error.len(),
        ) {
            Ok((eval_at_0, eval_at_2))
        } else {
            Err(MetalError::new(decode_error_buffer(&error)))
        }
    }

    pub unsafe fn inclusive_prefix_sum_bit_rev_circle_domain_u32(
        runtime: *mut c_void,
        buffer: *mut c_void,
        log_len: u32,
        error_ptr: fn(&mut [i8; ERROR_BUFFER_LEN]) -> *mut i8,
    ) -> Result<(), MetalError> {
        let mut error = [0i8; ERROR_BUFFER_LEN];
        if stwo_metal_inclusive_prefix_sum_bit_rev_circle_domain_u32(
            runtime,
            buffer,
            log_len,
            error_ptr(&mut error),
            error.len(),
        ) {
            Ok(())
        } else {
            Err(MetalError::new(decode_error_buffer(&error)))
        }
    }

    pub unsafe fn fri_fold_circle_into_line_first_layer_u32x4(
        runtime: *mut c_void,
        src: *mut c_void,
        dst: *mut c_void,
        inverse_y_factors: *mut c_void,
        src_log_len: u32,
        alpha_limbs: [u32; 4],
        error_ptr: fn(&mut [i8; ERROR_BUFFER_LEN]) -> *mut i8,
    ) -> Result<(), MetalError> {
        let mut error = [0i8; ERROR_BUFFER_LEN];
        if stwo_metal_fri_fold_circle_into_line_first_layer_u32x4(
            runtime,
            src,
            dst,
            inverse_y_factors,
            src_log_len,
            alpha_limbs.as_ptr(),
            error_ptr(&mut error),
            error.len(),
        ) {
            Ok(())
        } else {
            Err(MetalError::new(decode_error_buffer(&error)))
        }
    }

    pub unsafe fn fri_fold_circle_into_line_accumulate_coords_u32x4(
        runtime: *mut c_void,
        src_0: *mut c_void,
        src_1: *mut c_void,
        src_2: *mut c_void,
        src_3: *mut c_void,
        dst_0: *mut c_void,
        dst_1: *mut c_void,
        dst_2: *mut c_void,
        dst_3: *mut c_void,
        inverse_y_factors: *mut c_void,
        src_log_len: u32,
        alpha_limbs: [u32; 4],
        alpha_sq_limbs: [u32; 4],
        error_ptr: fn(&mut [i8; ERROR_BUFFER_LEN]) -> *mut i8,
    ) -> Result<(), MetalError> {
        let mut error = [0i8; ERROR_BUFFER_LEN];
        if stwo_metal_fri_fold_circle_into_line_accumulate_coords_u32x4(
            runtime,
            src_0,
            src_1,
            src_2,
            src_3,
            dst_0,
            dst_1,
            dst_2,
            dst_3,
            inverse_y_factors,
            src_log_len,
            alpha_limbs.as_ptr(),
            alpha_sq_limbs.as_ptr(),
            error_ptr(&mut error),
            error.len(),
        ) {
            Ok(())
        } else {
            Err(MetalError::new(decode_error_buffer(&error)))
        }
    }

    pub unsafe fn fri_fold_circle_into_line_first_layer_coords_u32x4(
        runtime: *mut c_void,
        src_0: *mut c_void,
        src_1: *mut c_void,
        src_2: *mut c_void,
        src_3: *mut c_void,
        dst_0: *mut c_void,
        dst_1: *mut c_void,
        dst_2: *mut c_void,
        dst_3: *mut c_void,
        inverse_y_factors: *mut c_void,
        src_log_len: u32,
        alpha_limbs: [u32; 4],
        error_ptr: fn(&mut [i8; ERROR_BUFFER_LEN]) -> *mut i8,
    ) -> Result<(), MetalError> {
        let mut error = [0i8; ERROR_BUFFER_LEN];
        if stwo_metal_fri_fold_circle_into_line_first_layer_coords_u32x4(
            runtime,
            src_0,
            src_1,
            src_2,
            src_3,
            dst_0,
            dst_1,
            dst_2,
            dst_3,
            inverse_y_factors,
            src_log_len,
            alpha_limbs.as_ptr(),
            error_ptr(&mut error),
            error.len(),
        ) {
            Ok(())
        } else {
            Err(MetalError::new(decode_error_buffer(&error)))
        }
    }

    pub unsafe fn fri_fold_line_step_coords_u32x4(
        runtime: *mut c_void,
        src_0: *mut c_void,
        src_1: *mut c_void,
        src_2: *mut c_void,
        src_3: *mut c_void,
        dst_0: *mut c_void,
        dst_1: *mut c_void,
        dst_2: *mut c_void,
        dst_3: *mut c_void,
        inverse_x_factors: *mut c_void,
        src_log_len: u32,
        alpha_limbs: [u32; 4],
        error_ptr: fn(&mut [i8; ERROR_BUFFER_LEN]) -> *mut i8,
    ) -> Result<(), MetalError> {
        let mut error = [0i8; ERROR_BUFFER_LEN];
        if stwo_metal_fri_fold_line_step_coords_u32x4(
            runtime,
            src_0,
            src_1,
            src_2,
            src_3,
            dst_0,
            dst_1,
            dst_2,
            dst_3,
            inverse_x_factors,
            src_log_len,
            alpha_limbs.as_ptr(),
            error_ptr(&mut error),
            error.len(),
        ) {
            Ok(())
        } else {
            Err(MetalError::new(decode_error_buffer(&error)))
        }
    }

    pub unsafe fn fri_fold_line_step_u32x4(
        runtime: *mut c_void,
        src: *mut c_void,
        dst: *mut c_void,
        inverse_x_factors: *mut c_void,
        src_log_len: u32,
        alpha_limbs: [u32; 4],
        error_ptr: fn(&mut [i8; ERROR_BUFFER_LEN]) -> *mut i8,
    ) -> Result<(), MetalError> {
        let mut error = [0i8; ERROR_BUFFER_LEN];
        if stwo_metal_fri_fold_line_step_u32x4(
            runtime,
            src,
            dst,
            inverse_x_factors,
            src_log_len,
            alpha_limbs.as_ptr(),
            error_ptr(&mut error),
            error.len(),
        ) {
            Ok(())
        } else {
            Err(MetalError::new(decode_error_buffer(&error)))
        }
    }

    pub unsafe fn generate_wide_fibonacci_trace_u32(
        runtime: *mut c_void,
        input_a: *mut c_void,
        input_b: *mut c_void,
        trace: *mut c_void,
        input_log_len: u32,
        n_columns: u32,
        error_ptr: fn(&mut [i8; ERROR_BUFFER_LEN]) -> *mut i8,
    ) -> Result<(), MetalError> {
        let mut error = [0i8; ERROR_BUFFER_LEN];
        if stwo_metal_generate_wide_fibonacci_trace_u32(
            runtime,
            input_a,
            input_b,
            trace,
            input_log_len,
            n_columns,
            error_ptr(&mut error),
            error.len(),
        ) {
            Ok(())
        } else {
            Err(MetalError::new(decode_error_buffer(&error)))
        }
    }

    pub unsafe fn accumulate_wide_fibonacci_quotients_u32x4(
        runtime: *mut c_void,
        trace_evaluations: *mut c_void,
        random_coeff_powers: *mut c_void,
        denominator_inverses: *mut c_void,
        dst: *mut c_void,
        domain_log_size: u32,
        eval_domain_log_size: u32,
        n_constraints: u32,
        error_ptr: fn(&mut [i8; ERROR_BUFFER_LEN]) -> *mut i8,
    ) -> Result<(), MetalError> {
        let mut error = [0i8; ERROR_BUFFER_LEN];
        if stwo_metal_accumulate_wide_fibonacci_quotients_u32x4(
            runtime,
            trace_evaluations,
            random_coeff_powers,
            denominator_inverses,
            dst,
            domain_log_size,
            eval_domain_log_size,
            n_constraints,
            error_ptr(&mut error),
            error.len(),
        ) {
            Ok(())
        } else {
            Err(MetalError::new(decode_error_buffer(&error)))
        }
    }

    pub unsafe fn accumulate_partial_numerators_u32x4(
        runtime: *mut c_void,
        columns: *mut c_void,
        column_indices: *mut c_void,
        b_coeffs: *mut c_void,
        c_coeffs: *mut c_void,
        dst: *mut c_void,
        row_count: u32,
        n_terms: u32,
        error_ptr: fn(&mut [i8; ERROR_BUFFER_LEN]) -> *mut i8,
    ) -> Result<(), MetalError> {
        let mut error = [0i8; ERROR_BUFFER_LEN];
        if stwo_metal_accumulate_partial_numerators_u32x4(
            runtime,
            columns,
            column_indices,
            b_coeffs,
            c_coeffs,
            dst,
            row_count,
            n_terms,
            error_ptr(&mut error),
            error.len(),
        ) {
            Ok(())
        } else {
            Err(MetalError::new(decode_error_buffer(&error)))
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub unsafe fn accumulate_partial_numerators_batched_u32x4(
        runtime: *mut c_void,
        columns: *mut c_void,
        column_indices: *mut c_void,
        b_coeffs: *mut c_void,
        c_coeffs: *mut c_void,
        term_offsets: *mut c_void,
        term_counts: *mut c_void,
        dst: *mut c_void,
        row_count: u32,
        n_batches: u32,
        error_ptr: fn(&mut [i8; ERROR_BUFFER_LEN]) -> *mut i8,
    ) -> Result<(), MetalError> {
        let mut error = [0i8; ERROR_BUFFER_LEN];
        if stwo_metal_accumulate_partial_numerators_batched_u32x4(
            runtime,
            columns,
            column_indices,
            b_coeffs,
            c_coeffs,
            term_offsets,
            term_counts,
            dst,
            row_count,
            n_batches,
            error_ptr(&mut error),
            error.len(),
        ) {
            Ok(())
        } else {
            Err(MetalError::new(decode_error_buffer(&error)))
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub unsafe fn compute_quotients_and_combine_u32x4(
        runtime: *mut c_void,
        partial_coord_0: *mut c_void,
        partial_coord_1: *mut c_void,
        partial_coord_2: *mut c_void,
        partial_coord_3: *mut c_void,
        sample_points: *mut c_void,
        first_linear_terms: *mut c_void,
        partial_log_sizes: *mut c_void,
        partial_offsets: *mut c_void,
        domain_x: *mut c_void,
        domain_y: *mut c_void,
        dst: *mut c_void,
        lifting_log_size: u32,
        n_accumulations: u32,
        error_ptr: fn(&mut [i8; ERROR_BUFFER_LEN]) -> *mut i8,
    ) -> Result<(), MetalError> {
        let mut error = [0i8; ERROR_BUFFER_LEN];
        if stwo_metal_compute_quotients_and_combine_u32x4(
            runtime,
            partial_coord_0,
            partial_coord_1,
            partial_coord_2,
            partial_coord_3,
            sample_points,
            first_linear_terms,
            partial_log_sizes,
            partial_offsets,
            domain_x,
            domain_y,
            dst,
            lifting_log_size,
            n_accumulations,
            error_ptr(&mut error),
            error.len(),
        ) {
            Ok(())
        } else {
            Err(MetalError::new(decode_error_buffer(&error)))
        }
    }

    pub unsafe fn compute_quotients_and_combine_packed_u32x4(
        runtime: *mut c_void,
        partials: *mut c_void,
        sample_points: *mut c_void,
        first_linear_terms: *mut c_void,
        partial_log_sizes: *mut c_void,
        partial_offsets: *mut c_void,
        domain_x: *mut c_void,
        domain_y: *mut c_void,
        dst: *mut c_void,
        lifting_log_size: u32,
        n_accumulations: u32,
        error_ptr: fn(&mut [i8; ERROR_BUFFER_LEN]) -> *mut i8,
    ) -> Result<(), MetalError> {
        let mut error = [0i8; ERROR_BUFFER_LEN];
        if stwo_metal_compute_quotients_and_combine_packed_u32x4(
            runtime,
            partials,
            sample_points,
            first_linear_terms,
            partial_log_sizes,
            partial_offsets,
            domain_x,
            domain_y,
            dst,
            lifting_log_size,
            n_accumulations,
            error_ptr(&mut error),
            error.len(),
        ) {
            Ok(())
        } else {
            Err(MetalError::new(decode_error_buffer(&error)))
        }
    }

    pub unsafe fn blake2s_build_leaves_lifted_u32(
        runtime: *mut c_void,
        flat_columns: *mut c_void,
        column_offsets: *mut c_void,
        column_log_sizes: *mut c_void,
        dst: *mut c_void,
        n_columns: u32,
        lifting_log_size: u32,
        error_ptr: fn(&mut [i8; ERROR_BUFFER_LEN]) -> *mut i8,
    ) -> Result<(), MetalError> {
        let mut error = [0i8; ERROR_BUFFER_LEN];
        if stwo_metal_blake2s_build_leaves_lifted_u32(
            runtime,
            flat_columns,
            column_offsets,
            column_log_sizes,
            dst,
            n_columns,
            lifting_log_size,
            error_ptr(&mut error),
            error.len(),
        ) {
            Ok(())
        } else {
            Err(MetalError::new(decode_error_buffer(&error)))
        }
    }

    pub unsafe fn blake2s_build_leaves_lifted_wide_chunk_u32(
        runtime: *mut c_void,
        column_buffers: *const *mut c_void,
        state: *mut c_void,
        dst: *mut c_void,
        column_log_sizes: *const u32,
        n_columns: u32,
        lifting_log_size: u32,
        processed_bytes_before: u32,
        is_first_chunk: bool,
        is_final_chunk: bool,
        error_ptr: fn(&mut [i8; ERROR_BUFFER_LEN]) -> *mut i8,
    ) -> Result<(), MetalError> {
        let mut error = [0i8; ERROR_BUFFER_LEN];
        if stwo_metal_blake2s_build_leaves_lifted_wide_chunk_u32(
            runtime,
            column_buffers,
            state,
            dst,
            column_log_sizes,
            n_columns,
            lifting_log_size,
            processed_bytes_before,
            is_first_chunk as u32,
            is_final_chunk as u32,
            error_ptr(&mut error),
            error.len(),
        ) {
            Ok(())
        } else {
            Err(MetalError::new(decode_error_buffer(&error)))
        }
    }

    pub unsafe fn blake2s_build_next_layer_u32(
        runtime: *mut c_void,
        prev_layer: *mut c_void,
        dst: *mut c_void,
        next_len: u32,
        error_ptr: fn(&mut [i8; ERROR_BUFFER_LEN]) -> *mut i8,
    ) -> Result<(), MetalError> {
        let mut error = [0i8; ERROR_BUFFER_LEN];
        if stwo_metal_blake2s_build_next_layer_u32(
            runtime,
            prev_layer,
            dst,
            next_len,
            error_ptr(&mut error),
            error.len(),
        ) {
            Ok(())
        } else {
            Err(MetalError::new(decode_error_buffer(&error)))
        }
    }

    pub unsafe fn blake2s_build_merkle_layers_u32(
        runtime: *mut c_void,
        leaf_layer: *mut c_void,
        layer_ptrs: *const *mut c_void,
        leaf_log_size: u32,
        error_ptr: fn(&mut [i8; ERROR_BUFFER_LEN]) -> *mut i8,
    ) -> Result<(), MetalError> {
        let mut error = [0i8; ERROR_BUFFER_LEN];
        if stwo_metal_blake2s_build_merkle_layers_u32(
            runtime,
            leaf_layer,
            layer_ptrs,
            leaf_log_size,
            error_ptr(&mut error),
            error.len(),
        ) {
            Ok(())
        } else {
            Err(MetalError::new(decode_error_buffer(&error)))
        }
    }

    pub unsafe fn buffer_read_indices(
        runtime: *mut c_void,
        buffer: *mut c_void,
        indices: *const u32,
        indices_len: usize,
        host_ptr: *mut u32,
        error_ptr: fn(&mut [i8; ERROR_BUFFER_LEN]) -> *mut i8,
    ) -> Result<(), MetalError> {
        let mut error = [0i8; ERROR_BUFFER_LEN];
        if stwo_metal_u32_buffer_read_indices(
            runtime,
            buffer,
            indices,
            indices_len,
            host_ptr,
            error_ptr(&mut error),
            error.len(),
        ) {
            Ok(())
        } else {
            Err(MetalError::new(decode_error_buffer(&error)))
        }
    }
}

#[cfg(not(stwo_metal_link))]
mod ffi {
    use super::{c_void, MetalError, NonNull};

    pub unsafe fn runtime_create(
        _metallib_bytes: *const u8,
        _metallib_len: usize,
        _error_ptr: fn(&mut [i8; 512]) -> *mut i8,
    ) -> Result<NonNull<c_void>, MetalError> {
        Err(MetalError::new(
            "Metal support was not linked into stwo-metal-sys.",
        ))
    }

    pub unsafe fn runtime_destroy(_runtime: *mut c_void) {}

    pub unsafe fn buffer_from_host(
        _runtime: *mut c_void,
        _host_ptr: *const u32,
        _len: usize,
        _error_ptr: fn(&mut [i8; 512]) -> *mut i8,
    ) -> Result<NonNull<c_void>, MetalError> {
        Err(MetalError::new(
            "Metal support was not linked into stwo-metal-sys.",
        ))
    }

    pub unsafe fn buffer_alloc_zeroed(
        _runtime: *mut c_void,
        _len: usize,
        _error_ptr: fn(&mut [i8; 512]) -> *mut i8,
    ) -> Result<NonNull<c_void>, MetalError> {
        Err(MetalError::new(
            "Metal support was not linked into stwo-metal-sys.",
        ))
    }

    pub unsafe fn buffer_alloc_uninitialized(
        _runtime: *mut c_void,
        _len: usize,
        _error_ptr: fn(&mut [i8; 512]) -> *mut i8,
    ) -> Result<NonNull<c_void>, MetalError> {
        Err(MetalError::new(
            "Metal support was not linked into stwo-metal-sys.",
        ))
    }

    pub unsafe fn buffer_destroy(_buffer: *mut c_void) {}

    pub unsafe fn buffer_read(
        _runtime: *mut c_void,
        _buffer: *mut c_void,
        _host_ptr: *mut u32,
        _len: usize,
        _error_ptr: fn(&mut [i8; 512]) -> *mut i8,
    ) -> Result<(), MetalError> {
        Err(MetalError::new(
            "Metal support was not linked into stwo-metal-sys.",
        ))
    }

    pub unsafe fn buffer_host_ptr(_buffer: *mut c_void) -> *const u32 {
        core::ptr::null()
    }

    pub unsafe fn buffer_get(_buffer: *mut c_void, _index: usize) -> u32 {
        panic!("Metal buffer access was requested without linked Metal support")
    }

    pub unsafe fn buffer_set(_buffer: *mut c_void, _index: usize, _value: u32) {
        panic!("Metal buffer mutation was requested without linked Metal support")
    }

    pub unsafe fn buffer_copy(
        _src: *mut c_void,
        _dst: *mut c_void,
        _len: usize,
        _dst_offset: usize,
        _error_ptr: fn(&mut [i8; 512]) -> *mut i8,
    ) -> Result<(), MetalError> {
        Err(MetalError::new(
            "Metal support was not linked into stwo-metal-sys.",
        ))
    }

    pub unsafe fn buffer_copy_range(
        _src: *mut c_void,
        _dst: *mut c_void,
        _src_offset: usize,
        _len: usize,
        _dst_offset: usize,
        _error_ptr: fn(&mut [i8; 512]) -> *mut i8,
    ) -> Result<(), MetalError> {
        Err(MetalError::new(
            "Metal support was not linked into stwo-metal-sys.",
        ))
    }

    pub unsafe fn bit_reverse_u32(
        _runtime: *mut c_void,
        _buffer: *mut c_void,
        _log_len: u32,
        _error_ptr: fn(&mut [i8; 512]) -> *mut i8,
    ) -> Result<(), MetalError> {
        Err(MetalError::new(
            "Metal support was not linked into stwo-metal-sys.",
        ))
    }

    pub unsafe fn bit_reverse_u32x4(
        _runtime: *mut c_void,
        _buffer: *mut c_void,
        _log_len: u32,
        _error_ptr: fn(&mut [i8; 512]) -> *mut i8,
    ) -> Result<(), MetalError> {
        Err(MetalError::new(
            "Metal support was not linked into stwo-metal-sys.",
        ))
    }

    pub unsafe fn invert_m31_values_u32(
        _runtime: *mut c_void,
        _buffer: *mut c_void,
        _len: usize,
        _error_ptr: fn(&mut [i8; 512]) -> *mut i8,
    ) -> Result<(), MetalError> {
        Err(MetalError::new(
            "Metal support was not linked into stwo-metal-sys.",
        ))
    }

    pub unsafe fn precompute_twiddle_level_u32(
        _runtime: *mut c_void,
        _dst: *mut c_void,
        _offset: usize,
        _initial_xy: [u32; 2],
        _step_xy: [u32; 2],
        _level_log_size: u32,
        _error_ptr: fn(&mut [i8; 512]) -> *mut i8,
    ) -> Result<(), MetalError> {
        Err(MetalError::new(
            "Metal support was not linked into stwo-metal-sys.",
        ))
    }

    pub unsafe fn rfft_evaluate_u32(
        _runtime: *mut c_void,
        _values: *mut c_void,
        _twiddles: *mut c_void,
        _values_log_len: u32,
        _error_ptr: fn(&mut [i8; 512]) -> *mut i8,
    ) -> Result<(), MetalError> {
        Err(MetalError::new(
            "Metal support was not linked into stwo-metal-sys.",
        ))
    }

    pub unsafe fn ifft_interpolate_u32(
        _runtime: *mut c_void,
        _values: *mut c_void,
        _inverse_twiddles: *mut c_void,
        _values_log_len: u32,
        _scale_factor: u32,
        _error_ptr: fn(&mut [i8; 512]) -> *mut i8,
    ) -> Result<(), MetalError> {
        Err(MetalError::new(
            "Metal support was not linked into stwo-metal-sys.",
        ))
    }

    pub unsafe fn ifft_line_interpolate_u32(
        _runtime: *mut c_void,
        _values: *mut c_void,
        _inverse_line_twiddles: *mut c_void,
        _values_log_len: u32,
        _scale_factor: u32,
        _error_ptr: fn(&mut [i8; 512]) -> *mut i8,
    ) -> Result<(), MetalError> {
        Err(MetalError::new(
            "Metal support was not linked into stwo-metal-sys.",
        ))
    }

    pub unsafe fn batch_eval_at_point_base_field_u32(
        _runtime: *mut c_void,
        _flat_coeffs: *mut c_void,
        _factors: *mut c_void,
        _dst: *mut c_void,
        _coeffs_log_len: u32,
        _n_polys: u32,
        _error_ptr: fn(&mut [i8; 512]) -> *mut i8,
    ) -> Result<(), MetalError> {
        Err(MetalError::new(
            "Metal support was not linked into stwo-metal-sys.",
        ))
    }

    pub unsafe fn batch_eval_first_pass_base_field_u32(
        _runtime: *mut c_void,
        _flat_coeffs: *mut c_void,
        _factors: *mut c_void,
        _dst: *mut c_void,
        _coeffs_log_len: u32,
        _n_polys: u32,
        _error_ptr: fn(&mut [i8; 512]) -> *mut i8,
    ) -> Result<(), MetalError> {
        Err(MetalError::new(
            "Metal support was not linked into stwo-metal-sys.",
        ))
    }

    pub unsafe fn permute_coset_to_circle_domain_bit_reversed_u32(
        _runtime: *mut c_void,
        _src: *mut c_void,
        _dst: *mut c_void,
        _log_len: u32,
        _error_ptr: fn(&mut [i8; 512]) -> *mut i8,
    ) -> Result<(), MetalError> {
        Err(MetalError::new(
            "Metal support was not linked into stwo-metal-sys.",
        ))
    }

    pub unsafe fn pack_secure_column_coords_u32x4(
        _runtime: *mut c_void,
        _coord_0: *mut c_void,
        _coord_1: *mut c_void,
        _coord_2: *mut c_void,
        _coord_3: *mut c_void,
        _dst: *mut c_void,
        _element_len: u32,
        _error_ptr: fn(&mut [i8; 512]) -> *mut i8,
    ) -> Result<(), MetalError> {
        Err(MetalError::new(
            "Metal support was not linked into stwo-metal-sys.",
        ))
    }

    pub unsafe fn unpack_secure_column_coords_u32x4(
        _runtime: *mut c_void,
        _src: *mut c_void,
        _coord_0: *mut c_void,
        _coord_1: *mut c_void,
        _coord_2: *mut c_void,
        _coord_3: *mut c_void,
        _element_len: u32,
        _error_ptr: fn(&mut [i8; 512]) -> *mut i8,
    ) -> Result<(), MetalError> {
        Err(MetalError::new(
            "Metal support was not linked into stwo-metal-sys.",
        ))
    }

    pub unsafe fn accumulate_secure_columns_coords_u32x4(
        _runtime: *mut c_void,
        _lhs_0: *mut c_void,
        _lhs_1: *mut c_void,
        _lhs_2: *mut c_void,
        _lhs_3: *mut c_void,
        _rhs_0: *mut c_void,
        _rhs_1: *mut c_void,
        _rhs_2: *mut c_void,
        _rhs_3: *mut c_void,
        _dst_0: *mut c_void,
        _dst_1: *mut c_void,
        _dst_2: *mut c_void,
        _dst_3: *mut c_void,
        _element_len: u32,
        _error_ptr: fn(&mut [i8; 512]) -> *mut i8,
    ) -> Result<(), MetalError> {
        Err(MetalError::new(
            "Metal support was not linked into stwo-metal-sys.",
        ))
    }

    pub unsafe fn lift_accumulate_secure_columns_coords_u32x4(
        _runtime: *mut c_void,
        _lifted_0: *mut c_void,
        _lifted_1: *mut c_void,
        _lifted_2: *mut c_void,
        _lifted_3: *mut c_void,
        _current_0: *mut c_void,
        _current_1: *mut c_void,
        _current_2: *mut c_void,
        _current_3: *mut c_void,
        _dst_0: *mut c_void,
        _dst_1: *mut c_void,
        _dst_2: *mut c_void,
        _dst_3: *mut c_void,
        _current_log_size: u32,
        _log_ratio: u32,
        _error_ptr: fn(&mut [i8; 512]) -> *mut i8,
    ) -> Result<(), MetalError> {
        Err(MetalError::new(
            "Metal support was not linked into stwo-metal-sys.",
        ))
    }

    pub unsafe fn fix_first_variable_base_field_u32(
        _runtime: *mut c_void,
        _src: *mut c_void,
        _dst: *mut c_void,
        _src_log_len: u32,
        _assignment_limbs: [u32; 4],
        _error_ptr: fn(&mut [i8; 512]) -> *mut i8,
    ) -> Result<(), MetalError> {
        Err(MetalError::new(
            "Metal support was not linked into stwo-metal-sys.",
        ))
    }

    pub unsafe fn fix_first_variable_secure_field_u32x4(
        _runtime: *mut c_void,
        _src: *mut c_void,
        _dst: *mut c_void,
        _src_log_len: u32,
        _assignment_limbs: [u32; 4],
        _error_ptr: fn(&mut [i8; 512]) -> *mut i8,
    ) -> Result<(), MetalError> {
        Err(MetalError::new(
            "Metal support was not linked into stwo-metal-sys.",
        ))
    }

    pub unsafe fn gkr_gen_eq_evals_u32x4(
        _runtime: *mut c_void,
        _factors: *mut c_void,
        _dst: *mut c_void,
        _y_size: u32,
        _v_limbs: [u32; 4],
        _error_ptr: fn(&mut [i8; 512]) -> *mut i8,
    ) -> Result<(), MetalError> {
        Err(MetalError::new(
            "Metal support was not linked into stwo-metal-sys.",
        ))
    }

    pub unsafe fn gkr_next_grand_product_layer_u32x4(
        _runtime: *mut c_void,
        _src: *mut c_void,
        _dst: *mut c_void,
        _input_log_len: u32,
        _error_ptr: fn(&mut [i8; 512]) -> *mut i8,
    ) -> Result<(), MetalError> {
        Err(MetalError::new(
            "Metal support was not linked into stwo-metal-sys.",
        ))
    }

    pub unsafe fn gkr_next_logup_generic_layer_u32x4(
        _runtime: *mut c_void,
        _numerators: *mut c_void,
        _denominators: *mut c_void,
        _next_numerators: *mut c_void,
        _next_denominators: *mut c_void,
        _input_log_len: u32,
        _error_ptr: fn(&mut [i8; 512]) -> *mut i8,
    ) -> Result<(), MetalError> {
        Err(MetalError::new(
            "Metal support was not linked into stwo-metal-sys.",
        ))
    }

    pub unsafe fn gkr_next_logup_multiplicities_layer_u32(
        _runtime: *mut c_void,
        _numerators: *mut c_void,
        _denominators: *mut c_void,
        _next_numerators: *mut c_void,
        _next_denominators: *mut c_void,
        _input_log_len: u32,
        _error_ptr: fn(&mut [i8; 512]) -> *mut i8,
    ) -> Result<(), MetalError> {
        Err(MetalError::new(
            "Metal support was not linked into stwo-metal-sys.",
        ))
    }

    pub unsafe fn gkr_next_logup_singles_layer_u32x4(
        _runtime: *mut c_void,
        _denominators: *mut c_void,
        _next_numerators: *mut c_void,
        _next_denominators: *mut c_void,
        _input_log_len: u32,
        _error_ptr: fn(&mut [i8; 512]) -> *mut i8,
    ) -> Result<(), MetalError> {
        Err(MetalError::new(
            "Metal support was not linked into stwo-metal-sys.",
        ))
    }

    pub unsafe fn gkr_sum_grand_product_u32x4(
        _runtime: *mut c_void,
        _eq_evals: *mut c_void,
        _input_layer: *mut c_void,
        _n_terms: u32,
        _error_ptr: fn(&mut [i8; 512]) -> *mut i8,
    ) -> Result<([u32; 4], [u32; 4]), MetalError> {
        Err(MetalError::new(
            "Metal support was not linked into stwo-metal-sys.",
        ))
    }

    pub unsafe fn gkr_sum_logup_generic_u32x4(
        _runtime: *mut c_void,
        _eq_evals: *mut c_void,
        _numerators: *mut c_void,
        _denominators: *mut c_void,
        _n_terms: u32,
        _lambda_limbs: [u32; 4],
        _error_ptr: fn(&mut [i8; 512]) -> *mut i8,
    ) -> Result<([u32; 4], [u32; 4]), MetalError> {
        Err(MetalError::new(
            "Metal support was not linked into stwo-metal-sys.",
        ))
    }

    pub unsafe fn gkr_sum_logup_multiplicities_u32(
        _runtime: *mut c_void,
        _eq_evals: *mut c_void,
        _numerators: *mut c_void,
        _denominators: *mut c_void,
        _n_terms: u32,
        _lambda_limbs: [u32; 4],
        _error_ptr: fn(&mut [i8; 512]) -> *mut i8,
    ) -> Result<([u32; 4], [u32; 4]), MetalError> {
        Err(MetalError::new(
            "Metal support was not linked into stwo-metal-sys.",
        ))
    }

    pub unsafe fn gkr_sum_logup_singles_u32x4(
        _runtime: *mut c_void,
        _eq_evals: *mut c_void,
        _denominators: *mut c_void,
        _n_terms: u32,
        _lambda_limbs: [u32; 4],
        _error_ptr: fn(&mut [i8; 512]) -> *mut i8,
    ) -> Result<([u32; 4], [u32; 4]), MetalError> {
        Err(MetalError::new(
            "Metal support was not linked into stwo-metal-sys.",
        ))
    }

    pub unsafe fn inclusive_prefix_sum_bit_rev_circle_domain_u32(
        _runtime: *mut c_void,
        _buffer: *mut c_void,
        _log_len: u32,
        _error_ptr: fn(&mut [i8; 512]) -> *mut i8,
    ) -> Result<(), MetalError> {
        Err(MetalError::new(
            "Metal support was not linked into stwo-metal-sys.",
        ))
    }

    pub unsafe fn fri_fold_circle_into_line_first_layer_u32x4(
        _runtime: *mut c_void,
        _src: *mut c_void,
        _dst: *mut c_void,
        _inverse_y_factors: *mut c_void,
        _src_log_len: u32,
        _alpha_limbs: [u32; 4],
        _error_ptr: fn(&mut [i8; 512]) -> *mut i8,
    ) -> Result<(), MetalError> {
        Err(MetalError::new(
            "Metal support was not linked into stwo-metal-sys.",
        ))
    }

    pub unsafe fn fri_fold_circle_into_line_accumulate_coords_u32x4(
        _runtime: *mut c_void,
        _src_0: *mut c_void,
        _src_1: *mut c_void,
        _src_2: *mut c_void,
        _src_3: *mut c_void,
        _dst_0: *mut c_void,
        _dst_1: *mut c_void,
        _dst_2: *mut c_void,
        _dst_3: *mut c_void,
        _inverse_y_factors: *mut c_void,
        _src_log_len: u32,
        _alpha_limbs: [u32; 4],
        _alpha_sq_limbs: [u32; 4],
        _error_ptr: fn(&mut [i8; 512]) -> *mut i8,
    ) -> Result<(), MetalError> {
        Err(MetalError::new(
            "Metal support was not linked into stwo-metal-sys.",
        ))
    }

    pub unsafe fn fri_fold_circle_into_line_first_layer_coords_u32x4(
        _runtime: *mut c_void,
        _src_0: *mut c_void,
        _src_1: *mut c_void,
        _src_2: *mut c_void,
        _src_3: *mut c_void,
        _dst_0: *mut c_void,
        _dst_1: *mut c_void,
        _dst_2: *mut c_void,
        _dst_3: *mut c_void,
        _inverse_y_factors: *mut c_void,
        _src_log_len: u32,
        _alpha_limbs: [u32; 4],
        _error_ptr: fn(&mut [i8; 512]) -> *mut i8,
    ) -> Result<(), MetalError> {
        Err(MetalError::new(
            "Metal support was not linked into stwo-metal-sys.",
        ))
    }

    pub unsafe fn fri_fold_line_step_coords_u32x4(
        _runtime: *mut c_void,
        _src_0: *mut c_void,
        _src_1: *mut c_void,
        _src_2: *mut c_void,
        _src_3: *mut c_void,
        _dst_0: *mut c_void,
        _dst_1: *mut c_void,
        _dst_2: *mut c_void,
        _dst_3: *mut c_void,
        _inverse_x_factors: *mut c_void,
        _src_log_len: u32,
        _alpha_limbs: [u32; 4],
        _error_ptr: fn(&mut [i8; 512]) -> *mut i8,
    ) -> Result<(), MetalError> {
        Err(MetalError::new(
            "Metal support was not linked into stwo-metal-sys.",
        ))
    }

    pub unsafe fn fri_fold_line_step_u32x4(
        _runtime: *mut c_void,
        _src: *mut c_void,
        _dst: *mut c_void,
        _inverse_x_factors: *mut c_void,
        _src_log_len: u32,
        _alpha_limbs: [u32; 4],
        _error_ptr: fn(&mut [i8; 512]) -> *mut i8,
    ) -> Result<(), MetalError> {
        Err(MetalError::new(
            "Metal support was not linked into stwo-metal-sys.",
        ))
    }

    pub unsafe fn generate_wide_fibonacci_trace_u32(
        _runtime: *mut c_void,
        _input_a: *mut c_void,
        _input_b: *mut c_void,
        _trace: *mut c_void,
        _input_log_len: u32,
        _n_columns: u32,
        _error_ptr: fn(&mut [i8; 512]) -> *mut i8,
    ) -> Result<(), MetalError> {
        Err(MetalError::new(
            "Metal support was not linked into stwo-metal-sys.",
        ))
    }

    pub unsafe fn accumulate_wide_fibonacci_quotients_u32x4(
        _runtime: *mut c_void,
        _trace_evaluations: *mut c_void,
        _random_coeff_powers: *mut c_void,
        _denominator_inverses: *mut c_void,
        _dst: *mut c_void,
        _domain_log_size: u32,
        _eval_domain_log_size: u32,
        _n_constraints: u32,
        _error_ptr: fn(&mut [i8; 512]) -> *mut i8,
    ) -> Result<(), MetalError> {
        Err(MetalError::new(
            "Metal support was not linked into stwo-metal-sys.",
        ))
    }

    pub unsafe fn accumulate_partial_numerators_u32x4(
        _runtime: *mut c_void,
        _columns: *mut c_void,
        _column_indices: *mut c_void,
        _b_coeffs: *mut c_void,
        _c_coeffs: *mut c_void,
        _dst: *mut c_void,
        _row_count: u32,
        _n_terms: u32,
        _error_ptr: fn(&mut [i8; 512]) -> *mut i8,
    ) -> Result<(), MetalError> {
        Err(MetalError::new(
            "Metal support was not linked into stwo-metal-sys.",
        ))
    }

    #[allow(clippy::too_many_arguments)]
    pub unsafe fn accumulate_partial_numerators_batched_u32x4(
        _runtime: *mut c_void,
        _columns: *mut c_void,
        _column_indices: *mut c_void,
        _b_coeffs: *mut c_void,
        _c_coeffs: *mut c_void,
        _term_offsets: *mut c_void,
        _term_counts: *mut c_void,
        _dst: *mut c_void,
        _row_count: u32,
        _n_batches: u32,
        _error_ptr: fn(&mut [i8; 512]) -> *mut i8,
    ) -> Result<(), MetalError> {
        Err(MetalError::new(
            "Metal support was not linked into stwo-metal-sys.",
        ))
    }

    #[allow(clippy::too_many_arguments)]
    pub unsafe fn compute_quotients_and_combine_u32x4(
        _runtime: *mut c_void,
        _partial_coord_0: *mut c_void,
        _partial_coord_1: *mut c_void,
        _partial_coord_2: *mut c_void,
        _partial_coord_3: *mut c_void,
        _sample_points: *mut c_void,
        _first_linear_terms: *mut c_void,
        _partial_log_sizes: *mut c_void,
        _partial_offsets: *mut c_void,
        _domain_x: *mut c_void,
        _domain_y: *mut c_void,
        _dst: *mut c_void,
        _lifting_log_size: u32,
        _n_accumulations: u32,
        _error_ptr: fn(&mut [i8; 512]) -> *mut i8,
    ) -> Result<(), MetalError> {
        Err(MetalError::new(
            "Metal support was not linked into stwo-metal-sys.",
        ))
    }

    pub unsafe fn compute_quotients_and_combine_packed_u32x4(
        _runtime: *mut c_void,
        _partials: *mut c_void,
        _sample_points: *mut c_void,
        _first_linear_terms: *mut c_void,
        _partial_log_sizes: *mut c_void,
        _partial_offsets: *mut c_void,
        _domain_x: *mut c_void,
        _domain_y: *mut c_void,
        _dst: *mut c_void,
        _lifting_log_size: u32,
        _n_accumulations: u32,
        _error_ptr: fn(&mut [i8; 512]) -> *mut i8,
    ) -> Result<(), MetalError> {
        Err(MetalError::new(
            "Metal support was not linked into stwo-metal-sys.",
        ))
    }

    pub unsafe fn blake2s_build_merkle_layers_u32(
        _runtime: *mut c_void,
        _leaf_layer: *mut c_void,
        _layer_ptrs: *const *mut c_void,
        _leaf_log_size: u32,
        _error_ptr: fn(&mut [i8; 512]) -> *mut i8,
    ) -> Result<(), MetalError> {
        Err(MetalError::new(
            "Metal support was not linked into stwo-metal-sys.",
        ))
    }

    pub unsafe fn buffer_read_indices(
        _runtime: *mut c_void,
        _buffer: *mut c_void,
        _indices: *const u32,
        _indices_len: usize,
        _host_ptr: *mut u32,
        _error_ptr: fn(&mut [i8; 512]) -> *mut i8,
    ) -> Result<(), MetalError> {
        Err(MetalError::new(
            "Metal support was not linked into stwo-metal-sys.",
        ))
    }
}
