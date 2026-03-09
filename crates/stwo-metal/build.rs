use std::env;

const CUDA_MODE_VAR: &str = "STWO_CUDA_MODE";

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
}

fn main() {
    println!("cargo:rustc-check-cfg=cfg(stwo_cuda_link)");
    println!("cargo:rerun-if-env-changed={CUDA_MODE_VAR}");

    if CudaMode::from_env() != CudaMode::NoCuda {
        println!("cargo:rustc-cfg=stwo_cuda_link");
    }
}
