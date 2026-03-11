use std::collections::HashSet;

use stwo::core::vcs_lifted::blake2_merkle::{Blake2sM31MerkleChannel, Blake2sMerkleHasher};
use stwo::prover::backend::{Backend, BackendForChannel};
use stwo_metal::{
    accumulate_wide_fibonacci_quotients, accumulate_wide_fibonacci_quotients_from_batch,
    cuda_backend_surface_status, declare_exemplar_hybrid_fri_workload,
    declare_exemplar_metal_workload_boundary, declare_wide_fibonacci_benchmark_boundary,
    evaluate_polys_on_domain_batch, execute_metal_evaluation_program_v1_on_metal,
    execute_selected_metal_evaluation_program_v1_on_metal,
    lower_registered_metal_evaluation_program_v1, metal_evaluation_program_overlays_v1,
    plan_exemplar_metal_prove_by_name, plan_exemplar_prove_by_name,
    select_metal_evaluation_program_dispatch_v1, validate_metal_evaluation_program_v1,
    BaseFieldVec, CudaBackend, CudaBackendSurface, CudaBackendSurfaceStatus, CudaExecutionIntent,
    CudaExecutionPlan, MetalBackend, MetalBaseFieldColumnBatch, MetalBaseFieldVec,
    MetalBenchmarkInputError, MetalBenchmarkProgramError, MetalBenchmarkProgramExecutionError,
    MetalBenchmarkProveCoreBreakdown, MetalBenchmarkProveCoreError,
    MetalBenchmarkProveValuesBreakdown, MetalBenchmarkProveValuesStaging, MetalBenchmarkTarget,
    MetalBenchmarkTraceShapeError, MetalCpuQuotientEvaluationInput,
    MetalCpuWideFibonacciWitnessInput, MetalEvaluationProgramBaseInstV1,
    MetalEvaluationProgramBudgetV1, MetalEvaluationProgramCapabilityProfileV1,
    MetalEvaluationProgramDispatchKindV1, MetalEvaluationProgramExecutionError,
    MetalEvaluationProgramExtInstV1, MetalEvaluationProgramHeaderV1,
    MetalEvaluationProgramInterpreterError, MetalEvaluationProgramLoweringError,
    MetalEvaluationProgramOverlayV1, MetalEvaluationProgramRuntimeInputsV1,
    MetalEvaluationProgramSectionDescV1, MetalEvaluationProgramSectionKindV1,
    MetalSampledValuesColumnDescV1, MetalSampledValuesHeaderV1,
    MetalSampledValuesDispatchKindV1, MetalSampledValuesExecutionError,
    MetalSampledValuesInterpreterError, MetalSampledValuesLoweringError,
    MetalSampledValuesTreeDescV1, MetalSampledValuesValidationError,
    MetalSecureFieldValueV1,
    MetalEvaluationProgramSpecializationV1, MetalEvaluationProgramTraceViewV1,
    MetalEvaluationProgramValidationError, MetalExecutionIntent, MetalExecutionPlan,
    MetalFriBlake2sSubpath, MetalFriFirstLayer, MetalFriInnerLayerRow, MetalFriInnerProofSlice,
    MetalFriLayerDecommitment, MetalFriProofSlice, MetalFriProver, MetalFriReadyEvaluationInput,
    MetalGeneratedWideFibonacciBenchmarkBreakdown, MetalGeneratedWideFibonacciBenchmarkError,
    MetalGeneratedWideFibonacciBenchmarkIteration, MetalGeneratedWideFibonacciBenchmarkRun,
    MetalGeneratedWideFibonacciBenchmarkSample,
    MetalHybridFriWorkload, MetalLineCommitment, MetalLineEvaluation,
    MetalQuotientEvaluationInput, MetalSecureFieldVec,
    MetalWideFibonacciBatchQuotientRequest, MetalWideFibonacciBenchmarkBoundary,
    MetalWideFibonacciQuotientError, MetalWideFibonacciQuotientRequest,
    MetalWideFibonacciQuotients, MetalWideFibonacciTrace, MetalWideFibonacciTraceError,
    MetalWideFibonacciTraceRequest, MetalWideFibonacciWitnessInputs, MetalWorkloadBoundary,
    MetalWorkloadHandoffError, MetalWorkloadOwnership, MetalWorkloadStage,
    OwnedConstraintEvalAbiV1, OwnedMetalEvaluationProgramV1, OwnedMetalSampledValuesV1,
    SecureFieldVec,
    StwoCudaWideFibonacciEvalAbiV1, STWO_CUDA_BACKEND_SURFACES_V1,
    STWO_METAL_EVAL_PROGRAM_ABI_MAJOR_V1, STWO_METAL_EVAL_PROGRAM_MAGIC_V1,
    STWO_METAL_EVAL_PROGRAM_SECURE_EXT_DEGREE_V1,
    STWO_METAL_SAMPLED_VALUES_ABI_MAJOR_V1, STWO_METAL_SAMPLED_VALUES_MAGIC_V1,
    WIDE_FIBONACCI_PROVE_LOG20_TARGET, execute_selected_metal_sampled_values_v1,
    interpret_metal_sampled_values_v1, interpret_metal_sampled_values_v1_reference,
    lower_metal_sampled_values_v1, select_metal_sampled_values_dispatch_v1,
};

