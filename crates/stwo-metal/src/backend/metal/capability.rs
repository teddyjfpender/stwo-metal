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
    FriFirstLayerFoldCircleIntoLine,
    FriFirstLayerProofNative,
    FriLineFold,
    FriFirstInnerLayerCommitmentNative,
    FriFirstInnerLayerDecommitNative,
    FriFirstInnerLayerProofRowNative,
    FriInnerLayerSequenceNative,
    FriCommitmentSliceBounded,
    FriInnerProofSliceBounded,
    FriProofSliceBounded,
    FriProverTranscriptOwnedBounded,
    FriFirstInnerLayerCommitmentCpuBridge,
    QuotientAccumulate,
}

#[non_exhaustive]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum MetalBackendSurfaceStatus {
    Supported,
    SupportedExplicitCpuBridge,
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
    MetalBackendSurface::FriFirstInnerLayerCommitmentCpuBridge,
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
        | MetalBackendSurface::SecureFieldColumnBitReverse
        | MetalBackendSurface::FriFirstLayerFoldCircleIntoLine
        | MetalBackendSurface::FriFirstLayerProofNative
        | MetalBackendSurface::FriLineFold
        | MetalBackendSurface::FriFirstInnerLayerCommitmentNative
        | MetalBackendSurface::FriFirstInnerLayerDecommitNative
        | MetalBackendSurface::FriFirstInnerLayerProofRowNative
        | MetalBackendSurface::FriInnerLayerSequenceNative
        | MetalBackendSurface::FriCommitmentSliceBounded
        | MetalBackendSurface::FriInnerProofSliceBounded
        | MetalBackendSurface::FriProofSliceBounded
        | MetalBackendSurface::FriProverTranscriptOwnedBounded => {
            MetalBackendSurfaceStatus::Supported
        }
        MetalBackendSurface::FriFirstInnerLayerCommitmentCpuBridge => {
            MetalBackendSurfaceStatus::SupportedExplicitCpuBridge
        }
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
        MetalBackendSurface::FriFirstLayerFoldCircleIntoLine => {
            "The bounded FRI first-layer fold from circle evaluation into line evaluation is implemented through a native Metal kernel."
        }
        MetalBackendSurface::FriFirstLayerProofNative => {
            "The bounded FRI first-layer circle commitment and decommit boundary is implemented through a native stwo-metal proof surface."
        }
        MetalBackendSurface::FriLineFold => {
            "The bounded FRI line-fold path is implemented through native Metal kernels with host-orchestrated repeated folding."
        }
        MetalBackendSurface::FriFirstInnerLayerCommitmentNative => {
            "The first inner FRI-layer commitment is implemented through a stwo-metal-owned native line-evaluation and commitment boundary."
        }
        MetalBackendSurface::FriFirstInnerLayerDecommitNative => {
            "The first inner FRI-layer query and decommit boundary is implemented through the native stwo-metal line-commitment surface."
        }
        MetalBackendSurface::FriFirstInnerLayerProofRowNative => {
            "The first inner FRI layer is implemented as a native proof-facing row with a stable root and decommit API."
        }
        MetalBackendSurface::FriInnerLayerSequenceNative => {
            "A bounded native inner-layer FRI sequence is implemented on top of the first proof-facing row."
        }
        MetalBackendSurface::FriCommitmentSliceBounded => {
            "A bounded FRI commitment slice is implemented on top of the native inner-layer sequence."
        }
        MetalBackendSurface::FriInnerProofSliceBounded => {
            "A bounded proof-facing inner FRI proof slice is implemented on top of the commitment slice."
        }
        MetalBackendSurface::FriProofSliceBounded => {
            "A bounded full FRI proof slice is implemented by composing the native first-layer proof boundary with the inner proof slice."
        }
        MetalBackendSurface::FriProverTranscriptOwnedBounded => {
            "A bounded transcript-owned Metal FRI prover is implemented with vendored channel ordering for commit and decommit."
        }
        MetalBackendSurface::FriFirstInnerLayerCommitmentCpuBridge => {
            "The first inner FRI-layer line-evaluation and commitment handoff is available only through an explicit CPU bridge."
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
