//! Cairo component lowering and end-to-end prove pipeline.
//!
//! Loads a pre-compiled Cairo `prover_input.json`, generates witness traces
//! using stwo-cairo's claim generator, extracts FrameworkEval components from
//! CairoComponents, and attempts to lower each one through
//! `lower_framework_eval_to_v1`.
//!
//! With `--verify`, additionally runs the full stwo-cairo proof on SimdBackend
//! and verifies the proof using the standard verifier, printing timing
//! comparisons between Metal V1 lowering and the SIMD proof pipeline.
//!
//! When built with `metal-runtime`, the binary additionally executes each
//! successfully lowered V1 program on both the Metal GPU and the CPU reference
//! interpreter using synthetic trace data, comparing the results to validate
//! that the GPU kernel produces identical output to the CPU interpreter.
//!
//! Usage:
//!   cargo run --manifest-path fixtures/standalone-benchmarks/Cargo.toml \
//!     --features cairo-prove --bin cairo_prove -- <path-to-prover_input.json>
//!
//!   # With end-to-end verification:
//!   cargo run --manifest-path fixtures/standalone-benchmarks/Cargo.toml \
//!     --features cairo-prove --bin cairo_prove -- --verify <path-to-prover_input.json>
//!
//!   # With Metal GPU execution validation:
//!   cargo run --manifest-path fixtures/standalone-benchmarks/Cargo.toml \
//!     --features cairo-prove,metal-runtime --bin cairo_prove -- <path-to-prover_input.json>
//!
//! # Architecture: Metal prove pipeline for stwo-cairo
//!
//! ## Current state (2026-03-12)
//!
//! The `--metal` flag runs the full prove pipeline and produces a verified proof:
//!
//! 1. **Metal V1 lowering**: All 31 Cairo components lower successfully to
//!    `MetalEvaluationProgramV1` via `lower_framework_eval_to_v1`.
//!
//! 2. **Composition**: Currently uses **SimdBackend** (not Metal GPU) due to
//!    two blockers in the V1 programs:
//!    - Register budget: GPU limit is 256 ext regs, 9/31 components need more
//!    - Detached-register bug: `From<BaseField>` in RecordedBaseValue creates
//!      invalid instructions with register 65535
//!    - Row-offset gap: CPU interpreter and GPU kernel don't support offset≠0
//!    See DN-0013 for the fix plan.
//!
//! 3. **Post-composition**: SimdBackend handles commit, OODS, prove_values, FRI.
//!
//! 4. **Verification**: The standard stwo-cairo verifier validates the proof.
//!
//! With `--verify`, additionally runs SimdBackend proof for timing comparison.
//!
//! ## Future state (Option B -- full Metal pipeline)
//!
//! To achieve a full Metal proving pipeline, the following architecture is
//! needed. The key insight is that `prove_ex` in stwo is monolithic: it handles
//! composition, OODS, FRI, and decommit in one function call. We need to either
//! split it or replace it.
//!
//! ### Approach: Phase-split Metal proving
//!
//! The prove pipeline has five phases, each with a Metal acceleration path:
//!
//! **Phase 1 -- Trace commitment** (CPU or Metal)
//!   - Input: Raw trace columns from stwo-cairo witness generation
//!   - Operation: Interpolate + Merkle commit (preprocessed, base, interaction)
//!   - Metal path: `MetalBackend::interpolate_columns` + `CommitmentTreeProver`
//!     with `MetalBackend`. The existing `prove_runtime_v1.rs` already supports
//!     `CommitmentSchemeProver<MetalBackend, Blake2sMerkleChannel>`.
//!
//! **Phase 2 -- Composition polynomial** (Metal via V1 programs)
//!   - Input: Committed trace polynomials + random_coeff from channel
//!   - Operation: Evaluate constraint quotients on eval domain, accumulate
//!   - Metal path: `compute_composition_polynomial_multi_v1` already does this.
//!     Each component's `FrameworkEval` is lowered to `MetalEvaluationProgramV1`
//!     and executed on GPU. The result is a `SecureCirclePoly` composition
//!     polynomial that gets committed.
//!   - Integration point: Instead of calling
//!     `component_provers.compute_composition_polynomial()` (which uses
//!     `evaluate_constraint_quotients_on_domain` per component on SimdBackend),
//!     call `compute_composition_polynomial_multi_v1` with the lowered programs.
//!
//! **Phase 3 -- OODS sampling + prove-values** (CPU + Metal sampled-values)
//!   - Input: OODS point from channel, committed polynomials
//!   - Operation: Sample trace & composition at OODS point, FRI quotients
//!   - Metal path: `stage_prove_values_v1` + `execute_prove_values_v1` already
//!     own this phase. The sampled-values ABI
//!     (`execute_selected_metal_sampled_values_v1`) handles the OODS evaluation.
//!
//! **Phase 4 -- FRI commitment** (CPU, potentially Metal for Merkle)
//!   - Input: FRI quotient polynomials
//!   - Operation: FRI folding + Merkle commitment layers
//!   - Metal path: `FriProver` runs on CPU currently. Metal acceleration of
//!     Merkle hashing is possible but not the bottleneck.
//!
//! **Phase 5 -- Decommitment** (CPU)
//!   - Input: Query positions from channel grinding
//!   - Operation: Open Merkle paths at query positions
//!   - Metal path: Tree decommit is I/O-bound; CPU is adequate.
//!
//! ### Integration with stwo-cairo
//!
//! The recommended integration path:
//!
//! ```text
//! prove_cairo_metal<MC>(input, params) -> CairoProof<MC::H>
//!   |
//!   +-- witness gen: create_cairo_claim_generator (unchanged, CPU)
//!   +-- trace commit: CommitmentSchemeProver<MetalBackend, MC> (Phase 1)
//!   +-- lower components: lower_framework_eval_to_v1 per component
//!   +-- composition: compute_composition_polynomial_multi_v1 (Phase 2)
//!   +-- commit composition poly to commitment scheme
//!   +-- prove values: stage_prove_values_v1 + execute_prove_values_v1 (Phase 3-5)
//!   +-- assemble CairoProof
//! ```
//!
//! This mirrors `prove_cairo_with_precompute` but replaces the `prove_ex` call
//! with the phase-split Metal flow from `execute_prove_core_v1`.
//!
//! ### Key requirement for Option B
//!
//! The main prerequisite is that ALL Cairo components must lower successfully.
//! Currently tracked by the lowering report this binary produces. Once 100%
//! lowering coverage is achieved, the multi-component composition can be used
//! to replace the SimdBackend composition phase entirely.

#[cfg(not(feature = "cairo-prove"))]
fn main() {
    eprintln!("This binary requires the `cairo-prove` feature.");
    eprintln!("Run with: cargo run --features cairo-prove --bin cairo_prove -- <input.json>");
    std::process::exit(1);
}

#[cfg(feature = "cairo-prove")]
fn main() {
    cairo_prove_main::run();
}

#[cfg(feature = "cairo-prove")]
mod cairo_prove_main {
    use std::sync::Arc;
    use std::time::Instant;

    use cairo_air::cairo_components::CairoComponents;
    use cairo_air::relations::CommonLookupElements;
    use stwo::core::air::Component;
    use stwo::core::channel::{Blake2sChannel, Channel};
    use stwo::core::fields::qm31::SecureField;
    use stwo::core::fri::FriConfig;
    use stwo::core::pcs::PcsConfig;
    use stwo::core::poly::circle::CanonicCoset;
    use stwo::core::proof_of_work::GrindOps;
    use stwo::core::utils::MaybeOwned;
    use stwo::core::vcs_lifted::blake2_merkle::Blake2sMerkleChannel;
    use stwo::prover::backend::simd::SimdBackend;
    #[cfg(feature = "metal-runtime")]
    #[allow(unused_imports)]
    use stwo::prover::backend::{BackendForChannel, Backend, Column};
    use stwo::prover::mempool::BaseColumnPool;
    use stwo::prover::poly::circle::PolyOps;
    use stwo::prover::{CommitmentSchemeProver, CommitmentTreeProver};
    use stwo_cairo_adapter::ProverInput;
    use stwo_cairo_common::preprocessed_columns::preprocessed_trace::PreProcessedTrace;
    use stwo_cairo_prover::witness::cairo::create_cairo_claim_generator;
    use stwo_cairo_prover::witness::preprocessed_trace::gen_trace;
    use stwo_constraint_framework::FrameworkEval;
    use stwo_metal::program::{
        lower_framework_eval_to_v1_with_logup, MetalEvaluationProgramLoweringError,
        OwnedMetalEvaluationProgramV1,
    };

    /// CLI arguments parsed from argv.
    struct CliArgs {
        input_path: String,
        verify: bool,
        metal: bool,
    }

    fn parse_args() -> CliArgs {
        let args: Vec<String> = std::env::args().collect();
        let mut verify = false;
        let mut metal = false;
        let mut positional: Vec<String> = Vec::new();

        for arg in args.iter().skip(1) {
            match arg.as_str() {
                "--verify" => verify = true,
                "--metal" => metal = true,
                _ if arg.starts_with('-') => {
                    eprintln!("Unknown flag: {}", arg);
                    std::process::exit(1);
                }
                _ => positional.push(arg.clone()),
            }
        }

        let input_path = if let Some(path) = positional.first() {
            path.clone()
        } else if let Ok(path) = std::env::var("PROVER_INPUT_JSON") {
            path
        } else {
            eprintln!("Usage: cairo_prove [--verify] [--metal] <path-to-prover_input.json>");
            eprintln!("   or: PROVER_INPUT_JSON=<path> cairo_prove [--verify] [--metal]");
            eprintln!();
            eprintln!("Flags:");
            eprintln!("  --verify   Run full stwo-cairo proof on SimdBackend and verify");
            eprintln!("  --metal    Use Metal GPU for composition polynomial (requires metal-runtime)");
            std::process::exit(1);
        };

        CliArgs {
            input_path,
            verify,
            metal,
        }
    }

    /// Attempt to lower a single FrameworkEval component.
    /// Returns Ok with the program on success, Err with the error on failure.
    fn try_lower<E: FrameworkEval>(
        eval: &E,
        n_interactions: u32,
        claimed_sum: SecureField,
    ) -> Result<OwnedMetalEvaluationProgramV1, MetalEvaluationProgramLoweringError> {
        lower_framework_eval_to_v1_with_logup(eval, n_interactions, 0, 0, claimed_sum, eval.log_size())
    }

    /// A macro that handles the repetitive pattern of: if the component is Some,
    /// try to lower it and record the result.
    macro_rules! lower_component {
        ($components:expr, $field:ident, $name:expr, $results:expr) => {
            if let Some(ref component) = $components.$field {
                let eval: &_ = &**component; // Deref through FrameworkComponent to get Eval
                let n_interactions = component.trace_log_degree_bounds().len() as u32;
                let claimed_sum = component.claimed_sum();
                let lower_start = Instant::now();
                let result = try_lower(eval, n_interactions, claimed_sum);
                let lower_elapsed = lower_start.elapsed().as_secs_f64() * 1000.0;
                let success = result.is_ok();
                let (program, error) = match result {
                    Ok(prog) => (Some(prog), None),
                    Err(e) => (None, Some(format!("{:?}", e))),
                };
                $results.push(LoweringResult {
                    name: $name.to_string(),
                    success,
                    error,
                    program,
                    n_constraints: component.n_constraints(),
                    log_size: eval.log_size(),
                    max_constraint_log_degree_bound: eval.max_constraint_log_degree_bound(),
                    lowering_ms: lower_elapsed,
                    trace_locations: component.trace_locations().to_vec(),
                    preprocessed_column_indices: component.preprocessed_column_indices().to_vec(),
                    claimed_sum,
                });
            }
        };
    }