#[test]
fn companion_surface_exports_backend_core_types() {
    fn assert_backend<B: Backend>() {}
    fn assert_backend_for_channel<B: BackendForChannel<Blake2sM31MerkleChannel>>() {}

    assert_backend::<MetalBackend>();
    assert_backend_for_channel::<MetalBackend>();
    let _ = std::mem::size_of::<CudaBackend>();
    let _ = std::mem::size_of::<BaseFieldVec>();
    let _ = std::mem::size_of::<MetalBackend>();
    let _ = std::mem::size_of::<MetalBaseFieldColumnBatch>();
    let _ = std::mem::size_of::<MetalBaseFieldVec>();
    let _ = std::mem::size_of::<MetalFriFirstLayer<Blake2sMerkleHasher>>();
    let _ = std::mem::size_of::<MetalFriInnerLayerRow<Blake2sMerkleHasher>>();
    let _ = std::mem::size_of::<MetalFriInnerProofSlice<Blake2sMerkleHasher>>();
    let _ = std::mem::size_of::<MetalFriProofSlice<Blake2sMerkleHasher>>();
    let _ = std::mem::size_of::<
        MetalFriProver<stwo::core::vcs_lifted::blake2_merkle::Blake2sMerkleChannel>,
    >();
    let _ = std::mem::size_of::<MetalBenchmarkTarget>();
    let _ = std::mem::size_of::<MetalWideFibonacciBenchmarkBoundary>();
    let _ = std::mem::size_of::<MetalWideFibonacciTrace>();
    let _ = std::mem::size_of::<MetalWideFibonacciTraceRequest<'static>>();
    let _ = std::mem::size_of::<MetalWideFibonacciTraceError>();
    let _ = accumulate_wide_fibonacci_quotients
        as fn(
            MetalWideFibonacciQuotientRequest<'_>,
        ) -> Result<MetalWideFibonacciQuotients, MetalWideFibonacciQuotientError>;
    let _ = accumulate_wide_fibonacci_quotients_from_batch
        as fn(
            MetalWideFibonacciBatchQuotientRequest<'_>,
        ) -> Result<MetalWideFibonacciQuotients, MetalWideFibonacciQuotientError>;
    let _ = evaluate_polys_on_domain_batch;
    let _ = std::mem::size_of::<MetalWideFibonacciBatchQuotientRequest<'static>>();
    let _ = std::mem::size_of::<MetalWideFibonacciQuotientRequest<'static>>();
    let _ = std::mem::size_of::<MetalWideFibonacciQuotients>();
    let _ = std::mem::size_of::<MetalWideFibonacciQuotientError>();
    let _ = std::mem::size_of::<MetalWideFibonacciWitnessInputs>();
    let _ = std::mem::size_of::<MetalBenchmarkInputError>();
    let _ = std::mem::size_of::<MetalFriBlake2sSubpath>();
    let _ = std::mem::size_of::<MetalFriLayerDecommitment<Blake2sMerkleHasher>>();
    let _ = std::mem::size_of::<MetalCpuQuotientEvaluationInput>();
    let _ = std::mem::size_of::<MetalCpuWideFibonacciWitnessInput>();
    let _ = std::mem::size_of::<MetalFriReadyEvaluationInput>();
    let _ = std::mem::size_of::<MetalQuotientEvaluationInput>();
    let _ = std::mem::size_of::<MetalWorkloadBoundary>();
    let _ = std::mem::size_of::<MetalWorkloadHandoffError<'static>>();
    let _ = std::mem::size_of::<MetalHybridFriWorkload>();
    let _ = std::mem::size_of::<MetalLineEvaluation>();
    let _ = std::mem::size_of::<MetalLineCommitment<Blake2sMerkleHasher>>();
    let _ = std::mem::size_of::<MetalSecureFieldVec>();
    let _ = std::mem::size_of::<SecureFieldVec>();
    let _ = std::mem::size_of::<MetalSampledValuesHeaderV1>();
    let _ = std::mem::size_of::<MetalSampledValuesTreeDescV1>();
    let _ = std::mem::size_of::<MetalSampledValuesColumnDescV1>();
    let _ = std::mem::size_of::<MetalSecureFieldValueV1>();
    let _ = std::mem::size_of::<OwnedMetalSampledValuesV1>();
    let _ = std::mem::size_of::<MetalSampledValuesLoweringError>();
    let _ = std::mem::size_of::<MetalSampledValuesValidationError>();
    let _ = std::mem::size_of::<MetalSampledValuesInterpreterError>();
    let _ = std::mem::size_of::<MetalSampledValuesExecutionError>();
    let _ = std::mem::size_of::<MetalSampledValuesDispatchKindV1>();
}

