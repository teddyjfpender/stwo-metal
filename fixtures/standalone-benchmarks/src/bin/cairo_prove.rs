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
        /// If set, run benchmark mode: N iterations with JSON output.
        bench: Option<usize>,
    }

    fn parse_args() -> CliArgs {
        let args: Vec<String> = std::env::args().collect();
        let mut verify = false;
        let mut metal = false;
        let mut bench: Option<usize> = None;
        let mut positional: Vec<String> = Vec::new();

        let mut i = 1;
        while i < args.len() {
            match args[i].as_str() {
                "--verify" => verify = true,
                "--metal" => metal = true,
                "--bench" => {
                    i += 1;
                    bench = Some(
                        args.get(i)
                            .and_then(|s| s.parse().ok())
                            .unwrap_or_else(|| {
                                eprintln!("--bench requires a positive integer argument");
                                std::process::exit(1);
                            }),
                    );
                }
                _ if args[i].starts_with('-') => {
                    eprintln!("Unknown flag: {}", args[i]);
                    std::process::exit(1);
                }
                _ => positional.push(args[i].clone()),
            }
            i += 1;
        }

        let input_path = if let Some(path) = positional.first() {
            path.clone()
        } else if let Ok(path) = std::env::var("PROVER_INPUT_JSON") {
            path
        } else {
            eprintln!("Usage: cairo_prove [--verify] [--metal] [--bench N] <path-to-prover_input.json>");
            eprintln!("   or: PROVER_INPUT_JSON=<path> cairo_prove [--verify] [--metal]");
            eprintln!();
            eprintln!("Flags:");
            eprintln!("  --verify   Run full stwo-cairo proof on SimdBackend and verify");
            eprintln!("  --metal    Use Metal GPU for composition polynomial (requires metal-runtime)");
            eprintln!("  --bench N  Run N iterations, output JSON timing (combine with --metal or --verify)");
            std::process::exit(1);
        };

        CliArgs {
            input_path,
            verify,
            metal,
            bench,
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

    /// Count total Cairo VM cycles from state transitions.
    fn count_cycles(input: &ProverInput) -> u64 {
        input
            .state_transitions
            .casm_states_by_opcode
            .counts()
            .iter()
            .map(|(_, count)| *count)
            .sum::<usize>() as u64
    }

    /// Benchmark mode: run the prover N times and output JSON timing.
    ///
    /// With `--metal --bench N`: runs Metal pipeline N times.
    /// With `--verify --bench N`: runs SIMD prove N times.
    /// With `--metal --verify --bench N`: runs both, Metal first then SIMD.
    fn run_bench(input: ProverInput, n_iters: usize, metal: bool, simd: bool) {
        let cycles = count_cycles(&input);
        eprintln!("Benchmark: {} cycles, {} iterations", cycles, n_iters);

        #[allow(unused_mut)]
        let mut metal_times_ms: Vec<f64> = Vec::new();
        let mut simd_times_ms: Vec<f64> = Vec::new();

        #[cfg(feature = "metal-runtime")]
        if metal {
            // Warm up GPU pipeline (shader compilation, twiddle cache).
            let _ = stwo_metal_sys::metal::U32Buffer::blake2s_grind_batch(
                &[0u32; 8], 32, 0, 1,
            );

            // Build twiddles + preprocessed tree once, reuse across iterations.
            let preprocessed_trace = Arc::new(PreProcessedTrace::canonical());
            let cache_start = Instant::now();
            let cached = build_cached_metal_artifacts(preprocessed_trace.clone());
            let cache_ms = cache_start.elapsed().as_secs_f64() * 1000.0;
            eprintln!("  Cache build: {:.1} ms (amortized across {} iters)", cache_ms, n_iters);

            for i in 0..n_iters {
                let input_clone = input.clone();
                let preprocessed_trace = preprocessed_trace.clone();
                let t = Instant::now();
                run_metal_prove_only(input_clone, preprocessed_trace, Some(&cached));
                let ms = t.elapsed().as_secs_f64() * 1000.0;
                metal_times_ms.push(ms);
                eprintln!("  Metal iter {}/{}: {:.1} ms", i + 1, n_iters, ms);
            }
        }

        #[cfg(not(feature = "metal-runtime"))]
        if metal {
            eprintln!("WARNING: --metal requires metal-runtime feature");
        }

        if simd {
            for i in 0..n_iters {
                let input_clone = input.clone();
                let t = Instant::now();
                run_simd_prove_only(input_clone);
                let ms = t.elapsed().as_secs_f64() * 1000.0;
                simd_times_ms.push(ms);
                eprintln!("  SIMD iter {}/{}: {:.1} ms", i + 1, n_iters, ms);
            }
        }

        // Output JSON result.
        let metal_json = if metal_times_ms.is_empty() {
            "null".to_string()
        } else {
            format!(
                "[{}]",
                metal_times_ms
                    .iter()
                    .map(|t| format!("{:.1}", t))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        };
        let simd_json = if simd_times_ms.is_empty() {
            "null".to_string()
        } else {
            format!(
                "[{}]",
                simd_times_ms
                    .iter()
                    .map(|t| format!("{:.1}", t))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        };
        // Use BENCH_JSON prefix so scripts can reliably extract the result
        // from among verbose pipeline output.
        println!(
            r#"BENCH_JSON {{"cycles":{},"metal_ms":{},"simd_ms":{}}}"#,
            cycles, metal_json, simd_json,
        );
    }

    /// Run Metal prove pipeline only (no verification, no comparison).
    /// Returns the total pipeline time.
    #[cfg(feature = "metal-runtime")]
    fn run_metal_prove_only(
        input: ProverInput,
        preprocessed_trace: Arc<PreProcessedTrace>,
        cached_artifacts: Option<&CachedMetalArtifacts>,
    ) {
        run_metal_full_pipeline(input, preprocessed_trace, false, None, cached_artifacts);
    }

    /// Run SIMD prove only (no verification).
    fn run_simd_prove_only(input: ProverInput) {
        use stwo_cairo_prover::prover::{prove_cairo, ChannelHash, ProverParameters};
        use cairo_air::PreProcessedTraceVariant;

        let prover_params = ProverParameters {
            channel_hash: ChannelHash::Blake2s,
            channel_salt: 0,
            pcs_config: default_pcs_config(),
            preprocessed_trace: PreProcessedTraceVariant::Canonical,
            store_polynomials_coefficients: false,
            include_all_preprocessed_columns: false,
        };

        let _proof = prove_cairo::<Blake2sMerkleChannel>(input, prover_params)
            .expect("stwo-cairo prove_cairo failed");
    }

    pub fn run() {
        let cli = parse_args();

        println!("Loading prover input from: {}", cli.input_path);
        let input_json = std::fs::read_to_string(&cli.input_path)
            .unwrap_or_else(|e| panic!("Failed to read {}: {}", cli.input_path, e));

        println!("Deserializing ProverInput...");
        let input: ProverInput = serde_json::from_str(&input_json)
            .unwrap_or_else(|e| panic!("Failed to parse ProverInput: {}", e));

        // Benchmark mode: run N iterations and output JSON timing.
        if let Some(n_iters) = cli.bench {
            run_bench(input, n_iters, cli.metal, cli.verify);
            return;
        }

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
            run_metal_full_pipeline(input, preprocessed_trace, cli.verify, input_for_verify, None);
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
        extract_column_gpu: Option<&dyn Fn(usize, usize, stwo::core::poly::circle::CircleDomain) -> stwo_metal::MetalBaseFieldVec>,
        random_coeff: SecureField,
        _log_blowup_factor: u32,
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
            execute_compiled_metal_evaluation_program_v1_async,
            execute_compiled_metal_evaluation_program_v1_tg,
            execute_compiled_async_gpu_trace,
            complete_compiled_metal_evaluation_program_v1_async,
            execute_selected_metal_evaluation_program_v1_on_metal,
            interpret_metal_evaluation_program_v1,
            CommandBufferHandle,
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
        let mut total_jit_ms = 0.0f64;

        // JIT-compiled native Metal shaders: default on, opt-out via NO_JIT=1.
        let use_compiled = std::env::var("NO_JIT").is_err();
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

        // --- Phase 1: Extract columns and classify all components ---
        let extract_start = Instant::now();
        let force_cpu = std::env::var("FORCE_CPU").is_ok();

        struct ComponentWork<'a> {
            name: &'a str,
            log_size: u32,
            eval_domain_log_size: u32,
            program: &'a stwo_metal::program::OwnedMetalEvaluationProgramV1,
            interaction_cols: Vec<Vec<Vec<BaseField>>>,
            /// GPU-resident columns for JIT dispatch (avoids CPU round-trip).
            /// When `Some`, `interaction_cols` is empty — populated lazily on fallback.
            gpu_interaction_cols: Option<Vec<Vec<stwo_metal::MetalBaseFieldVec>>>,
            coeff_start: usize,
            coeff_end: usize,
            use_simd: bool,
        }

        let mut components: Vec<ComponentWork<'_>> = Vec::new();
        let mut coeff_offset = 0usize;

        for r in &successful_results {
            let program = r.program.as_ref().unwrap();
            let n_interactions = program.header().n_interactions as usize;
            let n_constraints = program.constraint_roots().len();
            // CRITICAL: use max_constraint_log_degree_bound, not log_size + 1.
            let eval_domain = CanonicCoset::new(r.max_constraint_log_degree_bound).circle_domain();
            let eval_domain_log_size = r.max_constraint_log_degree_bound;

            let eval_rows = 1u64 << eval_domain_log_size;
            let ext_regs = program.header().max_ext_regs;
            let use_simd = force_cpu
                || eval_rows < HYBRID_GPU_MIN_EVAL_ROWS
                || (ext_regs > HYBRID_HIGH_EXT_REG_THRESHOLD
                    && eval_rows < HYBRID_GPU_MIN_EVAL_ROWS_HIGH_EXT);

            // Determine if this component will use GPU+JIT dispatch.
            // If so, extract columns to GPU-resident BaseFieldVec (no CPU download).
            let has_jit = !use_simd
                && shader_cache.contains_key(&program.header().semantic_hash);
            let use_gpu_extraction = has_jit && extract_column_gpu.is_some();

            let mut interaction_cols: Vec<Vec<Vec<BaseField>>> = Vec::new();
            let mut gpu_cols: Option<Vec<Vec<stwo_metal::MetalBaseFieldVec>>> = None;

            if use_gpu_extraction {
                // GPU-resident extraction: keep columns on GPU for JIT dispatch.
                let extractor = extract_column_gpu.unwrap();
                let mut gpu_interactions: Vec<Vec<stwo_metal::MetalBaseFieldVec>> = Vec::new();
                for interaction_idx in 0..n_interactions {
                    let tree_idx = interaction_idx;
                    if interaction_idx == 0 && !r.preprocessed_column_indices.is_empty() {
                        let cols: Vec<stwo_metal::MetalBaseFieldVec> = r
                            .preprocessed_column_indices
                            .iter()
                            .map(|&idx| extractor(0, idx, eval_domain))
                            .collect();
                        gpu_interactions.push(cols);
                    } else if interaction_idx == 0 && r.preprocessed_column_indices.is_empty() {
                        gpu_interactions.push(vec![]);
                    } else {
                        let location = r
                            .trace_locations
                            .iter()
                            .find(|loc| loc.tree_index == tree_idx);
                        if let Some(loc) = location {
                            let cols: Vec<stwo_metal::MetalBaseFieldVec> = (loc.col_start..loc.col_end)
                                .map(|col_idx| extractor(tree_idx, col_idx, eval_domain))
                                .collect();
                            gpu_interactions.push(cols);
                        } else {
                            gpu_interactions.push(vec![]);
                        }
                    }
                }
                // Leave interaction_cols empty for GPU+JIT path.
                for _ in 0..n_interactions {
                    interaction_cols.push(vec![]);
                }
                gpu_cols = Some(gpu_interactions);
            } else {
                // CPU extraction (SIMD / interpreter path).
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
            }

            let coeff_start = coeff_offset;
            coeff_offset += n_constraints;

            components.push(ComponentWork {
                name: &r.name,
                log_size: r.log_size,
                eval_domain_log_size,
                program,
                interaction_cols,
                gpu_interaction_cols: gpu_cols,
                coeff_start,
                coeff_end: coeff_offset,
                use_simd,
            });
        }

        let total_extract_ms = extract_start.elapsed().as_secs_f64() * 1000.0;

        // --- Phase 2: Partition into GPU and SIMD batches ---
        let (gpu_components, simd_components): (Vec<_>, Vec<_>) =
            components.into_iter().partition::<Vec<_>, _>(|c| !c.use_simd);
        println!("    dispatch: {} GPU, {} SIMD/CPU", gpu_components.len(), simd_components.len());

        // --- TG Size Sweep (opt-in via TG_SWEEP=1) ---
        // Isolated A/B test for JIT-compiled components, sweeping threadgroup sizes.
        // Runs each JIT component N_WARMUP+N_RUNS times per TG size and prints median.
        if std::env::var("TG_SWEEP").is_ok() {
            const TG_SIZES: &[u32] = &[0, 64, 96, 128, 160, 192, 256];
            const N_WARMUP: usize = 2;
            const N_RUNS: usize = 5;

            println!("\n  === TG SIZE SWEEP ===");
            for comp in gpu_components.iter().chain(simd_components.iter()) {
                // Skip GPU-extracted components (no CPU data for TG sweep).
                if comp.gpu_interaction_cols.is_some() { continue; }

                let cache_entry = shader_cache.get(&comp.program.header().semantic_hash);
                let (source, name) = match cache_entry {
                    Some((ref s, ref n)) => (s.as_str(), n.as_str()),
                    None => continue,
                };

                let interaction_refs: Vec<Vec<&[BaseField]>> = comp.interaction_cols
                    .iter()
                    .map(|cols| cols.iter().map(|c| c.as_slice()).collect())
                    .collect();
                let interaction_slice_refs: Vec<&[&[BaseField]]> =
                    interaction_refs.iter().map(|cols| cols.as_slice()).collect();
                let random_coeff_powers =
                    &all_random_coeff_powers[comp.coeff_start..comp.coeff_end];

                println!("  Component: {:40} log_size={} eval_rows={} ext_regs={}",
                    comp.name, comp.log_size, 1u64 << comp.eval_domain_log_size,
                    comp.program.header().max_ext_regs);

                for &tg in TG_SIZES {
                    let label = if tg == 0 { "default".to_string() } else { format!("{:>3}", tg) };

                    // Warmup runs.
                    for _ in 0..N_WARMUP {
                        let rt = MetalEvaluationProgramRuntimeInputsV1 {
                            trace: MetalEvaluationProgramTraceViewV1 {
                                trace_interactions: &interaction_slice_refs,
                                preprocessed_columns: &[],
                            },
                            base_params: &[],
                            ext_params: &[],
                            random_coeff_powers,
                        };
                        let _ = execute_compiled_metal_evaluation_program_v1_tg(rt, source, name, tg);
                    }

                    // Timed runs.
                    let mut times = Vec::with_capacity(N_RUNS);
                    for _ in 0..N_RUNS {
                        let rt = MetalEvaluationProgramRuntimeInputsV1 {
                            trace: MetalEvaluationProgramTraceViewV1 {
                                trace_interactions: &interaction_slice_refs,
                                preprocessed_columns: &[],
                            },
                            base_params: &[],
                            ext_params: &[],
                            random_coeff_powers,
                        };
                        let t = Instant::now();
                        let _ = execute_compiled_metal_evaluation_program_v1_tg(rt, source, name, tg);
                        times.push(t.elapsed().as_secs_f64() * 1000.0);
                    }
                    times.sort_by(|a, b| a.partial_cmp(b).unwrap());
                    let median = times[N_RUNS / 2];
                    let min = times[0];
                    let max = times[N_RUNS - 1];
                    println!("    TG={:>7}: median={:.2}ms  min={:.2}ms  max={:.2}ms",
                        label, median, min, max);
                }
                println!();
            }
            println!("  === END TG SIZE SWEEP ===\n");
        }

        // Helper: apply vanishing polynomial inverse (denom_inv) in-place.
        fn apply_denom_inv(row_res: &mut [SecureField], log_size: u32, eval_domain_log_size: u32) {
            use stwo::core::fields::m31::BaseField;
            use stwo::core::poly::circle::CanonicCoset;
            let trace_domain = CanonicCoset::new(log_size);
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
            for (row_index, value) in row_res.iter_mut().enumerate() {
                *value = *value * denom_inv[row_index >> log_size];
            }
        }

        // --- Phase 3: Parallel dispatch (GPU thread + SIMD on main thread) ---
        // GPU thread: submit all JIT components async (non-blocking), then
        // submit interpreter components sync, then wait on all async handles.
        // This pipelines Metal command buffer scheduling across JIT kernels.
        let v1_start = Instant::now();

        let (gpu_quotients, simd_quotients, gpu_count, compiled_count, simd_count, cpu_count, gpu_kernel_ms, simd_kernel_ms, total_denom_ms) = std::thread::scope(|s| {
            let gpu_handle = s.spawn(|| {
                let mut quotients: Vec<(u32, Vec<SecureField>)> = Vec::new();
                let mut gpu_ct = 0usize;
                let mut compiled_ct = 0usize;
                let mut cpu_ct = 0usize;
                let mut wait_ms = 0.0f64;
                let mut denom_ms = 0.0f64;

                // Phase 3a: Submit all JIT-compilable components async.
                struct PendingGpu<'a> {
                    handle: CommandBufferHandle,
                    dst: stwo_metal_sys::metal::U32Buffer,
                    comp: &'a ComponentWork<'a>,
                }
                let mut pending: Vec<PendingGpu<'_>> = Vec::new();
                let mut sync_components: Vec<&ComponentWork<'_>> = Vec::new();
                let mut gpu_trace_ct = 0usize; // Components using GPU buffer pass-through

                let submit_start = Instant::now();
                for comp in &gpu_components {
                    let cache_entry = shader_cache.get(&comp.program.header().semantic_hash);
                    if let Some((ref source, ref name)) = cache_entry {
                        // GPU buffer pass-through: if we have GPU-resident columns,
                        // build a flat U32Buffer directly and dispatch without CPU round-trip.
                        if let Some(ref gpu_interactions) = comp.gpu_interaction_cols {
                            let n_rows = 1usize << comp.eval_domain_log_size;
                            // Build interaction_offsets and flat GPU trace buffer.
                            let mut interaction_offsets: Vec<u32> = Vec::with_capacity(gpu_interactions.len() + 1);
                            let mut total_cols = 0u32;
                            interaction_offsets.push(0);
                            for interaction in gpu_interactions {
                                total_cols += interaction.len() as u32;
                                interaction_offsets.push(total_cols);
                            }
                            let total_elements = (total_cols as usize) * n_rows;
                            if total_elements > 0 {
                                let random_coeff_powers =
                                    &all_random_coeff_powers[comp.coeff_start..comp.coeff_end];
                                let gpu_trace = stwo_metal_sys::metal::U32Buffer::zeroed(total_elements);
                                match gpu_trace {
                                    Ok(mut gpu_trace) => {
                                        let mut write_offset = 0usize;
                                        let mut concat_ok = true;
                                        for interaction in gpu_interactions {
                                            for col in interaction {
                                                if let Err(e) = gpu_trace.copy_from_offset(col.gpu_buffer(), write_offset) {
                                                    eprintln!(
                                                        "    [GPU CONCAT FAIL] component '{}': {}",
                                                        comp.name, e.message(),
                                                    );
                                                    concat_ok = false;
                                                    break;
                                                }
                                                write_offset += col.len();
                                            }
                                            if !concat_ok { break; }
                                        }
                                        if concat_ok {
                                            match execute_compiled_async_gpu_trace(
                                                gpu_trace,
                                                &interaction_offsets,
                                                n_rows,
                                                random_coeff_powers,
                                                source,
                                                name,
                                            ) {
                                                Ok((handle, dst)) => {
                                                    compiled_ct += 1;
                                                    gpu_trace_ct += 1;
                                                    pending.push(PendingGpu { handle, dst, comp });
                                                    continue;
                                                }
                                                Err(ref e) => {
                                                    eprintln!(
                                                        "    [GPU TRACE JIT FALLBACK] component '{}': {:?}",
                                                        comp.name, e,
                                                    );
                                                }
                                            }
                                        }
                                    }
                                    Err(ref e) => {
                                        eprintln!(
                                            "    [GPU ALLOC FAIL] component '{}': {}",
                                            comp.name, e.message(),
                                        );
                                    }
                                }
                                // Fall through to CPU path on any GPU error.
                                sync_components.push(comp);
                                continue;
                            }
                        }

                        // CPU path: build CPU slice refs and dispatch via existing async JIT.
                        let interaction_refs: Vec<Vec<&[BaseField]>> = comp.interaction_cols
                            .iter()
                            .map(|cols| cols.iter().map(|c| c.as_slice()).collect())
                            .collect();
                        let interaction_slice_refs: Vec<&[&[BaseField]]> =
                            interaction_refs.iter().map(|cols| cols.as_slice()).collect();
                        let random_coeff_powers =
                            &all_random_coeff_powers[comp.coeff_start..comp.coeff_end];

                        let runtime = MetalEvaluationProgramRuntimeInputsV1 {
                            trace: MetalEvaluationProgramTraceViewV1 {
                                trace_interactions: &interaction_slice_refs,
                                preprocessed_columns: &[],
                            },
                            base_params: &[],
                            ext_params: &[],
                            random_coeff_powers,
                        };

                        match execute_compiled_metal_evaluation_program_v1_async(
                            runtime, source, name,
                        ) {
                            Ok((handle, dst)) => {
                                compiled_ct += 1;
                                pending.push(PendingGpu { handle, dst, comp });
                            }
                            Err(ref e) => {
                                eprintln!(
                                    "    [ASYNC JIT FALLBACK] component '{}': {:?}",
                                    comp.name, e,
                                );
                                sync_components.push(comp);
                            }
                        }
                    } else {
                        sync_components.push(comp);
                    }
                }
                let submit_ms = submit_start.elapsed().as_secs_f64() * 1000.0;

                // Phase 3b: Dispatch sync (interpreter) components while GPU is running.
                for comp in &sync_components {
                    // If this component had GPU-resident columns (fell back from JIT),
                    // download them to CPU for interpreter dispatch.
                    let fallback_cpu_cols: Option<Vec<Vec<Vec<BaseField>>>> =
                        comp.gpu_interaction_cols.as_ref().map(|gpu_interactions| {
                            gpu_interactions
                                .iter()
                                .map(|interaction| {
                                    interaction.iter().map(|col| col.to_vec()).collect()
                                })
                                .collect()
                        });

                    let source_cols = fallback_cpu_cols.as_ref()
                        .unwrap_or(&comp.interaction_cols);

                    let interaction_refs: Vec<Vec<&[BaseField]>> = source_cols
                        .iter()
                        .map(|cols| cols.iter().map(|c| c.as_slice()).collect())
                        .collect();
                    let interaction_slice_refs: Vec<&[&[BaseField]]> =
                        interaction_refs.iter().map(|cols| cols.as_slice()).collect();
                    let random_coeff_powers =
                        &all_random_coeff_powers[comp.coeff_start..comp.coeff_end];

                    let runtime = MetalEvaluationProgramRuntimeInputsV1 {
                        trace: MetalEvaluationProgramTraceViewV1 {
                            trace_interactions: &interaction_slice_refs,
                            preprocessed_columns: &[],
                        },
                        base_params: &[],
                        ext_params: &[],
                        random_coeff_powers,
                    };

                    let kernel_start = Instant::now();
                    let res = execute_selected_metal_evaluation_program_v1_on_metal(
                        comp.program, runtime, profile,
                    );
                    let comp_kernel_ms = kernel_start.elapsed().as_secs_f64() * 1000.0;
                    wait_ms += comp_kernel_ms;

                    let mut row_res = match res {
                        Ok((values, dispatch)) => {
                            if dispatch == MetalEvaluationProgramDispatchKindV1::JitCompiled {
                                compiled_ct += 1;
                            } else {
                                gpu_ct += 1;
                            }
                            if comp_kernel_ms > 1.0 {
                                println!(
                                    "      {:40} log_size={:>2} eval_rows={:>8} kernel={:.1}ms [GPU sync]",
                                    comp.name, comp.log_size, 1u64 << comp.eval_domain_log_size,
                                    comp_kernel_ms
                                );
                            }
                            values
                        }
                        Err(ref e) => {
                            eprintln!(
                                "    [GPU FALLBACK] component '{}' (log_size={}, ext_regs={}): {:?}",
                                comp.name, comp.log_size, comp.program.header().max_ext_regs, e,
                            );
                            cpu_ct += 1;
                            let runtime_cpu = MetalEvaluationProgramRuntimeInputsV1 {
                                trace: MetalEvaluationProgramTraceViewV1 {
                                    trace_interactions: &interaction_slice_refs,
                                    preprocessed_columns: &[],
                                },
                                base_params: &[],
                                ext_params: &[],
                                random_coeff_powers,
                            };
                            interpret_metal_evaluation_program_v1(comp.program, runtime_cpu)
                                .expect("CPU interpreter should not fail for GPU-fallback component")
                        }
                    };

                    let denom_start = Instant::now();
                    apply_denom_inv(&mut row_res, comp.log_size, comp.eval_domain_log_size);
                    denom_ms += denom_start.elapsed().as_secs_f64() * 1000.0;

                    quotients.push((comp.eval_domain_log_size, row_res));
                }

                // Phase 3c: Wait on all async JIT handles in submission order.
                let n_async = pending.len();
                let wait_start = Instant::now();
                for p in pending {
                    let comp = p.comp;
                    match complete_compiled_metal_evaluation_program_v1_async(p.handle, p.dst) {
                        Ok(mut row_res) => {
                            let denom_start = Instant::now();
                            apply_denom_inv(&mut row_res, comp.log_size, comp.eval_domain_log_size);
                            denom_ms += denom_start.elapsed().as_secs_f64() * 1000.0;
                            quotients.push((comp.eval_domain_log_size, row_res));
                        }
                        Err(ref e) => {
                            eprintln!(
                                "    [ASYNC WAIT FAIL] component '{}': {:?}",
                                comp.name, e,
                            );
                        }
                    }
                }
                wait_ms += wait_start.elapsed().as_secs_f64() * 1000.0;

                let kernel_ms = submit_ms + wait_ms;
                if n_async > 0 {
                    println!(
                        "      async pipeline: submit={:.1}ms wait={:.1}ms ({} async [{} gpu-trace], {} sync)",
                        submit_ms, wait_ms,
                        n_async, gpu_trace_ct, gpu_ct + cpu_ct
                    );
                }

                (quotients, gpu_ct, compiled_ct, cpu_ct, kernel_ms, denom_ms)
            });

            // Main thread: process SIMD components while GPU thread is busy.
            let mut simd_quotients: Vec<(u32, Vec<SecureField>)> = Vec::new();
            let mut simd_ct = 0usize;
            let mut simd_k_ms = 0.0f64;
            let mut simd_d_ms = 0.0f64;

            for comp in &simd_components {
                let interaction_refs: Vec<Vec<&[BaseField]>> = comp.interaction_cols
                    .iter()
                    .map(|cols| cols.iter().map(|c| c.as_slice()).collect())
                    .collect();
                let interaction_slice_refs: Vec<&[&[BaseField]]> =
                    interaction_refs.iter().map(|cols| cols.as_slice()).collect();
                let random_coeff_powers =
                    &all_random_coeff_powers[comp.coeff_start..comp.coeff_end];

                let runtime = MetalEvaluationProgramRuntimeInputsV1 {
                    trace: MetalEvaluationProgramTraceViewV1 {
                        trace_interactions: &interaction_slice_refs,
                        preprocessed_columns: &[],
                    },
                    base_params: &[],
                    ext_params: &[],
                    random_coeff_powers,
                };

                let kernel_start = Instant::now();
                simd_ct += 1;
                let mut row_res =
                    interpret_metal_evaluation_program_v1(comp.program, runtime)
                        .expect("SIMD interpreter should not fail");
                simd_k_ms += kernel_start.elapsed().as_secs_f64() * 1000.0;

                let denom_start = Instant::now();
                apply_denom_inv(&mut row_res, comp.log_size, comp.eval_domain_log_size);
                simd_d_ms += denom_start.elapsed().as_secs_f64() * 1000.0;

                simd_quotients.push((comp.eval_domain_log_size, row_res));
            }

            let (gpu_q, gpu_ct, compiled_ct, cpu_ct, gpu_k_ms, gpu_d_ms) =
                gpu_handle.join().unwrap();
            (
                gpu_q, simd_quotients,
                gpu_ct, compiled_ct, simd_ct, cpu_ct,
                gpu_k_ms, simd_k_ms, gpu_d_ms + simd_d_ms,
            )
        });

        // --- Phase 4: Merge and accumulate ---
        let mut quotient_results = gpu_quotients;
        quotient_results.extend(simd_quotients);

        let v1_ms = v1_start.elapsed().as_secs_f64() * 1000.0;
        println!(
            "    V1 execution: {:.1} ms ({} JIT-compiled, {} GPU interpreter, {} SIMD/CPU hybrid, {} CPU fallback)",
            v1_ms, compiled_count, gpu_count, simd_count, cpu_count
        );
        println!(
            "      Breakdown: jit_compile={:.1}ms, extract={:.1}ms, gpu_kernel={:.1}ms, simd_kernel={:.1}ms (overlapped), denom_inv={:.1}ms",
            total_jit_ms, total_extract_ms, gpu_kernel_ms, simd_kernel_ms, total_denom_ms
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
            None, // No GPU extraction for SimdBackend
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
        metal_twiddles: &stwo::prover::poly::twiddles::TwiddleTree<stwo_metal::MetalBackend>,
    ) -> stwo::prover::poly::circle::SecureCirclePoly<SimdBackend> {
        compute_metal_composition_poly_impl(
            results,
            |tree_idx, col_idx, domain| {
                extract_column_on_domain_metal(
                    &commitment_scheme.trees[tree_idx].polynomials[col_idx],
                    domain,
                    metal_twiddles,
                )
            },
            Some(&|tree_idx, col_idx, domain| {
                extract_column_on_domain_metal_gpu(
                    &commitment_scheme.trees[tree_idx].polynomials[col_idx],
                    domain,
                    metal_twiddles,
                )
            }),
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
    /// committed polynomial. Uses GPU RFFT to evaluate on the target domain,
    /// keeping coefficients on GPU and avoiding CPU-side polynomial evaluation.
    #[cfg(feature = "metal-runtime")]
    fn extract_column_on_domain_metal(
        poly: &stwo::prover::Poly<stwo_metal::MetalBackend>,
        eval_domain: stwo::core::poly::circle::CircleDomain,
        twiddles: &stwo::prover::poly::twiddles::TwiddleTree<stwo_metal::MetalBackend>,
    ) -> Vec<stwo::core::fields::m31::BaseField> {
        use stwo::prover::backend::Column;

        if poly.evals.domain == eval_domain {
            // Already on the right domain — download evaluations directly from GPU.
            return poly.evals.values.to_cpu();
        }

        // GPU evaluation: extend coefficients and RFFT on Metal.
        use stwo::prover::poly::circle::PolyOps;
        let coeffs_ref = poly.coeffs.as_ref()
            .expect("extract_column_on_domain_metal requires store_polynomials_coefficients=true");
        let eval = stwo_metal::MetalBackend::evaluate(coeffs_ref, eval_domain, twiddles);
        eval.values.to_cpu()
    }

    /// Extract polynomial evaluation on a target domain from a MetalBackend
    /// committed polynomial, returning a GPU-resident `BaseFieldVec` to avoid
    /// downloading to CPU.  Used for GPU+JIT composition dispatch.
    #[cfg(feature = "metal-runtime")]
    fn extract_column_on_domain_metal_gpu(
        poly: &stwo::prover::Poly<stwo_metal::MetalBackend>,
        eval_domain: stwo::core::poly::circle::CircleDomain,
        twiddles: &stwo::prover::poly::twiddles::TwiddleTree<stwo_metal::MetalBackend>,
    ) -> stwo_metal::MetalBaseFieldVec {
        if poly.evals.domain == eval_domain {
            // Already on the right domain — clone the GPU buffer (no CPU download).
            return poly.evals.values.clone();
        }

        // GPU evaluation: extend coefficients and RFFT on Metal.
        use stwo::prover::poly::circle::PolyOps;
        let coeffs_ref = poly.coeffs.as_ref()
            .expect("extract_column_on_domain_metal_gpu requires store_polynomials_coefficients=true");
        let eval = stwo_metal::MetalBackend::evaluate(coeffs_ref, eval_domain, twiddles);
        eval.values
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
    /// Cached Metal artifacts that are identical for all Cairo programs at the
    /// same trace size.  Building these once and reusing across bench iterations
    /// eliminates ~1,691ms of fixed preprocessing per run.
    #[cfg(feature = "metal-runtime")]
    struct CachedMetalArtifacts {
        twiddles: stwo::prover::poly::twiddles::TwiddleTree<stwo_metal::MetalBackend>,
        preprocessed_tree: CommitmentTreeProver<stwo_metal::MetalBackend, Blake2sMerkleChannel>,
    }

    /// Build the cached Metal artifacts (twiddles + preprocessed tree).
    /// This runs once and the result is reused across bench iterations.
    #[cfg(feature = "metal-runtime")]
    fn build_cached_metal_artifacts(
        preprocessed_trace: Arc<PreProcessedTrace>,
    ) -> CachedMetalArtifacts {
        use stwo::core::fields::m31::BaseField;
        use stwo::prover::backend::Column;
        use stwo::prover::poly::circle::PolyOps;
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

        // 1. Twiddles.
        let t_tw = Instant::now();
        eprintln!("  [cache] Precomputing Metal twiddles (log_domain_size={})...", max_domain_size);
        let twiddles = MetalBackend::precompute_twiddles(
            CanonicCoset::new(max_domain_size)
                .circle_domain()
                .half_coset,
        );
        let twiddles_ms = t_tw.elapsed().as_secs_f64() * 1000.0;

        // 2. Preprocessed coefficients (I/O or compute).
        let t_pp = Instant::now();
        let cached_cpu_polys = load_preprocessed_coeffs_cpu(PREPROCESSED_CACHE_PATH);
        let preprocessed_polys = if let Some(cpu_polys) = cached_cpu_polys {
            upload_coeffs_to_gpu(cpu_polys)
        } else {
            eprintln!("  [cache] Computing preprocessed coefficients (gen_trace + IFFT)...");
            let simd_preprocessed_evals = gen_trace(preprocessed_trace);
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
            let polys = MetalBackend::interpolate_columns(metal_preprocessed_evals, &twiddles);
            save_preprocessed_coeffs(&polys, PREPROCESSED_CACHE_PATH);
            polys
        };

        // 3. RFFT + Merkle → CommitmentTreeProver.
        let base_column_pool = BaseColumnPool::new();
        let preprocessed_evaluated = MetalBackend::evaluate_polynomials(
            preprocessed_polys,
            pcs_config.fri_config.log_blowup_factor,
            &twiddles,
            true,
            &base_column_pool,
        );
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
        let preprocessed_tree = CommitmentTreeProver {
            polynomials: preprocessed_evaluated,
            commitment: merkle_tree,
        };
        let cache_ms = t_pp.elapsed().as_secs_f64() * 1000.0;
        eprintln!(
            "  [cache] Built artifacts: twiddles={:.1}ms, preprocessed tree={:.1}ms",
            twiddles_ms, cache_ms,
        );

        CachedMetalArtifacts {
            twiddles,
            preprocessed_tree,
        }
    }

    fn run_metal_full_pipeline(
        input: ProverInput,
        preprocessed_trace: Arc<PreProcessedTrace>,
        verify: bool,
        input_for_simd_comparison: Option<ProverInput>,
        cached_artifacts: Option<&CachedMetalArtifacts>,
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
        let using_cache = cached_artifacts.is_some();

        println!("\n=== Full MetalBackend pipeline{} ===\n",
            if using_cache { " (cached twiddles+tree)" } else { "" });
        let pipeline_start = Instant::now();

        // When cached artifacts are available, reference them directly.
        // Otherwise fall through to the original build path.
        let (twiddles_ms, preproc_ms);

        // Owned twiddles + tree used only in the uncached path.  The cached
        // path borrows from `cached_artifacts` instead.
        let owned_twiddles;
        let owned_preprocessed_tree;

        if let Some(artifacts) = cached_artifacts {
            // Fast path: reuse pre-built twiddles and preprocessed tree.
            twiddles_ms = 0.0;
            preproc_ms = 0.0;
            owned_twiddles = None;
            owned_preprocessed_tree = None;
            let _ = &artifacts; // used below via metal_twiddles / preprocessed_tree_ref
        } else {
            // Slow path: build from scratch (used for single-shot --metal runs).
            let coeff_io_handle = std::thread::spawn(|| {
                let t = Instant::now();
                let result = load_preprocessed_coeffs_cpu(PREPROCESSED_CACHE_PATH);
                (result, t.elapsed())
            });

            let t_tw = Instant::now();
            println!("  Precomputing Metal twiddles (log_domain_size={})...", max_domain_size);
            let tw = MetalBackend::precompute_twiddles(
                CanonicCoset::new(max_domain_size)
                    .circle_domain()
                    .half_coset,
            );
            twiddles_ms = t_tw.elapsed().as_secs_f64() * 1000.0;

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
                    MetalBackend::interpolate_columns(metal_preprocessed_evals, &tw);
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

            // Build commitment tree: RFFT then Merkle.
            let base_column_pool_tmp = BaseColumnPool::new();
            let preprocessed_evaluated = MetalBackend::evaluate_polynomials(
                preprocessed_polys,
                pcs_config.fri_config.log_blowup_factor,
                &tw,
                true,
                &base_column_pool_tmp,
            );
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
            preproc_ms = t_pp.elapsed().as_secs_f64() * 1000.0;
            println!(
                "    total preprocessed={:.1}ms",
                preproc_ms,
            );
            owned_twiddles = Some(tw);
            owned_preprocessed_tree = Some(CommitmentTreeProver {
                polynomials: preprocessed_evaluated,
                commitment: merkle_tree,
            });
        }

        // Resolve references: either from cache or from owned values built above.
        let metal_twiddles: &_ = match cached_artifacts {
            Some(a) => &a.twiddles,
            None => owned_twiddles.as_ref().unwrap(),
        };

        // Spawn claim generator on background thread (CPU-only, overlaps with
        // commit below).  `input` is not used again after this point.
        let preprocessed_trace_for_claim = preprocessed_trace.clone();
        let claim_gen_handle = std::thread::spawn(move || {
            create_cairo_claim_generator(input, preprocessed_trace_for_claim)
        });

        // 3. MetalBackend commitment scheme.
        let base_column_pool = BaseColumnPool::new();
        let channel = &mut stwo::core::channel::Blake2sChannel::default();
        let channel_salt: u32 = 0;
        channel.mix_felts(&[SecureField::from(channel_salt)]);
        pcs_config.mix_into(channel);

        let mut commitment_scheme =
            CommitmentSchemeProver::<MetalBackend, Blake2sMerkleChannel>::with_memory_pool(
                pcs_config,
                metal_twiddles,
                &base_column_pool,
            );
        commitment_scheme.store_polynomials_coefficients = true;

        // Commit preprocessed tree: borrow from cache or take ownership of fresh build.
        match cached_artifacts {
            Some(a) => {
                commitment_scheme.commit_tree(
                    MaybeOwned::Borrowed(&a.preprocessed_tree),
                    channel,
                );
            }
            None => {
                commitment_scheme.commit_tree(
                    MaybeOwned::Owned(owned_preprocessed_tree.unwrap()),
                    channel,
                );
            }
        }

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
            &metal_twiddles,
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
        println!("=== Full MetalBackend pipeline results{} ===\n",
            if using_cache { " (cached)" } else { "" });
        println!("  Metal twiddles:                    {:>8.1} ms{}", twiddles_ms,
            if using_cache { " (cached)" } else { "" });
        println!("  Preprocessed tree (GPU):           {:>8.1} ms{}", preproc_ms,
            if using_cache { " (cached)" } else { "" });
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
