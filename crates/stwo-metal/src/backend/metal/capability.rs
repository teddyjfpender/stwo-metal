use stwo_metal_sys::metal;

#[non_exhaustive]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum MetalBackendSurface {
    WideFibonacciBenchmarkTargetDeclared,
    WideFibonacciWitnessInputBoundaryDeclared,
    WideFibonacciTraceGenerationNative,
    WideFibonacciTraceCpuBridge,
    PolyOpsCpuBridge,
    AccumulationOpsCpuBridge,
    QuotientOpsCpuBridge,
    FriOpsCpuBridge,
    MleOpsCpuBridge,
    GkrOpsCpuBridge,
    Blake2sHashColumnCpuBridge,
    Blake2sMerkleOpsLiftedCpuBridge,
    Blake2sGrindCpuBridge,
    Blake2sBackendForChannelCpuBridge,
    BaseFieldColumnSet,
    BaseFieldColumnFromIterator,
    BaseFieldColumnBitReverse,
    BaseFieldTwiddlePrecomputeNative,
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
    FriBlake2sSubpathDeclared,
    WorkloadBoundaryHybridDeclared,
    WorkloadWideFibonacciWitnessCpuHandoff,
    WideFibonacciQuotientAccumulateNative,
    WorkloadFriReadyEvaluationCpuHandoff,
    WorkloadQuotientEvaluationCpuHandoff,
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
    MetalBackendSurface::WideFibonacciBenchmarkTargetDeclared,
    MetalBackendSurface::WideFibonacciWitnessInputBoundaryDeclared,
    MetalBackendSurface::WideFibonacciTraceGenerationNative,
    MetalBackendSurface::WideFibonacciTraceCpuBridge,
    MetalBackendSurface::PolyOpsCpuBridge,
    MetalBackendSurface::AccumulationOpsCpuBridge,
    MetalBackendSurface::QuotientOpsCpuBridge,
    MetalBackendSurface::FriOpsCpuBridge,
    MetalBackendSurface::MleOpsCpuBridge,
    MetalBackendSurface::GkrOpsCpuBridge,
    MetalBackendSurface::Blake2sHashColumnCpuBridge,
    MetalBackendSurface::Blake2sMerkleOpsLiftedCpuBridge,
    MetalBackendSurface::Blake2sGrindCpuBridge,
    MetalBackendSurface::Blake2sBackendForChannelCpuBridge,
    MetalBackendSurface::BaseFieldColumnSet,
    MetalBackendSurface::BaseFieldColumnFromIterator,
    MetalBackendSurface::BaseFieldColumnBitReverse,
    MetalBackendSurface::BaseFieldTwiddlePrecomputeNative,
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
    MetalBackendSurface::FriFirstInnerLayerCommitmentCpuBridge,
    MetalBackendSurface::QuotientAccumulate,
];

pub const fn metal_backend_surface_status(
    surface: MetalBackendSurface,
) -> MetalBackendSurfaceStatus {
    match surface {
        MetalBackendSurface::WideFibonacciBenchmarkTargetDeclared
        | MetalBackendSurface::WideFibonacciWitnessInputBoundaryDeclared
        | MetalBackendSurface::WideFibonacciTraceGenerationNative
        | MetalBackendSurface::WideFibonacciTraceCpuBridge
        | MetalBackendSurface::BaseFieldColumnSet
        | MetalBackendSurface::BaseFieldColumnFromIterator
        | MetalBackendSurface::BaseFieldColumnBitReverse
        | MetalBackendSurface::BaseFieldTwiddlePrecomputeNative
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
        | MetalBackendSurface::FriProverTranscriptOwnedBounded
        | MetalBackendSurface::FriBlake2sSubpathDeclared
        | MetalBackendSurface::WorkloadBoundaryHybridDeclared
        | MetalBackendSurface::WorkloadWideFibonacciWitnessCpuHandoff
        | MetalBackendSurface::WideFibonacciQuotientAccumulateNative
        | MetalBackendSurface::WorkloadFriReadyEvaluationCpuHandoff
        | MetalBackendSurface::WorkloadQuotientEvaluationCpuHandoff => {
            MetalBackendSurfaceStatus::Supported
        }
        MetalBackendSurface::FriFirstInnerLayerCommitmentCpuBridge
        | MetalBackendSurface::PolyOpsCpuBridge => {
            MetalBackendSurfaceStatus::SupportedExplicitCpuBridge
        }
        MetalBackendSurface::AccumulationOpsCpuBridge
        | MetalBackendSurface::QuotientOpsCpuBridge
        | MetalBackendSurface::FriOpsCpuBridge
        | MetalBackendSurface::MleOpsCpuBridge
        | MetalBackendSurface::GkrOpsCpuBridge
        | MetalBackendSurface::Blake2sHashColumnCpuBridge
        | MetalBackendSurface::Blake2sMerkleOpsLiftedCpuBridge
        | MetalBackendSurface::Blake2sGrindCpuBridge
        | MetalBackendSurface::Blake2sBackendForChannelCpuBridge => {
            MetalBackendSurfaceStatus::SupportedExplicitCpuBridge
        }
        MetalBackendSurface::QuotientAccumulate => MetalBackendSurfaceStatus::UnsupportedPlanned,
    }
}

