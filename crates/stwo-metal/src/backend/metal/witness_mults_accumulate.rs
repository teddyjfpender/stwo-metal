//! GPU-accelerated multiplicity accumulation for Cairo opcode components.
//!
//! Each function takes flat u32 inputs (pc, ap, fp tuples) plus memory lookup
//! tables and pre-allocated multiplicity buffers, dispatches a Metal GPU kernel
//! that atomically increments the multiplicity counters, then reads back the
//! results.
//!
//! This eliminates the CPU `write_trace_mults_only()` from the critical path.

use stwo_metal_sys::metal::{ffi, MetalError, U32Buffer};

/// Error type for GPU mults accumulation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GpuMultsError {
    Runtime { message: String },
}

impl core::fmt::Display for GpuMultsError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Runtime { message } => f.write_str(message),
        }
    }
}

impl std::error::Error for GpuMultsError {}

impl From<MetalError> for GpuMultsError {
    fn from(value: MetalError) -> Self {
        Self::Runtime {
            message: value.message().to_string(),
        }
    }
}

/// Result of GPU mults accumulation: the three multiplicity buffers
/// after atomic increments have been applied.
pub struct GpuMultsResult {
    pub addr_to_id_mults: U32Buffer,
    pub id_to_big_mults: U32Buffer,
    pub id_to_small_mults: U32Buffer,
}

/// Dispatch a GPU mults accumulation kernel for one opcode.
///
/// # Arguments
///
/// * `inputs` - Flat u32 data: (pc, ap, fp) per row.
/// * `address_to_id` - The address-to-raw-ID lookup table (already on GPU).
/// * `big_values` - Row-major [n_big][8] u32 F252 values (already on GPU).
/// * `small_values` - Row-major [n_small][4] u32 small values (already on GPU).
/// * `addr_to_id_mults` - Pre-allocated zeroed buffer for addr_to_id multiplicities.
/// * `id_to_big_mults` - Pre-allocated zeroed buffer for id_to_big multiplicities.
/// * `id_to_small_mults` - Pre-allocated zeroed buffer for id_to_small multiplicities.
/// * `n_rows` - Number of actual rows to process.
/// * `kernel_fn` - The specific FFI function for this opcode.
fn dispatch_mults(
    inputs: &U32Buffer,
    address_to_id: &U32Buffer,
    big_values: &U32Buffer,
    small_values: &U32Buffer,
    addr_to_id_mults: &U32Buffer,
    id_to_big_mults: &U32Buffer,
    id_to_small_mults: &U32Buffer,
    n_rows: u32,
    kernel_fn: unsafe fn(
        *mut std::ffi::c_void, *mut std::ffi::c_void,
        *mut std::ffi::c_void, *mut std::ffi::c_void, *mut std::ffi::c_void,
        *mut std::ffi::c_void, *mut std::ffi::c_void, *mut std::ffi::c_void,
        u32, fn(&mut [i8; 512]) -> *mut i8,
    ) -> Result<(), MetalError>,
) -> Result<(), GpuMultsError> {
    U32Buffer::dispatch_mults_accumulate(
        inputs,
        address_to_id,
        big_values,
        small_values,
        addr_to_id_mults,
        id_to_big_mults,
        id_to_small_mults,
        n_rows,
        kernel_fn,
    )?;
    Ok(())
}

/// Accumulate multiplicities for all GPU-ready opcodes in a single batch.
///
/// Takes the shared multiplicity buffers and dispatches one kernel per opcode.
/// All opcodes write to the same multiplicity buffers via atomics.
///
/// # Arguments
///
/// * `opcode_inputs` - Vec of (opcode_name, inputs_data, n_rows) for each opcode.
/// * `address_to_id` - Already-uploaded GPU buffer.
/// * `big_values` - Already-uploaded GPU buffer.
/// * `small_values` - Already-uploaded GPU buffer.
/// * `addr_to_id_mults_len` - Size of addr_to_id multiplicity buffer.
/// * `id_to_big_mults_len` - Size of id_to_big multiplicity buffer.
/// * `id_to_small_mults_len` - Size of id_to_small multiplicity buffer.
pub fn accumulate_all_opcode_mults(
    opcode_inputs: &[(&str, &[u32], usize)],
    address_to_id: &U32Buffer,
    big_values: &U32Buffer,
    small_values: &U32Buffer,
    addr_to_id_mults_len: usize,
    id_to_big_mults_len: usize,
    id_to_small_mults_len: usize,
) -> Result<GpuMultsResult, GpuMultsError> {
    // Allocate zeroed multiplicity buffers on GPU.
    let addr_mults = U32Buffer::from_slice(&vec![0u32; addr_to_id_mults_len])?;
    let big_mults = U32Buffer::from_slice(&vec![0u32; id_to_big_mults_len])?;
    let small_mults = U32Buffer::from_slice(&vec![0u32; id_to_small_mults_len])?;

    for &(name, data, n_rows) in opcode_inputs {
        if n_rows == 0 { continue; }
        let inputs_buf = U32Buffer::from_slice(data)?;

        let kernel_fn = match name {
            "add_opcode_small" => ffi::mults_add_opcode_small as _,
            "assert_eq_double_deref" => ffi::mults_assert_eq_double_deref as _,
            "jnz_opcode_taken" => ffi::mults_jnz_opcode_taken as _,
            "jump_opcode_rel_imm" => ffi::mults_jump_opcode_rel_imm as _,
            "call_opcode_rel_imm" => ffi::mults_call_opcode_rel_imm as _,
            "ret_opcode" => ffi::mults_ret_opcode as _,
            other => {
                return Err(GpuMultsError::Runtime {
                    message: format!("Unknown opcode for GPU mults: {other}"),
                });
            }
        };

        dispatch_mults(
            &inputs_buf,
            address_to_id,
            big_values,
            small_values,
            &addr_mults,
            &big_mults,
            &small_mults,
            n_rows as u32,
            kernel_fn,
        )?;
    }

    Ok(GpuMultsResult {
        addr_to_id_mults: addr_mults,
        id_to_big_mults: big_mults,
        id_to_small_mults: small_mults,
    })
}
