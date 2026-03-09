use stwo_metal_sys::metal;

#[non_exhaustive]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum MetalBackendSurface {
    BaseFieldColumnSet,
    BaseFieldColumnFromIterator,
    BaseFieldColumnBitReverse,
    BaseFieldCosetToCircleDomainBitReverse,
    SecureFieldColumnSet,
    SecureFieldColumnFromIterator,
    SecureFieldColumnBitReverse,
    QuotientAccumulate,
}

#[non_exhaustive]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum MetalBackendSurfaceStatus {
    Supported,
    UnsupportedPlanned,
}

pub use metal::MetalRuntimeSupport;

pub const STWO_METAL_BACKEND_SURFACES_V1: &[MetalBackendSurface] = &[
    MetalBackendSurface::BaseFieldColumnSet,
    MetalBackendSurface::BaseFieldColumnFromIterator,
    MetalBackendSurface::BaseFieldColumnBitReverse,
    MetalBackendSurface::BaseFieldCosetToCircleDomainBitReverse,
    MetalBackendSurface::SecureFieldColumnSet,
    MetalBackendSurface::SecureFieldColumnFromIterator,
    MetalBackendSurface::SecureFieldColumnBitReverse,
    MetalBackendSurface::QuotientAccumulate,
];

pub const fn metal_backend_surface_status(
    surface: MetalBackendSurface,
) -> MetalBackendSurfaceStatus {
    match surface {
        MetalBackendSurface::BaseFieldColumnSet
        | MetalBackendSurface::BaseFieldColumnFromIterator
        | MetalBackendSurface::BaseFieldColumnBitReverse
        | MetalBackendSurface::BaseFieldCosetToCircleDomainBitReverse
        | MetalBackendSurface::SecureFieldColumnSet
        | MetalBackendSurface::SecureFieldColumnFromIterator
        | MetalBackendSurface::SecureFieldColumnBitReverse => MetalBackendSurfaceStatus::Supported,
        MetalBackendSurface::QuotientAccumulate => MetalBackendSurfaceStatus::UnsupportedPlanned,
    }
}

pub const fn metal_backend_surface_detail(surface: MetalBackendSurface) -> &'static str {
    match surface {
        MetalBackendSurface::BaseFieldColumnSet => "Base-field Metal column mutation is supported.",
        MetalBackendSurface::BaseFieldColumnFromIterator => {
            "Base-field Metal column construction from iterators is supported."
        }
        MetalBackendSurface::BaseFieldColumnBitReverse => {
            "Base-field bit reversal is implemented through a native Metal kernel."
        }
        MetalBackendSurface::BaseFieldCosetToCircleDomainBitReverse => {
            "Base-field coset-order to circle-domain bit-reversed permutation is implemented through a native Metal kernel."
        }
        MetalBackendSurface::SecureFieldColumnSet => {
            "Secure-field Metal column mutation is supported."
        }
        MetalBackendSurface::SecureFieldColumnFromIterator => {
            "Secure-field Metal column construction from iterators is supported."
        }
        MetalBackendSurface::SecureFieldColumnBitReverse => {
            "Secure-field bit reversal is implemented through a native Metal kernel."
        }
        MetalBackendSurface::QuotientAccumulate => {
            "Constraint quotient accumulation remains in the planned Metal migration set."
        }
    }
}

pub fn metal_runtime_support() -> MetalRuntimeSupport {
    metal::metal_runtime_support()
}

pub fn metal_runtime_error() -> Option<String> {
    metal::metal_runtime_error()
}