#[test]
fn companion_surface_exports_planner_api() {
    let plan = plan_exemplar_prove_by_name(CudaExecutionIntent::RequireCuda, &["poseidon_example"])
        .unwrap();
    let metal_plan = plan_exemplar_metal_prove_by_name(
        MetalExecutionIntent::PreferMetal,
        &["fibonacci_example"],
    )
    .unwrap();

    assert_eq!(plan, CudaExecutionPlan::CudaFull);
    assert_eq!(metal_plan, MetalExecutionPlan::MetalFriHybrid);
}

#[test]
fn companion_surface_exports_capability_diagnostics() {
    let surfaces: HashSet<_> = STWO_CUDA_BACKEND_SURFACES_V1.iter().copied().collect();

    assert_eq!(surfaces.len(), STWO_CUDA_BACKEND_SURFACES_V1.len());
    assert_eq!(
        cuda_backend_surface_status(CudaBackendSurface::GkrSumAsPolyInFirstVariable),
        CudaBackendSurfaceStatus::Supported
    );
}

#[test]
fn companion_surface_exports_workload_boundary_api() {
    let boundary = declare_exemplar_metal_workload_boundary(
        MetalExecutionIntent::PreferMetal,
        "fibonacci_example",
    )
    .unwrap();
    let workload = declare_exemplar_hybrid_fri_workload(
        MetalExecutionIntent::PreferMetal,
        "fibonacci_example",
        stwo::core::fri::FriConfig::new(3, 2, 3, 2),
    )
    .unwrap();

    assert_eq!(boundary.plan(), MetalExecutionPlan::MetalFriHybrid);
    assert_eq!(
        workload.boundary().plan(),
        MetalExecutionPlan::MetalFriHybrid
    );
    assert_eq!(
        boundary.stage_ownership(MetalWorkloadStage::FriBlake2s),
        Some(MetalWorkloadOwnership::MetalNative)
    );

    let benchmark_boundary = declare_wide_fibonacci_benchmark_boundary(
        MetalExecutionIntent::PreferMetal,
        WIDE_FIBONACCI_PROVE_LOG20_TARGET,
    )
    .unwrap();
    assert_eq!(
        benchmark_boundary.workload_boundary().plan(),
        MetalExecutionPlan::MetalFriHybrid
    );
    let evaluation_program = benchmark_boundary.evaluation_program_v1().unwrap();
    assert_eq!(
        evaluation_program.header().n_constraints,
        benchmark_boundary.target().n_columns - 2
    );
}

