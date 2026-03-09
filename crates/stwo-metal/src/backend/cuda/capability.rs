#[non_exhaustive]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum CudaBackendSurface {
    SecureFieldColumnSet,
    SecureFieldColumnFromIterator,
    Blake2sHashColumnSet,
    Blake2sHashColumnFromIterator,
    Blake2sHashColumnBitReverse,
    Poseidon252HashColumnBitReverse,
    FriDecompose,
    GkrNextLayer,
    GkrSumAsPolyInFirstVariable,
}

#[non_exhaustive]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum CudaBackendSurfaceStatus {
    Supported,
    UnsupportedDeprecated,
    UnsupportedPlanned,
}

pub const STWO_CUDA_BACKEND_SURFACES_V1: &[CudaBackendSurface] = &[
    CudaBackendSurface::SecureFieldColumnSet,
    CudaBackendSurface::SecureFieldColumnFromIterator,
    CudaBackendSurface::Blake2sHashColumnSet,
    CudaBackendSurface::Blake2sHashColumnFromIterator,
    CudaBackendSurface::Blake2sHashColumnBitReverse,
    CudaBackendSurface::Poseidon252HashColumnBitReverse,
    CudaBackendSurface::FriDecompose,
    CudaBackendSurface::GkrNextLayer,
    CudaBackendSurface::GkrSumAsPolyInFirstVariable,
];

pub const fn cuda_backend_surface_status(surface: CudaBackendSurface) -> CudaBackendSurfaceStatus {
    match surface {
        CudaBackendSurface::SecureFieldColumnSet
        | CudaBackendSurface::SecureFieldColumnFromIterator
        | CudaBackendSurface::Blake2sHashColumnSet
        | CudaBackendSurface::Blake2sHashColumnFromIterator
        | CudaBackendSurface::GkrNextLayer
        | CudaBackendSurface::GkrSumAsPolyInFirstVariable => CudaBackendSurfaceStatus::Supported,
        CudaBackendSurface::Blake2sHashColumnBitReverse
        | CudaBackendSurface::Poseidon252HashColumnBitReverse => {
            CudaBackendSurfaceStatus::UnsupportedDeprecated
        }
        CudaBackendSurface::FriDecompose => CudaBackendSurfaceStatus::UnsupportedDeprecated,
    }
}

pub const fn cuda_backend_surface_detail(surface: CudaBackendSurface) -> &'static str {
    match surface {
        CudaBackendSurface::SecureFieldColumnSet => {
            "Secure-field device column mutation is supported."
        }
        CudaBackendSurface::SecureFieldColumnFromIterator => {
            "Secure-field column construction from iterators is supported."
        }
        CudaBackendSurface::Blake2sHashColumnSet => {
            "Blake2s hash-column element mutation is supported."
        }
        CudaBackendSurface::Blake2sHashColumnFromIterator => {
            "Blake2s hash-column construction from iterators is supported."
        }
        CudaBackendSurface::Blake2sHashColumnBitReverse => {
            "Blake2s hash-column bit reversal is outside the supported companion capability claim."
        }
        CudaBackendSurface::Poseidon252HashColumnBitReverse => {
            "Poseidon252 hash-column bit reversal is outside the supported companion capability claim."
        }
        CudaBackendSurface::FriDecompose => {
            "FRI decompose is deprecated upstream and is not part of supported Stwo CUDA flows."
        }
        CudaBackendSurface::GkrNextLayer => "GKR next-layer evaluation is supported.",
        CudaBackendSurface::GkrSumAsPolyInFirstVariable => {
            "GKR univariate sum projection is supported."
        }
    }
}

pub const fn cuda_backend_surface_supported(surface: CudaBackendSurface) -> bool {
    matches!(
        cuda_backend_surface_status(surface),
        CudaBackendSurfaceStatus::Supported
    )
}

pub(crate) fn panic_for_unsupported_surface(
    surface: CudaBackendSurface,
    operation: &'static str,
) -> ! {
    let status = cuda_backend_surface_status(surface);
    let detail = cuda_backend_surface_detail(surface);

    match status {
        CudaBackendSurfaceStatus::Supported => {
            panic!(
                "CudaBackend internal error: capability helper used for supported surface {surface:?} during {operation}"
            )
        }
        CudaBackendSurfaceStatus::UnsupportedPlanned
        | CudaBackendSurfaceStatus::UnsupportedDeprecated => {
            panic!(
                "CudaBackend unsupported surface during {operation}: {surface:?} is {status:?}. {detail}"
            )
        }
    }
}
