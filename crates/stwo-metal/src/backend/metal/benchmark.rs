use stwo::core::channel::{Blake2sChannel, Channel};
use stwo::core::fields::m31::BaseField;
use stwo::core::fields::qm31::SecureField;
use stwo::core::fields::qm31::SECURE_EXTENSION_DEGREE;
use stwo::core::pcs::utils::get_lifting_log_size;
use stwo::core::pcs::TreeVec;
use stwo::core::proof::StarkProof;
use stwo::core::vcs_lifted::blake2_merkle::{Blake2sMerkleChannel, Blake2sMerkleHasher};
use stwo::prover::{CommitmentSchemeProver, ComponentProvers, ProvingError};

use super::artifact::{MetalGeneratedRouteKind, MetalRegisteredBenchmarkOperation};
use super::eval_program_v1::{
    execute_selected_metal_evaluation_program_v1_on_metal, interpret_metal_evaluation_program_v1,
    lower_registered_metal_evaluation_program_v1, MetalEvaluationProgramBudgetV1,
    MetalEvaluationProgramCapabilityProfileV1, MetalEvaluationProgramDispatchKindV1,
    MetalEvaluationProgramExecutionError, MetalEvaluationProgramInterpreterError,
    MetalEvaluationProgramLoweringError, MetalEvaluationProgramRuntimeInputsV1,
    MetalEvaluationProgramSpecializationV1, MetalEvaluationProgramTraceViewV1,
    OwnedMetalEvaluationProgramV1, select_metal_evaluation_program_dispatch_v1,
};
use super::execution_plan::{
    registered_execution_binding, registered_execution_seed, RegisteredMetalExecutionSeed,
};
use super::planner::{MetalExecutionIntent, MetalPlannerError};
use super::witness::{MetalWideFibonacciTrace, MetalWideFibonacciTraceError};
use super::workload::{declare_exemplar_metal_workload_boundary, MetalWorkloadBoundary};
use super::workload_contract::{MetalWorkloadOwnership, MetalWorkloadStage};

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