    /// Like lower_component but for Vec fields (e.g. memory_id_to_big).
    macro_rules! lower_component_vec {
        ($components:expr, $field:ident, $name:expr, $results:expr) => {
            for (i, component) in $components.$field.iter().enumerate() {
                let eval: &_ = &**component;
                let n_interactions = component.trace_log_degree_bounds().len() as u32;
                let claimed_sum = component.claimed_sum();
                let lower_start = Instant::now();
                let result = try_lower(eval, n_interactions, claimed_sum);
                let lower_elapsed = lower_start.elapsed().as_secs_f64() * 1000.0;
                let success = result.is_ok();
                let (program, error) = match result {
                    Ok(prog) => (Some(prog), None),
                    Err(e) => (None, Some(format!("{:?}", e))),
                };
                $results.push(LoweringResult {
                    name: format!("{}[{}]", $name, i),
                    success,
                    error,
                    program,
                    n_constraints: component.n_constraints(),
                    log_size: eval.log_size(),
                    max_constraint_log_degree_bound: eval.max_constraint_log_degree_bound(),
                    lowering_ms: lower_elapsed,
                    trace_locations: component.trace_locations().to_vec(),
                    preprocessed_column_indices: component.preprocessed_column_indices().to_vec(),
                    claimed_sum,
                });
            }
        };
    }

    /// Per-component lowering result. The `program` field is used for Metal
    /// GPU execution validation when `metal-runtime` is enabled.
    #[allow(dead_code)]
    struct LoweringResult {
        name: String,
        success: bool,
        error: Option<String>,
        program: Option<OwnedMetalEvaluationProgramV1>,
        n_constraints: usize,
        log_size: u32,
        max_constraint_log_degree_bound: u32,
        lowering_ms: f64,
        /// Trace column locations for Metal composition (tree_index, col_start, col_end).
        trace_locations: Vec<stwo::core::pcs::TreeSubspan>,
        /// Preprocessed column indices for Metal composition.
        preprocessed_column_indices: Vec<usize>,
        /// Claimed sum used for logup cumsum_shift computation.
        claimed_sum: SecureField,
    }

    /// Adapter that wraps a `stwo::prover::TreeBuilder<MetalBackend>` and
    /// implements the stwo-cairo `TreeBuilder<SimdBackend>` trait by converting
    /// SimdBackend evaluations to MetalBackend (CPU→GPU upload) on the fly.
    #[cfg(feature = "metal-runtime")]
    struct ConvertingTreeBuilder<'a, 'b> {
        inner: stwo::prover::TreeBuilder<'a, 'b, stwo_metal::MetalBackend, Blake2sMerkleChannel>,
    }

    #[cfg(feature = "metal-runtime")]
    impl stwo_cairo_prover::witness::utils::TreeBuilder<SimdBackend>
        for ConvertingTreeBuilder<'_, '_>
    {
        fn extend_evals(
            &mut self,
            columns: Vec<stwo::prover::poly::circle::CircleEvaluation<
                SimdBackend,
                stwo::core::fields::m31::BaseField,
                stwo::prover::poly::BitReversedOrder,
            >>,
        ) -> stwo::core::pcs::TreeSubspan {
            use stwo::prover::backend::Column;
            let metal_columns: Vec<stwo::prover::poly::circle::CircleEvaluation<
                stwo_metal::MetalBackend,
                stwo::core::fields::m31::BaseField,
                stwo::prover::poly::BitReversedOrder,
            >> = columns
                .into_iter()
                .map(|eval| {
                    let domain = eval.domain;
                    let metal_values = eval.values.to_cpu().into_iter().collect();
                    stwo::prover::poly::circle::CircleEvaluation::new(domain, metal_values)
                })
                .collect();
            self.inner.extend_evals(metal_columns)
        }
    }

    /// Prover parameters used for both lowering setup and full proof.
    fn default_pcs_config() -> PcsConfig {
        PcsConfig {
            pow_bits: 20,
            fri_config: FriConfig {
                log_last_layer_degree_bound: 0,
                log_blowup_factor: 1,
                n_queries: 15,
                line_fold_step: 1,
            },
            lifting_log_size: None,
        }
    }

    /// Lower all Cairo components and return lowering results + timing.
    fn lower_all_components(components: &CairoComponents) -> (Vec<LoweringResult>, f64) {
        let lowering_start = Instant::now();
        let mut results: Vec<LoweringResult> = Vec::new();

        // Opcodes
        lower_component!(components, add_opcode, "add_opcode", results);
        lower_component!(components, add_opcode_small, "add_opcode_small", results);
        lower_component!(components, add_ap_opcode, "add_ap_opcode", results);
        lower_component!(components, assert_eq_opcode, "assert_eq_opcode", results);
        lower_component!(components, assert_eq_opcode_imm, "assert_eq_opcode_imm", results);
        lower_component!(components, assert_eq_opcode_double_deref, "assert_eq_opcode_double_deref", results);
        lower_component!(components, blake_compress_opcode, "blake_compress_opcode", results);
        lower_component!(components, call_opcode_abs, "call_opcode_abs", results);
        lower_component!(components, call_opcode_rel_imm, "call_opcode_rel_imm", results);
        lower_component!(components, generic_opcode, "generic_opcode", results);
        lower_component!(components, jnz_opcode_non_taken, "jnz_opcode_non_taken", results);
        lower_component!(components, jnz_opcode_taken, "jnz_opcode_taken", results);
        lower_component!(components, jump_opcode_abs, "jump_opcode_abs", results);
        lower_component!(components, jump_opcode_double_deref, "jump_opcode_double_deref", results);
        lower_component!(components, jump_opcode_rel, "jump_opcode_rel", results);
        lower_component!(components, jump_opcode_rel_imm, "jump_opcode_rel_imm", results);
        lower_component!(components, mul_opcode, "mul_opcode", results);
        lower_component!(components, mul_opcode_small, "mul_opcode_small", results);
        lower_component!(components, qm_31_add_mul_opcode, "qm_31_add_mul_opcode", results);
        lower_component!(components, ret_opcode, "ret_opcode", results);
        lower_component!(components, verify_instruction, "verify_instruction", results);

        // Blake
        lower_component!(components, blake_round, "blake_round", results);
        lower_component!(components, blake_g, "blake_g", results);
        lower_component!(components, blake_round_sigma, "blake_round_sigma", results);
        lower_component!(components, triple_xor_32, "triple_xor_32", results);
        lower_component!(components, verify_bitwise_xor_12, "verify_bitwise_xor_12", results);

        // Builtins
        lower_component!(components, add_mod_builtin, "add_mod_builtin", results);
        lower_component!(components, bitwise_builtin, "bitwise_builtin", results);
        lower_component!(components, mul_mod_builtin, "mul_mod_builtin", results);
        lower_component!(components, pedersen_builtin, "pedersen_builtin", results);
        lower_component!(components, pedersen_builtin_narrow_windows, "pedersen_builtin_narrow_windows", results);
        lower_component!(components, poseidon_builtin, "poseidon_builtin", results);
        lower_component!(components, range_check96_builtin, "range_check96_builtin", results);
        lower_component!(components, range_check_builtin, "range_check_builtin", results);

        // Pedersen aggregator
        lower_component!(components, pedersen_aggregator_window_bits_18, "pedersen_aggregator_window_bits_18", results);
        lower_component!(components, partial_ec_mul_window_bits_18, "partial_ec_mul_window_bits_18", results);
        lower_component!(components, pedersen_points_table_window_bits_18, "pedersen_points_table_window_bits_18", results);
        lower_component!(components, pedersen_aggregator_window_bits_9, "pedersen_aggregator_window_bits_9", results);
        lower_component!(components, partial_ec_mul_window_bits_9, "partial_ec_mul_window_bits_9", results);
        lower_component!(components, pedersen_points_table_window_bits_9, "pedersen_points_table_window_bits_9", results);

        // Poseidon
        lower_component!(components, poseidon_aggregator, "poseidon_aggregator", results);
        lower_component!(components, poseidon_3_partial_rounds_chain, "poseidon_3_partial_rounds_chain", results);
        lower_component!(components, poseidon_full_round_chain, "poseidon_full_round_chain", results);
        lower_component!(components, cube_252, "cube_252", results);
        lower_component!(components, poseidon_round_keys, "poseidon_round_keys", results);

        // Range checks and memory
        lower_component!(components, range_check_252_width_27, "range_check_252_width_27", results);
        lower_component!(components, memory_address_to_id, "memory_address_to_id", results);
        lower_component_vec!(components, memory_id_to_big, "memory_id_to_big", results);
        lower_component!(components, memory_id_to_small, "memory_id_to_small", results);

        // Range check tables
        lower_component!(components, range_check_6, "range_check_6", results);
        lower_component!(components, range_check_8, "range_check_8", results);
        lower_component!(components, range_check_11, "range_check_11", results);
        lower_component!(components, range_check_12, "range_check_12", results);
        lower_component!(components, range_check_18, "range_check_18", results);
        lower_component!(components, range_check_20, "range_check_20", results);
        lower_component!(components, range_check_4_3, "range_check_4_3", results);
        lower_component!(components, range_check_4_4, "range_check_4_4", results);
        lower_component!(components, range_check_9_9, "range_check_9_9", results);
        lower_component!(components, range_check_7_2_5, "range_check_7_2_5", results);
        lower_component!(components, range_check_3_6_6_3, "range_check_3_6_6_3", results);
        lower_component!(components, range_check_4_4_4_4, "range_check_4_4_4_4", results);
        lower_component!(components, range_check_3_3_3_3_3, "range_check_3_3_3_3_3", results);

        // Bitwise XOR verifiers
        lower_component!(components, verify_bitwise_xor_4, "verify_bitwise_xor_4", results);
        lower_component!(components, verify_bitwise_xor_7, "verify_bitwise_xor_7", results);
        lower_component!(components, verify_bitwise_xor_8, "verify_bitwise_xor_8", results);
        lower_component!(components, verify_bitwise_xor_9, "verify_bitwise_xor_9", results);

        let lowering_only_ms = lowering_start.elapsed().as_secs_f64() * 1000.0;
        (results, lowering_only_ms)
    }

    /// Print lowering results table.
    fn print_lowering_results(results: &[LoweringResult]) {
        let total = results.len();
        let succeeded = results.iter().filter(|r| r.success).count();
        let failed = total - succeeded;

        println!(
            "{:<50} {:>6} {:>10} {:>8} {:>10} {:>8} {:>8}",
            "COMPONENT", "LOG_SZ", "CONSTRNTS", "STATUS", "LOWER_MS", "B_REGS", "E_REGS"
        );
        println!("{}", "-".repeat(112));
        for r in results {
            let status = if r.success { "OK" } else { "FAIL" };
            let (base_regs, ext_regs) = r
                .program
                .as_ref()
                .map(|p| (p.header().max_base_regs, p.header().max_ext_regs))
                .unwrap_or((0, 0));
            println!(
                "{:<50} {:>6} {:>10} {:>8} {:>10.2} {:>8} {:>8}",
                r.name, r.log_size, r.n_constraints, status, r.lowering_ms, base_regs, ext_regs
            );
            if let Some(ref err) = r.error {
                println!("    Error: {}", err);
            }
        }

        println!("\n{}", "=".repeat(90));
        println!(
            "Lowering: {}/{} components lowered successfully, {} failed",
            succeeded, total, failed
        );
    }

    /// Run the full stwo-cairo proof on SimdBackend and verify it.
    ///
    /// Returns (prove_time_ms, verify_time_ms).
    fn run_full_proof_and_verify(input: ProverInput) -> (f64, f64) {
        use cairo_air::verifier::verify_cairo;
        use cairo_air::PreProcessedTraceVariant;
        use stwo_cairo_prover::prover::{prove_cairo, ChannelHash, ProverParameters};

        let prover_params = ProverParameters {
            channel_hash: ChannelHash::Blake2s,
            channel_salt: 0,
            pcs_config: default_pcs_config(),
            preprocessed_trace: PreProcessedTraceVariant::Canonical,
            store_polynomials_coefficients: false,
            include_all_preprocessed_columns: false,
        };

        println!("\n=== Running full stwo-cairo proof on SimdBackend ===\n");

        let prove_start = Instant::now();
        let cairo_proof = prove_cairo::<Blake2sMerkleChannel>(input, prover_params)
            .expect("stwo-cairo prove_cairo failed");
        let prove_ms = prove_start.elapsed().as_secs_f64() * 1000.0;
        println!("  Prove completed in {:.1} ms", prove_ms);

        println!("  Verifying proof...");
        let verify_start = Instant::now();
        verify_cairo::<Blake2sMerkleChannel>(cairo_proof.into())
            .expect("stwo-cairo verify_cairo failed");
        let verify_ms = verify_start.elapsed().as_secs_f64() * 1000.0;
        println!("  Verification passed in {:.1} ms", verify_ms);

        (prove_ms, verify_ms)
    }

