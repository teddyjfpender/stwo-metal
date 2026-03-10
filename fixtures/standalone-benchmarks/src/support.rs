use std::env;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

#[derive(Serialize)]
pub struct RunnerMetadata {
    pub runner_class: String,
    pub hostname: String,
    pub os: String,
    pub arch: String,
    pub cargo_profile: String,
    pub cpu: String,
    pub threads: usize,
    pub gpu_name: String,
    pub gpu_driver: String,
    pub cuda_toolkit: String,
    pub stwo_cuda_mode: String,
    pub stwo_metal_mode: String,
}

#[derive(Clone, Copy, Serialize)]
pub struct SummaryStats {
    pub min: f64,
    pub max: f64,
    pub mean: f64,
    pub median: f64,
    pub stddev: f64,
}

pub fn required_env_path(key: &str) -> PathBuf {
    PathBuf::from(env::var(key).unwrap_or_else(|_| panic!("missing required env: {key}")))
}

pub fn env_or(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_string())
}

pub fn env_u32(key: &str, default: u32) -> u32 {
    env::var(key)
        .ok()
        .map(|value| value.parse::<u32>().unwrap_or_else(|_| panic!("invalid {key}")))
        .unwrap_or(default)
}

pub fn env_usize(key: &str, default: usize) -> usize {
    env::var(key)
        .ok()
        .map(|value| {
            value
                .parse::<usize>()
                .unwrap_or_else(|_| panic!("invalid {key}"))
        })
        .unwrap_or(default)
}

pub fn env_flag(key: &str) -> bool {
    matches!(
        env::var(key).as_deref(),
        Ok("1") | Ok("true") | Ok("TRUE") | Ok("yes") | Ok("YES")
    )
}

pub fn epoch_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time went backwards")
        .as_millis()
}

pub fn cargo_profile() -> &'static str {
    if cfg!(debug_assertions) {
        "dev"
    } else {
        "release"
    }
}

pub fn runner_metadata() -> RunnerMetadata {
    let default_threads = std::thread::available_parallelism()
        .map(|parallelism| parallelism.get())
        .unwrap_or(1);
    RunnerMetadata {
        runner_class: env_or("STWO_BENCH_RUNNER_CLASS", "unspecified"),
        hostname: env_or("STWO_BENCH_HOSTNAME", "unknown"),
        os: env_or("STWO_BENCH_OS", env::consts::OS),
        arch: env_or("STWO_BENCH_ARCH", env::consts::ARCH),
        cargo_profile: cargo_profile().to_string(),
        cpu: env_or("STWO_BENCH_CPU", "unknown"),
        threads: env_usize("STWO_BENCH_THREADS", default_threads),
        gpu_name: env_or("STWO_BENCH_GPU_NAME", "unknown"),
        gpu_driver: env_or("STWO_BENCH_GPU_DRIVER", "unknown"),
        cuda_toolkit: env_or("STWO_BENCH_CUDA_TOOLKIT", "unknown"),
        stwo_cuda_mode: env_or("STWO_CUDA_MODE", "no-cuda"),
        stwo_metal_mode: env_or("STWO_METAL_MODE", "no-metal"),
    }
}

pub fn enforce_metal_benchmark_contract(
    benchmark_id: &str,
    plan_only: bool,
    runner: &RunnerMetadata,
) {
    if plan_only {
        return;
    }

    if cargo_profile() != "release" && !env_flag("STWO_BENCH_ALLOW_NON_RELEASE") {
        panic!(
            "{benchmark_id} requires a release build profile for non-plan execution. Re-run with `cargo run --release ...` or set STWO_BENCH_ALLOW_NON_RELEASE=1 to override."
        );
    }

    if runner.stwo_metal_mode == "metal-dev" && !env_flag("STWO_BENCH_ALLOW_DEV_METAL") {
        panic!(
            "{benchmark_id} requires STWO_METAL_MODE=metal-prod for non-plan benchmark measurements. Set STWO_METAL_MODE=metal-prod or STWO_BENCH_ALLOW_DEV_METAL=1 to override."
        );
    }

    if runner.stwo_metal_mode == "no-metal" {
        panic!("{benchmark_id} cannot run with STWO_METAL_MODE=no-metal.");
    }
}

pub fn summarize(samples_ms: &[f64]) -> SummaryStats {
    let mut sorted = samples_ms.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).expect("finite samples"));

    let len = sorted.len();
    let min = *sorted.first().expect("non-empty samples");
    let max = *sorted.last().expect("non-empty samples");
    let mean = sorted.iter().sum::<f64>() / len as f64;
    let median = if len % 2 == 0 {
        (sorted[len / 2 - 1] + sorted[len / 2]) / 2.0
    } else {
        sorted[len / 2]
    };
    let variance = sorted
        .iter()
        .map(|sample| {
            let delta = sample - mean;
            delta * delta
        })
        .sum::<f64>()
        / len as f64;

    SummaryStats {
        min,
        max,
        mean,
        median,
        stddev: variance.sqrt(),
    }
}

pub fn write_json<T: Serialize>(output_path: &Path, result: &T) {
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).expect("create benchmark output directory");
    }
    let json = serde_json::to_string_pretty(result).expect("serialize benchmark result");
    fs::write(output_path, json).expect("write benchmark result");
}
