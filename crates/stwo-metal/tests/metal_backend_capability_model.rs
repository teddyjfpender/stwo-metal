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
        MetalBackendSurface::WideFibonacciBenchmarkTargetDeclared,
        MetalBackendSurface::WideFibonacciWitnessInputBoundaryDeclared,
        MetalBackendSurface::WideFibonacciTraceGenerationNative,
        MetalBackendSurface::WideFibonacciTraceCpuBridge,
        MetalBackendSurface::BaseFieldColumnSet,
        MetalBackendSurface::BaseFieldColumnFromIterator,
        MetalBackendSurface::BaseFieldColumnBitReverse,
        MetalBackendSurface::BaseFieldTwiddlePrecomputeNative,
        MetalBackendSurface::BaseFieldRfftEvaluateNative,
        MetalBackendSurface::BaseFieldIfftInterpolateNative,
        MetalBackendSurface::BaseFieldCosetToCircleDomainBitReverse,
        MetalBackendSurface::SecureFieldColumnSet,
        MetalBackendSurface::SecureFieldColumnFromIterator,
        MetalBackendSurface::SecureFieldColumnBitReverse,
        MetalBackendSurface::FriFirstLayerFoldCircleIntoLine,
        MetalBackendSurface::FriFirstLayerProofNative,
        MetalBackendSurface::FriLineFold,
        MetalBackendSurface::FriFirstInnerLayerCommitmentNative,
        MetalBackendSurface::FriFirstInnerLayerDecommitNative,
        MetalBackendSurface::FriFirstInnerLayerProofRowNative,
        MetalBackendSurface::FriInnerLayerSequenceNative,
        MetalBackendSurface::FriCommitmentSliceBounded,
        MetalBackendSurface::FriInnerProofSliceBounded,
        MetalBackendSurface::FriProofSliceBounded,
        MetalBackendSurface::FriProverTranscriptOwnedBounded,
        MetalBackendSurface::FriBlake2sSubpathDeclared,
        MetalBackendSurface::WorkloadBoundaryHybridDeclared,
        MetalBackendSurface::WorkloadWideFibonacciWitnessCpuHandoff,
        MetalBackendSurface::WideFibonacciQuotientAccumulateNative,
        MetalBackendSurface::WorkloadFriReadyEvaluationCpuHandoff,
        MetalBackendSurface::WorkloadQuotientEvaluationCpuHandoff,
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

    assert_eq!(
        metal_backend_surface_status(MetalBackendSurface::FriFirstInnerLayerCommitmentCpuBridge),
        MetalBackendSurfaceStatus::SupportedExplicitCpuBridge
    );
    assert!(metal_backend_surface_detail(
        MetalBackendSurface::FriFirstInnerLayerCommitmentCpuBridge
    )
    .contains("explicit CPU bridge"));
    assert_eq!(
        metal_backend_surface_status(MetalBackendSurface::PolyOpsCpuBridge),
        MetalBackendSurfaceStatus::SupportedExplicitCpuBridge
    );
    assert!(
        metal_backend_surface_detail(MetalBackendSurface::PolyOpsCpuBridge)
            .contains("explicit CPU bridge")
    );
    assert_eq!(
        metal_backend_surface_status(MetalBackendSurface::AccumulationOpsCpuBridge),
        MetalBackendSurfaceStatus::SupportedExplicitCpuBridge
    );
    assert!(
        metal_backend_surface_detail(MetalBackendSurface::AccumulationOpsCpuBridge)
            .contains("explicit CPU bridge")
    );
    assert_eq!(
        metal_backend_surface_status(MetalBackendSurface::QuotientOpsCpuBridge),
        MetalBackendSurfaceStatus::SupportedExplicitCpuBridge
    );
    assert!(
        metal_backend_surface_detail(MetalBackendSurface::QuotientOpsCpuBridge)
            .contains("explicit CPU bridge")
    );
    assert_eq!(
        metal_backend_surface_status(MetalBackendSurface::FriOpsCpuBridge),
        MetalBackendSurfaceStatus::Supported
    );
    assert!(
        metal_backend_surface_detail(MetalBackendSurface::FriOpsCpuBridge).contains("native")
            || metal_backend_surface_detail(MetalBackendSurface::FriOpsCpuBridge)
                .contains("retired")
    );
    assert_eq!(
        metal_backend_surface_status(MetalBackendSurface::MleOpsCpuBridge),
        MetalBackendSurfaceStatus::Supported
    );
    assert!(
        metal_backend_surface_detail(MetalBackendSurface::MleOpsCpuBridge).contains("native Metal")
    );
    assert_eq!(
        metal_backend_surface_status(MetalBackendSurface::GkrOpsCpuBridge),
        MetalBackendSurfaceStatus::Supported
    );
    assert!(
        metal_backend_surface_detail(MetalBackendSurface::GkrOpsCpuBridge).contains("native Metal")
    );
    assert_eq!(
        metal_backend_surface_status(MetalBackendSurface::Blake2sHashColumnCpuBridge),
        MetalBackendSurfaceStatus::Supported
    );
    assert!(
        metal_backend_surface_detail(MetalBackendSurface::Blake2sHashColumnCpuBridge)
            .contains("no `CpuBackend` dependency")
    );
    assert_eq!(
        metal_backend_surface_status(MetalBackendSurface::Blake2sMerkleOpsLiftedCpuBridge),
        MetalBackendSurfaceStatus::Supported
    );
    assert!(
        metal_backend_surface_detail(MetalBackendSurface::Blake2sMerkleOpsLiftedCpuBridge)
            .contains("no `CpuBackend` dependency")
    );
    assert_eq!(
        metal_backend_surface_status(MetalBackendSurface::Blake2sGrindCpuBridge),
        MetalBackendSurfaceStatus::Supported
    );
    assert!(
        metal_backend_surface_detail(MetalBackendSurface::Blake2sGrindCpuBridge)
            .contains("no `CpuBackend` dependency")
    );
    assert_eq!(
        metal_backend_surface_status(MetalBackendSurface::Blake2sBackendForChannelCpuBridge),
        MetalBackendSurfaceStatus::Supported
    );
    assert!(
        metal_backend_surface_detail(MetalBackendSurface::Blake2sBackendForChannelCpuBridge)
            .contains("no `CpuBackend` dependency")
    );
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
