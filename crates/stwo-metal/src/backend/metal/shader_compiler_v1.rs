//! Native Metal shader compiler for V1 bytecode programs.
//!
//! Translates an [`OwnedMetalEvaluationProgramV1`] into a Metal shader source
//! string where every V1 instruction is unrolled into a direct Metal
//! statement.  This eliminates the interpretation loop overhead and
//! switch/case dispatch present in the generic interpreter kernel.
//!
//! The generated kernel uses the same buffer bindings as the interpreter
//! kernel (`eval_program_v1_reference_u32x4`), except the instruction
//! buffers (`base_insts`, `ext_insts`, `constraint_roots`) and their count
//! uniforms are no longer needed — instructions are baked into the shader
//! source.

use super::eval_program_v1::{
    MetalEvaluationProgramBaseOpcodeV1, MetalEvaluationProgramExtOpcodeV1,
    OwnedMetalEvaluationProgramV1,
};

/// Compile a V1 evaluation program into a Metal shader source string.
///
/// The returned source is a complete, self-contained Metal shader file that
/// includes the necessary field arithmetic functions inline (so it does not
/// depend on external header files at compile time) and defines a single
/// `kernel void` entry point whose name encodes the program's semantic hash.
///
/// # Panics
///
/// Panics if any instruction contains an unknown opcode.  This should never
/// happen with programs produced by [`lower_framework_eval_to_v1`].
pub fn compile_v1_to_metal_source(program: &OwnedMetalEvaluationProgramV1) -> String {
    let header = program.header();

    let kernel_name = format!("eval_compiled_{:016x}", header.semantic_hash);

    let mut src = String::with_capacity(8192);

    // ── Preamble: inline field arithmetic ──────────────────────────────────
    emit_preamble(&mut src);

    // ── Kernel signature ───────────────────────────────────────────────────
    //
    // Buffer layout matches the interpreter kernel.  We keep `trace_values`,
    // `interaction_offsets`, `preprocessed_values`, `base_params`,
    // `ext_params`, `random_coeff_powers`, and `dst` at their original
    // binding indices.  The instruction/count buffers (6-8, 11-13) are not
    // bound because the instructions are baked in.
    src.push_str(&format!(
        "kernel void {kernel_name}(\n\
         \x20   device const uint *trace_values [[buffer(0)]],\n\
         \x20   device const uint *interaction_offsets [[buffer(1)]],\n\
         \x20   device const uint *preprocessed_values [[buffer(2)]],\n\
         \x20   device const uint *base_params [[buffer(3)]],\n\
         \x20   device const uint *ext_params [[buffer(4)]],\n\
         \x20   device const uint *random_coeff_powers [[buffer(5)]],\n\
         \x20   device uint *dst [[buffer(9)]],\n\
         \x20   constant uint &row_count [[buffer(10)]],\n\
         \x20   uint row_index [[thread_position_in_grid]]\n\
         ) {{\n"
    ));

    src.push_str("    if (row_index >= row_count) {\n");
    src.push_str("        return;\n");
    src.push_str("    }\n\n");

    // ── Shared instruction body + constraint accumulation ───────────────
    emit_instruction_body(program, &mut src);

    // ── Store result ───────────────────────────────────────────────────────
    src.push_str("    stwo_metal_store_qm31(dst, row_index, acc);\n");
    src.push_str("}\n");

    src
}

/// Returns the kernel function name for a given semantic hash.
pub fn compiled_kernel_name(semantic_hash: u64) -> String {
    format!("eval_compiled_{:016x}", semantic_hash)
}

/// Returns the fused kernel name for a given semantic hash.
///
/// The fused variant applies denominator inverse multiplication and writes
/// directly to 4 coordinate buffers, eliminating the GPU->CPU->GPU
/// round-trip in the composition pipeline.
pub fn compiled_fused_kernel_name(semantic_hash: u64) -> String {
    format!("eval_compiled_fused_{:016x}", semantic_hash)
}

