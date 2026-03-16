//! GPU-accelerated interaction trace generation for opcode components.
//!
//! Each function takes a GPU trace buffer (produced by the witness opcode kernel),
//! dispatches a Metal kernel to extract LookupData-equivalent values, then feeds
//! those into the generic interaction trace kernel via `GenericInteractionTraceBuilder`.
//!
//! Supported opcodes:
//! - ret_opcode (16 trace cols -> 82 interaction values -> 4 logup cols)
//! - jnz_opcode_taken (47 trace cols -> 82 interaction values -> 4 logup cols)
//! - assert_eq_opcode_double_deref (19 trace cols -> 55 interaction values -> 4 logup cols)
//! - jump_opcode_rel_imm (13 trace cols -> 49 interaction values -> 3 logup cols)
//! - call_opcode_rel_imm (24 trace cols -> 115 interaction values -> 5 logup cols)

use stwo::core::fields::m31::BaseField;
use stwo::core::fields::qm31::SecureField;
use stwo::prover::poly::circle::CircleEvaluation;
use stwo::prover::poly::BitReversedOrder;
use stwo_metal_sys::metal::U32Buffer;

use super::interaction_trace_generic::{
    ColumnDescriptor, GenericInteractionTraceBuilder,
    NUMERATOR_ENABLER_WEIGHTED, NUMERATOR_PAIR_SUM, NUMERATOR_SINGLE_NEG,
};
use super::interaction_trace_id_to_big::InteractionLookupElements;
use super::MetalBackend;

// Relation ID constants (must match the Metal shader and CPU code).
const REL_VERIFY_INSTRUCTION: u32 = 1719106205;
const REL_MEMORY_ADDR_TO_ID: u32 = 1444891767;
const REL_MEMORY_ID_TO_BIG: u32 = 1662111297;
const REL_OPCODES: u32 = 428564188;

/// Error type for opcode interaction trace generation on GPU.
#[derive(Clone, Debug)]
pub enum OpcodeInteractionTraceError {
    /// An error from the Metal runtime.
    Runtime(String),
}

impl core::fmt::Display for OpcodeInteractionTraceError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Runtime(msg) => f.write_str(msg),
        }
    }
}

impl std::error::Error for OpcodeInteractionTraceError {}

impl From<stwo_metal_sys::metal::MetalError> for OpcodeInteractionTraceError {
    fn from(e: stwo_metal_sys::metal::MetalError) -> Self {
        Self::Runtime(e.message().to_string())
    }
}

impl From<super::interaction_trace_id_to_big::InteractionTraceError>
    for OpcodeInteractionTraceError
{
    fn from(e: super::interaction_trace_id_to_big::InteractionTraceError) -> Self {
        Self::Runtime(format!("{e}"))
    }
}

/// Helper: given the GPU output buffer and the field layout for each logup column,
/// build a `GenericInteractionTraceBuilder` and dispatch the generic kernel.
///
/// `field_ranges` describes, for each logup column, the (offset, count) of the
/// values0 range and values1 range (if any) in the flat output, plus relation_ids
/// and numerator type.
fn dispatch_generic(
    output_buf: &U32Buffer,
    col_len: usize,
    n_rows_real: usize,
    n_output_fields: usize,
    logup_layout: &[LogupColumnLayout],
    lookup_elements: &InteractionLookupElements,
) -> Result<
    (
        Vec<CircleEvaluation<MetalBackend, BaseField, BitReversedOrder>>,
        SecureField,
    ),
    OpcodeInteractionTraceError,
> {
    let mut builder = GenericInteractionTraceBuilder::new();

    // Each output field becomes a value column in the builder.
    // We store them as slices from the output buffer.
    let mut col_indices = Vec::with_capacity(n_output_fields);
    for field_idx in 0..n_output_fields {
        let col_buf = output_buf.clone_range(field_idx * col_len, col_len)?;
        let data = col_buf.to_vec()?;
        let idx = builder.add_column(data);
        col_indices.push(idx);
    }

    // Build logup column descriptors.
    // Each group in the flat output starts with a relation_id field followed
    // by the actual values. The generic kernel takes the relation_id separately
    // and the values start one field after.
    for layout in logup_layout {
        let desc = ColumnDescriptor {
            // Skip the relation_id field (first of each group); values start at +1.
            values0_offset: col_indices[layout.values0_start + 1] as u32,
            values0_count: (layout.values0_count - 1) as u32,
            values1_offset: if layout.numerator_type != NUMERATOR_SINGLE_NEG {
                col_indices[layout.values1_start + 1] as u32
            } else {
                0
            },
            values1_count: if layout.numerator_type != NUMERATOR_SINGLE_NEG {
                (layout.values1_count - 1) as u32
            } else {
                0
            },
            relation_id0: layout.relation_id0,
            relation_id1: layout.relation_id1,
            numerator_type: layout.numerator_type,
            weight_offset: 0,
        };
        builder.add_logup_column(desc);
    }

    let result = builder.dispatch_metal(col_len, n_rows_real, lookup_elements)?;
    Ok(result)
}