pub const fn metal_backend_surface_detail(surface: MetalBackendSurface) -> &'static str {
    match surface {
        MetalBackendSurface::WideFibonacciBenchmarkTargetDeclared => {
            "A declared wide-fibonacci benchmark target is supported as a formal Metal port objective."
        }
        MetalBackendSurface::WideFibonacciWitnessInputBoundaryDeclared => {
            "A witness-input boundary is supported for the declared wide-fibonacci benchmark target."
        }
        MetalBackendSurface::WideFibonacciTraceGenerationNative => {
            "Wide-fibonacci trace generation is implemented through a native `.metal` kernel that writes a contiguous column-major trace buffer."
        }
        MetalBackendSurface::WideFibonacciTraceCpuBridge => {
            "The native Metal wide-fibonacci trace CPU bridge is supported and materializes ordinary CPU circle evaluations for unchanged upstream example prove/verify wiring."
        }
        MetalBackendSurface::PolyOpsCpuBridge => {
            "The `PolyOps` boundary is supported through an explicit CPU bridge over Metal-owned column storage; native Metal twiddle precompute is implemented, but interpolation and evaluation still bridge through the vendored CPU backend."
        }
        MetalBackendSurface::AccumulationOpsCpuBridge => {
            "The `AccumulationOps` boundary is supported through an explicit CPU bridge over Metal-owned secure-column storage."
        }
        MetalBackendSurface::QuotientOpsCpuBridge => {
            "The `QuotientOps` boundary is supported through an explicit CPU bridge over Metal-owned evaluation and accumulation storage."
        }
        MetalBackendSurface::FriOpsCpuBridge => {
            "The `FriOps` boundary is supported through an explicit CPU bridge that repacks Metal-owned secure columns into the bounded Metal fold kernels and uses the vendored CPU backend for decomposition."
        }
        MetalBackendSurface::MleOpsCpuBridge => {
            "The `MleOps` boundary is supported through an explicit CPU bridge over Metal-owned multilinear-evaluation storage."
        }
        MetalBackendSurface::GkrOpsCpuBridge => {
            "The `GkrOps` boundary is supported through an explicit CPU bridge over Metal-owned lookup-layer storage and CPU-owned oracle evaluation."
        }
        MetalBackendSurface::Blake2sHashColumnCpuBridge => {
            "The `ColumnOps<Blake2sHash>` boundary is supported through an explicit CPU bridge over host-owned Blake2s hash columns."
        }
        MetalBackendSurface::Blake2sMerkleOpsLiftedCpuBridge => {
            "The lifted Blake2s Merkle boundary is supported through an explicit CPU bridge over Metal-owned base-field columns."
        }
        MetalBackendSurface::Blake2sGrindCpuBridge => {
            "The Blake2s proof-of-work boundary is supported through an explicit CPU bridge."
        }
        MetalBackendSurface::Blake2sBackendForChannelCpuBridge => {
            "The Blake2s `BackendForChannel` boundary is supported through an explicit CPU bridge over Merkle and proof-of-work surfaces."
        }
        MetalBackendSurface::BaseFieldColumnSet => "Base-field Metal column mutation is supported.",
        MetalBackendSurface::BaseFieldColumnFromIterator => {
            "Base-field Metal column construction from iterators is supported."
        }
        MetalBackendSurface::BaseFieldColumnBitReverse => {
            "Base-field bit reversal is implemented through a native Metal kernel."
        }
        MetalBackendSurface::BaseFieldTwiddlePrecomputeNative => {
            "Base-field twiddle precompute and inverse-twiddle generation are implemented through native Metal kernels and parity-tested against the vendored CPU oracle."
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
        MetalBackendSurface::FriBlake2sSubpathDeclared => {
            "A declared bounded Blake2s FRI proving sub-path is implemented on top of the transcript-owned Metal prover."
        }
        MetalBackendSurface::WorkloadBoundaryHybridDeclared => {
            "A declared Stwo workload boundary is supported for routing one workload through the Metal FRI sub-path while keeping witness, quotient, and PCS ownership explicit."
        }
        MetalBackendSurface::WorkloadWideFibonacciWitnessCpuHandoff => {
            "A CPU-owned wide-fibonacci witness handoff is supported for feeding the native Metal trace boundary before quotient accumulation."
        }
        MetalBackendSurface::WideFibonacciQuotientAccumulateNative => {
            "A bounded wide-fibonacci quotient accumulation primitive is implemented through a native `.metal` kernel with explicit host/CUDA output bridging in the current benchmark path."
        }
        MetalBackendSurface::WorkloadFriReadyEvaluationCpuHandoff => {
            "A CPU-owned FRI-ready evaluation handoff is supported for the declared Metal hybrid workload boundary with explicit ownership."
        }
        MetalBackendSurface::WorkloadQuotientEvaluationCpuHandoff => {
            "A CPU-owned quotient evaluation handoff is supported for the declared Metal hybrid workload boundary with explicit ownership."
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
