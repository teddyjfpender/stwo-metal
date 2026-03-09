use std::collections::HashSet;

use stwo_metal::{
    cuda_backend_surface_detail, cuda_backend_surface_status, CudaBackendSurface,
    CudaBackendSurfaceStatus, STWO_CUDA_BACKEND_SURFACES_V1,
};

#[test]
fn tracked_cuda_backend_surfaces_v1_are_unique() {
    let surfaces: HashSet<_> = STWO_CUDA_BACKEND_SURFACES_V1.iter().copied().collect();

    assert_eq!(surfaces.len(), STWO_CUDA_BACKEND_SURFACES_V1.len());
}

#[test]
fn fri_decompose_is_classified_as_unsupported_deprecated() {
    assert_eq!(
        cuda_backend_surface_status(CudaBackendSurface::FriDecompose),
        CudaBackendSurfaceStatus::UnsupportedDeprecated
    );
    assert!(
        cuda_backend_surface_detail(CudaBackendSurface::FriDecompose)
            .contains("deprecated upstream")
    );
}

#[test]
fn column_mutation_and_construction_surfaces_are_now_supported() {
    for surface in [
        CudaBackendSurface::SecureFieldColumnSet,
        CudaBackendSurface::SecureFieldColumnFromIterator,
        CudaBackendSurface::Blake2sHashColumnSet,
        CudaBackendSurface::Blake2sHashColumnFromIterator,
        CudaBackendSurface::GkrNextLayer,
        CudaBackendSurface::GkrSumAsPolyInFirstVariable,
    ] {
        assert_eq!(
            cuda_backend_surface_status(surface),
            CudaBackendSurfaceStatus::Supported,
            "surface {surface:?} should now be marked supported"
        );
        assert!(
            cuda_backend_surface_detail(surface).contains("supported"),
            "surface {surface:?} should describe its supported status"
        );
    }
}

#[test]
fn residual_tracked_surfaces_still_have_explicit_non_supported_status() {
    for surface in [
        CudaBackendSurface::Blake2sHashColumnBitReverse,
        CudaBackendSurface::Poseidon252HashColumnBitReverse,
    ] {
        assert_eq!(
            cuda_backend_surface_status(surface),
            CudaBackendSurfaceStatus::UnsupportedDeprecated,
            "surface {surface:?} should now be deprecated out of the supported companion claim"
        );
        assert!(
            cuda_backend_surface_detail(surface)
                .contains("outside the supported companion capability claim"),
            "surface {surface:?} should describe its unsupported companion status"
        );
    }
}

#[test]
fn fri_decompose_remains_explicitly_unsupported_deprecated() {
    assert_eq!(
        cuda_backend_surface_status(CudaBackendSurface::FriDecompose),
        CudaBackendSurfaceStatus::UnsupportedDeprecated
    );
}
