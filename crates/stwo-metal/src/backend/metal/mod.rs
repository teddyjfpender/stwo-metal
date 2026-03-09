mod backend;
mod capability;
mod column;
mod fri;
mod poly;

pub use backend::MetalBackend;
pub use capability::{
    metal_backend_surface_detail, metal_backend_surface_status, metal_runtime_error,
    metal_runtime_support, MetalBackendSurface, MetalBackendSurfaceStatus, MetalRuntimeSupport,
    STWO_METAL_BACKEND_SURFACES_V1,
};
pub use fri::{fold_circle_into_line_first_layer, fold_line};
pub use poly::permute_coset_to_circle_domain_bit_reversed;

pub use crate::stwo_metal::{
    BaseFieldVec as MetalBaseFieldVec, SecureFieldVec as MetalSecureFieldVec,
};
