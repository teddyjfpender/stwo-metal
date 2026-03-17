//! GPU-accelerated interaction trace generation for opcode components.
//!
//! Two dispatch modes:
//!
//! **Two-phase (legacy):** Extract LookupData values via a Metal kernel, then feed
//! them into the generic interaction trace kernel. Each opcode requires two dispatches.
//!
//! **Fused (single-pass):** A single Metal kernel reads trace columns AND computes
//! logup fractions in one dispatch. LookupData values stay in registers and are
//! never written to global memory. ~3x faster than the two-phase approach.
//!
//! Supported opcodes:
//! - ret_opcode (16 trace cols, 4 logup cols)
//! - jnz_opcode_taken (47 trace cols, 4 logup cols)
//! - assert_eq_opcode_double_deref (19 trace cols, 4 logup cols)
//! - jump_opcode_rel_imm (13 trace cols, 3 logup cols)
//! - call_opcode_rel_imm (24 trace cols, 5 logup cols)
//! - add_opcode_small (39 trace cols, 5 logup cols)

use stwo::core::fields::m31::{BaseField, M31};
use stwo::core::fields::qm31::{SecureField, SECURE_EXTENSION_DEGREE};
use stwo::core::poly::circle::CanonicCoset;
use stwo::prover::backend::simd::m31::{PackedBaseField, N_LANES};
use stwo::prover::backend::simd::prefix_sum::inclusive_prefix_sum;
use stwo::prover::poly::circle::CircleEvaluation;
use stwo::prover::poly::BitReversedOrder;
use stwo_metal_sys::metal::U32Buffer;

use super::interaction_trace_generic::{
    ColumnDescriptor, GenericInteractionTraceBuilder,
    NUMERATOR_ENABLER_WEIGHTED, NUMERATOR_PAIR_SUM, NUMERATOR_SINGLE_NEG,
};
use super::interaction_trace_id_to_big::InteractionLookupElements;
use super::MetalBackend;
use crate::stwo_metal::base_field_vec::BaseFieldVec;

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

// ===========================================================================
// Fused (single-pass) interaction trace dispatch
// ===========================================================================

/// Parse the output of a fused kernel into CircleEvaluations + claimed_sum.
///
/// The fused kernel output is [n_logup_cols * 4][col_len] u32 running-sum QM31.
/// Non-last columns are zero-copy from GPU. The last column needs prefix-sum
/// finalization on CPU (same as GenericInteractionTraceBuilder::dispatch_metal).
fn parse_fused_output(
    trace_buf: &U32Buffer,
    n_logup_cols: usize,
    col_len: usize,
) -> Result<
    (
        Vec<CircleEvaluation<MetalBackend, BaseField, BitReversedOrder>>,
        SecureField,
    ),
    OpcodeInteractionTraceError,
> {
    let log_size = col_len.ilog2();
    let packed_len = col_len / N_LANES;
    let domain = CanonicCoset::new(log_size).circle_domain();
    let mut all_evals: Vec<CircleEvaluation<MetalBackend, BaseField, BitReversedOrder>> =
        Vec::new();

    // Non-last columns: zero-copy from GPU buffer.
    for logup_col in 0..(n_logup_cols - 1) {
        let base_m31_col = logup_col * 4;
        for coord in 0..SECURE_EXTENSION_DEGREE {
            let col_idx = base_m31_col + coord;
            let offset = col_idx * col_len;
            let col_buf = trace_buf.clone_range(offset, col_len)?;
            all_evals.push(CircleEvaluation::new(
                domain,
                BaseFieldVec::from_buffer(col_buf),
            ));
        }
    }

    // Last column: read back for prefix-sum finalization.
    {
        let last_logup = n_logup_cols - 1;
        let base_m31_col = last_logup * 4;

        let mut coord_columns: [stwo::prover::backend::simd::column::BaseColumn;
            SECURE_EXTENSION_DEGREE] = std::array::from_fn(|coord| {
            let col_idx = base_m31_col + coord;
            let gpu_offset = col_idx * col_len;
            let col_raw = trace_buf
                .clone_range(gpu_offset, col_len)
                .expect("clone_range for prefix-sum column")
                .to_vec()
                .expect("to_vec for prefix-sum column");
            let mut col_data = Vec::with_capacity(packed_len);
            for vec_row in 0..packed_len {
                let mut arr = [M31(0); N_LANES];
                for lane in 0..N_LANES {
                    arr[lane] = M31(col_raw[vec_row * N_LANES + lane]);
                }
                col_data.push(PackedBaseField::from_array(arr));
            }
            stwo::prover::backend::simd::column::BaseColumn::from_simd(col_data)
        });

        // Compute cumsum_shift = sum_of_all_elements / domain_size.
        let coordinate_sums: [BaseField; SECURE_EXTENSION_DEGREE] =
            std::array::from_fn(|i| {
                coord_columns[i]
                    .data
                    .iter()
                    .copied()
                    .sum::<PackedBaseField>()
                    .pointwise_sum()
            });
        let claimed_sum = SecureField::from_m31_array(coordinate_sums);
        let cumsum_shift =
            claimed_sum / BaseField::from_u32_unchecked(1 << log_size);
        let packed_cumsum_shift =
            stwo::prover::backend::simd::qm31::PackedSecureField::broadcast(cumsum_shift);

        // Subtract cumsum_shift from each element.
        for (i, col) in coord_columns.iter_mut().enumerate() {
            for x in col.data.iter_mut() {
                *x -= packed_cumsum_shift.into_packed_m31s()[i];
            }
        }

        // Inclusive prefix sum on each coordinate.
        let coord_prefix_sum = coord_columns.map(inclusive_prefix_sum);

        // Re-upload prefix-summed columns to Metal.
        for col in coord_prefix_sum {
            let raw: Vec<u32> = col.data.iter()
                .flat_map(|packed| packed.to_array().map(|v| v.0))
                .collect();
            let col_buf = U32Buffer::from_slice(&raw)?;
            all_evals.push(CircleEvaluation::new(
                domain,
                BaseFieldVec::from_buffer(col_buf),
            ));
        }

        return Ok((all_evals, claimed_sum));
    }
}