    pub fn run() {
        let cli = parse_args();

        println!("Loading prover input from: {}", cli.input_path);
        let input_json = std::fs::read_to_string(&cli.input_path)
            .unwrap_or_else(|e| panic!("Failed to read {}: {}", cli.input_path, e));

        println!("Deserializing ProverInput...");
        let input: ProverInput = serde_json::from_str(&input_json)
            .unwrap_or_else(|e| panic!("Failed to parse ProverInput: {}", e));

        // Clone input before consuming it for the lowering phase.
        // The full Metal pipeline needs a fresh claim generator.
        let input_for_proof = if cli.verify {
            Some(input.clone())
        } else {
            None
        };
        // Use the canonical preprocessed trace (includes pedersen tables)
        // so that builtins inputs work correctly.
        println!("Creating preprocessed trace (canonical, includes pedersen)...");

        // Warm up the GPU grind pipeline before the timed section.
        // The Metal shader pipeline compilation takes ~30-40ms on first call;
        // this ensures it doesn't inflate the timed interaction PoW grind measurement.
        #[cfg(feature = "metal-runtime")]
        if cli.metal {
            let _ = stwo_metal_sys::metal::U32Buffer::blake2s_grind_batch(
                &[0u32; 8], 32, 0, 1,
            );
        }

        let preprocessed_trace = Arc::new(PreProcessedTrace::canonical());

        // Full Metal pipeline: run everything on MetalBackend from scratch.
        // This avoids any SimdBackend pre-lowering overhead.
        #[cfg(feature = "metal-runtime")]
        if cli.metal {
            let input_for_verify = if cli.verify {
                Some(input.clone())
            } else {
                None
            };
            run_metal_full_pipeline(input, preprocessed_trace, cli.verify, input_for_verify);
            return;
        }

        let lowering_total_start = Instant::now();

        // Step 1: Create claim generator and write base trace.
        let t0 = Instant::now();
        println!("Creating claim generator...");
        let cairo_claim_generator =
            create_cairo_claim_generator(input, preprocessed_trace.clone());
        let claim_gen_ms = t0.elapsed().as_secs_f64() * 1000.0;

        let pcs_config = default_pcs_config();
        let log_max_rows: u32 = 27;

        let max_domain_size = {
            let cairo_air_log_degree_bound = 1;
            log_max_rows
                + std::cmp::max(
                    cairo_air_log_degree_bound,
                    pcs_config.fri_config.log_blowup_factor,
                )
        };

        let t1 = Instant::now();
        println!(
            "Precomputing twiddles (log_domain_size={})...",
            max_domain_size
        );
        let twiddles = SimdBackend::precompute_twiddles(
            CanonicCoset::new(max_domain_size)
                .circle_domain()
                .half_coset,
        );
        let twiddles_ms = t1.elapsed().as_secs_f64() * 1000.0;

        let t2 = Instant::now();
        let base_column_pool = BaseColumnPool::new();
        let ifft_start = Instant::now();
        let preprocessed_trace_polys =
            SimdBackend::interpolate_columns(gen_trace(preprocessed_trace.clone()), &twiddles);
        let ifft_ms = ifft_start.elapsed().as_secs_f64() * 1000.0;
        let tree_start = Instant::now();
        let preprocessed_tree = CommitmentTreeProver::<SimdBackend, Blake2sMerkleChannel>::new(
            preprocessed_trace_polys,
            pcs_config.fri_config.log_blowup_factor,
            &twiddles,
            true,
            pcs_config.lifting_log_size,
            &base_column_pool,
        );
        let tree_ms = tree_start.elapsed().as_secs_f64() * 1000.0;
        println!(
            "  Preprocessed tree breakdown: IFFT={:.1}ms, RFFT+Merkle={:.1}ms",
            ifft_ms, tree_ms,
        );
        let preproc_commit_ms = t2.elapsed().as_secs_f64() * 1000.0;

        // Setup protocol channel.
        let channel = &mut Blake2sChannel::default();
        let channel_salt: u32 = 0;
        channel.mix_felts(&[SecureField::from(channel_salt)]);
        pcs_config.mix_into(channel);

        let mut commitment_scheme =
            CommitmentSchemeProver::<SimdBackend, Blake2sMerkleChannel>::with_memory_pool(
                pcs_config,
                &twiddles,
                &base_column_pool,
            );
        // Store polynomial coefficients to enable batch_eval_at_point in prove_values,
        // avoiding the expensive barycentric weight precomputation (~250ms).
        commitment_scheme.store_polynomials_coefficients = true;

        // Commit preprocessed tree.
        commitment_scheme.commit_tree(MaybeOwned::Owned(preprocessed_tree), channel);

        // Write base trace (witness generation).
        let t3 = Instant::now();
        println!("Generating base trace...");
        let mut tree_builder = commitment_scheme.tree_builder();
        let (claim, interaction_generator) =
            cairo_claim_generator.write_trace(&mut tree_builder);
        let base_trace_gen_ms = t3.elapsed().as_secs_f64() * 1000.0;

        let t4 = Instant::now();
        claim.mix_into::<Blake2sMerkleChannel>(channel);
        tree_builder.commit(channel);
        let base_trace_commit_ms = t4.elapsed().as_secs_f64() * 1000.0;

        // Draw interaction elements.
        let t5 = Instant::now();
        let interaction_pow_bits = cairo_air::verifier::INTERACTION_POW_BITS;
        #[cfg(feature = "metal-runtime")]
        let interaction_pow = if cli.metal {
            use stwo_metal::MetalBackend;
            MetalBackend::grind(channel, interaction_pow_bits)
        } else {
            SimdBackend::grind(channel, interaction_pow_bits)
        };
        #[cfg(not(feature = "metal-runtime"))]
        let interaction_pow = SimdBackend::grind(channel, interaction_pow_bits);
        channel.mix_u64(interaction_pow);
        let interaction_elements = CommonLookupElements::draw(channel);
        let interaction_pow_ms = t5.elapsed().as_secs_f64() * 1000.0;

        // Write interaction trace.
        let t6 = Instant::now();
        println!("Generating interaction trace...");
        let mut tree_builder = commitment_scheme.tree_builder();
        let interaction_claim = interaction_generator
            .write_interaction_trace(&mut tree_builder, &interaction_elements);
        let interaction_trace_gen_ms = t6.elapsed().as_secs_f64() * 1000.0;

        let t7 = Instant::now();
        interaction_claim.mix_into(channel);
        tree_builder.commit(channel);
        let interaction_trace_commit_ms = t7.elapsed().as_secs_f64() * 1000.0;

        // Print witness gen + trace commit breakdown.
        println!("\n=== Pre-lowering phase breakdown ===");
        println!("  create_cairo_claim_generator:  {:>8.1} ms", claim_gen_ms);
        println!("  precompute_twiddles:           {:>8.1} ms", twiddles_ms);
        println!("  preprocessed tree commit:      {:>8.1} ms", preproc_commit_ms);
        println!("  base trace generation:         {:>8.1} ms", base_trace_gen_ms);
        println!("  base trace commit (Merkle):    {:>8.1} ms", base_trace_commit_ms);
        println!("  interaction PoW grind:         {:>8.1} ms", interaction_pow_ms);
        println!("  interaction trace generation:  {:>8.1} ms", interaction_trace_gen_ms);
        println!("  interaction trace commit:      {:>8.1} ms", interaction_trace_commit_ms);
        let pre_lowering_accounted = claim_gen_ms + twiddles_ms + preproc_commit_ms
            + base_trace_gen_ms + base_trace_commit_ms + interaction_pow_ms
            + interaction_trace_gen_ms + interaction_trace_commit_ms;
        println!("  ─────────────────────────────────────────");
        println!("  accounted total:               {:>8.1} ms", pre_lowering_accounted);
        println!();

        // Build CairoComponents.
        println!("Building CairoComponents...");
        let components = CairoComponents::new(
            &claim,
            &interaction_elements,
            &interaction_claim,
            &preprocessed_trace.ids(),
        );

        // Now lower each component.
        println!("\n=== Lowering Cairo components to Metal V1 programs ===\n");
        let (results, lowering_only_ms) = lower_all_components(&components);
        let lowering_total_ms = lowering_total_start.elapsed().as_secs_f64() * 1000.0;

        let total = results.len();
        let succeeded = results.iter().filter(|r| r.success).count();
        let failed = total - succeeded;

        print_lowering_results(&results);
        println!(
            "Lowering time: {:.1} ms (pure lowering), {:.1} ms (total incl. witness gen)",
            lowering_only_ms, lowering_total_ms
        );

        // Metal prove pipeline: reuse the pre-lowering commitment scheme
        // (which already has preprocessed, base, and interaction trees committed)
        // and only replace the composition step with Metal V1 GPU composition.
        #[cfg(feature = "metal-runtime")]
        if cli.metal {
            if failed > 0 {
                println!("\nCannot run Metal prove: {}/{} components failed to lower.", failed, total);
                std::process::exit(1);
            }

            println!("\n=== Running Metal-accelerated prove ===\n");

            let metal_pipeline_start = Instant::now();

            // Get SimdBackend component provers for mask_points computation.
            let component_provers = stwo_cairo_prover::utils::cairo_provers(&components);

            let (proof, prove_values_ms, composition_ms) =
                run_metal_composition_and_prove(
                    commitment_scheme,
                    channel,
                    &results,
                    &component_provers,
                    false, // include_all_preprocessed_columns
                );

            let metal_prove_ms = metal_pipeline_start.elapsed().as_secs_f64() * 1000.0;

            println!("\n{}", "=".repeat(90));
            println!("=== Metal-accelerated prove results ===\n");
            println!(
                "  Pre-lowering (witness gen + trace commit): {:>8.1} ms",
                pre_lowering_accounted
            );
            println!(
                "  Composition (Metal V1 GPU):                {:>8.1} ms",
                composition_ms
            );
            println!(
                "  prove_values (SimdBackend):                {:>8.1} ms",
                prove_values_ms
            );
            println!(
                "  Metal prove total (post-lowering):         {:>8.1} ms",
                metal_prove_ms
            );

            // Verify if --verify is set.
            if cli.verify {
                use cairo_air::verifier::verify_cairo_ex;
                use cairo_air::CairoProof;
                use cairo_air::PreProcessedTraceVariant;

                let cairo_proof = CairoProof {
                    claim: claim.clone(),
                    interaction_pow,
                    interaction_claim: interaction_claim.clone(),
                    extended_stark_proof: proof,
                    channel_salt: 0,
                    preprocessed_trace_variant: PreProcessedTraceVariant::Canonical,
                };

                println!("  Verifying Metal proof...");
                let verify_start = Instant::now();
                verify_cairo_ex::<Blake2sMerkleChannel>(
                    cairo_proof.into(),
                    false,
                )
                .expect("Metal proof verification failed");
                let verify_ms = verify_start.elapsed().as_secs_f64() * 1000.0;
                println!("  Verification passed in {:.1} ms", verify_ms);
            }

            // Also run SimdBackend proof for timing comparison if --verify.
            if let Some(input_for_proof) = input_for_proof {
                let (prove_ms, verify_ms) = run_full_proof_and_verify(input_for_proof);
                let metal_total_ms = pre_lowering_accounted + metal_prove_ms;
                println!("\n  Comparison:");
                println!(
                    "    Metal total (pre-lowering + prove): {:>8.1} ms",
                    metal_total_ms
                );
                println!(
                    "    SimdBackend:                        {:>8.1} ms (prove {:.1} + verify {:.1})",
                    prove_ms + verify_ms, prove_ms, verify_ms
                );
            }

            return;
        }

        #[cfg(not(feature = "metal-runtime"))]
        if cli.metal {
            println!("--metal requires the metal-runtime feature. Build with: --features cairo-prove,metal-runtime");
            std::process::exit(1);
        }

        // Metal GPU vs CPU execution validation.
        #[cfg(feature = "metal-runtime")]
        {
            let exec_failed = run_metal_execution(&results);
            if exec_failed > 0 {
                std::process::exit(2);
            }
        }
        #[cfg(not(feature = "metal-runtime"))]
        {
            println!(
                "\nNote: Metal GPU execution skipped (build with --features metal-runtime to enable)."
            );
        }

        // Run full proof + verification if --verify is set.
        if let Some(input_for_proof) = input_for_proof {
            let (prove_ms, verify_ms) = run_full_proof_and_verify(input_for_proof);

            println!("\n{}", "=".repeat(90));
            println!("=== End-to-end timing comparison ===\n");
            println!(
                "  Metal V1 lowering (IR compile):       {:>10.1} ms",
                lowering_only_ms
            );
            println!(
                "  Metal V1 lowering (total w/ witness):  {:>10.1} ms",
                lowering_total_ms
            );
            println!(
                "  stwo-cairo SimdBackend prove:          {:>10.1} ms",
                prove_ms
            );
            println!(
                "  stwo-cairo verification:               {:>10.1} ms",
                verify_ms
            );
            println!(
                "  stwo-cairo prove + verify:             {:>10.1} ms",
                prove_ms + verify_ms
            );
            println!();
            println!("  Proof verified successfully.");
            println!(
                "  Metal V1 programs produce correct lowerings for {}/{} components.",
                succeeded, total
            );
            if failed > 0 {
                println!("  {} components failed to lower (see errors above).", failed);
            }
        }

        if failed > 0 {
            std::process::exit(1);
        }
    }

