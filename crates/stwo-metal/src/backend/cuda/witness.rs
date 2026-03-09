use super::BaseFieldVec;
use crate::stwo_cuda::abi_v1::{
    CudaPoseidonLookupElementsAbiV1, StwoCudaPoseidonInteractionTraceRequestV1,
    StwoCudaPoseidonTraceRequestV1, StwoCudaWideFibonacciTraceRequestV1,
};
use crate::stwo_cuda::bindings;

pub struct WideFibonacciTraceRequest<'a> {
    pub input_a: &'a BaseFieldVec,
    pub input_b: &'a BaseFieldVec,
    pub input_len: u32,
    pub trace_columns: &'a [*const u32],
    pub n_columns: u32,
}

pub fn generate_wide_fibonacci_trace(request: WideFibonacciTraceRequest<'_>) {
    let ffi_request = StwoCudaWideFibonacciTraceRequestV1 {
        input_a: request.input_a.device_ptr,
        input_b: request.input_b.device_ptr,
        input_len: request.input_len,
        trace_columns: request.trace_columns.as_ptr(),
        trace_columns_len: request.trace_columns.len() as u32,
        n_columns: request.n_columns,
    };
    unsafe {
        bindings::generate_wide_fibonacci_trace_v1(&ffi_request)
            .expect_ok("generate_wide_fibonacci_trace_v1");
    }
}

pub struct PoseidonTraceRequest<'a> {
    pub trace_columns: &'a [*const u32],
    pub lookup_init: &'a [*const u32],
    pub lookup_final: &'a [*const u32],
    pub trace_log_len: u32,
}

pub fn generate_poseidon_traces(request: PoseidonTraceRequest<'_>) {
    let ffi_request = StwoCudaPoseidonTraceRequestV1 {
        trace_columns: request.trace_columns.as_ptr(),
        trace_columns_len: request.trace_columns.len() as u32,
        lookup_init: request.lookup_init.as_ptr(),
        lookup_init_len: request.lookup_init.len() as u32,
        lookup_final: request.lookup_final.as_ptr(),
        lookup_final_len: request.lookup_final.len() as u32,
        trace_log_size: request.trace_log_len,
    };
    unsafe {
        bindings::generate_poseidon_traces_v1(&ffi_request)
            .expect_ok("generate_poseidon_traces_v1");
    }
}

pub struct PoseidonInteractionTraceRequest<'a, T: CudaPoseidonLookupElementsAbiV1> {
    pub lookup_elements: &'a T,
    pub lookup_init: &'a [*const u32],
    pub lookup_final: &'a [*const u32],
    pub log_size: u32,
    pub interaction_traces: &'a [*const u32],
    pub claimed_sum: &'a BaseFieldVec,
}

pub fn generate_poseidon_interaction_traces<T: CudaPoseidonLookupElementsAbiV1>(
    request: PoseidonInteractionTraceRequest<'_, T>,
) {
    let ffi_request = StwoCudaPoseidonInteractionTraceRequestV1 {
        lookup_elements: request
            .lookup_elements
            .to_cuda_poseidon_lookup_elements_abi_v1(),
        lookup_init: request.lookup_init.as_ptr(),
        lookup_init_len: request.lookup_init.len() as u32,
        lookup_final: request.lookup_final.as_ptr(),
        lookup_final_len: request.lookup_final.len() as u32,
        log_size: request.log_size,
        interaction_traces: request.interaction_traces.as_ptr(),
        interaction_traces_len: request.interaction_traces.len() as u32,
        claimed_sum: request.claimed_sum.device_ptr.cast_mut(),
    };
    unsafe {
        bindings::generate_poseidon_interaction_traces_v1(&ffi_request)
            .expect_ok("generate_poseidon_interaction_traces_v1");
    }
}
