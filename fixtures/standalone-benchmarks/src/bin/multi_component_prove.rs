//! Multi-component composition benchmark (D8, DN-0012).
//!
//! Benchmarks `compute_composition_polynomial_multi_v1` with 3+ components at
//! different `log_n_rows`, producing JSON output with per-component breakdown.
//!
//! Environment variables:
//! - `LOG_N_MIN`: minimum log_n scale (default 8)
//! - `LOG_N_MAX`: maximum log_n scale (default 16)
//! - `N_COLUMNS`: columns per component (default 10)
//! - `N_COMPONENTS`: number of components (default 3)
//! - `SAMPLES`: timing samples per scale (default 3)

#[cfg(feature = "metal-runtime")]
use std::time::Instant;

use serde::Serialize;
#[cfg(feature = "metal-runtime")]
use stwo::core::fields::m31::BaseField;
#[cfg(feature = "metal-runtime")]
use stwo::core::fields::qm31::SecureField;
#[cfg(feature = "metal-runtime")]
use stwo_metal::program::{
    lower_wide_fibonacci_evaluation_program_v1, MetalEvaluationProgramBudgetV1,
    MetalEvaluationProgramDispatchKindV1, MetalEvaluationProgramSpecializationV1,
};
#[cfg(feature = "metal-runtime")]
use stwo_metal::prove_runtime::{
    compute_composition_polynomial_multi_v1, MetalComponentContextV1,
    MetalProveRuntimeContextMultiV1,
};

use stwo_metal_standalone_benchmarks::support::env_u32;
#[cfg(feature = "metal-runtime")]
use stwo_metal_standalone_benchmarks::support::summarize;

const DEFAULT_LOG_N_MIN: u32 = 8;
const DEFAULT_LOG_N_MAX: u32 = 16;
const DEFAULT_N_COLUMNS: u32 = 10;
const DEFAULT_N_COMPONENTS: u32 = 3;
const DEFAULT_SAMPLES: u32 = 3;

#[derive(Serialize)]
struct BenchmarkOutput {
    benchmark_id: String,
    results: Vec<ScaleResult>,
}

#[derive(Serialize)]
struct ScaleResult {
    log_n: u32,
    n_components: u32,
    components: Vec<ComponentDesc>,
    total_constraints: u32,
    composition_ms: f64,
    samples_ms: Vec<f64>,
    breakdown: CompositionBreakdown,
}

#[derive(Serialize)]
struct ComponentDesc {
    log_n_rows: u32,
    n_columns: u32,
    n_constraints: u32,
    dispatch: String,
}

#[derive(Serialize)]
struct CompositionBreakdown {
    twiddle_ms: f64,
    trace_extension_ms: f64,
    eval_program_ms: f64,
    quotient_application_ms: f64,
    interpolation_ms: f64,
}

#[cfg(feature = "metal-runtime")]
fn dispatch_name(kind: &MetalEvaluationProgramDispatchKindV1) -> String {
    match kind {
        MetalEvaluationProgramDispatchKindV1::GenericMetalInterpreter => {
            "GenericMetalInterpreter".to_string()
        }
        MetalEvaluationProgramDispatchKindV1::GeneratedOverlay(overlay) => {
            format!("GeneratedOverlay({})", overlay.name)
        }
        MetalEvaluationProgramDispatchKindV1::JitCompiled => {
            "JitCompiled".to_string()
        }
    }
}

/// Distribute component log_n_rows values evenly from (log_n - spread) to log_n,
/// where spread = n_components - 1, clamped so the smallest is at least 3
/// (minimum for valid wide-fibonacci with 3+ columns).
#[cfg(feature = "metal-runtime")]
fn component_log_n_rows_spread(log_n: u32, n_components: u32) -> Vec<u32> {
    let spread = n_components.saturating_sub(1);
    let min_log = if log_n > spread + 2 {
        log_n - spread
    } else {
        3
    };
    let step = if n_components > 1 {
        (log_n - min_log) as f64 / (n_components - 1) as f64
    } else {
        0.0
    };
    (0..n_components)
        .map(|i| {
            let val = min_log as f64 + step * i as f64;
            val.round() as u32
        })
        .collect()
}

