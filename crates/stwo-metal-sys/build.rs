use std::path::{Path, PathBuf};
use std::process::Command;
use std::{env, fs};

const CUDA_MODE_VAR: &str = "STWO_CUDA_MODE";
const METAL_MODE_VAR: &str = "STWO_METAL_MODE";
const METAL_SOURCES: &[&str] = &[
    "fields",
    "twiddles",
    "eval_program_v1",
    "eval_at_point",
    "poly_utils",
    "rfft",
    "ifft",
    "quotients",
    "fold_circle_into_line",
    "fold_line",
    "fri_decompose",
    "mle",
    "gkr",
    "prefix_sum",
    "bit_reverse",
    "poly_order",
    "wide_fibonacci",
    "blake2s",
    "grind",
    "witness_memory_id_to_big",
    "witness_memory_addr_to_id",
    "interaction_trace_id_to_big",
    "interaction_trace_addr_to_id",
    "witness_range_check",
    "witness_verify_instruction",
    "witness_opcodes",
    "merkle_decommit",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CudaMode {
    NoCuda,
    CudaDev,
    CudaCi,
}

impl CudaMode {
    fn from_env() -> Self {
        match env::var(CUDA_MODE_VAR) {
            Ok(value) => match value.as_str() {
                "no-cuda" => Self::NoCuda,
                "cuda-dev" => Self::CudaDev,
                "cuda-ci" => Self::CudaCi,
                other => panic!(
                    "Unsupported {CUDA_MODE_VAR} value '{other}'. Supported values: no-cuda, cuda-dev, cuda-ci."
                ),
            },
            Err(_) => Self::default_for_host(),
        }
    }

    fn default_for_host() -> Self {
        if cfg!(target_os = "linux") {
            Self::CudaDev
        } else {
            Self::NoCuda
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::NoCuda => "no-cuda",
            Self::CudaDev => "cuda-dev",
            Self::CudaCi => "cuda-ci",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MetalMode {
    NoMetal,
    MetalDev,
    MetalCi,
    MetalProd,
}

impl MetalMode {
    fn from_env() -> Self {
        match env::var(METAL_MODE_VAR) {
            Ok(value) => match value.as_str() {
                "no-metal" => Self::NoMetal,
                "metal-dev" => Self::MetalDev,
                "metal-ci" => Self::MetalCi,
                "metal-prod" => Self::MetalProd,
                other => panic!(
                    "Unsupported {METAL_MODE_VAR} value '{other}'. Supported values: no-metal, metal-dev, metal-ci, metal-prod."
                ),
            },
            Err(_) => Self::default_for_host(),
        }
    }

    fn default_for_host() -> Self {
        if cfg!(target_os = "macos") && cfg!(target_arch = "aarch64") {
            Self::MetalDev
        } else {
            Self::NoMetal
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::NoMetal => "no-metal",
            Self::MetalDev => "metal-dev",
            Self::MetalCi => "metal-ci",
            Self::MetalProd => "metal-prod",
        }
    }
}

fn main() {
    println!("cargo:rustc-check-cfg=cfg(stwo_cuda_link)");
    println!("cargo:rustc-check-cfg=cfg(stwo_metal_link)");
    println!("cargo:rerun-if-changed=cuda");
    println!("cargo:rerun-if-changed=cuda/CMakeLists.txt");
    println!("cargo:rerun-if-changed=metal");
    println!("cargo:rerun-if-changed=metal/runtime.m");
    println!("cargo:rerun-if-changed=metal/fields.metal");
    println!("cargo:rerun-if-changed=metal/twiddles.metal");
    println!("cargo:rerun-if-changed=metal/eval_program_v1.metal");
    println!("cargo:rerun-if-changed=metal/eval_at_point.metal");
    println!("cargo:rerun-if-changed=metal/poly_utils.metal");
    println!("cargo:rerun-if-changed=metal/rfft.metal");
    println!("cargo:rerun-if-changed=metal/ifft.metal");
    println!("cargo:rerun-if-changed=metal/quotients.metal");
    println!("cargo:rerun-if-changed=metal/fold_circle_into_line.metal");
    println!("cargo:rerun-if-changed=metal/fold_line.metal");
    println!("cargo:rerun-if-changed=metal/fri_decompose.metal");
    println!("cargo:rerun-if-changed=metal/mle.metal");
    println!("cargo:rerun-if-changed=metal/gkr.metal");
    println!("cargo:rerun-if-changed=metal/prefix_sum.metal");
    println!("cargo:rerun-if-changed=metal/bit_reverse.metal");
    println!("cargo:rerun-if-changed=metal/poly_order.metal");
    println!("cargo:rerun-if-changed=metal/wide_fibonacci.metal");
    println!("cargo:rerun-if-changed=metal/blake2s.metal");
    println!("cargo:rerun-if-changed=metal/grind.metal");
    println!("cargo:rerun-if-changed=metal/witness_memory_id_to_big.metal");
    println!("cargo:rerun-if-changed=metal/witness_memory_addr_to_id.metal");
    println!("cargo:rerun-if-changed=metal/interaction_trace_id_to_big.metal");
    println!("cargo:rerun-if-changed=metal/interaction_trace_addr_to_id.metal");
    println!("cargo:rerun-if-changed=metal/witness_range_check.metal");
    println!("cargo:rerun-if-changed=metal/witness_verify_instruction.metal");
    println!("cargo:rerun-if-changed=metal/witness_opcodes.metal");
    println!("cargo:rerun-if-changed=metal/merkle_decommit.metal");
    for var in [
        CUDA_MODE_VAR,
        METAL_MODE_VAR,
        "CUDA_LIB_DIR",
        "CUDAToolkit_ROOT",
        "CUDA_HOME",
        "CUDA_PATH",
        "DEVELOPER_DIR",
        "NUM_JOBS",
    ] {
        println!("cargo:rerun-if-env-changed={var}");
    }

    let metal_mode = MetalMode::from_env();
    println!(
        "cargo:rustc-env=STWO_METAL_BUILD_MODE={}",
        metal_mode.as_str()
    );
    println!(
        "cargo:warning=Building stwo with Metal host mode '{}'.",
        metal_mode.as_str()
    );
    write_metal_autogen_stub();
    if metal_mode != MetalMode::NoMetal {
        build_metal_support(metal_mode);
    } else {
        println!(
            "cargo:warning=Skipping Metal native build and link setup because {METAL_MODE_VAR}=no-metal."
        );
    }

    let mode = CudaMode::from_env();
    println!(
        "cargo:warning=Building stwo with CUDA host mode '{}'.",
        mode.as_str()
    );

    if mode == CudaMode::NoCuda {
        println!(
            "cargo:warning=Skipping CUDA native build and link setup because {CUDA_MODE_VAR}=no-cuda."
        );
        return;
    }

    println!("cargo:rustc-cfg=stwo_cuda_link");

    let build_jobs = env::var("NUM_JOBS").unwrap_or_else(|_| "16".to_owned());
    let dst = cmake::Config::new("cuda")
        .profile("Release")
        .build_arg(format!("--jobs={build_jobs}"))
        .build();
    println!("cargo:rustc-link-search=native={}", dst.display());
    println!("cargo:rustc-link-lib=static=stwo_cuda");

    match cuda_lib_dir() {
        Some(path) => println!("cargo:rustc-link-search=native={}", path.display()),
        None if mode == CudaMode::CudaCi => {
            panic!(
                "Could not determine a CUDA runtime library directory. Set one of CUDA_LIB_DIR, CUDAToolkit_ROOT, CUDA_HOME, or CUDA_PATH for {CUDA_MODE_VAR}=cuda-ci."
            )
        }
        None => {
            println!(
                "cargo:warning=Could not determine CUDA runtime library directory from environment; relying on the system linker search path."
            );
        }
    }

    println!("cargo:rustc-link-lib=cudart");
    #[cfg(target_os = "linux")]
    println!("cargo:rustc-link-lib=stdc++");
}

fn build_metal_support(mode: MetalMode) {
    if !cfg!(target_os = "macos") {
        panic!(
            "{METAL_MODE_VAR}={} requires a macOS host toolchain.",
            mode.as_str()
        );
    }

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR must be set"));
    let metallib_path = out_dir.join("stwo-metal.metallib");
    let air_paths: Vec<PathBuf> = METAL_SOURCES
        .iter()
        .map(|source| {
            let air_path = out_dir.join(format!("{source}.air"));
            let mut command = metal_compile_command(source, &air_path, mode);
            run_command(&mut command, "compile Metal kernels", mode);
            air_path
        })
        .collect();
    let mut metallib_command = Command::new("xcrun");
    metallib_command.arg("metallib");
    for air_path in &air_paths {
        metallib_command.arg(air_path);
    }
    metallib_command.arg("-o").arg(&metallib_path);
    run_command(&mut metallib_command, "link Metal library", mode);
    write_metal_autogen(&metallib_path);

    cc::Build::new()
        .file("metal/runtime.m")
        .flag("-fobjc-arc")
        .flag("-fblocks")
        .compile("stwo_metal_runtime");

    println!("cargo:rustc-cfg=stwo_metal_link");
    println!("cargo:rustc-link-lib=framework=Foundation");
    println!("cargo:rustc-link-lib=framework=Metal");
    println!("cargo:rustc-link-lib=objc");
}

fn run_command(command: &mut Command, action: &str, mode: MetalMode) {
    let output = command.output().unwrap_or_else(|error| {
        panic!("Failed to {action}: could not launch command: {error}");
    });

    if output.status.success() {
        return;
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let detail = format!(
        "stdout:\n{stdout}\n\nstderr:\n{stderr}",
        stdout = stdout.trim(),
        stderr = stderr.trim()
    );

    match mode {
        MetalMode::MetalCi => panic!("Failed to {action} in CI mode.\n{detail}"),
        MetalMode::MetalDev => {
            panic!("Failed to {action} in dev mode.\n{detail}");
        }
        MetalMode::MetalProd => {
            panic!("Failed to {action} in production mode.\n{detail}");
        }
        MetalMode::NoMetal => unreachable!("command runner is not used in no-metal mode"),
    }
}

fn metal_compile_command(source: &str, air_path: &Path, mode: MetalMode) -> Command {
    let mut command = Command::new("xcrun");
    command.arg("metal").arg("-c");

    if mode == MetalMode::MetalProd {
        // Production mode keeps arithmetic semantics explicit and stable.
        command
            .arg("-fmetal-math-mode=safe")
            .arg("-fmetal-math-fp32-functions=precise");
    }

    command
        .arg(format!("metal/{source}.metal"))
        .arg("-o")
        .arg(air_path);
    command
}

fn write_metal_autogen_stub() {
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR must be set"));
    let generated = out_dir.join("metal_autogen.rs");
    fs::write(
        generated,
        "pub const STWO_METAL_KERNEL_LIBRARY: &[u8] = &[];\n\
         pub const STWO_METAL_BIT_REVERSE_U32_KERNEL: &str = \"bit_reverse_u32\";\n\
         pub const STWO_METAL_BIT_REVERSE_U32X4_KERNEL: &str = \"bit_reverse_u32x4\";\n\
         pub const STWO_METAL_INVERT_M31_VALUES_U32_KERNEL: &str = \"invert_m31_values_u32\";\n\
         pub const STWO_METAL_PRECOMPUTE_TWIDDLE_LEVEL_U32_KERNEL: &str = \"precompute_twiddle_level_u32\";\n\
         pub const STWO_METAL_RESCALE_M31_VALUES_U32_KERNEL: &str = \"rescale_m31_values_u32\";\n\
         pub const STWO_METAL_RFFT_CIRCLE_PART_U32_KERNEL: &str = \"rfft_circle_part_u32\";\n\
         pub const STWO_METAL_RFFT_LINE_PART_U32_KERNEL: &str = \"rfft_line_part_u32\";\n\
         pub const STWO_METAL_IFFT_CIRCLE_PART_U32_KERNEL: &str = \"ifft_circle_part_u32\";\n\
         pub const STWO_METAL_IFFT_LINE_PART_U32_KERNEL: &str = \"ifft_line_part_u32\";\n\
         pub const STWO_METAL_PERMUTE_COSET_TO_CIRCLE_DOMAIN_BIT_REVERSED_U32_KERNEL: &str = \"permute_coset_to_circle_domain_bit_reversed_u32\";\n\
         pub const STWO_METAL_FRI_FOLD_CIRCLE_INTO_LINE_FIRST_LAYER_U32X4_KERNEL: &str = \"fri_fold_circle_into_line_first_layer_u32x4\";\n\
         pub const STWO_METAL_FRI_FOLD_LINE_STEP_U32X4_KERNEL: &str = \"fri_fold_line_step_u32x4\";\n",
    )
    .expect("write metal autogen stub");
}

fn write_metal_autogen(metallib_path: &Path) {
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR must be set"));
    let generated = out_dir.join("metal_autogen.rs");
    let contents = format!(
        "pub const STWO_METAL_KERNEL_LIBRARY: &[u8] = include_bytes!(r#\"{}\"#);\n\
         pub const STWO_METAL_BIT_REVERSE_U32_KERNEL: &str = \"bit_reverse_u32\";\n\
         pub const STWO_METAL_BIT_REVERSE_U32X4_KERNEL: &str = \"bit_reverse_u32x4\";\n\
         pub const STWO_METAL_INVERT_M31_VALUES_U32_KERNEL: &str = \"invert_m31_values_u32\";\n\
         pub const STWO_METAL_PRECOMPUTE_TWIDDLE_LEVEL_U32_KERNEL: &str = \"precompute_twiddle_level_u32\";\n\
         pub const STWO_METAL_RESCALE_M31_VALUES_U32_KERNEL: &str = \"rescale_m31_values_u32\";\n\
         pub const STWO_METAL_RFFT_CIRCLE_PART_U32_KERNEL: &str = \"rfft_circle_part_u32\";\n\
         pub const STWO_METAL_RFFT_LINE_PART_U32_KERNEL: &str = \"rfft_line_part_u32\";\n\
         pub const STWO_METAL_IFFT_CIRCLE_PART_U32_KERNEL: &str = \"ifft_circle_part_u32\";\n\
         pub const STWO_METAL_IFFT_LINE_PART_U32_KERNEL: &str = \"ifft_line_part_u32\";\n\
         pub const STWO_METAL_PERMUTE_COSET_TO_CIRCLE_DOMAIN_BIT_REVERSED_U32_KERNEL: &str = \"permute_coset_to_circle_domain_bit_reversed_u32\";\n\
         pub const STWO_METAL_FRI_FOLD_CIRCLE_INTO_LINE_FIRST_LAYER_U32X4_KERNEL: &str = \"fri_fold_circle_into_line_first_layer_u32x4\";\n\
         pub const STWO_METAL_FRI_FOLD_LINE_STEP_U32X4_KERNEL: &str = \"fri_fold_line_step_u32x4\";\n",
        metallib_path.display()
    );
    fs::write(generated, contents).expect("write metal autogen file");
}

fn cuda_lib_dir() -> Option<PathBuf> {
    env::var_os("CUDA_LIB_DIR")
        .map(PathBuf::from)
        .filter(|path| path.exists())
        .or_else(|| {
            for root_var in ["CUDAToolkit_ROOT", "CUDA_HOME", "CUDA_PATH"] {
                if let Some(root) = env::var_os(root_var).map(PathBuf::from) {
                    if let Some(path) = candidate_cuda_lib_dir(&root) {
                        return Some(path);
                    }
                }
            }
            None
        })
}

fn candidate_cuda_lib_dir(root: &Path) -> Option<PathBuf> {
    [
        root.join("lib64"),
        root.join("lib"),
        root.join("targets/x86_64-linux/lib"),
    ]
    .into_iter()
    .find(|path| path.exists())
}