/// Upload alpha powers and z to GPU buffers for fused kernels.
fn upload_lookup_elements(
    lookup_elements: &InteractionLookupElements,
    max_combine_size: usize,
) -> Result<(U32Buffer, U32Buffer), OpcodeInteractionTraceError> {
    let n_powers = max_combine_size.min(lookup_elements.alpha_powers.len());
    let mut flat_alpha: Vec<u32> = Vec::with_capacity(n_powers * 4);
    for ap in &lookup_elements.alpha_powers[..n_powers] {
        flat_alpha.extend_from_slice(ap);
    }
    let alpha_buf = U32Buffer::from_slice(&flat_alpha)?;
    let z_buf = U32Buffer::from_slice(&lookup_elements.z)?;
    Ok((alpha_buf, z_buf))
}

/// Fused interaction trace for ret_opcode (single dispatch).
pub fn gpu_fused_interaction_trace_ret_opcode(
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
    let (alpha_buf, z_buf) = upload_lookup_elements(lookup_elements, 30)?;
    let trace_buf = U32Buffer::fused_interaction_ret_opcode(
        trace_cols, &alpha_buf, &z_buf,
        n_rows_real as u32, col_len as u32,
    )?;
    parse_fused_output(&trace_buf, 4, col_len)
}

/// Fused interaction trace for jump_opcode_rel_imm (single dispatch).
pub fn gpu_fused_interaction_trace_jump_opcode_rel_imm(
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
    let (alpha_buf, z_buf) = upload_lookup_elements(lookup_elements, 30)?;
    let trace_buf = U32Buffer::fused_interaction_jump_opcode_rel_imm(
        trace_cols, &alpha_buf, &z_buf,
        n_rows_real as u32, col_len as u32,
    )?;
    parse_fused_output(&trace_buf, 3, col_len)
}

/// Fused interaction trace for assert_eq_double_deref (single dispatch).
pub fn gpu_fused_interaction_trace_assert_eq_double_deref(
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
    let (alpha_buf, z_buf) = upload_lookup_elements(lookup_elements, 30)?;
    let trace_buf = U32Buffer::fused_interaction_assert_eq_double_deref(
        trace_cols, &alpha_buf, &z_buf,
        n_rows_real as u32, col_len as u32,
    )?;
    parse_fused_output(&trace_buf, 4, col_len)
}

/// Fused interaction trace for jnz_opcode_taken (single dispatch).
pub fn gpu_fused_interaction_trace_jnz_opcode_taken(
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
    let (alpha_buf, z_buf) = upload_lookup_elements(lookup_elements, 30)?;
    let trace_buf = U32Buffer::fused_interaction_jnz_opcode_taken(
        trace_cols, &alpha_buf, &z_buf,
        n_rows_real as u32, col_len as u32,
    )?;
    parse_fused_output(&trace_buf, 4, col_len)
}

/// Fused interaction trace for call_opcode_rel_imm (single dispatch).
pub fn gpu_fused_interaction_trace_call_opcode_rel_imm(
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
    let (alpha_buf, z_buf) = upload_lookup_elements(lookup_elements, 30)?;
    let trace_buf = U32Buffer::fused_interaction_call_opcode_rel_imm(
        trace_cols, &alpha_buf, &z_buf,
        n_rows_real as u32, col_len as u32,
    )?;
    parse_fused_output(&trace_buf, 5, col_len)
}

/// Fused interaction trace for add_opcode_small (single dispatch).
pub fn gpu_fused_interaction_trace_add_opcode_small(
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
    let (alpha_buf, z_buf) = upload_lookup_elements(lookup_elements, 30)?;
    let trace_buf = U32Buffer::fused_interaction_add_opcode_small(
        trace_cols, &alpha_buf, &z_buf,
        n_rows_real as u32, col_len as u32,
    )?;
    parse_fused_output(&trace_buf, 5, col_len)
}