/// Compile a V1 evaluation program into a Metal shader source string
/// containing **only** the fused composition kernel.
///
/// The returned source contains a single `eval_compiled_fused_<hash>` kernel
/// that:
/// - Evaluates the constraint accumulation.
/// - Multiplies the result by a per-row denominator inverse scalar
///   (`denom_inv[row_index >> log_n_rows]`).
/// - Writes the 4 QM31 coordinates directly to separate output buffers
///   (`coord_0..coord_3`), matching `SecureColumnByCoords` layout.
///
/// Unlike the previous implementation which prepended the entire standard
/// kernel source (doubling JIT compilation time), this generates only the
/// fused kernel with its own preamble.
pub fn compile_v1_to_metal_source_with_fused(
    program: &OwnedMetalEvaluationProgramV1,
) -> String {
    let header = program.header();
    let fused_name = compiled_fused_kernel_name(header.semantic_hash);

    let mut src = String::with_capacity(8192);

    // ── Preamble: inline field arithmetic ──────────────────────────────────
    emit_preamble(&mut src);

    // ── Fused kernel signature ──────────────────────────────────────────
    //
    // Buffer layout:
    //   buffer(0): trace_values
    //   buffer(1): interaction_offsets
    //   buffer(2): preprocessed_values
    //   buffer(3): base_params
    //   buffer(4): ext_params
    //   buffer(5): random_coeff_powers
    //   buffer(6): denom_inv — small array of M31 denominator inverses
    //   buffer(7): coord_0 — output coordinate buffer 0
    //   buffer(8): coord_1 — output coordinate buffer 1
    //   buffer(10): row_count
    //   buffer(11): coord_2 — output coordinate buffer 2
    //   buffer(12): coord_3 — output coordinate buffer 3
    //   buffer(13): log_n_rows — shift amount for denom_inv indexing
    src.push_str(&format!(
        "kernel void {fused_name}(\n\
         \x20   device const uint *trace_values [[buffer(0)]],\n\
         \x20   device const uint *interaction_offsets [[buffer(1)]],\n\
         \x20   device const uint *preprocessed_values [[buffer(2)]],\n\
         \x20   device const uint *base_params [[buffer(3)]],\n\
         \x20   device const uint *ext_params [[buffer(4)]],\n\
         \x20   device const uint *random_coeff_powers [[buffer(5)]],\n\
         \x20   device const uint *denom_inv [[buffer(6)]],\n\
         \x20   device uint *coord_0 [[buffer(7)]],\n\
         \x20   device uint *coord_1 [[buffer(8)]],\n\
         \x20   constant uint &row_count [[buffer(10)]],\n\
         \x20   device uint *coord_2 [[buffer(11)]],\n\
         \x20   device uint *coord_3 [[buffer(12)]],\n\
         \x20   constant uint &log_n_rows [[buffer(13)]],\n\
         \x20   uint row_index [[thread_position_in_grid]]\n\
         ) {{\n"
    ));

    src.push_str("    if (row_index >= row_count) {\n");
    src.push_str("        return;\n");
    src.push_str("    }\n\n");

    // ── Shared instruction body + constraint accumulation ───────────────
    emit_instruction_body(program, &mut src);

    // ── Fused denom_inv multiply + coordinate store ─────────────────────
    src.push_str("    // ── Fused denom_inv multiply + coordinate unpack ──\n");
    src.push_str("    uint denom_idx = row_index >> log_n_rows;\n");
    src.push_str("    StwoMetalQm31 result = stwo_metal_qm31_mul_base(acc, denom_inv[denom_idx]);\n");
    src.push_str("    coord_0[row_index] = result.a;\n");
    src.push_str("    coord_1[row_index] = result.b;\n");
    src.push_str("    coord_2[row_index] = result.c;\n");
    src.push_str("    coord_3[row_index] = result.d;\n");
    src.push_str("}\n");

    src
}

// ---------------------------------------------------------------------------
// Shared instruction body emission
// ---------------------------------------------------------------------------

