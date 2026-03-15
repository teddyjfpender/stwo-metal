//! GPU-accelerated witness trace generation for Cairo opcode components.
//!
//! Each function takes flat u32 inputs (pc, ap, fp tuples) plus the memory
//! lookup tables (address_to_id, big_values, small_values), dispatches a Metal
//! GPU kernel, and returns `Vec<CircleEvaluation<MetalBackend, ...>>` ready for
//! zero-copy commitment via `extend_opaque_evals`.
//!
//! Supported opcodes:
//! - add_opcode_small (39 columns)
//! - assert_eq_opcode_double_deref (19 columns)
//! - jnz_opcode_taken (47 columns)
//! - jump_opcode_rel_imm (13 columns)
//! - call_opcode_rel_imm (24 columns)
//! - ret_opcode (16 columns)

use stwo::core::fields::m31::BaseField;
use stwo::core::poly::circle::CanonicCoset;
use stwo::prover::poly::circle::CircleEvaluation;
use stwo::prover::poly::BitReversedOrder;
use stwo_metal_sys::metal::{MetalError, U32Buffer};

use super::MetalBackend;
use crate::stwo_metal::base_field_vec::BaseFieldVec;

/// Error type for opcode witness generation on GPU.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OpcodeTraceError {
    /// The column length is not a power of two.
    ColumnLengthNotPowerOfTwo { column_length: usize },
    /// An error occurred in the Metal runtime.
    Runtime { message: String },
}

impl core::fmt::Display for OpcodeTraceError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::ColumnLengthNotPowerOfTwo { column_length } => {
                write!(
                    f,
                    "opcode trace requires a power-of-two column length, got {column_length}"
                )
            }
            Self::Runtime { message } => f.write_str(message),
        }
    }
}

impl std::error::Error for OpcodeTraceError {}

impl From<MetalError> for OpcodeTraceError {
    fn from(value: MetalError) -> Self {
        Self::Runtime {
            message: value.message().to_string(),
        }
    }
}

/// Raw GPU trace buffer with column-major layout.
struct RawOpcodeTrace {
    values: U32Buffer,
    n_columns: usize,
    column_length: usize,
}

impl RawOpcodeTrace {
    /// Convert the raw GPU buffer into Metal-backed CircleEvaluations (zero-copy).
    fn to_metal_evaluations(
        &self,
    ) -> Vec<CircleEvaluation<MetalBackend, BaseField, BitReversedOrder>> {
        let log_size = self.column_length.ilog2();
        let domain = CanonicCoset::new(log_size).circle_domain();
        (0..self.n_columns)
            .map(|col| {
                let start = col * self.column_length;
                let col_buf = self
                    .values
                    .clone_range(start, self.column_length)
                    .expect("Metal opcode trace column slicing should succeed");
                CircleEvaluation::<MetalBackend, BaseField, BitReversedOrder>::new(
                    domain,
                    BaseFieldVec::from_buffer(col_buf),
                )
            })
            .collect()
    }
}

/// Shared logic: upload inputs and memory tables, call a GPU kernel, return raw trace.
fn dispatch_opcode_kernel(
    inputs: &[u32],
    address_to_id: &[u32],
    big_values: &[u32],
    small_values: &[u32],
    n_rows: usize,
    n_trace_columns: usize,
    kernel_fn: fn(
        &U32Buffer,
        &U32Buffer,
        &U32Buffer,
        &U32Buffer,
        u32,
        u32,
    ) -> Result<U32Buffer, MetalError>,
) -> Result<RawOpcodeTrace, OpcodeTraceError> {
    assert_ne!(n_rows, 0, "must have at least one input row");
    let column_length = std::cmp::max(n_rows.next_power_of_two(), 16);

    let inputs_buf = U32Buffer::from_slice(inputs)?;
    let addr_buf = U32Buffer::from_slice(address_to_id)?;
    let big_buf = U32Buffer::from_slice(big_values)?;
    let small_buf = U32Buffer::from_slice(small_values)?;

    let trace_buf = kernel_fn(
        &inputs_buf,
        &addr_buf,
        &big_buf,
        &small_buf,
        n_rows as u32,
        column_length as u32,
    )?;

    Ok(RawOpcodeTrace {
        values: trace_buf,
        n_columns: n_trace_columns,
        column_length,
    })
}