/// Layout descriptor for one logup column.
struct LogupColumnLayout {
    /// Start field index in the flat output for values0 (first field is relation_id).
    values0_start: usize,
    /// Total number of fields for values0 (including relation_id at position 0).
    values0_count: usize,
    /// Start field index for values1 (first field is relation_id).
    values1_start: usize,
    /// Total number of fields for values1 (including relation_id).
    values1_count: usize,
    /// Numerator type.
    numerator_type: u32,
    /// Relation ID for combine0 (the constant hash).
    relation_id0: u32,
    /// Relation ID for combine1.
    relation_id1: u32,
}

// ===========================================================================
// ret_opcode: 82 output values, 4 logup columns
// Layout: verify_inst(8) + addr_to_id_0(3) + id_to_big_0(30)
//       + addr_to_id_1(3) + id_to_big_1(30) + opcodes_0(4) + opcodes_1(4)
// Logup pairs:
//   col0: pair_sum(verify_inst[0..8], addr_to_id_0[8..11])
//   col1: pair_sum(id_to_big_0[11..41], addr_to_id_1[41..44])
//   col2: enabler_weighted(id_to_big_1[44..74], opcodes_0[74..78])
//   col3: single_neg(opcodes_1[78..82])
// ===========================================================================

/// Generate interaction trace for ret_opcode from GPU trace columns.
pub fn gpu_interaction_trace_ret_opcode(
    trace_cols: &U32Buffer,
    n_rows_real: usize,
    col_len: usize,
    lookup_elements: &InteractionLookupElements,
) -> Result<
    (
        Vec<CircleEvaluation<MetalBackend, BaseField, BitReversedOrder>>,
        SecureField,
    ),
    OpcodeInteractionTraceError,
> {
    let output_buf = U32Buffer::interaction_values_ret_opcode(
        trace_cols,
        n_rows_real as u32,
        col_len as u32,
    )?;

    let logup_layout = vec![
        LogupColumnLayout {
            values0_start: 0, values0_count: 8,
            values1_start: 8, values1_count: 3,
            numerator_type: NUMERATOR_PAIR_SUM,
            relation_id0: REL_VERIFY_INSTRUCTION, relation_id1: REL_MEMORY_ADDR_TO_ID,
        },
        LogupColumnLayout {
            values0_start: 11, values0_count: 30,
            values1_start: 41, values1_count: 3,
            numerator_type: NUMERATOR_PAIR_SUM,
            relation_id0: REL_MEMORY_ID_TO_BIG, relation_id1: REL_MEMORY_ADDR_TO_ID,
        },
        LogupColumnLayout {
            values0_start: 44, values0_count: 30,
            values1_start: 74, values1_count: 4,
            numerator_type: NUMERATOR_ENABLER_WEIGHTED,
            relation_id0: REL_MEMORY_ID_TO_BIG, relation_id1: REL_OPCODES,
        },
        LogupColumnLayout {
            values0_start: 78, values0_count: 4,
            values1_start: 0, values1_count: 0,
            numerator_type: NUMERATOR_SINGLE_NEG,
            relation_id0: REL_OPCODES, relation_id1: 0,
        },
    ];

    dispatch_generic(&output_buf, col_len, n_rows_real, 82, &logup_layout, lookup_elements)
}

// ===========================================================================
// jump_opcode_rel_imm: 49 output values, 3 logup columns
// Layout: verify_inst(8) + addr_to_id_0(3) + id_to_big_0(30)
//       + opcodes_0(4) + opcodes_1(4)
// Logup pairs:
//   col0: pair_sum(verify_inst[0..8], addr_to_id_0[8..11])
//   col1: enabler_weighted(id_to_big_0[11..41], opcodes_0[41..45])
//   col2: single_neg(opcodes_1[45..49])
// ===========================================================================

/// Generate interaction trace for jump_opcode_rel_imm from GPU trace columns.
pub fn gpu_interaction_trace_jump_opcode_rel_imm(
    trace_cols: &U32Buffer,
    n_rows_real: usize,
    col_len: usize,
    lookup_elements: &InteractionLookupElements,
) -> Result<
    (
        Vec<CircleEvaluation<MetalBackend, BaseField, BitReversedOrder>>,
        SecureField,
    ),
    OpcodeInteractionTraceError,