    // -----------------------------------------------------------------------
    // Metal-accelerated composition + prove_values
    // -----------------------------------------------------------------------

    /// Run Metal V1 GPU composition and SimdBackend prove_values.
    ///
    /// Reuses the existing commitment scheme (which already has preprocessed,
    /// base, and interaction trees committed on SimdBackend). Only the
    /// composition step uses GPU acceleration via Metal V1 evaluation programs.
    #[cfg(feature = "metal-runtime")]
    fn run_metal_composition_and_prove(
        mut commitment_scheme: CommitmentSchemeProver<'_, SimdBackend, Blake2sMerkleChannel>,
        channel: &mut Blake2sChannel,
        results: &[LoweringResult],
        components_for_prove: &[&dyn stwo::prover::ComponentProver<SimdBackend>],
        include_all_preprocessed_columns: bool,
    ) -> (
        stwo::core::proof::ExtendedStarkProof<
            stwo::core::vcs_lifted::blake2_merkle::Blake2sMerkleHasher,
        >,
        f64, // prove_values_ms
        f64, // composition_ms
    ) {
        use stwo::core::circle::CirclePoint;
        use stwo::core::fields::qm31::SECURE_EXTENSION_DEGREE;
        use stwo::core::pcs::utils::get_lifting_log_size;
        use stwo::core::proof::{ExtendedStarkProof, StarkProof};
        use stwo::core::verifier::PREPROCESSED_TRACE_IDX;
        use stwo::prover::ComponentProvers;

        // Composition (Metal V1 GPU)
        println!("  Composition (Metal V1 GPU)...");
        let n_preprocessed_columns =
            commitment_scheme.trees[PREPROCESSED_TRACE_IDX].polynomials.len();
        let component_provers_struct = ComponentProvers {
            components: components_for_prove.to_vec(),
            n_preprocessed_columns,
        };

        let random_coeff = channel.draw_secure_felt();
        let log_blowup_factor = commitment_scheme.config.fri_config.log_blowup_factor;

        let composition_start = Instant::now();
        let composition_poly = compute_metal_composition_poly(
            results,
            &commitment_scheme,
            random_coeff,
            log_blowup_factor,
        );
        let composition_ms = composition_start.elapsed().as_secs_f64() * 1000.0;

        // Commit composition polynomial on SimdBackend
        let mut tree_builder = commitment_scheme.tree_builder();
        let (left_half, right_half) = composition_poly.split_at_mid();
        let left_coord_polys = left_half.into_coordinate_polys();
        let right_coord_polys = right_half.into_coordinate_polys();
        tree_builder.extend_polys(left_coord_polys);
        tree_builder.extend_polys(right_coord_polys);
        tree_builder.commit(channel);

        // Draw OODS point and compute sample points
        let oods_point = CirclePoint::<SecureField>::get_random_point(channel);
        let split_composition_log_size = commitment_scheme
            .trees
            .last()
            .unwrap()
            .commitment
            .layers
            .len() as u32
            - 1;
        let lifting_log_size =
            get_lifting_log_size(&commitment_scheme.config, split_composition_log_size);
        if include_all_preprocessed_columns {
            let preprocessed_log_size = commitment_scheme.trees[PREPROCESSED_TRACE_IDX]
                .commitment
                .layers
                .len() as u32
                - 1;
            assert!(lifting_log_size >= preprocessed_log_size);
        }
        let max_log_degree_bound =
            lifting_log_size - commitment_scheme.config.fri_config.log_blowup_factor;

        let mut sample_points = component_provers_struct.components().mask_points(
            oods_point,
            max_log_degree_bound,
            include_all_preprocessed_columns,
        );
        sample_points.push(vec![vec![oods_point]; 2 * SECURE_EXTENSION_DEGREE]);

        // prove_values on SimdBackend
        let prove_values_start = Instant::now();
        let commitment_scheme_proof = commitment_scheme.prove_values(sample_points, channel);
        let prove_values_ms = prove_values_start.elapsed().as_secs_f64() * 1000.0;

        let proof = StarkProof(commitment_scheme_proof.proof);
        let extended_proof = ExtendedStarkProof {
            proof,
            aux: commitment_scheme_proof.aux,
        };

        (extended_proof, prove_values_ms, composition_ms)
    }

    /// Eval domain row count below which SIMD is faster than GPU.
    #[cfg(feature = "metal-runtime")]
    const HYBRID_GPU_MIN_EVAL_ROWS: u64 = 4096;

    /// For Variant B/C components (ext_regs > 256), register spilling makes GPU
    /// slow at moderate sizes. Use CPU interpreter below this threshold.
    #[cfg(feature = "metal-runtime")]
    const HYBRID_GPU_MIN_EVAL_ROWS_HIGH_EXT: u64 = 32768;

    /// ext_regs count above which a component is considered "high ext reg"
    /// (Variant B) and subject to the stricter GPU dispatch threshold.
    #[cfg(feature = "metal-runtime")]
    const HYBRID_HIGH_EXT_REG_THRESHOLD: u32 = 256;