pub struct MetalBenchmarkProveValuesStaging {
    pub oods_point: stwo::core::circle::CirclePoint<SecureField>,
    pub max_log_degree_bound: u32,
    pub sample_points: TreeVec<Vec<Vec<stwo::core::circle::CirclePoint<SecureField>>>>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MetalBenchmarkProveCoreBreakdown {
    pub composition_generation_ms: f64,
    pub composition_commit_ms: f64,
    pub prove_values_ms: f64,
    pub sanity_check_ms: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MetalBenchmarkProveValuesBreakdown {
    pub prove_values_ms: f64,
    pub sanity_check_ms: f64,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum MetalBenchmarkLaneError {
    PlanNotMetalCapable {
        workload_name: &'static str,
        plan: super::planner::MetalExecutionPlan,
    },
    UnsupportedWitnessOwnership {
        workload_name: &'static str,
        stage: MetalWorkloadStage,
    },
    UnsupportedFriOwnership {
        workload_name: &'static str,
        stage: MetalWorkloadStage,
    },
}

impl MetalWideFibonacciBenchmarkBoundary {
    pub fn workload_boundary(&self) -> &MetalWorkloadBoundary {
        &self.workload_boundary
    }

    pub fn target(&self) -> &MetalBenchmarkTarget {
        &self.target
    }

    pub fn validate_prove_values_lane(&self) -> Result<&'static str, MetalBenchmarkLaneError> {
        let workload_name = self.workload_boundary.workload_name();
        if !matches!(
            self.execution_seed.plan,
            super::planner::MetalExecutionPlan::MetalFriHybrid
                | super::planner::MetalExecutionPlan::MetalFull
        ) {
            return Err(MetalBenchmarkLaneError::PlanNotMetalCapable {
                workload_name,
                plan: self.execution_seed.plan,
            });
        }
        if self
            .execution_seed
            .stage_ownership(MetalWorkloadStage::WitnessMain)
            != Some(MetalWorkloadOwnership::CpuOwned)
        {
            return Err(MetalBenchmarkLaneError::UnsupportedWitnessOwnership {
                workload_name,
                stage: MetalWorkloadStage::WitnessMain,
            });
        }
        if self
            .execution_seed
            .stage_ownership(MetalWorkloadStage::FriBlake2s)
            != Some(MetalWorkloadOwnership::MetalNative)
        {
            return Err(MetalBenchmarkLaneError::UnsupportedFriOwnership {
                workload_name,
                stage: MetalWorkloadStage::FriBlake2s,
            });
        }

        Ok(workload_name)
    }

    pub fn stage_prove_values(
        &self,
        component_provers: &ComponentProvers<'_, super::MetalBackend>,
        channel: &mut Blake2sChannel,
        commitment_scheme: &CommitmentSchemeProver<'_, super::MetalBackend, Blake2sMerkleChannel>,
    ) -> Result<MetalBenchmarkProveValuesStaging, MetalBenchmarkLaneError> {
        self.validate_prove_values_lane()?;

        let oods_point = stwo::core::circle::CirclePoint::<SecureField>::get_random_point(channel);
        let split_composition_log_size = commitment_scheme
            .trees
            .last()
            .unwrap()
            .commitment
            .layers
            .len() as u32
            - 1;
        let lifting_log_size =
            get_lifting_log_size(&commitment_scheme.config, split_composition_log_size);
        let max_log_degree_bound =
            lifting_log_size - commitment_scheme.config.fri_config.log_blowup_factor;

        let mut sample_points =
            component_provers
                .components()
                .mask_points(oods_point, max_log_degree_bound, false);
        sample_points.push(vec![vec![oods_point]; 2 * SECURE_EXTENSION_DEGREE]);

        Ok(MetalBenchmarkProveValuesStaging {
            oods_point,
            max_log_degree_bound,
            sample_points,
        })
    }

    pub fn execute_prove_values(
        &self,
        component_provers: &ComponentProvers<'_, super::MetalBackend>,
        random_coeff: SecureField,
        channel: &mut Blake2sChannel,
        commitment_scheme: CommitmentSchemeProver<'_, super::MetalBackend, Blake2sMerkleChannel>,
        staging: MetalBenchmarkProveValuesStaging,
    ) -> Result<
        (
            StarkProof<Blake2sMerkleHasher>,
            MetalBenchmarkProveValuesBreakdown,
        ),
        ProvingError,
    > {
        let prove_values_start = std::time::Instant::now();
        let commitment_scheme_proof =
            commitment_scheme.prove_values(staging.sample_points, channel);
        let prove_values_ms = prove_values_start.elapsed().as_secs_f64() * 1000.0;
        let proof = StarkProof(commitment_scheme_proof.proof);

        let sanity_check_start = std::time::Instant::now();
        if extract_composition_oods_eval(&proof, staging.oods_point, staging.max_log_degree_bound)
            .unwrap()
            != component_provers
                .components()
                .eval_composition_polynomial_at_point(
                    staging.oods_point,
                    &proof.sampled_values,
                    random_coeff,
                    staging.max_log_degree_bound,
                )
        {
            return Err(ProvingError::ConstraintsNotSatisfied);
        }
        let sanity_check_ms = sanity_check_start.elapsed().as_secs_f64() * 1000.0;

        Ok((
            proof,
            MetalBenchmarkProveValuesBreakdown {
                prove_values_ms,
                sanity_check_ms,
            },
        ))
    }

    pub fn execute_prove_core(
        &self,
        components: &[&dyn stwo::prover::ComponentProver<super::MetalBackend>],
        channel: &mut Blake2sChannel,
        mut commitment_scheme: CommitmentSchemeProver<'_, super::MetalBackend, Blake2sMerkleChannel>,
    ) -> Result<
        (
            StarkProof<Blake2sMerkleHasher>,
            MetalBenchmarkProveCoreBreakdown,
        ),
        ProvingError,
    > {
        let n_preprocessed_columns = commitment_scheme.trees[stwo::core::verifier::PREPROCESSED_TRACE_IDX]
            .polynomials
            .len();
        let component_provers = ComponentProvers {
            components: components.to_vec(),
            n_preprocessed_columns,
        };
        let trace = commitment_scheme.trace();

        let random_coeff = channel.draw_secure_felt();

        let composition_generation_start = std::time::Instant::now();
        let composition_poly =
            component_provers.compute_composition_polynomial(random_coeff, &trace);
        let composition_generation_ms =
            composition_generation_start.elapsed().as_secs_f64() * 1000.0;

        let composition_commit_start = std::time::Instant::now();
        let mut tree_builder = commitment_scheme.tree_builder();
        let (left_comp_poly_half, right_comp_poly_half) = composition_poly.split_at_mid();
        tree_builder.extend_polys(left_comp_poly_half.into_coordinate_polys());
        tree_builder.extend_polys(right_comp_poly_half.into_coordinate_polys());
        tree_builder.commit(channel);
        let composition_commit_ms = composition_commit_start.elapsed().as_secs_f64() * 1000.0;

        let prove_values_stage =
            self.stage_prove_values(&component_provers, channel, &commitment_scheme)
                .expect("registered benchmark boundary must satisfy the prove-values lane contract");
        let (proof, prove_values_breakdown) = self.execute_prove_values(
            &component_provers,
            random_coeff,
            channel,
            commitment_scheme,
            prove_values_stage,
        )?;

        Ok((
            proof,
            MetalBenchmarkProveCoreBreakdown {
                composition_generation_ms,
                composition_commit_ms,
                prove_values_ms: prove_values_breakdown.prove_values_ms,
                sanity_check_ms: prove_values_breakdown.sanity_check_ms,
            },
        ))
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
            MetalEvaluationProgramLoweringError::InvalidWideFibonacciColumnCount { n_columns } => {
                MetalBenchmarkProgramError::InvalidSpecialization {
                    workload_name,
                    n_columns,
                }
            }
            MetalEvaluationProgramLoweringError::RegisterBudgetOverflow => {
                MetalBenchmarkProgramError::RegisterBudgetOverflow { workload_name }
            }
        })?;

        program
            .validate(MetalEvaluationProgramBudgetV1::new(2048, 512))
            .map_err(|_| MetalBenchmarkProgramError::InvalidProgramContract { workload_name })?;

        Ok(program)
    }

