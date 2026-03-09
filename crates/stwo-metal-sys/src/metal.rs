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
        fn stwo_metal_permute_coset_to_circle_domain_bit_reversed_u32(
            runtime: *mut c_void,
            src: *mut c_void,
            dst: *mut c_void,
            log_len: u32,
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
}