    /// Compute the Metal V1 composition polynomial using an extractor closure
    /// to obtain column evaluations on each component's eval domain.
    ///
    /// `extract_column(tree_idx, col_idx, eval_domain)` returns the column
    /// evaluation as a `Vec<BaseField>` on the given domain.
    #[cfg(feature = "metal-runtime")]
    fn compute_metal_composition_poly_impl(
        results: &[LoweringResult],
        extract_column: impl Fn(usize, usize, stwo::core::poly::circle::CircleDomain) -> Vec<stwo::core::fields::m31::BaseField>,
        random_coeff: SecureField,
        log_blowup_factor: u32,
    ) -> stwo::prover::poly::circle::SecureCirclePoly<SimdBackend> {
        use stwo::core::fields::m31::BaseField;
        use stwo::core::poly::circle::CanonicCoset;
        use stwo::prover::AccumulationOps;
        use stwo_metal::program::{
            MetalEvaluationProgramCapabilityProfileV1,
            MetalEvaluationProgramDispatchKindV1,
            MetalEvaluationProgramRuntimeInputsV1,
            MetalEvaluationProgramTraceViewV1,
            compile_v1_to_metal_source, compiled_kernel_name,
            execute_compiled_metal_evaluation_program_v1,
            execute_selected_metal_evaluation_program_v1_on_metal,
            interpret_metal_evaluation_program_v1,
        };
        use stwo_metal::MetalBackend;

        let successful_results: Vec<&LoweringResult> = results
            .iter()
            .filter(|r| r.success && r.program.is_some())
            .collect();

        println!("  Building Metal composition for {} components...", successful_results.len());

        // Pre-compute total constraint count for random_coeff_powers.
        let total_constraints: usize = successful_results
            .iter()
            .map(|r| r.program.as_ref().unwrap().constraint_roots().len())
            .sum();
        let mut all_random_coeff_powers =
            <MetalBackend as AccumulationOps>::generate_secure_powers(random_coeff, total_constraints);
        all_random_coeff_powers.reverse();

        let profile = MetalEvaluationProgramCapabilityProfileV1::current();
        let mut quotient_results: Vec<(u32, Vec<SecureField>)> = Vec::new();
        let mut coeff_offset: usize = 0;
        let mut gpu_count = 0usize;
        let mut compiled_count = 0usize;
        let mut cpu_count = 0usize;
        let mut simd_count = 0usize;
        let mut total_extract_ms = 0.0f64;
        let mut total_kernel_ms = 0.0f64;
        let mut total_denom_ms = 0.0f64;
        let mut total_jit_ms = 0.0f64;

        // JIT-compiled native Metal shaders: opt-in via USE_JIT=1.
        let use_compiled = std::env::var("USE_JIT").is_ok();
        let mut shader_cache: std::collections::HashMap<u64, (String, String)> =
            std::collections::HashMap::new();
        if use_compiled {
            let jit_start = Instant::now();
            for r in &successful_results {
                let program = r.program.as_ref().unwrap();
                let hash = program.header().semantic_hash;
                shader_cache.entry(hash).or_insert_with(|| {
                    let source = compile_v1_to_metal_source(program);
                    let name = compiled_kernel_name(hash);
                    (source, name)
                });
            }
            total_jit_ms = jit_start.elapsed().as_secs_f64() * 1000.0;
            println!("  JIT-compiled {} unique shaders in {:.1} ms",
                shader_cache.len(), total_jit_ms);
        }

        let v1_start = Instant::now();

        for r in &successful_results {
            let program = r.program.as_ref().unwrap();
            let n_interactions = program.header().n_interactions as usize;
            let n_constraints = program.constraint_roots().len();
            // CRITICAL: use max_constraint_log_degree_bound, not log_size + 1.
            let eval_domain = CanonicCoset::new(r.max_constraint_log_degree_bound).circle_domain();
            let extract_start = Instant::now();
            // Build per-interaction column data via the extractor closure.
            let mut interaction_cols: Vec<Vec<Vec<BaseField>>> = Vec::new();

            for interaction_idx in 0..n_interactions {
                let tree_idx = interaction_idx;

                if interaction_idx == 0 && !r.preprocessed_column_indices.is_empty() {
                    let cols: Vec<Vec<BaseField>> = r
                        .preprocessed_column_indices
                        .iter()
                        .map(|&idx| extract_column(0, idx, eval_domain))
                        .collect();
                    interaction_cols.push(cols);
                } else if interaction_idx == 0 && r.preprocessed_column_indices.is_empty() {
                    interaction_cols.push(vec![]);
                } else {
                    let location = r
                        .trace_locations
                        .iter()
                        .find(|loc| loc.tree_index == tree_idx);
                    if let Some(loc) = location {
                        let cols: Vec<Vec<BaseField>> = (loc.col_start..loc.col_end)
                            .map(|col_idx| extract_column(tree_idx, col_idx, eval_domain))
                            .collect();
                        interaction_cols.push(cols);
                    } else {
                        interaction_cols.push(vec![]);
                    }
                }
            }

            // Build reference slices.
            let interaction_refs: Vec<Vec<&[BaseField]>> = interaction_cols
                .iter()
                .map(|cols| cols.iter().map(|c| c.as_slice()).collect())
                .collect();
            let interaction_slice_refs: Vec<&[&[BaseField]]> =
                interaction_refs.iter().map(|cols| cols.as_slice()).collect();

            let random_coeff_powers =
                &all_random_coeff_powers[coeff_offset..coeff_offset + n_constraints];
            coeff_offset += n_constraints;

            // Determine eval domain log size from columns.
            let eval_domain_log_size = interaction_refs
                .iter()
                .flatten()
                .map(|col| col.len().trailing_zeros())
                .next()
                .unwrap_or(r.log_size + 1 + log_blowup_factor);

            let runtime = MetalEvaluationProgramRuntimeInputsV1 {
                trace: MetalEvaluationProgramTraceViewV1 {
                    trace_interactions: &interaction_slice_refs,
                    preprocessed_columns: &[],
                },
                base_params: &[],
                ext_params: &[],
                random_coeff_powers,
            };

            total_extract_ms += extract_start.elapsed().as_secs_f64() * 1000.0;

            // Hybrid dispatch: route small components (or high-ext-reg components
            // at moderate sizes) to CPU interpreter to avoid GPU launch overhead
            // and register spilling.
            let kernel_start = Instant::now();
            let eval_rows = 1u64 << eval_domain_log_size;
            let ext_regs = program.header().max_ext_regs;
            let use_simd = eval_rows < HYBRID_GPU_MIN_EVAL_ROWS
                || (ext_regs > HYBRID_HIGH_EXT_REG_THRESHOLD
                    && eval_rows < HYBRID_GPU_MIN_EVAL_ROWS_HIGH_EXT);

            // FORCE_CPU=1 bypasses GPU to isolate shader issues.
            let force_cpu = std::env::var("FORCE_CPU").is_ok();
            let gpu_result: Option<Vec<SecureField>> = if force_cpu || use_simd {
                None
            } else if let Some((ref source, ref name)) =
                shader_cache.get(&program.header().semantic_hash)
            {
                // Try JIT-compiled native shader (eliminates interpreter overhead).
                let res = execute_compiled_metal_evaluation_program_v1(
                    runtime, source, name,
                );
                match res {
                    Ok(values) => {
                        compiled_count += 1;
                        Some(values)
                    }
                    Err(ref e) => {
                        eprintln!(
                            "    [JIT FALLBACK] component '{}': {:?}, trying interpreter",
                            r.name, e,
                        );
                        // Fall back to interpreter kernel.
                        let runtime2 = MetalEvaluationProgramRuntimeInputsV1 {
                            trace: MetalEvaluationProgramTraceViewV1 {
                                trace_interactions: &interaction_slice_refs,
                                preprocessed_columns: &[],
                            },
                            base_params: &[],
                            ext_params: &[],
                            random_coeff_powers,
                        };
                        match execute_selected_metal_evaluation_program_v1_on_metal(
                            program, runtime2, profile,
                        ) {
                            Ok((values, _)) => { gpu_count += 1; Some(values) }
                            Err(_) => None
                        }
                    }
                }
            } else {
                let res = execute_selected_metal_evaluation_program_v1_on_metal(
                    program, runtime, profile,
                );
                if let Err(ref e) = res {
                    eprintln!(
                        "    [GPU FALLBACK] component '{}' (log_size={}, base_regs={}, ext_regs={}): {:?}",
                        r.name, r.log_size,
                        program.header().max_base_regs,
                        ext_regs,
                        e,
                    );
                }
                res.ok().map(|(v, dispatch)| {
                    if dispatch == MetalEvaluationProgramDispatchKindV1::JitCompiled {
                        compiled_count += 1;
                    } else {
                        gpu_count += 1;
                    }
                    v
                })
            };
            let mut row_res = match gpu_result {
                Some(res) => {
                    res
                }
                None => {
                    // Small component (hybrid SIMD), GPU error, or FORCE_CPU
                    // — use CPU interpreter.
                    let runtime_cpu = MetalEvaluationProgramRuntimeInputsV1 {
                        trace: MetalEvaluationProgramTraceViewV1 {
                            trace_interactions: &interaction_slice_refs,
                            preprocessed_columns: &[],
                        },
                        base_params: &[],
                        ext_params: &[],
                        random_coeff_powers,
                    };
                    if use_simd {
                        simd_count += 1;
                    } else {
                        cpu_count += 1;
                    }
                    match interpret_metal_evaluation_program_v1(program, runtime_cpu) {
                        Ok(res) => res,
                        Err(cpu_err) => {
                            panic!(
                                "CPU interpreter failed for component '{}' \
                                 (log_size={}, base_regs={}, ext_regs={}, n_interactions={}, \
                                  trace_interactions={:?}, n_constraints={}): {:?}",
                                r.name,
                                r.log_size,
                                program.header().max_base_regs,
                                ext_regs,
                                n_interactions,
                                interaction_refs.iter().map(|v| v.len()).collect::<Vec<_>>(),
                                n_constraints,
                                cpu_err,
                            );
                        }
                    }
                }
            };

            let comp_kernel_ms = kernel_start.elapsed().as_secs_f64() * 1000.0;
            total_kernel_ms += comp_kernel_ms;
            if comp_kernel_ms > 1.0 {
                println!(
                    "      {:40} log_size={:>2} eval_rows={:>8} kernel={:.1}ms",
                    r.name, r.log_size, 1u64 << eval_domain_log_size, comp_kernel_ms
                );
            }

            // Apply vanishing polynomial inverse (denom_inv).
            let denom_start = Instant::now();
            let trace_domain = CanonicCoset::new(r.log_size);
            let full_eval_domain = CanonicCoset::new(eval_domain_log_size).circle_domain();
            let log_expand = full_eval_domain.log_size() - trace_domain.log_size();
            let mut denom_inv: Vec<BaseField> = (0..(1 << log_expand))
                .map(|index| {
                    stwo::core::constraints::coset_vanishing(
                        trace_domain.coset(),
                        full_eval_domain.at(index),
                    )
                    .inverse()
                })
                .collect();
            stwo::core::utils::bit_reverse(&mut denom_inv);
            let log_n_rows = r.log_size;
            for (row_index, value) in row_res.iter_mut().enumerate() {
                *value = *value * denom_inv[row_index >> log_n_rows];
            }

            total_denom_ms += denom_start.elapsed().as_secs_f64() * 1000.0;

            quotient_results.push((eval_domain_log_size, row_res));
        }

        let v1_ms = v1_start.elapsed().as_secs_f64() * 1000.0;
        println!(
            "    V1 execution: {:.1} ms ({} JIT-compiled, {} GPU interpreter, {} SIMD/CPU hybrid, {} CPU fallback)",
            v1_ms, compiled_count, gpu_count, simd_count, cpu_count
        );
        println!(
            "      Breakdown: jit_compile={:.1}ms, extract={:.1}ms, kernel={:.1}ms, denom_inv={:.1}ms",
            total_jit_ms, total_extract_ms, total_kernel_ms, total_denom_ms
        );

        accumulate_quotients_to_simd_poly(quotient_results)
    }

    /// Composition from a SimdBackend commitment scheme (existing hybrid path).
    #[cfg(feature = "metal-runtime")]
    fn compute_metal_composition_poly(
        results: &[LoweringResult],
        commitment_scheme: &CommitmentSchemeProver<'_, SimdBackend, Blake2sMerkleChannel>,
        random_coeff: SecureField,
        log_blowup_factor: u32,
    ) -> stwo::prover::poly::circle::SecureCirclePoly<SimdBackend> {
        compute_metal_composition_poly_impl(
            results,
            |tree_idx, col_idx, domain| {
                extract_column_on_domain(
                    &commitment_scheme.trees[tree_idx].polynomials[col_idx],
                    domain,
                )
            },
            random_coeff,
            log_blowup_factor,
        )
    }

    /// Composition from a MetalBackend commitment scheme (full pipeline).
    #[cfg(feature = "metal-runtime")]
    fn compute_metal_composition_poly_metal(
        results: &[LoweringResult],
        commitment_scheme: &CommitmentSchemeProver<'_, stwo_metal::MetalBackend, Blake2sMerkleChannel>,
        random_coeff: SecureField,
        log_blowup_factor: u32,
    ) -> stwo::prover::poly::circle::SecureCirclePoly<SimdBackend> {
        compute_metal_composition_poly_impl(
            results,
            |tree_idx, col_idx, domain| {
                extract_column_on_domain_metal(
                    &commitment_scheme.trees[tree_idx].polynomials[col_idx],
                    domain,
                )
            },
            random_coeff,
            log_blowup_factor,
        )
    }

    #[cfg(feature = "metal-runtime")]
    fn extract_column_on_domain(
        poly: &stwo::prover::Poly<SimdBackend>,
        eval_domain: stwo::core::poly::circle::CircleDomain,
    ) -> Vec<stwo::core::fields::m31::BaseField> {
        use stwo::prover::poly::circle::PolyOps;

        if poly.evals.domain == eval_domain {
            // Already on the right domain — zero-copy via as_slice then to_vec.
            poly.evals.values.as_slice().to_vec()
        } else {
            // Need domain extension: IFFT → FFT on target domain.
            let twiddles_from =
                SimdBackend::precompute_twiddles(poly.evals.domain.half_coset);
            let coeffs = poly.evals.clone().interpolate_with_twiddles(&twiddles_from);
            let twiddles_to = SimdBackend::precompute_twiddles(eval_domain.half_coset);
            let extended = coeffs.evaluate_with_twiddles(eval_domain, &twiddles_to);
            extended.values.as_slice().to_vec()
        }
    }

    /// Extract polynomial evaluation on a target domain from a MetalBackend
    /// committed polynomial. Uses stored coefficients to evaluate on the target
    /// domain, bypassing extended evaluations entirely.
    #[cfg(feature = "metal-runtime")]
    fn extract_column_on_domain_metal(
        poly: &stwo::prover::Poly<stwo_metal::MetalBackend>,
        eval_domain: stwo::core::poly::circle::CircleDomain,
    ) -> Vec<stwo::core::fields::m31::BaseField> {
        use stwo::prover::backend::Column;

        if poly.evals.domain == eval_domain {
            // Already on the right domain — download evaluations directly from GPU.
            return poly.evals.values.to_cpu();
        }

        // Fallback: download coefficients, evaluate on CPU.
        use stwo::prover::backend::CpuBackend;
        use stwo::prover::poly::circle::PolyOps;
        let coeffs_ref = poly.coeffs.as_ref()
            .expect("extract_column_on_domain_metal requires store_polynomials_coefficients=true");
        let cpu_coeffs: Vec<stwo::core::fields::m31::BaseField> = coeffs_ref.coeffs.to_cpu();
        let cpu_coeffs_poly = stwo::prover::poly::circle::CircleCoefficients::<CpuBackend>::new(
            cpu_coeffs,
        );
        let twiddles_to = CpuBackend::precompute_twiddles(eval_domain.half_coset);
        let extended = cpu_coeffs_poly.evaluate_with_twiddles(eval_domain, &twiddles_to);
        extended.values
    }