    pub fn execute_evaluation_program_v1_on_trace(
        &self,
        trace: &MetalWideFibonacciTrace,
        random_coeff_powers: &[SecureField],
    ) -> Result<Vec<SecureField>, MetalBenchmarkProgramExecutionError> {
        let (row_res, _) = self.execute_selected_evaluation_program_v1_on_trace(
            trace,
            random_coeff_powers,
        )?;
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
        let program = self
            .evaluation_program_v1()
            .map_err(|source| MetalBenchmarkProgramExecutionError::ProgramContract { source })?;
        let trace_columns = self
            .materialize_trace_columns(trace)
            .map_err(|source| MetalBenchmarkProgramExecutionError::TraceShape { source })?;
        let trace_column_refs = trace_columns.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let trace_interactions = [&[][..], trace_column_refs.as_slice()];
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
        let program = self
            .evaluation_program_v1()
            .map_err(|source| MetalBenchmarkProgramExecutionError::ProgramContract { source })?;
        let trace_columns = self
            .materialize_trace_columns(trace)
            .map_err(|source| MetalBenchmarkProgramExecutionError::TraceShape { source })?;
        let trace_column_refs = trace_columns.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let trace_interactions = [&[][..], trace_column_refs.as_slice()];
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
}

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
}

fn extract_composition_oods_eval(
    proof: &StarkProof<Blake2sMerkleHasher>,
    oods_point: stwo::core::circle::CirclePoint<SecureField>,
    max_log_degree_bound: u32,
) -> Option<SecureField> {
    let [.., left_and_right_composition_mask] = &**proof.sampled_values else {
        return None;
    };
    let left_and_right_coordinate_evals: [SecureField; 2 * SECURE_EXTENSION_DEGREE] =
        left_and_right_composition_mask
            .iter()
            .map(|columns| {
                let &[eval] = &columns[..] else {
                    return None;
                };
                Some(eval)
            })
            .collect::<Option<Vec<_>>>()?
            .try_into()
            .ok()?;

    let (left_coordinate_evals, right_coordinate_evals) =
        left_and_right_coordinate_evals.split_at(SECURE_EXTENSION_DEGREE);
    let left_eval = SecureField::from_partial_evals(left_coordinate_evals.try_into().ok()?);
    let right_eval = SecureField::from_partial_evals(right_coordinate_evals.try_into().ok()?);
    Some(left_eval + oods_point.repeated_double(max_log_degree_bound - 1).x * right_eval)
}

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

#[cfg(test)]
mod tests {
    use stwo::core::fields::m31::BaseField;
    use stwo::core::fields::qm31::SecureField;

    use super::{
        declare_wide_fibonacci_benchmark_boundary, MetalBenchmarkInputError,
        MetalBenchmarkLaneError, MetalBenchmarkOperation, MetalBenchmarkProgramExecutionError,
        MetalBenchmarkReferencePlatform, MetalExecutionIntent, WIDE_FIBONACCI_PROVE_LOG20_TARGET,
        WIDE_FIBONACCI_TRACE_LOG20_TARGET,
    };
    use crate::backend::metal::artifact::MetalGeneratedRouteKind;
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
        assert_eq!(
            boundary.validate_prove_values_lane(),
            Ok("fibonacci_example")
        );
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

        assert_eq!(dispatch, MetalEvaluationProgramDispatchKindV1::GenericMetalInterpreter);
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
    fn benchmark_boundary_rejects_prove_values_lane_for_cpu_plan() {
        let boundary = declare_wide_fibonacci_benchmark_boundary(
            MetalExecutionIntent::ForceCpu,
            WIDE_FIBONACCI_PROVE_LOG20_TARGET,
        )
        .unwrap();

        assert_eq!(
            boundary.validate_prove_values_lane(),
            Err(MetalBenchmarkLaneError::PlanNotMetalCapable {
                workload_name: "fibonacci_example",
                plan: MetalExecutionPlan::CpuOnly,
            })
        );
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
}
