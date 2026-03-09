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
        | MetalBackendSurface::QuotientOpsCpuBridge => {
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
            "The `PolyOps` boundary is supported through an explicit CPU bridge over Metal-owned column storage and host twiddles."
        }
        MetalBackendSurface::AccumulationOpsCpuBridge => {
            "The `AccumulationOps` boundary is supported through an explicit CPU bridge over Metal-owned secure-column storage."
        }
        MetalBackendSurface::QuotientOpsCpuBridge => {
            "The `QuotientOps` boundary is supported through an explicit CPU bridge over Metal-owned evaluation and accumulation storage."
        }
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