#[test]
fn companion_surface_exports_constraint_eval_abi_values() {
    let abi = OwnedConstraintEvalAbiV1::WideFibonacci(StwoCudaWideFibonacciEvalAbiV1::new(7, 8));

    assert_eq!(abi.as_ptr().is_null(), false);
}

#[test]
fn companion_surface_exports_metal_evaluation_program_v1_api() {
    let header = MetalEvaluationProgramHeaderV1::new(5, 7, 1 << 2, 1, 2, 3, 4, 8, 4);
    let sections = [
        MetalEvaluationProgramSectionDescV1::new(
            MetalEvaluationProgramSectionKindV1::BaseConsts,
            4,
            0,
            1,
        ),
        MetalEvaluationProgramSectionDescV1::new(
            MetalEvaluationProgramSectionKindV1::ExtConsts,
            16,
            4,
            1,
        ),
        MetalEvaluationProgramSectionDescV1::new(
            MetalEvaluationProgramSectionKindV1::BaseInsts,
            16,
            20,
            1,
        ),
        MetalEvaluationProgramSectionDescV1::new(
            MetalEvaluationProgramSectionKindV1::ExtInsts,
            20,
            36,
            1,
        ),
        MetalEvaluationProgramSectionDescV1::new(
            MetalEvaluationProgramSectionKindV1::ConstraintRoots,
            4,
            56,
            1,
        ),
    ];

    assert_eq!(header.magic, STWO_METAL_EVAL_PROGRAM_MAGIC_V1);
    assert_eq!(header.abi_major, STWO_METAL_EVAL_PROGRAM_ABI_MAJOR_V1);
    assert_eq!(
        header.secure_ext_degree,
        STWO_METAL_EVAL_PROGRAM_SECURE_EXT_DEGREE_V1
    );
    assert_eq!(STWO_METAL_SAMPLED_VALUES_MAGIC_V1, u32::from_le_bytes(*b"SMS1"));
    assert_eq!(STWO_METAL_SAMPLED_VALUES_ABI_MAJOR_V1, 1);
    validate_metal_evaluation_program_v1(
        header,
        &sections,
        60,
        MetalEvaluationProgramBudgetV1::new(16, 8),
    )
    .unwrap();
    let lowered = lower_registered_metal_evaluation_program_v1(
        "fibonacci_example",
        MetalEvaluationProgramSpecializationV1 {
            log_n_rows: WIDE_FIBONACCI_PROVE_LOG20_TARGET.log_n_instances,
            n_columns: WIDE_FIBONACCI_PROVE_LOG20_TARGET.n_columns,
        },
    )
    .unwrap();
    let _ = std::mem::size_of::<MetalBenchmarkProgramError>();
    let _ = std::mem::size_of::<MetalBenchmarkProgramExecutionError>();
    let _ = std::mem::size_of::<MetalBenchmarkProveCoreBreakdown>();
    let _ = std::mem::size_of::<MetalBenchmarkProveCoreError>();
    let _ = std::mem::size_of::<MetalBenchmarkProveValuesBreakdown>();
    let _ = std::mem::size_of::<MetalBenchmarkProveValuesStaging>();
    let _ = std::mem::size_of::<MetalGeneratedWideFibonacciBenchmarkBreakdown>();
    let _ = std::mem::size_of::<MetalGeneratedWideFibonacciBenchmarkError>();
    let _ = std::mem::size_of::<MetalGeneratedWideFibonacciBenchmarkIteration>();
    let _ = std::mem::size_of::<MetalGeneratedWideFibonacciBenchmarkRun>();
    let _ = std::mem::size_of::<MetalGeneratedWideFibonacciBenchmarkSample>();
    let _ = std::mem::size_of::<MetalBenchmarkTraceShapeError>();
    let _ = std::mem::size_of::<MetalEvaluationProgramBaseInstV1>();
    let _ = std::mem::size_of::<MetalEvaluationProgramCapabilityProfileV1>();
    let _ = std::mem::size_of::<MetalEvaluationProgramDispatchKindV1>();
    let _ = std::mem::size_of::<MetalEvaluationProgramExecutionError>();
    let _ = std::mem::size_of::<MetalEvaluationProgramExtInstV1>();
    let _ = std::mem::size_of::<MetalEvaluationProgramInterpreterError>();
    let _ = std::mem::size_of::<MetalEvaluationProgramLoweringError>();
    let _ = std::mem::size_of::<MetalEvaluationProgramOverlayV1>();
    let _ = std::mem::size_of::<MetalEvaluationProgramHeaderV1>();
    let _ = std::mem::size_of::<MetalEvaluationProgramRuntimeInputsV1<'static>>();
    let _ = std::mem::size_of::<MetalEvaluationProgramSpecializationV1>();
    let _ = std::mem::size_of::<MetalEvaluationProgramSectionDescV1>();
    let _ = std::mem::size_of::<MetalEvaluationProgramTraceViewV1<'static>>();
    let _ = std::mem::size_of::<MetalEvaluationProgramValidationError>();
    let _ = std::mem::size_of::<OwnedMetalEvaluationProgramV1>();
    let _ = lower_metal_sampled_values_v1;
    let _ = interpret_metal_sampled_values_v1;
    let _ = interpret_metal_sampled_values_v1_reference;
    let _ = select_metal_sampled_values_dispatch_v1;
    let _ = execute_selected_metal_sampled_values_v1;
    assert_eq!(lowered.header().n_constraints, 98);
    assert_eq!(lowered.base_insts().len(), 1 + 98 * 7);
    assert_eq!(lowered.ext_insts().len(), 98);
    let overlays = metal_evaluation_program_overlays_v1();
    assert!(!overlays.is_empty());
    assert!(overlays
        .iter()
        .any(|overlay| overlay.name == "wide_fibonacci_eval_v1"));
    let _ = lower_registered_metal_evaluation_program_v1
        as fn(
            &'static str,
            MetalEvaluationProgramSpecializationV1,
        )
            -> Result<OwnedMetalEvaluationProgramV1, MetalEvaluationProgramLoweringError>;
    let _ = stwo_metal::interpret_metal_evaluation_program_v1
        as fn(
            &OwnedMetalEvaluationProgramV1,
            MetalEvaluationProgramRuntimeInputsV1<'_>,
        ) -> Result<
            Vec<stwo::core::fields::qm31::SecureField>,
            MetalEvaluationProgramInterpreterError,
        >;
    let _ = execute_metal_evaluation_program_v1_on_metal
        as fn(
            &OwnedMetalEvaluationProgramV1,
            MetalEvaluationProgramRuntimeInputsV1<'_>,
        ) -> Result<
            Vec<stwo::core::fields::qm31::SecureField>,
            MetalEvaluationProgramExecutionError,
        >;
    let _ = execute_selected_metal_evaluation_program_v1_on_metal
        as fn(
            &OwnedMetalEvaluationProgramV1,
            MetalEvaluationProgramRuntimeInputsV1<'_>,
            MetalEvaluationProgramCapabilityProfileV1,
        ) -> Result<
            (
                Vec<stwo::core::fields::qm31::SecureField>,
                MetalEvaluationProgramDispatchKindV1,
            ),
            MetalEvaluationProgramExecutionError,
        >;
    let _ = select_metal_evaluation_program_dispatch_v1
        as fn(
            &OwnedMetalEvaluationProgramV1,
            MetalEvaluationProgramCapabilityProfileV1,
        )
            -> Result<MetalEvaluationProgramDispatchKindV1, MetalEvaluationProgramExecutionError>;
}