    #[cfg(feature = "metal-runtime")]
    fn accumulate_quotients_to_simd_poly(
        quotient_results: Vec<(u32, Vec<SecureField>)>,
    ) -> stwo::prover::poly::circle::SecureCirclePoly<SimdBackend> {
        use stwo::core::fields::m31::BaseField;
        use stwo::core::poly::circle::CanonicCoset;
        use stwo::prover::poly::circle::{
            CircleCoefficients, CircleEvaluation, PolyOps, SecureCirclePoly,
        };
        use stwo::prover::poly::BitReversedOrder;
        use stwo::prover::secure_column::SecureColumnByCoords;
        use stwo::prover::backend::CpuBackend;
        use stwo::prover::backend::simd::column::BaseColumn;
        use stwo::prover::backend::Column;
        use stwo::prover::AccumulationOps;

        if quotient_results.is_empty() {
            panic!("No quotient results to accumulate");
        }

        let max_log_size = quotient_results.iter().map(|(ls, _)| *ls).max().unwrap();

        let accum_start = Instant::now();

        // Group quotient values by eval domain log_size (components with the
        // same eval domain size share the same sub-accumulation).
        let mut by_size: std::collections::BTreeMap<u32, SecureColumnByCoords<CpuBackend>> =
            std::collections::BTreeMap::new();

        for (eval_log_size, values) in &quotient_results {
            let size = values.len();
            let entry = by_size.entry(*eval_log_size).or_insert_with(|| {
                SecureColumnByCoords::<CpuBackend>::zeros(size)
            });
            // Pointwise-add this component's quotient values into the group.
            for (i, v) in values.iter().enumerate() {
                entry.set(i, entry.at(i) + *v);
            }
        }

        // Convert CpuBackend sub_accumulations to SimdBackend.
        let sub_accumulations_simd: Vec<SecureColumnByCoords<SimdBackend>> =
            by_size.into_values().map(|cpu_col| {
                SecureColumnByCoords::<SimdBackend>::from_cpu(cpu_col)
            }).collect();

        // Use SimdBackend::lift_and_accumulate (matches reference pipeline).
        let lifted = SimdBackend::lift_and_accumulate(sub_accumulations_simd);

        let accum_ms = accum_start.elapsed().as_secs_f64() * 1000.0;
        println!("    Accumulation + IFFT: {:.1} ms", accum_ms);

        if let Some(eval_col) = lifted {
            let domain = CanonicCoset::new(max_log_size).circle_domain();
            let twiddles = SimdBackend::precompute_twiddles(domain.half_coset);
            SecureCirclePoly(eval_col.columns.map(|c| {
                CircleEvaluation::<SimdBackend, BaseField, BitReversedOrder>::new(
                    domain,
                    c,
                )
                .interpolate_with_twiddles(&twiddles)
            }))
        } else {
            let size = 1usize << max_log_size;
            SecureCirclePoly(std::array::from_fn(|_| {
                CircleCoefficients::new(BaseColumn::zeros(size))
            }))
        }
    }

    // -----------------------------------------------------------------------
    // Full MetalBackend pipeline
    // -----------------------------------------------------------------------

    /// Cache path for preprocessed polynomial coefficients (canonical, deterministic).
    #[cfg(feature = "metal-runtime")]
    const PREPROCESSED_CACHE_PATH: &str = "preprocessed_coeffs.bin";

