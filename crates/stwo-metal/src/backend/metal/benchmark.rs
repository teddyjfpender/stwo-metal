use stwo::core::air::Component;
use stwo::core::channel::Blake2sChannel;
use stwo::core::constraints::coset_vanishing;
use stwo::core::fields::m31::BaseField;
use stwo::core::fields::qm31::SecureField;
#[allow(unused_imports)]
use stwo::core::fields::FieldExpOps;
use stwo::core::pcs::{CommitmentSchemeVerifier, PcsConfig, TreeVec};
use stwo::core::poly::circle::CanonicCoset;
use stwo::core::proof::StarkProof;
use stwo::core::vcs_lifted::blake2_merkle::{Blake2sMerkleChannel, Blake2sMerkleHasher};
use stwo::core::verifier::{verify, VerificationError, PREPROCESSED_TRACE_IDX};
use stwo::prover::poly::circle::PolyOps;
use stwo::prover::{
    CommitmentSchemeProver, ComponentProver, DomainEvaluationAccumulator, Trace,
};

use super::artifact::{MetalGeneratedRouteKind, MetalRegisteredBenchmarkOperation};
use super::eval_program_v1::{
    execute_selected_metal_evaluation_program_v1_on_metal, interpret_metal_evaluation_program_v1,
    lower_registered_metal_evaluation_program_v1, select_metal_evaluation_program_dispatch_v1,
    MetalEvaluationProgramBudgetV1, MetalEvaluationProgramCapabilityProfileV1,
    MetalEvaluationProgramDispatchKindV1, MetalEvaluationProgramExecutionError,
    MetalEvaluationProgramInterpreterError, MetalEvaluationProgramLoweringError,
    MetalEvaluationProgramRuntimeInputsV1, MetalEvaluationProgramSpecializationV1,
    MetalEvaluationProgramTraceViewV1, OwnedMetalEvaluationProgramV1,
};
use super::execution_plan::{
    registered_execution_binding, registered_execution_seed, RegisteredMetalExecutionSeed,
};
use super::planner::{MetalExecutionIntent, MetalPlannerError};
use super::poly::evaluate_polys_on_domain_batch;
use super::prove_runtime_v1::{
    commit_trace_with_breakdown as runtime_commit_trace_with_breakdown, execute_prove_core_v1,
    MetalCompositionDetailBreakdown, MetalProofMetadata, MetalProveCoreError,
    MetalProveRuntimeContextV1, MetalProveValuesDetailBreakdown, MetalProveValuesResult,
};
use super::quotient::{
    accumulate_wide_fibonacci_quotients, accumulate_wide_fibonacci_quotients_from_batch,
    MetalWideFibonacciBatchQuotientRequest, MetalWideFibonacciQuotientRequest,
};
use super::witness::{MetalWideFibonacciTrace, MetalWideFibonacciTraceError};
use super::workload::{declare_exemplar_metal_workload_boundary, MetalWorkloadBoundary};
use super::MetalBackend;

const MAIN_TRACE_IDX: usize = 1;

