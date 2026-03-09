use std::ffi::c_void;

use stwo::core::fields::qm31::SecureField;

use crate::stwo_cuda::abi_v1::{StwoCudaConstraintEvalRequestV1, StwoCudaQm31AbiV1};
use crate::stwo_cuda::bindings;
pub use crate::stwo_cuda::{BaseFieldVec, SecureFieldVec};

/// Immutable view of the inputs needed to launch the CUDA constraint-quotient kernel.
///
/// This keeps the raw FFI call and legacy `*mut c_void` ABI cast inside the CUDA backend module
/// while leaving the higher-level proving logic in `constraint-framework` unchanged.
pub struct ConstraintQuotientEvalRequest<'a> {
    pub quotient_columns: &'a [BaseFieldVec; 4],
    pub trace0_evaluations: &'a [*const u32],
    pub trace1_evaluations: &'a [*const u32],
    pub trace2_evaluations: &'a [*const u32],
    pub random_coeff_powers: &'a SecureFieldVec,
    pub denominator_inverses: &'a BaseFieldVec,
    pub domain_log_size: u32,
    pub eval_domain_log_size: u32,
    pub number_of_columns: u32,
    pub logup_counts: u32,
    pub eval: *const c_void,
    pub claimed_sum_shift: SecureField,
    pub should_accumulate: bool,
}

/// Returns an opaque immutable pointer for legacy CUDA ABIs that still tunnel Rust values through
/// `void*`.
pub fn opaque_ptr<T>(value: &T) -> *const c_void {
    value as *const T as *const c_void
}

/// Returns an opaque pointer for the current legacy evaluator ABI.
///
/// The adapter treats the evaluator as immutable. We only cast to `*mut c_void` at the FFI
/// boundary because the current native signature still requires that type.
pub fn opaque_eval_ptr<T>(eval: &T) -> *const c_void {
    opaque_ptr(eval)
}

pub fn launch_constraint_quotients_on_domain(request: ConstraintQuotientEvalRequest<'_>) {
    let ffi_request = StwoCudaConstraintEvalRequestV1 {
        quotient_columns: [
            request.quotient_columns[0].device_ptr,
            request.quotient_columns[1].device_ptr,
            request.quotient_columns[2].device_ptr,
            request.quotient_columns[3].device_ptr,
        ],
        trace0_evaluations: request.trace0_evaluations.as_ptr(),
        trace0_evaluations_len: request.trace0_evaluations.len() as u32,
        trace1_evaluations: request.trace1_evaluations.as_ptr(),
        trace1_evaluations_len: request.trace1_evaluations.len() as u32,
        trace2_evaluations: request.trace2_evaluations.as_ptr(),
        trace2_evaluations_len: request.trace2_evaluations.len() as u32,
        random_coeff_powers: request.random_coeff_powers.device_ptr,
        denominator_inverses: request.denominator_inverses.device_ptr,
        domain_log_size: request.domain_log_size,
        eval_domain_log_size: request.eval_domain_log_size,
        number_of_columns: request.number_of_columns,
        logup_counts: request.logup_counts,
        eval: request.eval,
        cumsum_shift: StwoCudaQm31AbiV1::from(request.claimed_sum_shift),
        should_accumulate: request.should_accumulate,
        // The supported Rust API always launches the proving-aligned evaluator path.
        // The internal ABI field remains for now so ABI v1 stays stable while the
        // raw/native debug path is still contained inside the native layer.
        use_assert_evaluator: false,
    };
    unsafe {
        bindings::evaluate_constraint_quotients_on_domain_v1(&ffi_request)
            .expect_ok("evaluate_constraint_quotients_on_domain_v1");
    }
}