#[cfg(feature = "metal-runtime")]
fn run_one_scale(
    log_n: u32,
    n_components: u32,
    n_columns: u32,
    samples: u32,
) -> ScaleResult {
    let log_n_rows_list = component_log_n_rows_spread(log_n, n_components);

    // Lower each component.
    let programs: Vec<_> = log_n_rows_list
        .iter()
        .map(|&log_n_rows| {
            let program = lower_wide_fibonacci_evaluation_program_v1(
                MetalEvaluationProgramSpecializationV1 {
                    log_n_rows,
                    n_columns,
                },
            )
            .expect("lowering should succeed");
            program
                .validate(MetalEvaluationProgramBudgetV1::new(2048, 512))
                .expect("program should validate");
            program
        })
        .collect();

    let components: Vec<MetalComponentContextV1> = programs
        .into_iter()
        .zip(log_n_rows_list.iter())
        .map(|(program, &log_n_rows)| MetalComponentContextV1::new(program, log_n_rows))
        .collect();
    let ctx = MetalProveRuntimeContextMultiV1::new(components);

    // Generate deterministic trace data for each component.
    let trace_storage: Vec<Vec<Vec<BaseField>>> = ctx
        .components
        .iter()
        .enumerate()
        .map(|(comp_idx, comp)| {
            let eval_domain_size = 1usize << (comp.log_n_rows + 1);
            (0..n_columns as usize)
                .map(|col_idx| {
                    (0..eval_domain_size)
                        .map(|row| {
                            BaseField::from_u32_unchecked(
                                ((comp_idx * 997 + col_idx * 7 + row * 13 + 42)
                                    % ((1u32 << 31) - 1) as usize) as u32,
                            )
                        })
                        .collect()
                })
                .collect()
        })
        .collect();

    let trace_refs: Vec<Vec<&[BaseField]>> = trace_storage
        .iter()
        .map(|comp_cols| comp_cols.iter().map(|col| col.as_slice()).collect())
        .collect();

    let random_coeff = SecureField::from_u32_unchecked(3, 5, 7, 11);

    // Run samples.
    let mut samples_ms = Vec::with_capacity(samples as usize);
    let mut last_dispatches: Vec<MetalEvaluationProgramDispatchKindV1> = Vec::new();
    let mut last_breakdown = None;

    for _ in 0..samples {
        let start = Instant::now();
        let (_, dispatches, breakdown) =
            compute_composition_polynomial_multi_v1(&ctx, &trace_refs, random_coeff)
                .expect("multi-component composition should succeed");
        let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
        samples_ms.push(elapsed_ms);
        last_dispatches = dispatches;
        last_breakdown = Some(breakdown);
    }

    let summary = summarize(&samples_ms);
    let breakdown = last_breakdown.expect("at least one sample");

    let component_descs: Vec<ComponentDesc> = ctx
        .components
        .iter()
        .zip(last_dispatches.iter())
        .map(|(comp, dispatch)| ComponentDesc {
            log_n_rows: comp.log_n_rows,
            n_columns,
            n_constraints: comp.n_constraints(),
            dispatch: dispatch_name(dispatch),
        })
        .collect();

    ScaleResult {
        log_n,
        n_components,
        components: component_descs,
        total_constraints: ctx.total_constraints(),
        composition_ms: summary.median,
        samples_ms,
        breakdown: CompositionBreakdown {
            twiddle_ms: breakdown.twiddle_ms,
            trace_extension_ms: breakdown.trace_extension_ms,
            eval_program_ms: breakdown.eval_program_ms,
            quotient_application_ms: breakdown.quotient_application_ms,
            interpolation_ms: breakdown.interpolation_ms,
        },
    }
}

fn main() {
    let log_n_min = env_u32("LOG_N_MIN", DEFAULT_LOG_N_MIN);
    let log_n_max = env_u32("LOG_N_MAX", DEFAULT_LOG_N_MAX);
    let n_columns = env_u32("N_COLUMNS", DEFAULT_N_COLUMNS);
    let n_components = env_u32("N_COMPONENTS", DEFAULT_N_COMPONENTS);
    let samples = env_u32("SAMPLES", DEFAULT_SAMPLES);

    eprintln!(
        "multi_component_prove benchmark: LOG_N_MIN={log_n_min} LOG_N_MAX={log_n_max} \
         N_COLUMNS={n_columns} N_COMPONENTS={n_components} SAMPLES={samples}"
    );

    #[cfg(not(feature = "metal-runtime"))]
    {
        eprintln!("metal-runtime feature not enabled, outputting empty result");
        let output = BenchmarkOutput {
            benchmark_id: "multi_component_composition_v1".to_string(),
            results: Vec::new(),
        };
        println!(
            "{}",
            serde_json::to_string_pretty(&output).expect("serialize")
        );
        return;
    }

    #[cfg(feature = "metal-runtime")]
    {
        let mut results = Vec::new();

        for log_n in log_n_min..=log_n_max {
            eprintln!("  log_n={log_n} ...");
            let result = run_one_scale(log_n, n_components, n_columns, samples);
            eprintln!(
                "    composition_ms={:.1} total_constraints={}",
                result.composition_ms, result.total_constraints
            );
            results.push(result);
        }

        let output = BenchmarkOutput {
            benchmark_id: "multi_component_composition_v1".to_string(),
            results,
        };

        println!(
            "{}",
            serde_json::to_string_pretty(&output).expect("serialize")
        );
    }
}