> {
    let output_buf = U32Buffer::interaction_values_jump_opcode_rel_imm(
        trace_cols,
        n_rows_real as u32,
        col_len as u32,
    )?;

    let logup_layout = vec![
        LogupColumnLayout {
            values0_start: 0, values0_count: 8,
            values1_start: 8, values1_count: 3,
            numerator_type: NUMERATOR_PAIR_SUM,
            relation_id0: REL_VERIFY_INSTRUCTION, relation_id1: REL_MEMORY_ADDR_TO_ID,
        },
        LogupColumnLayout {
            values0_start: 11, values0_count: 30,
            values1_start: 41, values1_count: 4,
            numerator_type: NUMERATOR_ENABLER_WEIGHTED,
            relation_id0: REL_MEMORY_ID_TO_BIG, relation_id1: REL_OPCODES,
        },
        LogupColumnLayout {
            values0_start: 45, values0_count: 4,
            values1_start: 0, values1_count: 0,
            numerator_type: NUMERATOR_SINGLE_NEG,
            relation_id0: REL_OPCODES, relation_id1: 0,
        },
    ];

    dispatch_generic(&output_buf, col_len, n_rows_real, 49, &logup_layout, lookup_elements)
}

// ===========================================================================
// assert_eq_opcode_double_deref: 55 output values, 4 logup columns
// Layout: verify_inst(8) + addr_to_id_0(3) + id_to_big_0(30)
//       + addr_to_id_1(3) + addr_to_id_2(3) + opcodes_0(4) + opcodes_1(4)
// Logup pairs:
//   col0: pair_sum(verify_inst[0..8], addr_to_id_0[8..11])
//   col1: pair_sum(id_to_big_0[11..41], addr_to_id_1[41..44])
//   col2: enabler_weighted(addr_to_id_2[44..47], opcodes_0[47..51])
//   col3: single_neg(opcodes_1[51..55])
// ===========================================================================

/// Generate interaction trace for assert_eq_opcode_double_deref from GPU trace columns.
pub fn gpu_interaction_trace_assert_eq_double_deref(
    trace_cols: &U32Buffer,
    n_rows_real: usize,
    col_len: usize,
    lookup_elements: &InteractionLookupElements,
) -> Result<
    (
        Vec<CircleEvaluation<MetalBackend, BaseField, BitReversedOrder>>,
        SecureField,
    ),
    OpcodeInteractionTraceError,
> {
    let output_buf = U32Buffer::interaction_values_assert_eq_double_deref(
        trace_cols,
        n_rows_real as u32,
        col_len as u32,
    )?;

    let logup_layout = vec![
        LogupColumnLayout {
            values0_start: 0, values0_count: 8,
            values1_start: 8, values1_count: 3,
            numerator_type: NUMERATOR_PAIR_SUM,
            relation_id0: REL_VERIFY_INSTRUCTION, relation_id1: REL_MEMORY_ADDR_TO_ID,
        },
        LogupColumnLayout {
            values0_start: 11, values0_count: 30,
            values1_start: 41, values1_count: 3,
            numerator_type: NUMERATOR_PAIR_SUM,
            relation_id0: REL_MEMORY_ID_TO_BIG, relation_id1: REL_MEMORY_ADDR_TO_ID,
        },
        LogupColumnLayout {
            values0_start: 44, values0_count: 3,
            values1_start: 47, values1_count: 4,
            numerator_type: NUMERATOR_ENABLER_WEIGHTED,
            relation_id0: REL_MEMORY_ADDR_TO_ID, relation_id1: REL_OPCODES,
        },
        LogupColumnLayout {
            values0_start: 51, values0_count: 4,
            values1_start: 0, values1_count: 0,
            numerator_type: NUMERATOR_SINGLE_NEG,
            relation_id0: REL_OPCODES, relation_id1: 0,
        },
    ];

    dispatch_generic(&output_buf, col_len, n_rows_real, 55, &logup_layout, lookup_elements)
}

// ===========================================================================
// jnz_opcode_taken: 82 output values, 4 logup columns
// Layout: verify_inst(8) + addr_to_id_0(3) + id_to_big_0(30)
//       + addr_to_id_1(3) + id_to_big_1(30) + opcodes_0(4) + opcodes_1(4)
// Logup pairs:
//   col0: pair_sum(verify_inst[0..8], addr_to_id_0[8..11])
//   col1: pair_sum(id_to_big_0[11..41], addr_to_id_1[41..44])
//   col2: enabler_weighted(id_to_big_1[44..74], opcodes_0[74..78])
//   col3: single_neg(opcodes_1[78..82])
// ===========================================================================

/// Generate interaction trace for jnz_opcode_taken from GPU trace columns.
pub fn gpu_interaction_trace_jnz_opcode_taken(
    trace_cols: &U32Buffer,
    n_rows_real: usize,
    col_len: usize,
    lookup_elements: &InteractionLookupElements,
) -> Result<
    (
        Vec<CircleEvaluation<MetalBackend, BaseField, BitReversedOrder>>,
        SecureField,
    ),
    OpcodeInteractionTraceError,
