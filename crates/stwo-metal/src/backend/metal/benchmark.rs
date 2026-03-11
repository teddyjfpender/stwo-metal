use stwo::core::fields::m31::BaseField;

use super::artifact::{MetalGeneratedRouteKind, MetalRegisteredBenchmarkOperation};
use super::execution_plan::{
    registered_execution_binding, registered_execution_seed, RegisteredMetalExecutionSeed,
};
use super::planner::{MetalExecutionIntent, MetalPlannerError};
use super::witness::{MetalWideFibonacciTrace, MetalWideFibonacciTraceError};
use super::workload::{declare_exemplar_metal_workload_boundary, MetalWorkloadBoundary};

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

impl MetalWideFibonacciBenchmarkBoundary {
    pub fn workload_boundary(&self) -> &MetalWorkloadBoundary {
        &self.workload_boundary
    }

    pub fn target(&self) -> &MetalBenchmarkTarget {
        &self.target
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

    use super::{
        declare_wide_fibonacci_benchmark_boundary, MetalBenchmarkInputError,
        MetalBenchmarkOperation, MetalBenchmarkReferencePlatform, MetalExecutionIntent,
        WIDE_FIBONACCI_PROVE_LOG20_TARGET, WIDE_FIBONACCI_TRACE_LOG20_TARGET,
    };
    use crate::backend::metal::artifact::MetalGeneratedRouteKind;
    use crate::backend::metal::{
        MetalExecutionPlan, MetalPlannerError, MetalWorkloadOwnership, MetalWorkloadStage,
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