// --- Public API: one function per opcode ---

/// Generate the add_opcode_small trace (39 columns) on GPU.
pub fn generate_add_opcode_small_trace(
    inputs: &[u32],
    address_to_id: &[u32],
    big_values: &[u32],
    small_values: &[u32],
    n_rows: usize,
) -> Result<Vec<CircleEvaluation<MetalBackend, BaseField, BitReversedOrder>>, OpcodeTraceError> {
    let raw = dispatch_opcode_kernel(
        inputs,
        address_to_id,
        big_values,
        small_values,
        n_rows,
        39,
        U32Buffer::witness_add_opcode_small_trace,
    )?;
    Ok(raw.to_metal_evaluations())
}

/// Generate the assert_eq_opcode_double_deref trace (19 columns) on GPU.
pub fn generate_assert_eq_double_deref_trace(
    inputs: &[u32],
    address_to_id: &[u32],
    big_values: &[u32],
    small_values: &[u32],
    n_rows: usize,
) -> Result<Vec<CircleEvaluation<MetalBackend, BaseField, BitReversedOrder>>, OpcodeTraceError> {
    let raw = dispatch_opcode_kernel(
        inputs,
        address_to_id,
        big_values,
        small_values,
        n_rows,
        19,
        U32Buffer::witness_assert_eq_double_deref_trace,
    )?;
    Ok(raw.to_metal_evaluations())
}

/// Generate the jnz_opcode_taken trace (47 columns) on GPU.
pub fn generate_jnz_opcode_taken_trace(
    inputs: &[u32],
    address_to_id: &[u32],
    big_values: &[u32],
    small_values: &[u32],
    n_rows: usize,
) -> Result<Vec<CircleEvaluation<MetalBackend, BaseField, BitReversedOrder>>, OpcodeTraceError> {
    let raw = dispatch_opcode_kernel(
        inputs,
        address_to_id,
        big_values,
        small_values,
        n_rows,
        47,
        U32Buffer::witness_jnz_opcode_taken_trace,
    )?;
    Ok(raw.to_metal_evaluations())
}

/// Generate the jump_opcode_rel_imm trace (13 columns) on GPU.
pub fn generate_jump_opcode_rel_imm_trace(
    inputs: &[u32],
    address_to_id: &[u32],
    big_values: &[u32],
    small_values: &[u32],
    n_rows: usize,
) -> Result<Vec<CircleEvaluation<MetalBackend, BaseField, BitReversedOrder>>, OpcodeTraceError> {
    let raw = dispatch_opcode_kernel(
        inputs,
        address_to_id,
        big_values,
        small_values,
        n_rows,
        13,
        U32Buffer::witness_jump_opcode_rel_imm_trace,
    )?;
    Ok(raw.to_metal_evaluations())
}

/// Generate the call_opcode_rel_imm trace (24 columns) on GPU.
pub fn generate_call_opcode_rel_imm_trace(
    inputs: &[u32],
    address_to_id: &[u32],
    big_values: &[u32],
    small_values: &[u32],
    n_rows: usize,
) -> Result<Vec<CircleEvaluation<MetalBackend, BaseField, BitReversedOrder>>, OpcodeTraceError> {
    let raw = dispatch_opcode_kernel(
        inputs,
        address_to_id,
        big_values,
        small_values,
        n_rows,
        24,
        U32Buffer::witness_call_opcode_rel_imm_trace,
    )?;
    Ok(raw.to_metal_evaluations())
}

/// Generate the ret_opcode trace (16 columns) on GPU.
pub fn generate_ret_opcode_trace(
    inputs: &[u32],
    address_to_id: &[u32],
    big_values: &[u32],
    small_values: &[u32],
    n_rows: usize,
) -> Result<Vec<CircleEvaluation<MetalBackend, BaseField, BitReversedOrder>>, OpcodeTraceError> {
    let raw = dispatch_opcode_kernel(
        inputs,
        address_to_id,
        big_values,
        small_values,
        n_rows,
        16,
        U32Buffer::witness_ret_opcode_trace,
    )?;
    Ok(raw.to_metal_evaluations())
}