    /// Save preprocessed polynomial coefficients to disk. Only coefficients are
    /// cached (~268 MB); evaluations and Merkle tree are recomputed on load via
    /// CommitmentTreeProver::new (~2.2s) which is faster than the full pipeline
    /// (gen_trace + upload + IFFT + RFFT + Merkle ~4.5s).
    #[cfg(feature = "metal-runtime")]
    fn save_preprocessed_coeffs(
        polys: &[stwo::prover::poly::circle::CircleCoefficients<stwo_metal::MetalBackend>],
        path: &str,
    ) {
        use stwo::core::fields::m31::BaseField;
        use stwo::prover::backend::Column;
        use std::io::{BufWriter, Write};

        let f = match std::fs::File::create(path) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("    Warning: could not create cache file: {}", e);
                return;
            }
        };
        let mut w = BufWriter::new(f);

        let _ = w.write_all(b"PPC2");
        let n = polys.len() as u32;
        let _ = w.write_all(&n.to_le_bytes());

        for coeffs in polys {
            let cpu: Vec<BaseField> = coeffs.coeffs.to_cpu();
            let len = cpu.len() as u32;
            let _ = w.write_all(&len.to_le_bytes());
            let bytes = unsafe {
                std::slice::from_raw_parts(cpu.as_ptr() as *const u8, cpu.len() * 4)
            };
            let _ = w.write_all(bytes);
        }
        let _ = w.flush();
    }

    /// Load preprocessed polynomial coefficients from disk cache.
    #[cfg(feature = "metal-runtime")]
    /// Phase 1: Read preprocessed coefficients from disk into CPU memory only.
    /// No GPU buffer allocation — safe to call from a background thread while
    /// the GPU is busy (e.g. computing twiddles).
    fn load_preprocessed_coeffs_cpu(
        path: &str,
    ) -> Option<Vec<Vec<stwo::core::fields::m31::BaseField>>> {
        use stwo::core::fields::m31::BaseField;
        use std::io::{BufReader, Read};

        let f = std::fs::File::open(path).ok()?;
        let mut r = BufReader::new(f);
        let mut buf4 = [0u8; 4];

        r.read_exact(&mut buf4).ok()?;
        if &buf4 != b"PPC2" {
            return None;
        }

        r.read_exact(&mut buf4).ok()?;
        let n = u32::from_le_bytes(buf4) as usize;

        let mut cpu_polys = Vec::with_capacity(n);
        for _ in 0..n {
            r.read_exact(&mut buf4).ok()?;
            let len = u32::from_le_bytes(buf4) as usize;
            let mut bytes = vec![0u8; len * 4];
            r.read_exact(&mut bytes).ok()?;
            let cpu: Vec<BaseField> = unsafe {
                std::slice::from_raw_parts(bytes.as_ptr() as *const BaseField, len).to_vec()
            };
            cpu_polys.push(cpu);
        }
        Some(cpu_polys)
    }

    /// Phase 2: Upload CPU-resident coefficients to Metal GPU buffers.
    #[cfg(feature = "metal-runtime")]
    fn upload_coeffs_to_gpu(
        cpu_polys: Vec<Vec<stwo::core::fields::m31::BaseField>>,
    ) -> Vec<stwo::prover::poly::circle::CircleCoefficients<stwo_metal::MetalBackend>> {
        cpu_polys
            .into_iter()
            .map(|cpu| {
                let metal_col: <stwo_metal::MetalBackend as stwo::prover::backend::ColumnOps<
                    stwo::core::fields::m31::BaseField,
                >>::Column = cpu.into_iter().collect();
                stwo::prover::poly::circle::CircleCoefficients::new(metal_col)
            })
            .collect()
    }

    /// Run the complete prove pipeline on MetalBackend: preprocessed tree
    /// commit (GPU IFFT + Merkle), trace commits via ConvertingTreeBuilder,
    /// V1 GPU composition, and MetalBackend prove_values.
    #[cfg(feature = "metal-runtime")]
    fn run_metal_full_pipeline(
        input: ProverInput,
        preprocessed_trace: Arc<PreProcessedTrace>,
        verify: bool,
        input_for_simd_comparison: Option<ProverInput>,
    ) {
        use cairo_air::cairo_components::CairoComponents;
        use cairo_air::relations::CommonLookupElements;
        use stwo::core::channel::Channel;
        use stwo::core::circle::CirclePoint;
        use stwo::core::fields::m31::BaseField;
        use stwo::core::fields::qm31::SECURE_EXTENSION_DEGREE;
        use stwo::core::pcs::utils::get_lifting_log_size;
        use stwo::core::poly::circle::CanonicCoset;
        use stwo::core::proof::{ExtendedStarkProof, StarkProof};
        use stwo::core::proof_of_work::GrindOps;
        use stwo::core::utils::MaybeOwned;
        use stwo::core::verifier::PREPROCESSED_TRACE_IDX;
        use stwo::prover::backend::Column;
        use stwo::prover::mempool::BaseColumnPool;
        use stwo::prover::poly::circle::PolyOps;
        use stwo::prover::{CommitmentSchemeProver, CommitmentTreeProver, ComponentProvers};
        use stwo_cairo_prover::witness::cairo::create_cairo_claim_generator;
        use stwo_cairo_prover::witness::preprocessed_trace::gen_trace;
        use stwo_metal::MetalBackend;

        let pcs_config = default_pcs_config();
        let log_max_rows: u32 = 27;
        let max_domain_size = {
            let cairo_air_log_degree_bound = 1;
            log_max_rows
                + std::cmp::max(
                    cairo_air_log_degree_bound,
                    pcs_config.fri_config.log_blowup_factor,
                )
        };
        let include_all_preprocessed_columns = false;

        println!("\n=== Full MetalBackend pipeline ===\n");
        let pipeline_start = Instant::now();

        // 1. Start coefficient I/O in background (CPU-only, no GPU allocation).
        //    Twiddle computation (~272ms) runs concurrently on the main thread,
        //    hiding twiddle cost behind the longer I/O wait (~555ms).
        let coeff_io_handle = std::thread::spawn(|| {
            let t = Instant::now();
            let result = load_preprocessed_coeffs_cpu(PREPROCESSED_CACHE_PATH);
            (result, t.elapsed())
        });

        // 2. MetalBackend twiddles (overlapped with disk I/O, no GPU contention).
        let t_tw = Instant::now();
        println!("  Precomputing Metal twiddles (log_domain_size={})...", max_domain_size);
        let metal_twiddles = MetalBackend::precompute_twiddles(
            CanonicCoset::new(max_domain_size)
                .circle_domain()
                .half_coset,
        );
        let twiddles_ms = t_tw.elapsed().as_secs_f64() * 1000.0;

        // 3. MetalBackend commitment scheme.
        let base_column_pool = BaseColumnPool::new();
        let channel = &mut stwo::core::channel::Blake2sChannel::default();
        let channel_salt: u32 = 0;
        channel.mix_felts(&[SecureField::from(channel_salt)]);
        pcs_config.mix_into(channel);

        let mut commitment_scheme =
            CommitmentSchemeProver::<MetalBackend, Blake2sMerkleChannel>::with_memory_pool(
                pcs_config,
                &metal_twiddles,
                &base_column_pool,
            );
        commitment_scheme.store_polynomials_coefficients = true;

        // 4. Join I/O, then upload to GPU (after twiddles finished — no GPU contention).
        let t_pp = Instant::now();
        let (cached_cpu_polys, io_dur) = coeff_io_handle.join()
            .expect("coefficient I/O thread should not panic");
        let preprocessed_polys = if let Some(cpu_polys) = cached_cpu_polys {
            let io_ms = io_dur.as_secs_f64() * 1000.0;
            let n_polys = cpu_polys.len();
            let upload_start = Instant::now();
            let metal_polys = upload_coeffs_to_gpu(cpu_polys);
            let upload_ms = upload_start.elapsed().as_secs_f64() * 1000.0;
            println!(
                "  Loaded {} preprocessed coefficients: I/O={:.1}ms (overlapped), upload={:.1}ms",
                n_polys, io_ms, upload_ms,
            );
            metal_polys
        } else {
            println!("  Computing preprocessed coefficients (gen_trace + IFFT)...");
            let simd_preprocessed_evals = gen_trace(preprocessed_trace.clone());
            let upload_start = Instant::now();
            let metal_preprocessed_evals: Vec<
                stwo::prover::poly::circle::CircleEvaluation<
                    MetalBackend,
                    BaseField,
                    stwo::prover::poly::BitReversedOrder,
                >,
            > = simd_preprocessed_evals
                .into_iter()
                .map(|eval| {
                    let domain = eval.domain;
                    let metal_values = eval.values.to_cpu().into_iter().collect();
                    stwo::prover::poly::circle::CircleEvaluation::new(domain, metal_values)
                })
                .collect();
            let upload_ms = upload_start.elapsed().as_secs_f64() * 1000.0;

            let ifft_start = Instant::now();
            let polys =
                MetalBackend::interpolate_columns(metal_preprocessed_evals, &metal_twiddles);
            let ifft_ms = ifft_start.elapsed().as_secs_f64() * 1000.0;
            println!(
                "    gen_trace+upload={:.1}ms, GPU IFFT={:.1}ms",
                t_pp.elapsed().as_secs_f64() * 1000.0 - ifft_ms + upload_ms,
                ifft_ms,
            );

            save_preprocessed_coeffs(&polys, PREPROCESSED_CACHE_PATH);
            println!("    Saved {} coefficients to {}", polys.len(), PREPROCESSED_CACHE_PATH);
            polys
        };

        // Print preprocessed polynomial size distribution.
        {
            let mut size_counts = std::collections::BTreeMap::new();
            for p in &preprocessed_polys {
                *size_counts.entry(p.log_size()).or_insert(0u32) += 1;
            }
            print!("    Preprocessed poly sizes:");
            for (log_size, count) in &size_counts {
                print!(" {}×log{}", count, log_size);
            }
            println!();
        }

        // Spawn claim generator on background thread (CPU-only, overlaps with
        // GPU RFFT + Merkle below).  `input` is not used again after this point.
        let preprocessed_trace_for_claim = preprocessed_trace.clone();
        let claim_gen_handle = std::thread::spawn(move || {
            create_cairo_claim_generator(input, preprocessed_trace_for_claim)
        });

        // Build commitment tree: RFFT then Merkle (split for profiling).
        let rfft_start = Instant::now();
        let preprocessed_evaluated = MetalBackend::evaluate_polynomials(
            preprocessed_polys,
            pcs_config.fri_config.log_blowup_factor,
            &metal_twiddles,
            true,
            &base_column_pool,
        );
        let rfft_ms = rfft_start.elapsed().as_secs_f64() * 1000.0;

        let merkle_start = Instant::now();
        let max_log_domain_size = preprocessed_evaluated
            .iter()
            .map(|poly| poly.evals.domain.log_size())
            .max()
            .unwrap_or_default();
        let lifting = pcs_config.lifting_log_size.unwrap_or(max_log_domain_size);
        let merkle_tree =
            stwo::prover::vcs_lifted::prover::MerkleProverLifted::<
                MetalBackend,
                <Blake2sMerkleChannel as stwo::core::channel::MerkleChannel>::H,
            >::commit(
                preprocessed_evaluated
                    .iter()
                    .map(|poly| &poly.evals.values)
                    .collect(),
                lifting,
            );
        let merkle_ms = merkle_start.elapsed().as_secs_f64() * 1000.0;

        let preprocessed_tree = CommitmentTreeProver {
            polynomials: preprocessed_evaluated,
            commitment: merkle_tree,
        };
        let preproc_ms = t_pp.elapsed().as_secs_f64() * 1000.0;
        println!(
            "    RFFT={:.1}ms, Merkle={:.1}ms, total preprocessed={:.1}ms",
            rfft_ms, merkle_ms, preproc_ms,
        );
        commitment_scheme.commit_tree(MaybeOwned::Owned(preprocessed_tree), channel);

        // 4. Write base trace via ConvertingTreeBuilder.
        let t_base = Instant::now();
        println!("  Creating claim generator + base trace...");
        let cairo_claim_generator = claim_gen_handle
            .join()
            .expect("claim generator thread should not panic");
        let mut converting_tb = ConvertingTreeBuilder {
            inner: commitment_scheme.tree_builder(),
        };
        let (claim, interaction_generator) =
            cairo_claim_generator.write_trace(&mut converting_tb);
        let base_gen_ms = t_base.elapsed().as_secs_f64() * 1000.0;

        // Predict lifting_log_size from the claim and start domain coord
        // precomputation on a background thread.  The CPU-only computation
        // (~248ms for log_size 21) overlaps with the remaining GPU-heavy
        // pipeline phases (base commit, interaction, composition ≈ 600ms+).
        let predicted_lifting_log_size = {
            let log_sizes = claim.log_sizes();
            // Index 1 = base trace columns; index 2 = interaction columns.
            // The composition tree eval domain = max_trace_log_size + 1
            // (log_degree_bound=1, log_blowup_factor=1 for Cairo).
            let max_trace_log_size = log_sizes[1]
                .iter()
                .chain(log_sizes[2].iter())
                .copied()
                .max()
                .unwrap_or(0);
            max_trace_log_size + pcs_config.fri_config.log_blowup_factor
        };
        let fri_config_for_precompute = pcs_config.fri_config;
        let domain_coords_handle = std::thread::spawn(move || {
            let domain_coords =
                stwo_metal::quotient::compute_quotient_domain_coords_cpu(predicted_lifting_log_size);
            let fri_factors = stwo_metal::precompute_fri_factors_cpu(
                predicted_lifting_log_size,
                &fri_config_for_precompute,
            );
            (domain_coords, fri_factors)
        });

        let t_base_commit = Instant::now();
        claim.mix_into::<Blake2sMerkleChannel>(channel);
        converting_tb.inner.commit(channel);
        let base_commit_ms = t_base_commit.elapsed().as_secs_f64() * 1000.0;

        // 5. Interaction PoW (Metal GPU).
        let t_pow = Instant::now();
        let interaction_pow_bits = cairo_air::verifier::INTERACTION_POW_BITS;
        let interaction_pow = MetalBackend::grind(channel, interaction_pow_bits);
        channel.mix_u64(interaction_pow);
        let interaction_elements = CommonLookupElements::draw(channel);
        let pow_ms = t_pow.elapsed().as_secs_f64() * 1000.0;

        // 6. Write interaction trace via ConvertingTreeBuilder.
        let t_inter = Instant::now();
        println!("  Generating interaction trace...");
        let mut converting_tb = ConvertingTreeBuilder {
            inner: commitment_scheme.tree_builder(),
        };
        let interaction_claim = interaction_generator
            .write_interaction_trace(&mut converting_tb, &interaction_elements);
        let inter_gen_ms = t_inter.elapsed().as_secs_f64() * 1000.0;

        let t_inter_commit = Instant::now();
        interaction_claim.mix_into(channel);
        converting_tb.inner.commit(channel);
        let inter_commit_ms = t_inter_commit.elapsed().as_secs_f64() * 1000.0;

        // 7. Build CairoComponents and lower.
        println!("  Building CairoComponents...");
        let components = CairoComponents::new(
            &claim,
            &interaction_elements,
            &interaction_claim,
            &preprocessed_trace.ids(),
        );

        println!("\n=== Lowering Cairo components to Metal V1 programs ===\n");
        let (results, lowering_only_ms) = lower_all_components(&components);

        let total = results.len();
        let succeeded = results.iter().filter(|r| r.success).count();
        let failed = total - succeeded;
        print_lowering_results(&results);
        println!(
            "Lowering time: {:.1} ms (pure lowering)",
            lowering_only_ms,
        );

        if failed > 0 {
            println!(
                "\nCannot run Metal prove: {}/{} components failed to lower.",
                failed, total
            );
            std::process::exit(1);
        }

        // 8. Composition (Metal V1 GPU from MetalBackend polynomials).
        println!("\n=== Metal composition + prove_values ===\n");

        let n_preprocessed_columns =
            commitment_scheme.trees[PREPROCESSED_TRACE_IDX].polynomials.len();
        let component_provers = stwo_cairo_prover::utils::cairo_provers(&components);
        let component_provers_struct = ComponentProvers {
            components: component_provers.clone(),
            n_preprocessed_columns,
        };

        let random_coeff = channel.draw_secure_felt();
        let log_blowup_factor = pcs_config.fri_config.log_blowup_factor;

        let composition_start = Instant::now();
        let composition_poly = compute_metal_composition_poly_metal(
            &results,
            &commitment_scheme,
            random_coeff,
            log_blowup_factor,
        );
        let composition_ms = composition_start.elapsed().as_secs_f64() * 1000.0;

        // 9. Commit composition polynomial on MetalBackend.
        let t_comp_commit = Instant::now();
        let mut tree_builder = commitment_scheme.tree_builder();
        let (left_half, right_half) = composition_poly.split_at_mid();
        // SimdBackend stores coefficients in transposed format for log_size > 16
        // (CACHED_FFT_LOG_SIZE). Evaluate on the polynomial's natural domain using
        // SimdBackend's RFFT (which handles transposed input correctly), then convert
        // evaluations to MetalBackend. extend_evals does MetalBackend IFFT to recover
        // standard-order coefficients, then commit does RFFT on the blowup domain.
        let left_coord_polys = left_half.into_coordinate_polys();
        let right_coord_polys = right_half.into_coordinate_polys();
        {
            use stwo::core::poly::circle::CanonicCoset;
            use stwo::prover::backend::Column;
            let simd_to_metal_evals = |polys: [stwo::prover::poly::circle::CircleCoefficients<SimdBackend>; stwo::core::fields::qm31::SECURE_EXTENSION_DEGREE]|
                -> Vec<stwo::prover::poly::circle::CircleEvaluation<MetalBackend, stwo::core::fields::m31::BaseField, stwo::prover::poly::BitReversedOrder>>
            {
                polys.into_iter().map(|p| {
                    let domain = CanonicCoset::new(p.log_size()).circle_domain();
                    let simd_eval = p.evaluate(domain);
                    let metal_values = simd_eval.values.to_cpu().into_iter().collect();
                    stwo::prover::poly::circle::CircleEvaluation::new(domain, metal_values)
                }).collect()
            };
            tree_builder.extend_evals(simd_to_metal_evals(left_coord_polys));
            tree_builder.extend_evals(simd_to_metal_evals(right_coord_polys));
        }
        tree_builder.commit(channel);

        let comp_commit_ms = t_comp_commit.elapsed().as_secs_f64() * 1000.0;

        // Join domain coord precomputation thread and upload to GPU cache.
        let split_composition_log_size = commitment_scheme
            .trees
            .last()
            .unwrap()
            .commitment
            .layers
            .len() as u32
            - 1;
        let lifting_log_size =
            get_lifting_log_size(&commitment_scheme.config, split_composition_log_size);
        assert_eq!(
            lifting_log_size, predicted_lifting_log_size,
            "predicted lifting_log_size mismatch: predicted {} but actual {}",
            predicted_lifting_log_size, lifting_log_size,
        );
        let ((domain_x, domain_y), fri_factors) = domain_coords_handle
            .join()
            .expect("domain coord + FRI factor precomputation thread should not panic");
        stwo_metal::quotient::upload_quotient_domain_coords(
            lifting_log_size,
            domain_x,
            domain_y,
        );
        stwo_metal::upload_fri_factors(fri_factors);

        // 10. Draw OODS point and compute sample points.
        let oods_point = CirclePoint::<SecureField>::get_random_point(channel);
        let max_log_degree_bound =
            lifting_log_size - commitment_scheme.config.fri_config.log_blowup_factor;

        let mut sample_points = component_provers_struct.components().mask_points(
            oods_point,
            max_log_degree_bound,
            include_all_preprocessed_columns,
        );
        sample_points.push(vec![vec![oods_point]; 2 * SECURE_EXTENSION_DEGREE]);

        // 11. prove_values on MetalBackend (GPU quotients + FRI).
        let prove_values_start = Instant::now();
        let commitment_scheme_proof = commitment_scheme.prove_values(sample_points, channel);
        let prove_values_ms = prove_values_start.elapsed().as_secs_f64() * 1000.0;

        let proof = StarkProof(commitment_scheme_proof.proof);
        let extended_proof = ExtendedStarkProof {
            proof,
            aux: commitment_scheme_proof.aux,
        };

        let pipeline_ms = pipeline_start.elapsed().as_secs_f64() * 1000.0;

        // Print timing breakdown.
        println!("\n{}", "=".repeat(90));
        println!("=== Full MetalBackend pipeline results ===\n");
        println!("  Metal twiddles:                    {:>8.1} ms", twiddles_ms);
        println!("  Preprocessed tree (GPU):           {:>8.1} ms", preproc_ms);
        println!("  Base trace gen + upload:           {:>8.1} ms", base_gen_ms);
        println!("  Base trace commit (Metal Merkle):  {:>8.1} ms", base_commit_ms);
        println!("  Interaction PoW (Metal GPU):       {:>8.1} ms", pow_ms);
        println!("  Interaction trace gen + upload:    {:>8.1} ms", inter_gen_ms);
        println!("  Interaction trace commit:          {:>8.1} ms", inter_commit_ms);
        println!("  Lowering:                          {:>8.1} ms", lowering_only_ms);
        println!("  Composition (Metal V1 GPU):        {:>8.1} ms", composition_ms);
        println!("  Composition commit:                {:>8.1} ms", comp_commit_ms);
        println!("  prove_values (MetalBackend):       {:>8.1} ms", prove_values_ms);
        println!("  ─────────────────────────────────────────");
        println!("  Total pipeline:                    {:>8.1} ms", pipeline_ms);

        // 12. Verify if requested.
        if verify {
            use cairo_air::verifier::verify_cairo_ex;
            use cairo_air::CairoProof;
            use cairo_air::PreProcessedTraceVariant;

            let cairo_proof = CairoProof {
                claim: claim.clone(),
                interaction_pow,
                interaction_claim: interaction_claim.clone(),
                extended_stark_proof: extended_proof,
                channel_salt: 0,
                preprocessed_trace_variant: PreProcessedTraceVariant::Canonical,
            };

            println!("\n  Verifying Metal proof...");
            let verify_start = Instant::now();
            verify_cairo_ex::<Blake2sMerkleChannel>(cairo_proof.into(), false)
                .expect("Metal proof verification failed");
            let verify_ms = verify_start.elapsed().as_secs_f64() * 1000.0;
            println!("  Verification passed in {:.1} ms", verify_ms);
        }

        // 13. SimdBackend comparison if available.
        if let Some(input_for_proof) = input_for_simd_comparison {
            let (prove_ms, verify_ms) = run_full_proof_and_verify(input_for_proof);
            println!("\n  Comparison:");
            println!(
                "    Metal full pipeline:            {:>8.1} ms",
                pipeline_ms
            );
            println!(
                "    SimdBackend:                    {:>8.1} ms (prove {:.1} + verify {:.1})",
                prove_ms + verify_ms, prove_ms, verify_ms
            );
        }
    }

    /// Execute each successfully lowered V1 program on both the Metal GPU and
    /// the CPU reference interpreter using synthetic trace data, comparing
    /// results element-by-element to validate correctness.
    ///
    /// Returns the number of programs that failed (GPU/CPU mismatch or error).
    #[cfg(feature = "metal-runtime")]
    fn run_metal_execution(results: &[LoweringResult]) -> usize {
        use stwo::core::fields::m31::BaseField;
        use stwo_metal::capability::MetalRuntimeSupport;
        use stwo_metal::metal_runtime_support;
        use stwo_metal::program::{
            execute_selected_metal_evaluation_program_v1_on_metal,
            interpret_metal_evaluation_program_v1,
            MetalEvaluationProgramBaseOpcodeV1,
            MetalEvaluationProgramCapabilityProfileV1,
            MetalEvaluationProgramRuntimeInputsV1,
            MetalEvaluationProgramTraceViewV1,
        };

        let support = metal_runtime_support();
        println!("\n=== Metal V1 GPU Execution Validation ===\n");
        println!("Metal runtime support: {:?}", support);

        if support != MetalRuntimeSupport::Available {
            println!("Metal runtime not available, skipping GPU execution.");
            return 0;
        }

        let profile = MetalEvaluationProgramCapabilityProfileV1::current();
        let programs_to_run: Vec<_> = results
            .iter()
            .filter(|r| r.program.is_some())
            .collect();

        if programs_to_run.is_empty() {
            println!("No successfully lowered programs to execute.");
            return 0;
        }

        println!(
            "Executing {} lowered V1 programs on GPU and CPU...\n",
            programs_to_run.len()
        );

        struct ExecutionResult {
            name: String,
            log_size: u32,
            dispatch_kind: String,
            matched: bool,
            n_result_rows: usize,
            gpu_ms: f64,
            cpu_ms: f64,
            error: Option<String>,
        }

        let mut exec_results: Vec<ExecutionResult> = Vec::new();

        // Use a small fixed row count for synthetic execution. The constraint
        // evaluation is per-row, so a small count is sufficient to validate
        // GPU/CPU agreement while keeping execution fast.
        let synthetic_n_rows: usize = 64;

        for r in &programs_to_run {
            let program = r.program.as_ref().unwrap();
            let header = program.header();

            // Scan base instructions to determine the required column layout.
            // For each interaction, find the maximum column index referenced by
            // TraceCol instructions; similarly for preprocessed columns.
            let n_interactions = header.n_interactions as usize;
            let mut max_col_per_interaction: Vec<usize> = vec![0; n_interactions];
            let mut max_preprocessed_col: usize = 0;
            let mut has_preprocessed = false;

            for inst in program.base_insts() {
                match MetalEvaluationProgramBaseOpcodeV1::from_raw(inst.op) {
                    Some(MetalEvaluationProgramBaseOpcodeV1::TraceCol) => {
                        let interaction = inst.interaction as usize;
                        let column = inst.a as usize;
                        if interaction < n_interactions
                            && column + 1 > max_col_per_interaction[interaction]
                        {
                            max_col_per_interaction[interaction] = column + 1;
                        }
                    }
                    Some(MetalEvaluationProgramBaseOpcodeV1::PreprocessedCol) => {
                        has_preprocessed = true;
                        let column = inst.a as usize;
                        if column + 1 > max_preprocessed_col {
                            max_preprocessed_col = column + 1;
                        }
                    }
                    _ => {}
                }
            }

            // Build synthetic trace data with deterministic non-zero values
            // (seeded by interaction, column, and row indices) so the constraint
            // evaluation produces non-trivial, reproducible results.
            let mut interaction_column_storage: Vec<Vec<Vec<BaseField>>> =
                Vec::with_capacity(n_interactions);
            for interaction_idx in 0..n_interactions {
                let n_cols = max_col_per_interaction[interaction_idx];
                let mut columns = Vec::with_capacity(n_cols);
                for col_idx in 0..n_cols {
                    let column: Vec<BaseField> = (0..synthetic_n_rows)
                        .map(|row_idx| {
                            let seed = ((interaction_idx as u32).wrapping_mul(1_000_003))
                                .wrapping_add((col_idx as u32).wrapping_mul(10_007))
                                .wrapping_add(row_idx as u32)
                                .wrapping_add(1);
                            BaseField::from_u32_unchecked(seed % ((1u32 << 31) - 1))
                        })
                        .collect();
                    columns.push(column);
                }
                interaction_column_storage.push(columns);
            }

            // Build the nested reference slices required by the API:
            //   trace_interactions: &[&[&[BaseField]]]
            let interaction_col_refs: Vec<Vec<&[BaseField]>> = interaction_column_storage
                .iter()
                .map(|cols| cols.iter().map(|c| c.as_slice()).collect())
                .collect();
            let interaction_refs: Vec<&[&[BaseField]]> = interaction_col_refs
                .iter()
                .map(|cols| cols.as_slice())
                .collect();

            // Build preprocessed columns (if needed).
            let preprocessed_storage: Vec<Vec<BaseField>> = if has_preprocessed {
                (0..max_preprocessed_col)
                    .map(|col_idx| {
                        (0..synthetic_n_rows)
                            .map(|row_idx| {
                                let seed = ((col_idx as u32).wrapping_mul(7919))
                                    .wrapping_add(row_idx as u32)
                                    .wrapping_add(42);
                                BaseField::from_u32_unchecked(seed % ((1u32 << 31) - 1))
                            })
                            .collect()
                    })
                    .collect()
            } else {
                Vec::new()
            };
            let preprocessed_refs: Vec<&[BaseField]> = preprocessed_storage
                .iter()
                .map(|c| c.as_slice())
                .collect();

            // Build base_params and ext_params (must match header counts).
            let base_params: Vec<BaseField> = (0..header.n_base_params)
                .map(|i| BaseField::from_u32_unchecked((i + 100) % ((1u32 << 31) - 1)))
                .collect();
            let ext_params: Vec<SecureField> = (0..header.n_ext_params)
                .map(|i| {
                    SecureField::from_u32_unchecked(
                        (i * 4 + 200) % ((1u32 << 31) - 1),
                        (i * 4 + 201) % ((1u32 << 31) - 1),
                        (i * 4 + 202) % ((1u32 << 31) - 1),
                        (i * 4 + 203) % ((1u32 << 31) - 1),
                    )
                })
                .collect();

            // Build random coefficient powers (one per constraint root).
            let n_constraint_roots = program.constraint_roots().len();
            let random_coeff_powers: Vec<SecureField> = (0..n_constraint_roots)
                .map(|i| {
                    SecureField::from_u32_unchecked(
                        ((i as u32) * 4 + 3) % ((1u32 << 31) - 1),
                        ((i as u32) * 4 + 5) % ((1u32 << 31) - 1),
                        ((i as u32) * 4 + 7) % ((1u32 << 31) - 1),
                        ((i as u32) * 4 + 11) % ((1u32 << 31) - 1),
                    )
                })
                .collect();

            let runtime = MetalEvaluationProgramRuntimeInputsV1 {
                trace: MetalEvaluationProgramTraceViewV1 {
                    trace_interactions: &interaction_refs,
                    preprocessed_columns: &preprocessed_refs,
                },
                base_params: &base_params,
                ext_params: &ext_params,
                random_coeff_powers: &random_coeff_powers,
            };

            // Run CPU reference interpreter.
            let cpu_start = Instant::now();
            let cpu_result = interpret_metal_evaluation_program_v1(program, runtime);
            let cpu_ms = cpu_start.elapsed().as_secs_f64() * 1000.0;

            let cpu_values = match cpu_result {
                Ok(values) => values,
                Err(e) => {
                    exec_results.push(ExecutionResult {
                        name: r.name.clone(),
                        log_size: r.log_size,
                        dispatch_kind: "N/A".to_string(),
                        matched: false,
                        n_result_rows: 0,
                        gpu_ms: 0.0,
                        cpu_ms,
                        error: Some(format!("CPU interpreter error: {:?}", e)),
                    });
                    continue;
                }
            };

            // Run GPU execution via selected dispatch.
            let gpu_start = Instant::now();
            let gpu_result =
                execute_selected_metal_evaluation_program_v1_on_metal(program, runtime, profile);
            let gpu_ms = gpu_start.elapsed().as_secs_f64() * 1000.0;

            let (gpu_values, dispatch) = match gpu_result {
                Ok(pair) => pair,
                Err(e) => {
                    exec_results.push(ExecutionResult {
                        name: r.name.clone(),
                        log_size: r.log_size,
                        dispatch_kind: "N/A".to_string(),
                        matched: false,
                        n_result_rows: cpu_values.len(),
                        gpu_ms,
                        cpu_ms,
                        error: Some(format!("GPU execution error: {:?}", e)),
                    });
                    continue;
                }
            };

            // Compare GPU and CPU results element-by-element.
            let all_match = gpu_values.len() == cpu_values.len()
                && gpu_values
                    .iter()
                    .zip(cpu_values.iter())
                    .all(|(g, c)| g == c);

            let error = if !all_match {
                if gpu_values.len() != cpu_values.len() {
                    Some(format!(
                        "Result length mismatch: GPU={}, CPU={}",
                        gpu_values.len(),
                        cpu_values.len()
                    ))
                } else {
                    gpu_values
                        .iter()
                        .zip(cpu_values.iter())
                        .enumerate()
                        .find(|(_, (g, c))| g != c)
                        .map(|(idx, (gpu_val, cpu_val))| {
                            format!(
                                "First mismatch at row {}: GPU={:?}, CPU={:?}",
                                idx, gpu_val, cpu_val
                            )
                        })
                }
            } else {
                None
            };

            exec_results.push(ExecutionResult {
                name: r.name.clone(),
                log_size: r.log_size,
                dispatch_kind: format!("{:?}", dispatch),
                matched: all_match,
                n_result_rows: gpu_values.len(),
                gpu_ms,
                cpu_ms,
                error,
            });
        }

        // Print execution results.
        println!(
            "{:<50} {:>6} {:>30} {:>8} {:>8} {:>8} {:>6}",
            "COMPONENT", "LOG_SZ", "DISPATCH", "MATCH", "GPU_MS", "CPU_MS", "ROWS"
        );
        println!("{}", "-".repeat(120));
        for er in &exec_results {
            let match_str = if er.matched { "OK" } else { "FAIL" };
            println!(
                "{:<50} {:>6} {:>30} {:>8} {:>8.2} {:>8.2} {:>6}",
                er.name, er.log_size, er.dispatch_kind, match_str, er.gpu_ms, er.cpu_ms,
                er.n_result_rows,
            );
            if let Some(ref err) = er.error {
                println!("    Error: {}", err);
            }
        }

        let exec_total = exec_results.len();
        let exec_matched = exec_results.iter().filter(|er| er.matched).count();
        let exec_failed = exec_total - exec_matched;

        println!("\n{}", "=".repeat(120));
        println!(
            "Execution summary: {}/{} programs matched (GPU == CPU), {} failed",
            exec_matched, exec_total, exec_failed
        );

        if exec_failed > 0 {
            println!("\nWARNING: {} GPU/CPU mismatches detected!", exec_failed);
        }

        exec_failed
    }
}