/// Emit the base instructions, ext instructions, and constraint accumulation
/// into the given source string.  This is the shared body used by both the
/// standard kernel and the fused composition kernel.
///
/// On return, the source contains local variables `b0..bN`, `e0..eM`, and
/// the accumulated result in `StwoMetalQm31 acc`.
fn emit_instruction_body(program: &OwnedMetalEvaluationProgramV1, src: &mut String) {
    let header = program.header();
    let base_insts = program.base_insts();
    let ext_insts = program.ext_insts();
    let constraint_roots = program.constraint_roots();

    let n_base_regs = header.max_base_regs as usize;
    let n_ext_regs = header.max_ext_regs as usize;

    // Track which base/ext registers have been written to avoid re-declaring.
    let mut base_declared = vec![false; n_base_regs];
    let mut ext_declared = vec![false; n_ext_regs];

    // ── Base instructions ──────────────────────────────────────────────────
    src.push_str("    // ── Base instructions ──\n");
    for inst in base_insts {
        let dst = inst.dst as usize;
        let decl = if !base_declared[dst] {
            base_declared[dst] = true;
            "uint "
        } else {
            ""
        };
        let dst_var = format!("b{}", dst);

        let opcode = MetalEvaluationProgramBaseOpcodeV1::from_raw(inst.op)
            .unwrap_or_else(|| panic!("unknown base opcode {}", inst.op));

        match opcode {
            MetalEvaluationProgramBaseOpcodeV1::TraceCol => {
                let interaction = inst.interaction;
                let column = inst.a;
                let offset = inst.imm;
                src.push_str(&format!(
                    "    {decl}{dst_var} = stwo_metal_eval_program_trace_value(\
                     trace_values, interaction_offsets, row_count, \
                     {interaction}u, {column}u, row_index, {offset});\n"
                ));
            }
            MetalEvaluationProgramBaseOpcodeV1::PreprocessedCol => {
                let column = inst.a;
                src.push_str(&format!(
                    "    {decl}{dst_var} = stwo_metal_eval_program_preprocessed_value(\
                     preprocessed_values, row_count, {column}u, row_index);\n"
                ));
            }
            MetalEvaluationProgramBaseOpcodeV1::Param => {
                let slot = inst.a;
                src.push_str(&format!(
                    "    {decl}{dst_var} = base_params[{slot}u];\n"
                ));
            }
            MetalEvaluationProgramBaseOpcodeV1::Const => {
                let value = inst.a;
                src.push_str(&format!("    {decl}{dst_var} = {value}u;\n"));
            }
            MetalEvaluationProgramBaseOpcodeV1::Add => {
                let a = inst.a;
                let b = inst.b;
                src.push_str(&format!(
                    "    {decl}{dst_var} = stwo_metal_m31_add(b{a}, b{b});\n"
                ));
            }
            MetalEvaluationProgramBaseOpcodeV1::Sub => {
                let a = inst.a;
                let b = inst.b;
                src.push_str(&format!(
                    "    {decl}{dst_var} = stwo_metal_m31_sub(b{a}, b{b});\n"
                ));
            }
            MetalEvaluationProgramBaseOpcodeV1::Mul => {
                let a = inst.a;
                let b = inst.b;
                src.push_str(&format!(
                    "    {decl}{dst_var} = stwo_metal_m31_mul(b{a}, b{b});\n"
                ));
            }
            MetalEvaluationProgramBaseOpcodeV1::Neg => {
                let a = inst.a;
                src.push_str(&format!(
                    "    {decl}{dst_var} = stwo_metal_m31_neg(b{a});\n"
                ));
            }
            MetalEvaluationProgramBaseOpcodeV1::Inv => {
                let a = inst.a;
                src.push_str(&format!(
                    "    {decl}{dst_var} = stwo_metal_m31_inv(b{a});\n"
                ));
            }
        }
    }
    src.push('\n');

    // ── Ext instructions ───────────────────────────────────────────────────
    src.push_str("    // ── Ext instructions ──\n");
    for inst in ext_insts {
        let dst = inst.dst as usize;
        let decl = if !ext_declared[dst] {
            ext_declared[dst] = true;
            "StwoMetalQm31 "
        } else {
            ""
        };
        let dst_var = format!("e{}", dst);

        let opcode = MetalEvaluationProgramExtOpcodeV1::from_raw(inst.op)
            .unwrap_or_else(|| panic!("unknown ext opcode {}", inst.op));

        match opcode {
            MetalEvaluationProgramExtOpcodeV1::SecureCol => {
                let a = inst.a;
                let b = inst.b;
                let c = inst.c;
                let d = inst.d;
                src.push_str(&format!(
                    "    {decl}{dst_var} = StwoMetalQm31 {{ b{a}, b{b}, b{c}, b{d} }};\n"
                ));
            }
            MetalEvaluationProgramExtOpcodeV1::Param => {
                let slot = inst.a;
                src.push_str(&format!(
                    "    {decl}{dst_var} = stwo_metal_load_qm31(ext_params, {slot}u);\n"
                ));
            }
            MetalEvaluationProgramExtOpcodeV1::Const => {
                let a = inst.a;
                let b = inst.b;
                let c = inst.c;
                let d = inst.d;
                src.push_str(&format!(
                    "    {decl}{dst_var} = StwoMetalQm31 {{ {a}u, {b}u, {c}u, {d}u }};\n"
                ));
            }
            MetalEvaluationProgramExtOpcodeV1::Add => {
                let a = inst.a;
                let b = inst.b;
                src.push_str(&format!(
                    "    {decl}{dst_var} = stwo_metal_qm31_add(e{a}, e{b});\n"
                ));
            }
            MetalEvaluationProgramExtOpcodeV1::Sub => {
                let a = inst.a;
                let b = inst.b;
                src.push_str(&format!(
                    "    {decl}{dst_var} = stwo_metal_qm31_sub(e{a}, e{b});\n"
                ));
            }
            MetalEvaluationProgramExtOpcodeV1::Mul => {
                let a = inst.a;
                let b = inst.b;
                src.push_str(&format!(
                    "    {decl}{dst_var} = stwo_metal_qm31_mul(e{a}, e{b});\n"
                ));
            }
            MetalEvaluationProgramExtOpcodeV1::Neg => {
                let a = inst.a;
                src.push_str(&format!(
                    "    {decl}{dst_var} = stwo_metal_qm31_sub(\
                     StwoMetalQm31 {{ 0u, 0u, 0u, 0u }}, e{a});\n"
                ));
            }
        }
    }
    src.push('\n');

    // ── Constraint accumulation ────────────────────────────────────────────
    src.push_str("    // ── Constraint accumulation ──\n");
    src.push_str("    StwoMetalQm31 acc = StwoMetalQm31 { 0u, 0u, 0u, 0u };\n");
    for (i, &root) in constraint_roots.iter().enumerate() {
        src.push_str(&format!(
            "    acc = stwo_metal_qm31_add(acc, \
             stwo_metal_qm31_mul(e{root}, \
             stwo_metal_load_qm31(random_coeff_powers, {i}u)));\n"
        ));
    }
    src.push('\n');
}

// ---------------------------------------------------------------------------
// Preamble: inline field arithmetic and helper functions
// ---------------------------------------------------------------------------