> {
    let output_buf = U32Buffer::interaction_values_jnz_opcode_taken(
        trace_cols,
        n_rows_real as u32,
        col_len as u32,
    )?;

    let logup_layout = vec![
        LogupColumnLayout {
            values0_start: 0, values0_count: 8,
            values1_start: 8, values1_count: 3,
            numerator_type: NUMERATOR_PAIR_SUM,
            relation_id0: REL_VERIFY_INSTRUCTION, relation_id1: REL_MEMORY_ADDR_TO_ID,
        },
        LogupColumnLayout {
            values0_start: 11, values0_count: 30,
            values1_start: 41, values1_count: 3,
            numerator_type: NUMERATOR_PAIR_SUM,
            relation_id0: REL_MEMORY_ID_TO_BIG, relation_id1: REL_MEMORY_ADDR_TO_ID,
        },
        LogupColumnLayout {
            values0_start: 44, values0_count: 30,
            values1_start: 74, values1_count: 4,
            numerator_type: NUMERATOR_ENABLER_WEIGHTED,
            relation_id0: REL_MEMORY_ID_TO_BIG, relation_id1: REL_OPCODES,
        },
        LogupColumnLayout {
            values0_start: 78, values0_count: 4,
            values1_start: 0, values1_count: 0,
            numerator_type: NUMERATOR_SINGLE_NEG,
            relation_id0: REL_OPCODES, relation_id1: 0,
        },
    ];

    dispatch_generic(&output_buf, col_len, n_rows_real, 82, &logup_layout, lookup_elements)
}

// ===========================================================================
// call_opcode_rel_imm: 115 output values, 5 logup columns
// Layout: verify_inst(8) + addr_to_id_0(3) + id_to_big_0(30)
//       + addr_to_id_1(3) + id_to_big_1(30) + addr_to_id_2(3)
//       + id_to_big_2(30) + opcodes_0(4) + opcodes_1(4)
// Logup pairs:
//   col0: pair_sum(verify_inst[0..8], addr_to_id_0[8..11])
//   col1: pair_sum(id_to_big_0[11..41], addr_to_id_1[41..44])
//   col2: pair_sum(id_to_big_1[44..74], addr_to_id_2[74..77])
//   col3: enabler_weighted(id_to_big_2[77..107], opcodes_0[107..111])
//   col4: single_neg(opcodes_1[111..115])
// ===========================================================================

/// Generate interaction trace for call_opcode_rel_imm from GPU trace columns.
pub fn gpu_interaction_trace_call_opcode_rel_imm(
    trace_cols: &U32Buffer,
    n_rows_real: usize,
    col_len: usize,
    lookup_elements: &InteractionLookupElements,
) -> Result<
    (
        Vec<CircleEvaluation<MetalBackend, BaseField, BitReversedOrder>>,
        SecureField,
    ),
    OpcodeInteractionTraceError,
> {
    let output_buf = U32Buffer::interaction_values_call_opcode_rel_imm(
        trace_cols,
        n_rows_real as u32,
        col_len as u32,
    )?;

    let logup_layout = vec![
        LogupColumnLayout {
            values0_start: 0, values0_count: 8,
            values1_start: 8, values1_count: 3,
            numerator_type: NUMERATOR_PAIR_SUM,
            relation_id0: REL_VERIFY_INSTRUCTION, relation_id1: REL_MEMORY_ADDR_TO_ID,
        },
        LogupColumnLayout {
            values0_start: 11, values0_count: 30,
            values1_start: 41, values1_count: 3,
            numerator_type: NUMERATOR_PAIR_SUM,
            relation_id0: REL_MEMORY_ID_TO_BIG, relation_id1: REL_MEMORY_ADDR_TO_ID,
        },
        LogupColumnLayout {
            values0_start: 44, values0_count: 30,
            values1_start: 74, values1_count: 3,
            numerator_type: NUMERATOR_PAIR_SUM,
            relation_id0: REL_MEMORY_ID_TO_BIG, relation_id1: REL_MEMORY_ADDR_TO_ID,
        },
        LogupColumnLayout {
            values0_start: 77, values0_count: 30,
            values1_start: 107, values1_count: 4,
            numerator_type: NUMERATOR_ENABLER_WEIGHTED,
            relation_id0: REL_MEMORY_ID_TO_BIG, relation_id1: REL_OPCODES,
        },
        LogupColumnLayout {
            values0_start: 111, values0_count: 4,
            values1_start: 0, values1_count: 0,
            numerator_type: NUMERATOR_SINGLE_NEG,
            relation_id0: REL_OPCODES, relation_id1: 0,
        },
    ];

    dispatch_generic(&output_buf, col_len, n_rows_real, 115, &logup_layout, lookup_elements)
}