// ---------------------------------------------------------------------------
// Benchmark-specific types
// ---------------------------------------------------------------------------

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum MetalBenchmarkOperation {
    TraceGeneration,
    ProveVerify,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum MetalBenchmarkReferencePlatform {
    Rtx4090Cuda,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct MetalBenchmarkTarget {
    pub benchmark_id: &'static str,
    pub workload_name: &'static str,
    pub family: &'static str,
    pub operation: MetalBenchmarkOperation,
    pub log_n_instances: u32,
    pub n_columns: u32,
    pub reference_platform: MetalBenchmarkReferencePlatform,
    pub reference_elapsed_ms: f64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MetalWideFibonacciWitnessInputs {
    log_n_instances: u32,
    n_columns: u32,
    input_a: Vec<BaseField>,
    input_b: Vec<BaseField>,
    execution_seed: RegisteredMetalExecutionSeed,
}

impl MetalWideFibonacciWitnessInputs {
    pub fn log_n_instances(&self) -> u32 {
        self.log_n_instances
    }

    pub fn n_columns(&self) -> u32 {
        self.n_columns
    }

    pub fn input_a(&self) -> &[BaseField] {
        &self.input_a
    }

    pub fn input_b(&self) -> &[BaseField] {
        &self.input_b
    }

    pub fn generate_trace(&self) -> Result<MetalWideFibonacciTrace, MetalWideFibonacciTraceError> {
        self.execution_seed.generate_wide_fibonacci_trace(
            &self.input_a,
            &self.input_b,
            self.n_columns,
        )
    }
}

#[derive(Clone, Debug)]
pub struct MetalWideFibonacciBenchmarkBoundary {
    workload_boundary: MetalWorkloadBoundary,
    target: MetalBenchmarkTarget,
    execution_seed: RegisteredMetalExecutionSeed,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MetalWideFibonacciSentinel {
    pub first_column_first_value: u32,
    pub second_column_first_value: u32,
    pub last_column_first_value: u32,
    pub last_column_last_value: u32,
}

#[derive(Clone, Debug)]
pub struct MetalGeneratedWideFibonacciBenchmarkSample {
    prove_values_result: MetalProveValuesResult,
    pub sentinel: MetalWideFibonacciSentinel,
    pub breakdown: MetalGeneratedWideFibonacciBenchmarkBreakdown,
}

impl MetalGeneratedWideFibonacciBenchmarkSample {
    pub fn prove_values_result(&self) -> &MetalProveValuesResult {
        &self.prove_values_result
    }

    pub fn proof_metadata(&self) -> MetalProofMetadata {
        self.prove_values_result.proof_metadata()
    }

    pub fn proof(&self) -> &StarkProof<Blake2sMerkleHasher> {
        self.prove_values_result.proof()
    }
}

#[derive(Clone, Debug)]
pub struct MetalGeneratedWideFibonacciBenchmarkIteration {
    pub sample: MetalGeneratedWideFibonacciBenchmarkSample,
    pub prove_elapsed_ms: f64,
    pub verify_elapsed_ms: f64,
}

#[derive(Clone, Debug)]
pub struct MetalGeneratedWideFibonacciBenchmarkRun {
    warmup_iterations: usize,
    timed_iterations: Vec<MetalGeneratedWideFibonacciBenchmarkIteration>,
}

impl MetalGeneratedWideFibonacciBenchmarkRun {
    pub fn warmup_iterations(&self) -> usize {
        self.warmup_iterations
    }

    pub fn timed_iterations(&self) -> &[MetalGeneratedWideFibonacciBenchmarkIteration] {
        &self.timed_iterations
    }

    pub fn cold_start_iteration(&self) -> Option<&MetalGeneratedWideFibonacciBenchmarkIteration> {
        self.timed_iterations.first()
    }

    pub fn steady_state_iterations(&self) -> &[MetalGeneratedWideFibonacciBenchmarkIteration] {
        if self.timed_iterations.len() >= 2 {
            &self.timed_iterations[1..]
        } else {
            &[]
        }
    }

    pub fn final_iteration(&self) -> Option<&MetalGeneratedWideFibonacciBenchmarkIteration> {
        self.timed_iterations.last()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MetalGeneratedWideFibonacciBenchmarkBreakdown {
    pub setup_and_preprocessed_commit_ms: f64,
    pub trace_generation_ms: f64,
    pub trace_commit_ms: f64,
    pub trace_commit_interpolation_ms: f64,
    pub trace_commit_extension_ms: f64,
    pub trace_commit_merkle_ms: f64,
    pub prove_core_ms: f64,
    pub prove_core_evaluation_program_v1_ms: f64,
    pub prove_core_composition_generation_ms: f64,
    pub prove_core_composition_commit_ms: f64,
    pub prove_core_prove_values_ms: f64,
    pub prove_core_sanity_check_ms: f64,
    /// Sub-phase timing within composition generation.
    pub prove_core_composition_detail: MetalCompositionDetailBreakdown,
    /// Sub-phase timing within prove values.
    pub prove_core_prove_values_detail: MetalProveValuesDetailBreakdown,
}

// ---------------------------------------------------------------------------
// Benchmark error types
// ---------------------------------------------------------------------------

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum MetalBenchmarkInputError {
    UnsupportedWitnessOwnership,
    InvalidColumnCount {
        n_columns: u32,
    },
    InputLengthMismatch {
        expected_len: usize,
        actual_a: usize,
        actual_b: usize,
    },
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum MetalBenchmarkProgramError {
    UnsupportedComponent {
        workload_name: &'static str,
    },
    InvalidSpecialization {
        workload_name: &'static str,
        n_columns: u32,
    },
    RegisterBudgetOverflow {
        workload_name: &'static str,
    },
    InvalidProgramContract {
        workload_name: &'static str,
    },
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum MetalBenchmarkTraceShapeError {
    LengthMismatch {
        expected_len: usize,
        actual_len: usize,
    },
    ColumnCountMismatch {
        expected_columns: u32,
        actual_columns: u32,
    },
}

#[derive(Debug, Eq, PartialEq)]
pub enum MetalBenchmarkProgramExecutionError {
    ProgramContract {
        source: MetalBenchmarkProgramError,
    },
    TraceShape {
        source: MetalBenchmarkTraceShapeError,
    },
    ReferenceExecution {
        source: MetalEvaluationProgramInterpreterError,
    },
    Execution {
        source: MetalEvaluationProgramExecutionError,
    },
    CompositionShape {
        message: String,
    },
}

#[derive(Debug)]
pub enum MetalBenchmarkProveCoreError {
    ProgramContract {
        source: MetalBenchmarkProgramError,
    },
    ProveRuntime {
        source: MetalProveCoreError,
    },
}

impl From<MetalProveCoreError> for MetalBenchmarkProveCoreError {
    fn from(source: MetalProveCoreError) -> Self {
        Self::ProveRuntime { source }
    }
}

#[derive(Debug)]
pub enum MetalGeneratedWideFibonacciBenchmarkError {
    Input {
        source: MetalBenchmarkInputError,
    },
    TraceGeneration {
        source: MetalWideFibonacciTraceError,
    },
    Program {
        source: MetalBenchmarkProgramExecutionError,
    },
    ProveCore {
        source: MetalBenchmarkProveCoreError,
    },
    Verify {
        source: VerificationError,
    },
}

// ---------------------------------------------------------------------------
// Benchmark boundary implementation
// ---------------------------------------------------------------------------

impl MetalWideFibonacciBenchmarkBoundary {
    pub fn workload_boundary(&self) -> &MetalWorkloadBoundary {
        &self.workload_boundary
    }

    pub fn target(&self) -> &MetalBenchmarkTarget {
        &self.target
    }

    /// Create a V1 prove runtime context from this boundary's program and shape.
    fn prove_runtime_context_v1(
        &self,
    ) -> Result<MetalProveRuntimeContextV1, MetalBenchmarkProgramError> {
        let program = self.evaluation_program_v1()?;
        Ok(MetalProveRuntimeContextV1::new(
            program,
            self.target.log_n_instances,
        ))
    }

    pub fn evaluation_program_v1(
        &self,
    ) -> Result<OwnedMetalEvaluationProgramV1, MetalBenchmarkProgramError> {
        let workload_name = self.workload_boundary.workload_name();
        let program = lower_registered_metal_evaluation_program_v1(
            workload_name,
            MetalEvaluationProgramSpecializationV1 {
                log_n_rows: self.target.log_n_instances,
                n_columns: self.target.n_columns,
            },
        )
        .map_err(|error| match error {
            MetalEvaluationProgramLoweringError::UnsupportedComponent { .. } => {
                MetalBenchmarkProgramError::UnsupportedComponent { workload_name }
            }
            MetalEvaluationProgramLoweringError::InvalidWideFibonacciColumnCount { n_columns }
            | MetalEvaluationProgramLoweringError::InvalidVirtualSnosColumnCount { n_columns } => {
                MetalBenchmarkProgramError::InvalidSpecialization {
                    workload_name,
                    n_columns,
                }
            }
            MetalEvaluationProgramLoweringError::RegisterBudgetOverflow => {
                MetalBenchmarkProgramError::RegisterBudgetOverflow { workload_name }
            }
            MetalEvaluationProgramLoweringError::AbiLayoutMismatch { .. }
            | MetalEvaluationProgramLoweringError::AbiAlignmentMismatch { .. }
            | MetalEvaluationProgramLoweringError::AbiFieldOffsetMismatch { .. } => {
                MetalBenchmarkProgramError::InvalidProgramContract { workload_name }
            }
        })?;

        program
            .validate(MetalEvaluationProgramBudgetV1::new(2048, 512))
            .map_err(|_| MetalBenchmarkProgramError::InvalidProgramContract { workload_name })?;

        Ok(program)
    }

    pub fn ingest_cpu_witness_inputs(
        &self,
        input_a: &[BaseField],
        input_b: &[BaseField],
    ) -> Result<MetalWideFibonacciWitnessInputs, MetalBenchmarkInputError> {
        let log_n_instances = self
            .execution_seed
            .validate_wide_fibonacci_witness_shape(
                input_a.len(),
                input_b.len(),
                self.target.n_columns,
            )
            .map_err(|error| match error {
                super::execution_plan::RegisteredMetalExecutionSeedError::UnsupportedCpuOwnership {
                    ..
                }
                | super::execution_plan::RegisteredMetalExecutionSeedError::UnsupportedWitnessHook {
                    ..
                } => MetalBenchmarkInputError::UnsupportedWitnessOwnership,
                super::execution_plan::RegisteredMetalExecutionSeedError::WitnessInputLengthMismatch {
                    input_a_len,
                    input_b_len,
                    ..
                } => {
                    let expected_len = 1usize << self.target.log_n_instances;
                    if input_a_len == expected_len && input_b_len == expected_len {
                        unreachable!(
                            "seed witness-shape validation only reports mismatched lengths when the benchmark expected-length precondition is not already satisfied"
                        );
                    }
                    MetalBenchmarkInputError::InputLengthMismatch {
                        expected_len,
                        actual_a: input_a_len,
                        actual_b: input_b_len,
                    }
                }
                super::execution_plan::RegisteredMetalExecutionSeedError::WitnessInputLengthNotPowerOfTwo {
                    input_len,
                    ..
                } => MetalBenchmarkInputError::InputLengthMismatch {
                    expected_len: 1usize << self.target.log_n_instances,
                    actual_a: input_len,
                    actual_b: input_b.len(),
                },
                super::execution_plan::RegisteredMetalExecutionSeedError::InvalidWitnessColumnCount {
                    n_columns,
                    ..
                } => MetalBenchmarkInputError::InvalidColumnCount { n_columns },
                other => unreachable!(
                    "benchmark witness staging should only delegate to witness-shape seed checks, got {other:?}"
                ),
            })?;
        if self.target.n_columns < 3 {
            return Err(MetalBenchmarkInputError::InvalidColumnCount {
                n_columns: self.target.n_columns,
            });
        }

        let expected_len = 1usize << self.target.log_n_instances;
        if log_n_instances != self.target.log_n_instances
            || input_a.len() != expected_len
            || input_b.len() != expected_len
        {
            return Err(MetalBenchmarkInputError::InputLengthMismatch {
                expected_len,
                actual_a: input_a.len(),
                actual_b: input_b.len(),
            });
        }

        Ok(MetalWideFibonacciWitnessInputs {
            log_n_instances,
            n_columns: self.target.n_columns,
            input_a: input_a.to_vec(),
            input_b: input_b.to_vec(),
            execution_seed: self.execution_seed,
        })
    }

    pub fn execute_evaluation_program_v1_on_trace(
        &self,
        trace: &MetalWideFibonacciTrace,
        random_coeff_powers: &[SecureField],
    ) -> Result<Vec<SecureField>, MetalBenchmarkProgramExecutionError> {
        let (row_res, _) =
            self.execute_selected_evaluation_program_v1_on_trace(trace, random_coeff_powers)?;
        Ok(row_res)
    }

    pub fn select_evaluation_program_v1_dispatch(
        &self,
    ) -> Result<MetalEvaluationProgramDispatchKindV1, MetalBenchmarkProgramExecutionError> {
        let program = self
            .evaluation_program_v1()
            .map_err(|source| MetalBenchmarkProgramExecutionError::ProgramContract { source })?;
        select_metal_evaluation_program_dispatch_v1(
            &program,
            MetalEvaluationProgramCapabilityProfileV1::current(),
        )
        .map_err(|source| MetalBenchmarkProgramExecutionError::Execution { source })
    }

    pub fn execute_selected_evaluation_program_v1_on_trace(
        &self,
        trace: &MetalWideFibonacciTrace,
        random_coeff_powers: &[SecureField],
    ) -> Result<
        (Vec<SecureField>, MetalEvaluationProgramDispatchKindV1),
        MetalBenchmarkProgramExecutionError,
    > {
        let trace_columns = self
            .materialize_trace_columns(trace)
            .map_err(|source| MetalBenchmarkProgramExecutionError::TraceShape { source })?;
        let trace_column_refs = trace_columns.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let trace_interactions = [&[][..], trace_column_refs.as_slice()];
        let program = self
            .evaluation_program_v1()
            .map_err(|source| MetalBenchmarkProgramExecutionError::ProgramContract { source })?;
        let runtime = MetalEvaluationProgramRuntimeInputsV1 {
            trace: MetalEvaluationProgramTraceViewV1 {
                trace_interactions: &trace_interactions,
                preprocessed_columns: &[],
            },
            base_params: &[],
            ext_params: &[],
            random_coeff_powers,
        };
        execute_selected_metal_evaluation_program_v1_on_metal(
            &program,
            runtime,
            MetalEvaluationProgramCapabilityProfileV1::current(),
        )
        .map_err(|source| MetalBenchmarkProgramExecutionError::Execution { source })
    }

    pub fn interpret_evaluation_program_v1_on_trace(
        &self,
        trace: &MetalWideFibonacciTrace,
        random_coeff_powers: &[SecureField],
    ) -> Result<Vec<SecureField>, MetalBenchmarkProgramExecutionError> {
        let trace_columns = self
            .materialize_trace_columns(trace)
            .map_err(|source| MetalBenchmarkProgramExecutionError::TraceShape { source })?;
        let trace_column_refs = trace_columns.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let trace_interactions = [&[][..], trace_column_refs.as_slice()];
        let program = self
            .evaluation_program_v1()
            .map_err(|source| MetalBenchmarkProgramExecutionError::ProgramContract { source })?;
        let runtime = MetalEvaluationProgramRuntimeInputsV1 {
            trace: MetalEvaluationProgramTraceViewV1 {
                trace_interactions: &trace_interactions,
                preprocessed_columns: &[],
            },
            base_params: &[],
            ext_params: &[],
            random_coeff_powers,
        };
        interpret_metal_evaluation_program_v1(&program, runtime)
            .map_err(|source| MetalBenchmarkProgramExecutionError::ReferenceExecution { source })
    }

    fn materialize_trace_columns(
        &self,
        trace: &MetalWideFibonacciTrace,
    ) -> Result<Vec<Vec<BaseField>>, MetalBenchmarkTraceShapeError> {
        let expected_len = 1usize << self.target.log_n_instances;
        if trace.input_len() != expected_len {
            return Err(MetalBenchmarkTraceShapeError::LengthMismatch {
                expected_len,
                actual_len: trace.input_len(),
            });
        }
        if trace.n_columns() != self.target.n_columns {
            return Err(MetalBenchmarkTraceShapeError::ColumnCountMismatch {
                expected_columns: self.target.n_columns,
                actual_columns: trace.n_columns(),
            });
        }

        Ok((0..trace.n_columns() as usize)
            .map(|column_index| trace.column_values(column_index))
            .collect())
    }

    pub fn run_generated_blake2s_sample(
        &self,
        input_a: &[BaseField],
        input_b: &[BaseField],
        config: PcsConfig,
    ) -> Result<MetalGeneratedWideFibonacciBenchmarkSample, MetalGeneratedWideFibonacciBenchmarkError>
    {
        let setup_start = std::time::Instant::now();
        let twiddles = MetalBackend::precompute_twiddles(
            CanonicCoset::new(
                self.target.log_n_instances + 1 + config.fri_config.log_blowup_factor,
            )
            .circle_domain()
            .half_coset,
        );

        let prover_channel = &mut Blake2sChannel::default();
        let mut commitment_scheme =
            CommitmentSchemeProver::<MetalBackend, Blake2sMerkleChannel>::new(config, &twiddles);

        let mut tree_builder = commitment_scheme.tree_builder();
        tree_builder.extend_evals(vec![]);
        tree_builder.commit(prover_channel);

        commitment_scheme.set_store_polynomials_coefficients();
        let setup_and_preprocessed_commit_ms = setup_start.elapsed().as_secs_f64() * 1000.0;

        let trace_generation_start = std::time::Instant::now();
        let witness_inputs = self
            .ingest_cpu_witness_inputs(input_a, input_b)
            .map_err(|source| MetalGeneratedWideFibonacciBenchmarkError::Input { source })?;
        let native_trace = witness_inputs.generate_trace().map_err(|source| {
            MetalGeneratedWideFibonacciBenchmarkError::TraceGeneration { source }
        })?;
        let sentinel = sentinel_from_metal_trace(
            &native_trace,
            1usize << self.target.log_n_instances,
            self.target.n_columns as usize,
        );
        let trace = native_trace.to_metal_evaluations();
        let trace_generation_ms = trace_generation_start.elapsed().as_secs_f64() * 1000.0;

        let trace_commit_start = std::time::Instant::now();
        let trace_commit_breakdown = runtime_commit_trace_with_breakdown(
            &mut commitment_scheme,
            prover_channel,
            trace,
            &twiddles,
        );
        let trace_commit_ms = trace_commit_start.elapsed().as_secs_f64() * 1000.0;

        let component = WideFibonacciBenchmarkComponent::new(
            self.target.log_n_instances,
            self.target.n_columns as usize,
        );
        let ctx = self.prove_runtime_context_v1().map_err(|source| {
            MetalGeneratedWideFibonacciBenchmarkError::ProveCore {
                source: MetalBenchmarkProveCoreError::ProgramContract { source },
            }
        })?;
        let prove_core_start = std::time::Instant::now();
        let (prove_values_result, prove_core_breakdown) =
            execute_prove_core_v1(&ctx, &[&component], prover_channel, commitment_scheme)
                .map_err(|source| MetalGeneratedWideFibonacciBenchmarkError::ProveCore {
                    source: MetalBenchmarkProveCoreError::from(source),
                })?;
        let prove_core_ms = prove_core_start.elapsed().as_secs_f64() * 1000.0;

        Ok(MetalGeneratedWideFibonacciBenchmarkSample {
            prove_values_result,
            sentinel,
            breakdown: MetalGeneratedWideFibonacciBenchmarkBreakdown {
                setup_and_preprocessed_commit_ms,
                trace_generation_ms,
                trace_commit_ms,
                trace_commit_interpolation_ms: trace_commit_breakdown.interpolation_ms,
                trace_commit_extension_ms: trace_commit_breakdown.extension_ms,
                trace_commit_merkle_ms: trace_commit_breakdown.merkle_ms,
                prove_core_ms,
                prove_core_evaluation_program_v1_ms: prove_core_breakdown.evaluation_program_v1_ms,
                prove_core_composition_generation_ms: prove_core_breakdown
                    .composition_generation_ms,
                prove_core_composition_commit_ms: prove_core_breakdown.composition_commit_ms,
                prove_core_prove_values_ms: prove_core_breakdown.prove_values_ms,
                prove_core_sanity_check_ms: prove_core_breakdown.sanity_check_ms,
                prove_core_composition_detail: prove_core_breakdown.composition_detail,
                prove_core_prove_values_detail: prove_core_breakdown.prove_values_detail,
            },
        })
    }

    pub fn verify_generated_blake2s_sample(
        &self,
        sample: &MetalGeneratedWideFibonacciBenchmarkSample,
    ) -> Result<(), VerificationError> {
        let component = WideFibonacciBenchmarkComponent::new(
            self.target.log_n_instances,
            self.target.n_columns as usize,
        );
        verify_wide_fibonacci_blake(&component, sample.proof())
    }

    pub fn run_generated_blake2s_iteration(
        &self,
        input_a: &[BaseField],
        input_b: &[BaseField],
        config: PcsConfig,
    ) -> Result<
        MetalGeneratedWideFibonacciBenchmarkIteration,
        MetalGeneratedWideFibonacciBenchmarkError,
    > {
        let prove_start = std::time::Instant::now();
        let sample = self.run_generated_blake2s_sample(input_a, input_b, config)?;
        let prove_elapsed_ms = prove_start.elapsed().as_secs_f64() * 1000.0;

        let verify_start = std::time::Instant::now();
        self.verify_generated_blake2s_sample(&sample)
            .map_err(|source| MetalGeneratedWideFibonacciBenchmarkError::Verify { source })?;
        let verify_elapsed_ms = verify_start.elapsed().as_secs_f64() * 1000.0;

        Ok(MetalGeneratedWideFibonacciBenchmarkIteration {
            sample,
            prove_elapsed_ms,
            verify_elapsed_ms,
        })
    }

    pub fn run_generated_blake2s_benchmark(
        &self,
        input_a: &[BaseField],
        input_b: &[BaseField],
        config: PcsConfig,
        warmup_iterations: usize,
        timed_iterations: usize,
    ) -> Result<MetalGeneratedWideFibonacciBenchmarkRun, MetalGeneratedWideFibonacciBenchmarkError>
    {
        for _ in 0..warmup_iterations {
            let _ = self.run_generated_blake2s_iteration(input_a, input_b, config)?;
        }

        let mut iterations = Vec::with_capacity(timed_iterations);
        for _ in 0..timed_iterations {
            iterations.push(self.run_generated_blake2s_iteration(input_a, input_b, config)?);
        }

        Ok(MetalGeneratedWideFibonacciBenchmarkRun {
            warmup_iterations,
            timed_iterations: iterations,
        })
    }
}

// ---------------------------------------------------------------------------
// Wide fibonacci benchmark component
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct WideFibonacciBenchmarkComponent {
    log_n_rows: u32,
    n_columns: usize,
}

impl WideFibonacciBenchmarkComponent {
    fn new(log_n_rows: u32, n_columns: usize) -> Self {
        assert!(
            n_columns >= 3,
            "wide fibonacci benchmark requires at least 3 columns"
        );
        Self {
            log_n_rows,
            n_columns,
        }
    }

    fn denom_inverse(&self, point: stwo::core::circle::CirclePoint<SecureField>) -> SecureField {
        coset_vanishing(CanonicCoset::new(self.log_n_rows).coset(), point).inverse()
    }
}

impl stwo::core::air::Component for WideFibonacciBenchmarkComponent {
    fn n_constraints(&self) -> usize {
        self.n_columns - 2
    }

    fn max_constraint_log_degree_bound(&self) -> u32 {
        self.log_n_rows + 1
    }

    fn trace_log_degree_bounds(&self) -> TreeVec<Vec<u32>> {
        TreeVec::new(vec![vec![], vec![self.log_n_rows; self.n_columns]])
    }

    fn mask_points(
        &self,
        point: stwo::core::circle::CirclePoint<SecureField>,
        _max_log_degree_bound: u32,
    ) -> TreeVec<Vec<Vec<stwo::core::circle::CirclePoint<SecureField>>>> {
        TreeVec::new(vec![
            vec![],
            (0..self.n_columns).map(|_| vec![point]).collect(),
        ])
    }

    fn preprocessed_column_indices(&self) -> Vec<usize> {
        vec![]
    }

    fn evaluate_constraint_quotients_at_point(
        &self,
        point: stwo::core::circle::CirclePoint<SecureField>,
        mask: &TreeVec<Vec<Vec<SecureField>>>,
        evaluation_accumulator: &mut stwo::core::air::accumulation::PointEvaluationAccumulator,
        _max_log_degree_bound: u32,
    ) {
        let trace_mask = &mask[MAIN_TRACE_IDX];
        assert_eq!(trace_mask.len(), self.n_columns);

        let denom_inverse = self.denom_inverse(point);
        let mut a = trace_mask[0][0];
        let mut b = trace_mask[1][0];
        for column in trace_mask.iter().skip(2) {
            let c = column[0];
            evaluation_accumulator.accumulate(denom_inverse * (c - (a.square() + b.square())));
            a = b;
            b = c;
        }
    }
}

impl ComponentProver<MetalBackend> for WideFibonacciBenchmarkComponent {
    fn evaluate_constraint_quotients_on_domain(
        &self,
        trace: &Trace<'_, MetalBackend>,
        evaluation_accumulator: &mut DomainEvaluationAccumulator<MetalBackend>,
    ) {
        if self.n_constraints() == 0 {
            return;
        }

        let eval_domain = CanonicCoset::new(self.max_constraint_log_degree_bound()).circle_domain();
        let trace_domain = CanonicCoset::new(self.log_n_rows);
        let trace_columns = &trace.polys[MAIN_TRACE_IDX];
        assert_eq!(trace_columns.len(), self.n_columns);

        if trace_columns.iter().any(|poly| poly.coeffs.is_none())
            && trace_columns
                .iter()
                .any(|poly| poly.evals.domain != eval_domain)
        {
            panic!("wide fibonacci benchmark requires stored trace coefficients for eval-domain extension");
        }

        let log_expand = eval_domain.log_size() - trace_domain.log_size();
        let mut denominator_inverses = (0..(1 << log_expand))
            .map(|index| coset_vanishing(trace_domain.coset(), eval_domain.at(index)).inverse())
            .collect::<Vec<_>>();
        stwo::core::utils::bit_reverse(&mut denominator_inverses);

        let [mut accum] =
            evaluation_accumulator.columns([(eval_domain.log_size(), self.n_constraints())]);
        accum.random_coeff_powers.reverse();
        let quotients = if trace_columns
            .iter()
            .all(|poly| poly.evals.domain == eval_domain)
        {
            let trace1_evaluation_refs = trace_columns
                .iter()
                .map(|poly| &poly.evals.values)
                .collect::<Vec<_>>();
            accumulate_wide_fibonacci_quotients(MetalWideFibonacciQuotientRequest {
                trace_evaluations: &trace1_evaluation_refs,
                random_coeff_powers: &accum.random_coeff_powers,
                denominator_inverses: &denominator_inverses,
                domain_log_size: trace_domain.log_size(),
                eval_domain_log_size: eval_domain.log_size(),
            })
        } else {
            let twiddles = MetalBackend::precompute_twiddles(eval_domain.half_coset);
            let coeffs = trace_columns
                .iter()
                .map(|poly| {
                    poly.coeffs.as_ref().expect(
                        "wide fibonacci benchmark requires stored trace coefficients for eval-domain extension",
                    )
                })
                .collect::<Vec<_>>();
            let trace_batch = evaluate_polys_on_domain_batch(&coeffs, eval_domain, &twiddles)
                .expect("wide-fibonacci trace batch evaluation should stay on Metal");
            accumulate_wide_fibonacci_quotients_from_batch(MetalWideFibonacciBatchQuotientRequest {
                trace_evaluations: &trace_batch,
                random_coeff_powers: &accum.random_coeff_powers,
                denominator_inverses: &denominator_inverses,
                domain_log_size: trace_domain.log_size(),
                eval_domain_log_size: eval_domain.log_size(),
            })
        }
        .expect("wide-fibonacci quotient accumulation should succeed through the native Metal path");
        let quotient_columns = quotients.into_coordinate_base_columns();
        for (dst, src) in accum.col.columns.iter_mut().zip(quotient_columns) {
            *dst = src;
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn sentinel_from_metal_trace(
    trace: &MetalWideFibonacciTrace,
    input_len: usize,
    n_columns: usize,
) -> MetalWideFibonacciSentinel {
    MetalWideFibonacciSentinel {
        first_column_first_value: trace.value(0, 0).0,
        second_column_first_value: trace.value(1, 0).0,
        last_column_first_value: trace.value(n_columns - 1, 0).0,
        last_column_last_value: trace.value(n_columns - 1, input_len - 1).0,
    }
}

fn verify_wide_fibonacci_blake(
    component: &WideFibonacciBenchmarkComponent,
    proof: &StarkProof<Blake2sMerkleHasher>,
) -> Result<(), VerificationError> {
    let verifier_channel = &mut Blake2sChannel::default();
    let commitment_scheme =
        &mut CommitmentSchemeVerifier::<Blake2sMerkleChannel>::new(proof.config);
    let sizes = component.trace_log_degree_bounds();
    commitment_scheme.commit(
        proof.commitments[PREPROCESSED_TRACE_IDX],
        &sizes[PREPROCESSED_TRACE_IDX],
        verifier_channel,
    );
    commitment_scheme.commit(
        proof.commitments[MAIN_TRACE_IDX],
        &sizes[MAIN_TRACE_IDX],
        verifier_channel,
    );
    verify(
        &[component as &dyn stwo::core::air::Component],
        verifier_channel,
        commitment_scheme,
        proof.clone(),
    )
}

// ---------------------------------------------------------------------------
// Target constants
// ---------------------------------------------------------------------------

pub const WIDE_FIBONACCI_TRACE_LOG20_TARGET: MetalBenchmarkTarget = MetalBenchmarkTarget {
    benchmark_id: "wide_fibonacci_trace_generation_v1",
    workload_name: "fibonacci_example",
    family: "wide_fibonacci",
    operation: MetalBenchmarkOperation::TraceGeneration,
    log_n_instances: 20,
    n_columns: 100,
    reference_platform: MetalBenchmarkReferencePlatform::Rtx4090Cuda,
    reference_elapsed_ms: 90.0,
};

pub const WIDE_FIBONACCI_PROVE_LOG20_TARGET: MetalBenchmarkTarget = MetalBenchmarkTarget {
    benchmark_id: "wide_fibonacci_prove_verify_v1",
    workload_name: "fibonacci_example",
    family: "wide_fibonacci",
    operation: MetalBenchmarkOperation::ProveVerify,
    log_n_instances: 20,
    n_columns: 100,
    reference_platform: MetalBenchmarkReferencePlatform::Rtx4090Cuda,
    reference_elapsed_ms: 90.0,
};

// ---------------------------------------------------------------------------
// Boundary declaration
// ---------------------------------------------------------------------------

pub fn declare_wide_fibonacci_benchmark_boundary(
    intent: MetalExecutionIntent,
    target: MetalBenchmarkTarget,
) -> Result<MetalWideFibonacciBenchmarkBoundary, MetalPlannerError<'static>> {
    let benchmark_route = match target.operation {
        MetalBenchmarkOperation::TraceGeneration => {
            MetalGeneratedRouteKind::BenchmarkTraceGeneration
        }
        MetalBenchmarkOperation::ProveVerify => MetalGeneratedRouteKind::BenchmarkProveVerify,
    };
    let binding = registered_execution_binding(intent, target.workload_name, benchmark_route)?;
    let execution_seed = registered_execution_seed(intent, target.workload_name, benchmark_route)?;
    let benchmark_operation = match target.operation {
        MetalBenchmarkOperation::TraceGeneration => {
            MetalRegisteredBenchmarkOperation::TraceGeneration
        }
        MetalBenchmarkOperation::ProveVerify => MetalRegisteredBenchmarkOperation::ProveVerify,
    };

    assert_eq!(
        binding.workload_boundary.lowering.workload_family,
        target.family
    );
    assert_eq!(binding.declaration_lowering.workload_family, target.family);
    assert!(binding
        .declaration_lowering
        .abi_symbols
        .iter()
        .all(|symbol| symbol.starts_with("metal.")));
    assert!(binding
        .supported_benchmark_operations
        .contains(&benchmark_operation));
    assert_eq!(execution_seed.component_name, target.workload_name);
    assert_eq!(execution_seed.plan, binding.workload_boundary.plan);
    assert!(execution_seed
        .abi_symbols
        .iter()
        .all(|symbol| symbol.starts_with("metal.")));
    assert!(execution_seed.build_modules.contains(&"benchmark"));
    assert!(binding
        .workload_boundary
        .generated_inventory
        .specialization_keys
        .contains(&"log_n_instances"));
    assert!(binding
        .workload_boundary
        .generated_inventory
        .specialization_keys
        .contains(&"n_columns"));
    let workload_boundary = declare_exemplar_metal_workload_boundary(intent, target.workload_name)?;
    let inventory = workload_boundary.generated_inventory();

    assert_eq!(binding.workload_boundary.generated_inventory, inventory);
    assert_eq!(binding.workload_boundary.plan, workload_boundary.plan());

    Ok(MetalWideFibonacciBenchmarkBoundary {
        workload_boundary,
        target,
        execution_seed,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use stwo::core::air::Component;
    use stwo::core::channel::Blake2sChannel;
    use stwo::core::fields::m31::BaseField;
    use stwo::core::fields::qm31::SecureField;
    use stwo::core::pcs::TreeVec;
    use stwo::core::pcs::PcsConfig;
    use stwo::core::pcs::quotients::CommitmentSchemeProof;
    use stwo::core::proof::StarkProof;

    use stwo::prover::ComponentProvers;

    use super::{
        declare_wide_fibonacci_benchmark_boundary, MetalBenchmarkInputError,
        MetalBenchmarkOperation, MetalBenchmarkProgramExecutionError,
        MetalBenchmarkReferencePlatform, MetalExecutionIntent, WideFibonacciBenchmarkComponent,
        WIDE_FIBONACCI_PROVE_LOG20_TARGET, WIDE_FIBONACCI_TRACE_LOG20_TARGET,
    };
    use crate::backend::metal::artifact::MetalGeneratedRouteKind;
    use crate::backend::metal::prove_runtime_v1::{
        interpret_post_composition_sampled_values_v1, MetalPostCompositionSampledValuesV1,
        MetalProveRuntimeLaneError, validate_prove_runtime_lane_v1,
    };
    use crate::backend::metal::sampled_values_v1::{
        lower_metal_sampled_values_v1, MetalSampledValuesDispatchKindV1,
    };
    use crate::backend::metal::{
        metal_runtime_support, MetalEvaluationProgramDispatchKindV1, MetalExecutionPlan,
        MetalPlannerError, MetalRuntimeSupport, MetalWorkloadOwnership, MetalWorkloadStage,
        UnsupportedGeneratedMetalRoute,
    };

    #[test]
    fn wide_fibonacci_targets_are_fixed_to_log20_and_rtx4090_reference() {
        assert_eq!(WIDE_FIBONACCI_TRACE_LOG20_TARGET.log_n_instances, 20);
        assert_eq!(WIDE_FIBONACCI_TRACE_LOG20_TARGET.n_columns, 100);
        assert_eq!(
            WIDE_FIBONACCI_TRACE_LOG20_TARGET.reference_platform,
            MetalBenchmarkReferencePlatform::Rtx4090Cuda
        );
        assert_eq!(WIDE_FIBONACCI_PROVE_LOG20_TARGET.log_n_instances, 20);
        assert_eq!(WIDE_FIBONACCI_PROVE_LOG20_TARGET.n_columns, 100);
        assert_eq!(WIDE_FIBONACCI_PROVE_LOG20_TARGET.reference_elapsed_ms, 90.0);
        assert_eq!(
            WIDE_FIBONACCI_PROVE_LOG20_TARGET.operation,
            MetalBenchmarkOperation::ProveVerify
        );
    }

    #[test]
    fn wide_fibonacci_benchmark_boundary_is_hybrid_and_cpu_witness_owned() {
        let boundary = declare_wide_fibonacci_benchmark_boundary(
            MetalExecutionIntent::PreferMetal,
            WIDE_FIBONACCI_PROVE_LOG20_TARGET,
        )
        .unwrap();

        assert_eq!(
            boundary.workload_boundary().plan(),
            MetalExecutionPlan::MetalFriHybrid
        );
        assert_eq!(
            boundary
                .workload_boundary()
                .stage_ownership(MetalWorkloadStage::WitnessMain),
            Some(MetalWorkloadOwnership::CpuOwned)
        );
        assert_eq!(
            boundary
                .workload_boundary()
                .generated_inventory()
                .registration_key,
            "fibonacci_example"
        );
        assert_eq!(
            boundary
                .workload_boundary()
                .generated_inventory()
                .specialization_keys,
            &["log_n_instances", "n_columns"]
        );
        validate_prove_runtime_lane_v1(
            boundary.workload_boundary().workload_name(),
            boundary.workload_boundary().plan(),
            boundary
                .workload_boundary()
                .stage_ownership(MetalWorkloadStage::WitnessMain),
            boundary
                .workload_boundary()
                .stage_ownership(MetalWorkloadStage::FriBlake2s),
        )
        .unwrap();
    }

    #[test]
    fn wide_fibonacci_benchmark_boundary_validates_input_lengths() {
        let boundary = declare_wide_fibonacci_benchmark_boundary(
            MetalExecutionIntent::PreferMetal,
            WIDE_FIBONACCI_TRACE_LOG20_TARGET,
        )
        .unwrap();
        let ok_len = 1usize << 20;
        let input_a = vec![BaseField::from_u32_unchecked(1); ok_len];
        let input_b = vec![BaseField::from_u32_unchecked(2); ok_len - 1];

        let error = boundary
            .ingest_cpu_witness_inputs(&input_a, &input_b)
            .unwrap_err();

        assert_eq!(
            error,
            MetalBenchmarkInputError::InputLengthMismatch {
                expected_len: ok_len,
                actual_a: ok_len,
                actual_b: ok_len - 1,
            }
        );
    }

    #[test]
    fn benchmark_witness_inputs_carry_registered_execution_seed() {
        let target = super::MetalBenchmarkTarget {
            log_n_instances: 3,
            n_columns: 8,
            ..WIDE_FIBONACCI_TRACE_LOG20_TARGET
        };
        let boundary =
            declare_wide_fibonacci_benchmark_boundary(MetalExecutionIntent::PreferMetal, target)
                .unwrap();
        let input_a = vec![BaseField::from_u32_unchecked(1); 1 << target.log_n_instances];
        let input_b = vec![BaseField::from_u32_unchecked(2); 1 << target.log_n_instances];

        let inputs = boundary
            .ingest_cpu_witness_inputs(&input_a, &input_b)
            .unwrap();
        let request = inputs
            .execution_seed
            .wide_fibonacci_trace_request(inputs.input_a(), inputs.input_b(), inputs.n_columns())
            .unwrap();

        assert_eq!(inputs.execution_seed.component_name, "fibonacci_example");
        assert_eq!(
            inputs.execution_seed.plan,
            boundary.workload_boundary().plan()
        );
        assert_eq!(request.n_columns, target.n_columns);
    }

    #[test]
    fn benchmark_boundary_executes_v1_program_on_generated_trace() {
        if !matches!(metal_runtime_support(), MetalRuntimeSupport::Available) {
            return;
        }

        let target = super::MetalBenchmarkTarget {
            log_n_instances: 3,
            n_columns: 6,
            ..WIDE_FIBONACCI_PROVE_LOG20_TARGET
        };
        let boundary =
            declare_wide_fibonacci_benchmark_boundary(MetalExecutionIntent::PreferMetal, target)
                .unwrap();
        let input_a = vec![BaseField::from_u32_unchecked(1); 1 << target.log_n_instances];
        let input_b = vec![BaseField::from_u32_unchecked(2); 1 << target.log_n_instances];
        let random_coeff_powers = vec![
            SecureField::from_u32_unchecked(3, 5, 7, 11),
            SecureField::from_u32_unchecked(13, 17, 19, 23),
            SecureField::from_u32_unchecked(29, 31, 37, 41),
            SecureField::from_u32_unchecked(43, 47, 53, 59),
        ];
        let trace = boundary
            .ingest_cpu_witness_inputs(&input_a, &input_b)
            .unwrap()
            .generate_trace()
            .unwrap();
        let dispatch = boundary.select_evaluation_program_v1_dispatch().unwrap();

        let reference = boundary
            .interpret_evaluation_program_v1_on_trace(&trace, &random_coeff_powers)
            .unwrap();
        let (device, selected_dispatch) = boundary
            .execute_selected_evaluation_program_v1_on_trace(&trace, &random_coeff_powers)
            .unwrap();

        assert_eq!(
            dispatch,
            MetalEvaluationProgramDispatchKindV1::GenericMetalInterpreter
        );
        assert_eq!(selected_dispatch, dispatch);
        assert_eq!(device, reference);
    }

    #[test]
    fn benchmark_boundary_rejects_trace_shape_drift_for_v1_execution() {
        let boundary = declare_wide_fibonacci_benchmark_boundary(
            MetalExecutionIntent::PreferMetal,
            WIDE_FIBONACCI_PROVE_LOG20_TARGET,
        )
        .unwrap();
        let short_target = super::MetalBenchmarkTarget {
            log_n_instances: 3,
            n_columns: 6,
            ..WIDE_FIBONACCI_PROVE_LOG20_TARGET
        };
        let short_boundary = declare_wide_fibonacci_benchmark_boundary(
            MetalExecutionIntent::PreferMetal,
            short_target,
        )
        .unwrap();
        let input_a = vec![BaseField::from_u32_unchecked(1); 1 << short_target.log_n_instances];
        let input_b = vec![BaseField::from_u32_unchecked(2); 1 << short_target.log_n_instances];
        let trace = short_boundary
            .ingest_cpu_witness_inputs(&input_a, &input_b)
            .unwrap()
            .generate_trace()
            .unwrap();

        let error = boundary
            .interpret_evaluation_program_v1_on_trace(
                &trace,
                &[SecureField::from_u32_unchecked(1, 0, 0, 0); 98],
            )
            .unwrap_err();

        assert_eq!(
            error,
            MetalBenchmarkProgramExecutionError::TraceShape {
                source: super::MetalBenchmarkTraceShapeError::LengthMismatch {
                    expected_len: 1 << 20,
                    actual_len: 1 << short_target.log_n_instances,
                },
            }
        );
    }

    #[test]
    fn benchmark_boundary_interprets_post_composition_sampled_values_v1() {
        let target = super::MetalBenchmarkTarget {
            log_n_instances: 3,
            n_columns: 6,
            ..WIDE_FIBONACCI_PROVE_LOG20_TARGET
        };
        let boundary =
            declare_wide_fibonacci_benchmark_boundary(MetalExecutionIntent::PreferMetal, target)
                .unwrap();
        let component = WideFibonacciBenchmarkComponent::new(
            target.log_n_instances,
            target.n_columns as usize,
        );
        let component_provers = ComponentProvers {
            components: vec![&component],
            n_preprocessed_columns: 0,
        };
        let point =
            stwo::core::circle::CirclePoint::get_random_point(&mut Blake2sChannel::default());
        let max_log_degree_bound = component.max_constraint_log_degree_bound();
        let sample_points =
            component_provers
                .components()
                .mask_points(point, max_log_degree_bound, false);
        let sampled_values = TreeVec(
            sample_points
                .0
                .iter()
                .enumerate()
                .map(|(tree_index, tree)| {
                    tree.iter()
                        .enumerate()
                        .map(|(column_index, column)| {
                            column
                                .iter()
                                .enumerate()
                                .map(|(sample_index, _)| {
                                    SecureField::from_u32_unchecked(
                                        (tree_index + 1) as u32,
                                        (column_index + 2) as u32,
                                        (sample_index + 3) as u32,
                                        (tree_index + column_index + sample_index + 4) as u32,
                                    )
                                })
                                .collect()
                        })
                        .collect()
                })
                .collect(),
        );
        let lowered = lower_metal_sampled_values_v1(&sampled_values).unwrap();
        let random_coeff = SecureField::from_u32_unchecked(3, 5, 7, 11);

        let interpreted = interpret_post_composition_sampled_values_v1(
            &component_provers.components(),
            point,
            &MetalPostCompositionSampledValuesV1::from_owned(lowered),
            random_coeff,
            max_log_degree_bound,
        )
        .unwrap();
        let legacy = component_provers.components().eval_composition_polynomial_at_point(
            point,
            &sampled_values,
            random_coeff,
            max_log_degree_bound,
        );

        assert_eq!(interpreted, legacy);
    }

    #[test]
    fn benchmark_boundary_rejects_prove_values_lane_for_cpu_plan() {
        let boundary = declare_wide_fibonacci_benchmark_boundary(
            MetalExecutionIntent::ForceCpu,
            WIDE_FIBONACCI_PROVE_LOG20_TARGET,
        )
        .unwrap();

        assert!(matches!(
            validate_prove_runtime_lane_v1(
                boundary.workload_boundary().workload_name(),
                boundary.workload_boundary().plan(),
                boundary
                    .workload_boundary()
                    .stage_ownership(MetalWorkloadStage::WitnessMain),
                boundary
                    .workload_boundary()
                    .stage_ownership(MetalWorkloadStage::FriBlake2s),
            ),
            Err(MetalProveRuntimeLaneError::PlanNotMetalCapable { .. })
        ));
    }

    #[test]
    fn benchmark_boundary_fails_closed_for_unsupported_generated_route() {
        let error = declare_wide_fibonacci_benchmark_boundary(
            MetalExecutionIntent::PreferMetal,
            super::MetalBenchmarkTarget {
                workload_name: "poseidon_example",
                family: "poseidon",
                operation: MetalBenchmarkOperation::ProveVerify,
                ..WIDE_FIBONACCI_PROVE_LOG20_TARGET
            },
        )
        .unwrap_err();

        assert_eq!(
            error,
            MetalPlannerError::UnsupportedGeneratedRoute(UnsupportedGeneratedMetalRoute {
                component_name: "poseidon_example",
                route: MetalGeneratedRouteKind::BenchmarkProveVerify,
            })
        );
    }

    #[test]
    fn generated_benchmark_run_exposes_cold_and_steady_state_views() {
        let iteration = super::MetalGeneratedWideFibonacciBenchmarkIteration {
            sample: super::MetalGeneratedWideFibonacciBenchmarkSample {
                prove_values_result: super::MetalProveValuesResult::new_for_test(
                    StarkProof(CommitmentSchemeProof {
                        config: PcsConfig::default(),
                        commitments: TreeVec::default(),
                        sampled_values: TreeVec::default(),
                        decommitments: TreeVec::default(),
                        queried_values: TreeVec::default(),
                        proof_of_work: 0,
                        fri_proof: stwo::core::fri::FriProof {
                            first_layer: stwo::core::fri::FriLayerProof {
                                fri_witness: Vec::new(),
                                decommitment: Default::default(),
                                commitment: Default::default(),
                            },
                            inner_layers: Vec::new(),
                            last_layer_poly: stwo::core::poly::line::LinePoly::new(vec![
                                SecureField::from_u32_unchecked(0, 0, 0, 0),
                            ]),
                        },
                    }),
                    SecureField::from_u32_unchecked(0, 0, 0, 0),
                    MetalSampledValuesDispatchKindV1::ReferenceInterpreter,
                ),
                sentinel: super::MetalWideFibonacciSentinel {
                    first_column_first_value: 1,
                    second_column_first_value: 2,
                    last_column_first_value: 3,
                    last_column_last_value: 4,
                },
                breakdown: super::MetalGeneratedWideFibonacciBenchmarkBreakdown {
                    setup_and_preprocessed_commit_ms: 1.0,
                    trace_generation_ms: 2.0,
                    trace_commit_ms: 3.0,
                    trace_commit_interpolation_ms: 4.0,
                    trace_commit_extension_ms: 5.0,
                    trace_commit_merkle_ms: 6.0,
                    prove_core_ms: 7.0,
                    prove_core_evaluation_program_v1_ms: 8.0,
                    prove_core_composition_generation_ms: 9.0,
                    prove_core_composition_commit_ms: 10.0,
                    prove_core_prove_values_ms: 11.0,
                    prove_core_sanity_check_ms: 12.0,
                    prove_core_composition_detail: super::super::prove_runtime_v1::MetalCompositionDetailBreakdown {
                        twiddle_ms: 0.5,
                        trace_extension_ms: 1.5,
                        eval_program_ms: 8.0,
                        quotient_application_ms: 0.3,
                        interpolation_ms: 0.7,
                    },
                    prove_core_prove_values_detail: super::super::prove_runtime_v1::MetalProveValuesDetailBreakdown {
                        prepare_ms: 1.0,
                        sampled_values_v1_ms: 2.0,
                        fri_and_decommit_ms: 8.0,
                    },
                },
            },
            prove_elapsed_ms: 13.0,
            verify_elapsed_ms: 14.0,
        };
        let run = super::MetalGeneratedWideFibonacciBenchmarkRun {
            warmup_iterations: 2,
            timed_iterations: vec![iteration.clone(), iteration],
        };

        assert_eq!(run.warmup_iterations(), 2);
        assert_eq!(run.timed_iterations().len(), 2);
        assert_eq!(run.cold_start_iteration().unwrap().prove_elapsed_ms, 13.0);
        assert_eq!(run.steady_state_iterations().len(), 1);
        assert_eq!(run.final_iteration().unwrap().verify_elapsed_ms, 14.0);
    }
}
