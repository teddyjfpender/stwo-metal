use std::collections::HashSet;

use stwo_metal::{
    metal_backend_surface_detail, metal_backend_surface_status, metal_runtime_support,
    MetalBackendSurface, MetalBackendSurfaceStatus, MetalRuntimeSupport,
    STWO_METAL_BACKEND_SURFACES_V1,
};

#[test]
fn tracked_metal_backend_surfaces_v1_are_unique() {
    let surfaces: HashSet<_> = STWO_METAL_BACKEND_SURFACES_V1.iter().copied().collect();
    assert_eq!(surfaces.len(), STWO_METAL_BACKEND_SURFACES_V1.len());
}

#[test]
fn first_metal_slice_marks_base_field_support_explicitly() {
    for surface in [
        MetalBackendSurface::BaseFieldColumnSet,
        MetalBackendSurface::BaseFieldColumnFromIterator,
        MetalBackendSurface::BaseFieldColumnBitReverse,
        MetalBackendSurface::BaseFieldCosetToCircleDomainBitReverse,
        MetalBackendSurface::SecureFieldColumnSet,
        MetalBackendSurface::SecureFieldColumnFromIterator,
        MetalBackendSurface::SecureFieldColumnBitReverse,
    ] {
        assert_eq!(
            metal_backend_surface_status(surface),
            MetalBackendSurfaceStatus::Supported
        );
        assert!(
            metal_backend_surface_detail(surface).contains("supported")
                || metal_backend_surface_detail(surface).contains("implemented")
        );
    }
}

#[test]
fn metal_runtime_support_reports_a_formal_state() {
    assert!(matches!(
        metal_runtime_support(),
        MetalRuntimeSupport::Available
            | MetalRuntimeSupport::DisabledByConfiguration
            | MetalRuntimeSupport::UnsupportedTarget
            | MetalRuntimeSupport::InitializationFailed
    ));
}

#[test]
fn later_migration_targets_remain_explicitly_planned() {
    for surface in [MetalBackendSurface::QuotientAccumulate] {
        assert_eq!(
            metal_backend_surface_status(surface),
            MetalBackendSurfaceStatus::UnsupportedPlanned
        );
    }
}