fn emit_preamble(src: &mut String) {
    src.push_str(
        "\
#include <metal_stdlib>
using namespace metal;

// ── M31 field constants and arithmetic ─────────────────────────────────────
constant uint STWO_METAL_M31_P = 2147483647u;

static inline uint stwo_metal_m31_add(uint lhs, uint rhs) {
    uint sum = lhs + rhs;
    return sum >= STWO_METAL_M31_P ? sum - STWO_METAL_M31_P : sum;
}

static inline uint stwo_metal_m31_sub(uint lhs, uint rhs) {
    return lhs >= rhs ? lhs - rhs : lhs + STWO_METAL_M31_P - rhs;
}

static inline uint stwo_metal_m31_neg(uint value) {
    uint negated = STWO_METAL_M31_P - value;
    return negated == STWO_METAL_M31_P ? 0u : negated;
}

static inline uint stwo_metal_m31_mul(uint lhs, uint rhs) {
    ulong product = (ulong)lhs * (ulong)rhs;
    ulong reduced =
        (((((product >> 31u) + product + 1u) >> 31u) + product) & (ulong)STWO_METAL_M31_P);
    return (uint)reduced;
}

static inline uint stwo_metal_m31_square(uint value) {
    return stwo_metal_m31_mul(value, value);
}

static inline uint stwo_metal_m31_pow_to_power_of_two(uint squarings, uint value) {
    uint result = value;
    for (uint i = 0; i < squarings; ++i) {
        result = stwo_metal_m31_square(result);
    }
    return result;
}

static inline uint stwo_metal_m31_inv(uint value) {
    uint t0 = stwo_metal_m31_mul(stwo_metal_m31_pow_to_power_of_two(2u, value), value);
    uint t1 = stwo_metal_m31_mul(stwo_metal_m31_pow_to_power_of_two(1u, t0), t0);
    uint t2 = stwo_metal_m31_mul(stwo_metal_m31_pow_to_power_of_two(3u, t1), t0);
    uint t3 = stwo_metal_m31_mul(stwo_metal_m31_pow_to_power_of_two(1u, t2), t0);
    uint t4 = stwo_metal_m31_mul(stwo_metal_m31_pow_to_power_of_two(8u, t3), t3);
    uint t5 = stwo_metal_m31_mul(stwo_metal_m31_pow_to_power_of_two(8u, t4), t3);
    return stwo_metal_m31_mul(stwo_metal_m31_pow_to_power_of_two(7u, t5), t2);
}

// ── QM31 (secure field) types and arithmetic ───────────────────────────────
struct StwoMetalQm31 {
    uint a;
    uint b;
    uint c;
    uint d;
};

static inline StwoMetalQm31 stwo_metal_qm31_add(StwoMetalQm31 lhs, StwoMetalQm31 rhs) {
    return StwoMetalQm31 {
        stwo_metal_m31_add(lhs.a, rhs.a),
        stwo_metal_m31_add(lhs.b, rhs.b),
        stwo_metal_m31_add(lhs.c, rhs.c),
        stwo_metal_m31_add(lhs.d, rhs.d),
    };
}

static inline StwoMetalQm31 stwo_metal_qm31_sub(StwoMetalQm31 lhs, StwoMetalQm31 rhs) {
    return StwoMetalQm31 {
        stwo_metal_m31_sub(lhs.a, rhs.a),
        stwo_metal_m31_sub(lhs.b, rhs.b),
        stwo_metal_m31_sub(lhs.c, rhs.c),
        stwo_metal_m31_sub(lhs.d, rhs.d),
    };
}

static inline StwoMetalQm31 stwo_metal_qm31_mul_base(StwoMetalQm31 value, uint scalar) {
    return StwoMetalQm31 {
        stwo_metal_m31_mul(value.a, scalar),
        stwo_metal_m31_mul(value.b, scalar),
        stwo_metal_m31_mul(value.c, scalar),
        stwo_metal_m31_mul(value.d, scalar),
    };
}

static inline StwoMetalQm31 stwo_metal_qm31_mul(StwoMetalQm31 lhs, StwoMetalQm31 rhs) {
    uint a0 = lhs.a;
    uint a1 = lhs.b;
    uint a2 = lhs.c;
    uint a3 = lhs.d;
    uint b0 = rhs.a;
    uint b1 = rhs.b;
    uint b2 = rhs.c;
    uint b3 = rhs.d;

    uint x0 = stwo_metal_m31_sub(stwo_metal_m31_mul(a0, b0), stwo_metal_m31_mul(a1, b1));
    uint x1 = stwo_metal_m31_add(stwo_metal_m31_mul(a0, b1), stwo_metal_m31_mul(a1, b0));
    uint y0 = stwo_metal_m31_sub(stwo_metal_m31_mul(a2, b2), stwo_metal_m31_mul(a3, b3));
    uint y1 = stwo_metal_m31_add(stwo_metal_m31_mul(a2, b3), stwo_metal_m31_mul(a3, b2));

    uint cross0 = stwo_metal_m31_sub(stwo_metal_m31_mul(a0, b2), stwo_metal_m31_mul(a1, b3));
    uint cross1 = stwo_metal_m31_add(stwo_metal_m31_mul(a0, b3), stwo_metal_m31_mul(a1, b2));
    uint cross2 = stwo_metal_m31_sub(stwo_metal_m31_mul(a2, b0), stwo_metal_m31_mul(a3, b1));
    uint cross3 = stwo_metal_m31_add(stwo_metal_m31_mul(a2, b1), stwo_metal_m31_mul(a3, b0));

    uint r_y0 = stwo_metal_m31_sub(stwo_metal_m31_mul(2u, y0), y1);
    uint r_y1 = stwo_metal_m31_add(y0, stwo_metal_m31_mul(2u, y1));

    return StwoMetalQm31 {
        stwo_metal_m31_add(x0, r_y0),
        stwo_metal_m31_add(x1, r_y1),
        stwo_metal_m31_add(cross0, cross2),
        stwo_metal_m31_add(cross1, cross3),
    };
}

static inline StwoMetalQm31 stwo_metal_load_qm31(device const uint *values, uint index) {
    uint base = index * 4u;
    return StwoMetalQm31 {
        values[base + 0u],
        values[base + 1u],
        values[base + 2u],
        values[base + 3u],
    };
}

static inline void stwo_metal_store_qm31(device uint *values, uint index, StwoMetalQm31 value) {
    uint base = index * 4u;
    values[base + 0u] = value.a;
    values[base + 1u] = value.b;
    values[base + 2u] = value.c;
    values[base + 3u] = value.d;
}

// ── Trace value helper ─────────────────────────────────────────────────────
static inline uint stwo_metal_bit_reverse(uint index, uint bits) {
    // Use Metal's hardware reverse_bits intrinsic and shift to get
    // 'bits'-wide reversal.  This replaces a variable-iteration loop with
    // a single ALU instruction + shift.
    return reverse_bits(index) >> (32u - bits);
}

static inline uint stwo_metal_offset_bit_reversed_circle_domain_index(
    uint i,
    uint domain_log_size,
    uint eval_log_size,
    int offset
) {
    uint prev = stwo_metal_bit_reverse(i, eval_log_size);
    uint half_size = 1u << (eval_log_size - 1u);
    int step = offset * int(1u << (eval_log_size - domain_log_size - 1u));
    if (prev < half_size) {
        prev = uint((int(prev) + step) % int(half_size));
        if (int(prev) < 0) prev += half_size;
    } else {
        int p = int(prev) - step;
        p = p % int(half_size);
        if (p < 0) p += int(half_size);
        prev = uint(p) + half_size;
    }
    return stwo_metal_bit_reverse(prev, eval_log_size);
}

static inline uint stwo_metal_eval_program_trace_value(
    device const uint *trace_values,
    device const uint *interaction_offsets,
    uint row_count,
    uint interaction,
    uint column,
    uint row_index,
    int offset
) {
    uint target_row;
    if (offset == 0) {
        target_row = row_index;
    } else {
        uint eval_log_size = 0u;
        uint tmp = row_count;
        while (tmp > 1u) { tmp >>= 1u; eval_log_size++; }
        uint domain_log_size = eval_log_size - 1u;
        target_row = stwo_metal_offset_bit_reversed_circle_domain_index(
            row_index, domain_log_size, eval_log_size, offset
        );
    }
    uint global_column = interaction_offsets[interaction] + column;
    return trace_values[global_column * row_count + target_row];
}

static inline uint stwo_metal_eval_program_preprocessed_value(
    device const uint *preprocessed_values,
    uint row_count,
    uint column,
    uint row_index
) {
    return preprocessed_values[column * row_count + row_index];
}

// ── Generated kernel ───────────────────────────────────────────────────────
",
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::metal::eval_program_v1::{
        MetalEvaluationProgramBaseInstV1, MetalEvaluationProgramBaseOpcodeV1,
        MetalEvaluationProgramExtInstV1, MetalEvaluationProgramExtOpcodeV1,
    };

    /// Build a minimal program from raw instructions for testing the compiler.
    fn build_test_program(
        base_insts: Vec<MetalEvaluationProgramBaseInstV1>,
        ext_insts: Vec<MetalEvaluationProgramExtInstV1>,
        constraint_roots: Vec<u32>,
        max_base_regs: u32,
        max_ext_regs: u32,
    ) -> OwnedMetalEvaluationProgramV1 {
        // Use the crate-internal builder (bypasses the public API but is
        // adequate for unit tests of the compiler itself).
        use crate::backend::metal::eval_program_v1::{
            MetalEvaluationProgramHeaderV1, MetalEvaluationProgramSectionDescV1,
            MetalEvaluationProgramSectionKindV1,
            STWO_METAL_EVAL_PROGRAM_CAP_BASE_INV_V1,
            STWO_METAL_EVAL_PROGRAM_CAP_EXT_MUL_V1,
            STWO_METAL_EVAL_PROGRAM_CAP_PREFINALIZED_LOGUP_V1,
        };

        let capability_bits = STWO_METAL_EVAL_PROGRAM_CAP_BASE_INV_V1
            | STWO_METAL_EVAL_PROGRAM_CAP_EXT_MUL_V1
            | STWO_METAL_EVAL_PROGRAM_CAP_PREFINALIZED_LOGUP_V1;

        let base_consts: Vec<u32> = Vec::new();
        let ext_consts: Vec<[u32; 4]> = Vec::new();

        let base_inst_size = std::mem::size_of::<MetalEvaluationProgramBaseInstV1>() as u32;
        let ext_inst_size = std::mem::size_of::<MetalEvaluationProgramExtInstV1>() as u32;

        let mut offset_bytes = 0u64;
        let mut next_section =
            |kind: MetalEvaluationProgramSectionKindV1, elem_size: u32, count: u64| {
                let section = MetalEvaluationProgramSectionDescV1::new(
                    kind,
                    elem_size,
                    offset_bytes,
                    count,
                );
                offset_bytes += elem_size as u64 * count;
                section
            };

        let sections = vec![
            next_section(MetalEvaluationProgramSectionKindV1::BaseConsts, 4, 0),
            next_section(MetalEvaluationProgramSectionKindV1::ExtConsts, 16, 0),
            next_section(
                MetalEvaluationProgramSectionKindV1::BaseInsts,
                base_inst_size,
                base_insts.len() as u64,
            ),
            next_section(
                MetalEvaluationProgramSectionKindV1::ExtInsts,
                ext_inst_size,
                ext_insts.len() as u64,
            ),
            next_section(
                MetalEvaluationProgramSectionKindV1::ConstraintRoots,
                4,
                constraint_roots.len() as u64,
            ),
        ];

        let header = MetalEvaluationProgramHeaderV1::new(
            sections.len() as u32,
            0x1234_5678_9abc_def0, // dummy semantic hash
            capability_bits,
            2, // n_interactions
            0, // n_base_params
            0, // n_ext_params
            constraint_roots.len() as u32,
            max_base_regs,
            max_ext_regs,
        );

        OwnedMetalEvaluationProgramV1::from_parts(
            header,
            sections,
            base_consts,
            ext_consts,
            base_insts,
            ext_insts,
            constraint_roots,
        )
    }

    #[test]
    fn test_compile_simple_program() {
        // Build a small program:
        //   b0 = TraceCol(interaction=1, column=0, offset=0)
        //   b1 = TraceCol(interaction=1, column=1, offset=0)
        //   b2 = Mul(b0, b1)
        //   b3 = Const(42)
        //   b4 = Sub(b2, b3)
        //   e0 = SecureCol(b4, b3, b3, b3)   (promotion with padding)
        //   constraint_roots = [0]

        let base_insts = vec![
            MetalEvaluationProgramBaseInstV1::trace_col(0, 1, 0, 0),
            MetalEvaluationProgramBaseInstV1::trace_col(1, 1, 1, 0),
            MetalEvaluationProgramBaseInstV1::binary(
                MetalEvaluationProgramBaseOpcodeV1::Mul,
                2,
                0,
                1,
            ),
            MetalEvaluationProgramBaseInstV1::const_value(3, 42),
            MetalEvaluationProgramBaseInstV1::binary(
                MetalEvaluationProgramBaseOpcodeV1::Sub,
                4,
                2,
                3,
            ),
        ];

        let ext_insts = vec![MetalEvaluationProgramExtInstV1::secure_col(0, 4, 3, 3, 3)];

        let source = compile_v1_to_metal_source(&build_test_program(
            base_insts,
            ext_insts,
            vec![0],
            5,
            1,
        ));

        // The kernel name should encode the semantic hash.
        assert!(source.contains("eval_compiled_123456789abcdef0"));

        // Verify that the base instructions are present as direct statements.
        assert!(source.contains("uint b0 = stwo_metal_eval_program_trace_value("));
        assert!(source.contains("uint b1 = stwo_metal_eval_program_trace_value("));
        assert!(source.contains("uint b2 = stwo_metal_m31_mul(b0, b1)"));
        assert!(source.contains("uint b3 = 42u"));
        assert!(source.contains("uint b4 = stwo_metal_m31_sub(b2, b3)"));

        // Verify ext instruction.
        assert!(source.contains("StwoMetalQm31 e0 = StwoMetalQm31 { b4, b3, b3, b3 }"));

        // Verify constraint accumulation.
        assert!(source.contains("stwo_metal_qm31_mul(e0, stwo_metal_load_qm31(random_coeff_powers, 0u))"));

        // Verify the store.
        assert!(source.contains("stwo_metal_store_qm31(dst, row_index, acc)"));
    }

    #[test]
    fn test_compile_all_base_opcodes() {
        let base_insts = vec![
            MetalEvaluationProgramBaseInstV1::trace_col(0, 0, 0, 0),
            MetalEvaluationProgramBaseInstV1 {
                op: MetalEvaluationProgramBaseOpcodeV1::PreprocessedCol as u8,
                interaction: 0,
                dst: 1,
                a: 5,
                b: 0,
                imm: 0,
            },
            MetalEvaluationProgramBaseInstV1 {
                op: MetalEvaluationProgramBaseOpcodeV1::Param as u8,
                interaction: 0,
                dst: 2,
                a: 3,
                b: 0,
                imm: 0,
            },
            MetalEvaluationProgramBaseInstV1::const_value(3, 100),
            MetalEvaluationProgramBaseInstV1::binary(
                MetalEvaluationProgramBaseOpcodeV1::Add,
                4,
                0,
                1,
            ),
            MetalEvaluationProgramBaseInstV1::binary(
                MetalEvaluationProgramBaseOpcodeV1::Sub,
                5,
                2,
                3,
            ),
            MetalEvaluationProgramBaseInstV1::binary(
                MetalEvaluationProgramBaseOpcodeV1::Mul,
                6,
                4,
                5,
            ),
            MetalEvaluationProgramBaseInstV1 {
                op: MetalEvaluationProgramBaseOpcodeV1::Neg as u8,
                interaction: 0,
                dst: 7,
                a: 6,
                b: 0,
                imm: 0,
            },
            MetalEvaluationProgramBaseInstV1 {
                op: MetalEvaluationProgramBaseOpcodeV1::Inv as u8,
                interaction: 0,
                dst: 8,
                a: 7,
                b: 0,
                imm: 0,
            },
        ];

        let source =
            compile_v1_to_metal_source(&build_test_program(base_insts, vec![], vec![], 9, 0));

        assert!(source.contains("stwo_metal_eval_program_trace_value("));
        assert!(source.contains("stwo_metal_eval_program_preprocessed_value("));
        assert!(source.contains("base_params[3u]"));
        assert!(source.contains("uint b3 = 100u"));
        assert!(source.contains("stwo_metal_m31_add(b0, b1)"));
        assert!(source.contains("stwo_metal_m31_sub(b2, b3)"));
        assert!(source.contains("stwo_metal_m31_mul(b4, b5)"));
        assert!(source.contains("stwo_metal_m31_neg(b6)"));
        assert!(source.contains("stwo_metal_m31_inv(b7)"));
    }

    #[test]
    fn test_compile_all_ext_opcodes() {
        // We need at least some base regs for SecureCol to reference.
        let base_insts = vec![
            MetalEvaluationProgramBaseInstV1::const_value(0, 1),
            MetalEvaluationProgramBaseInstV1::const_value(1, 2),
            MetalEvaluationProgramBaseInstV1::const_value(2, 3),
            MetalEvaluationProgramBaseInstV1::const_value(3, 4),
        ];

        let ext_insts = vec![
            MetalEvaluationProgramExtInstV1::secure_col(0, 0, 1, 2, 3),
            MetalEvaluationProgramExtInstV1 {
                op: MetalEvaluationProgramExtOpcodeV1::Param as u8,
                reserved0: 0,
                dst: 1,
                a: 0,
                b: 0,
                c: 0,
                d: 0,
            },
            MetalEvaluationProgramExtInstV1 {
                op: MetalEvaluationProgramExtOpcodeV1::Const as u8,
                reserved0: 0,
                dst: 2,
                a: 10,
                b: 20,
                c: 30,
                d: 40,
            },
            MetalEvaluationProgramExtInstV1 {
                op: MetalEvaluationProgramExtOpcodeV1::Add as u8,
                reserved0: 0,
                dst: 3,
                a: 0,
                b: 1,
                c: 0,
                d: 0,
            },
            MetalEvaluationProgramExtInstV1 {
                op: MetalEvaluationProgramExtOpcodeV1::Sub as u8,
                reserved0: 0,
                dst: 4,
                a: 2,
                b: 3,
                c: 0,
                d: 0,
            },
            MetalEvaluationProgramExtInstV1 {
                op: MetalEvaluationProgramExtOpcodeV1::Mul as u8,
                reserved0: 0,
                dst: 5,
                a: 3,
                b: 4,
                c: 0,
                d: 0,
            },
            MetalEvaluationProgramExtInstV1 {
                op: MetalEvaluationProgramExtOpcodeV1::Neg as u8,
                reserved0: 0,
                dst: 6,
                a: 5,
                b: 0,
                c: 0,
                d: 0,
            },
        ];

        let source = compile_v1_to_metal_source(&build_test_program(
            base_insts,
            ext_insts,
            vec![6],
            4,
            7,
        ));

        assert!(source.contains("StwoMetalQm31 e0 = StwoMetalQm31 { b0, b1, b2, b3 }"));
        assert!(source.contains("stwo_metal_load_qm31(ext_params, 0u)"));
        assert!(source.contains("StwoMetalQm31 { 10u, 20u, 30u, 40u }"));
        assert!(source.contains("stwo_metal_qm31_add(e0, e1)"));
        assert!(source.contains("stwo_metal_qm31_sub(e2, e3)"));
        assert!(source.contains("stwo_metal_qm31_mul(e3, e4)"));
        assert!(source.contains("stwo_metal_qm31_sub(StwoMetalQm31 { 0u, 0u, 0u, 0u }, e5)"));
    }

    #[test]
    fn test_compiled_kernel_name() {
        assert_eq!(
            compiled_kernel_name(0xdeadbeef_cafebabe),
            "eval_compiled_deadbeefcafebabe"
        );
    }

    #[test]
    fn test_register_reuse_no_redeclaration() {
        // A program that writes to register 0 twice should only declare it once.
        let base_insts = vec![
            MetalEvaluationProgramBaseInstV1::const_value(0, 1),
            MetalEvaluationProgramBaseInstV1::const_value(1, 2),
            // Overwrite register 0:
            MetalEvaluationProgramBaseInstV1::binary(
                MetalEvaluationProgramBaseOpcodeV1::Add,
                0,
                0,
                1,
            ),
        ];

        let source =
            compile_v1_to_metal_source(&build_test_program(base_insts, vec![], vec![], 2, 0));

        // First write declares: "uint b0 = 1u;"
        assert!(source.contains("uint b0 = 1u;"));
        // Second write to b0 should NOT have "uint " prefix.
        assert!(source.contains("    b0 = stwo_metal_m31_add(b0, b1);"));
    }

    #[test]
    fn test_multiple_constraint_roots() {
        let base_insts = vec![
            MetalEvaluationProgramBaseInstV1::const_value(0, 0),
        ];
        let ext_insts = vec![
            MetalEvaluationProgramExtInstV1 {
                op: MetalEvaluationProgramExtOpcodeV1::Const as u8,
                reserved0: 0,
                dst: 0,
                a: 1,
                b: 0,
                c: 0,
                d: 0,
            },
            MetalEvaluationProgramExtInstV1 {
                op: MetalEvaluationProgramExtOpcodeV1::Const as u8,
                reserved0: 0,
                dst: 1,
                a: 2,
                b: 0,
                c: 0,
                d: 0,
            },
            MetalEvaluationProgramExtInstV1 {
                op: MetalEvaluationProgramExtOpcodeV1::Const as u8,
                reserved0: 0,
                dst: 2,
                a: 3,
                b: 0,
                c: 0,
                d: 0,
            },
        ];

        let source = compile_v1_to_metal_source(&build_test_program(
            base_insts,
            ext_insts,
            vec![0, 1, 2],
            1,
            3,
        ));

        // All three constraints should be accumulated.
        assert!(source.contains("stwo_metal_qm31_mul(e0, stwo_metal_load_qm31(random_coeff_powers, 0u))"));
        assert!(source.contains("stwo_metal_qm31_mul(e1, stwo_metal_load_qm31(random_coeff_powers, 1u))"));
        assert!(source.contains("stwo_metal_qm31_mul(e2, stwo_metal_load_qm31(random_coeff_powers, 2u))"));
    }

    #[test]
    fn test_compile_fused_variant() {
        let base_insts = vec![
            MetalEvaluationProgramBaseInstV1::trace_col(0, 1, 0, 0),
            MetalEvaluationProgramBaseInstV1::trace_col(1, 1, 1, 0),
            MetalEvaluationProgramBaseInstV1::binary(
                MetalEvaluationProgramBaseOpcodeV1::Mul,
                2,
                0,
                1,
            ),
            MetalEvaluationProgramBaseInstV1::const_value(3, 42),
            MetalEvaluationProgramBaseInstV1::binary(
                MetalEvaluationProgramBaseOpcodeV1::Sub,
                4,
                2,
                3,
            ),
        ];
        let ext_insts = vec![MetalEvaluationProgramExtInstV1::secure_col(0, 4, 3, 3, 3)];

        let source = compile_v1_to_metal_source_with_fused(&build_test_program(
            base_insts,
            ext_insts,
            vec![0],
            5,
            1,
        ));

        // Should contain only the fused kernel (not the standard kernel).
        assert!(source.contains("eval_compiled_fused_123456789abcdef0"));
        // The standard (non-fused) kernel entry point should NOT be present.
        assert!(!source.contains("kernel void eval_compiled_123456789abcdef0("));

        // Fused kernel should have denom_inv buffer binding.
        assert!(source.contains("device const uint *denom_inv [[buffer(6)]]"));

        // Fused kernel should have coordinate output buffers.
        assert!(source.contains("device uint *coord_0 [[buffer(7)]]"));
        assert!(source.contains("device uint *coord_1 [[buffer(8)]]"));
        assert!(source.contains("device uint *coord_2 [[buffer(11)]]"));
        assert!(source.contains("device uint *coord_3 [[buffer(12)]]"));

        // Fused kernel should apply denom_inv and write coordinates.
        assert!(source.contains("denom_inv[denom_idx]"));
        assert!(source.contains("coord_0[row_index] = result.a;"));
        assert!(source.contains("coord_1[row_index] = result.b;"));
        assert!(source.contains("coord_2[row_index] = result.c;"));
        assert!(source.contains("coord_3[row_index] = result.d;"));

        // Should use reverse_bits instead of loop.
        assert!(source.contains("reverse_bits(index)"));
    }

    #[test]
    fn test_fused_kernel_name() {
        assert_eq!(
            compiled_fused_kernel_name(0xdeadbeef_cafebabe),
            "eval_compiled_fused_deadbeefcafebabe"
        );
    }
}
