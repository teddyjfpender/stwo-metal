use std::borrow::Cow;

use stwo::core::air::accumulation::PointEvaluationAccumulator;
use stwo::core::air::Component;
use stwo::core::circle::CirclePoint;
use stwo::core::fields::m31::BaseField;
use stwo::core::fields::qm31::SecureField;
use stwo::core::fields::FieldExpOps;
use stwo::core::pcs::TreeVec;
use stwo::core::poly::circle::CanonicCoset;
use stwo::core::utils::bit_reverse;
use stwo::core::ColumnVec;
use stwo::prover::backend::simd::SimdBackend;
use stwo::prover::backend::{Col, Column, CpuBackend};
use stwo::prover::poly::circle::{CircleCoefficients, CircleEvaluation, PolyOps};
use stwo::prover::poly::BitReversedOrder;
use stwo::prover::secure_column::SecureColumnByCoords;
use stwo::prover::{AccumulationOps, ComponentProver, DomainEvaluationAccumulator, Poly, Trace};
use stwo_constraint_framework::{
    CpuDomainEvaluator, FrameworkComponent, FrameworkEval, PREPROCESSED_TRACE_IDX,
};
use stwo_metal::{MetalBackend, MetalBaseFieldVec, MetalExecutionPlan, MetalWorkloadBoundary};

#[derive(Debug, Eq, PartialEq)]
pub enum AcceptanceMetalLaneError {
    PlanNotMetalCapable {
        workload_name: &'static str,
        plan: MetalExecutionPlan,
    },
}

impl core::fmt::Display for AcceptanceMetalLaneError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::PlanNotMetalCapable {
                workload_name,
                plan,
            } => write!(
                f,
                "registered Metal workload boundary for {workload_name} is not Metal-capable: {plan:?}"
            ),
        }
    }
}

impl std::error::Error for AcceptanceMetalLaneError {}

#[derive(Clone, Copy, Debug)]
pub struct AcceptanceMetalLane<'a> {
    boundary: &'a MetalWorkloadBoundary,
}

impl AcceptanceMetalLane<'_> {
    pub fn workload_name(&self) -> &'static str {
        self.boundary.workload_name()
    }

    pub fn boundary(&self) -> &MetalWorkloadBoundary {
        self.boundary
    }
}

pub fn acceptance_registered_metal_lane(
    boundary: &MetalWorkloadBoundary,
) -> Result<AcceptanceMetalLane<'_>, AcceptanceMetalLaneError> {
    match boundary.plan() {
        MetalExecutionPlan::MetalFriHybrid | MetalExecutionPlan::MetalFull => {
            Ok(AcceptanceMetalLane { boundary })
        }
        plan => Err(AcceptanceMetalLaneError::PlanNotMetalCapable {
            workload_name: boundary.workload_name(),
            plan,
        }),
    }
}

/// Registered acceptance-bridge catalog.
///
/// Inputs:
/// - one `AcceptanceMetalLane` obtained from a registered workload boundary
///
/// Outputs:
/// - framework and SIMD bridge constructors bound to that checked lane
///
/// Invariants:
/// - acceptance adapters are only created from registered Metal-capable lanes
/// - workload routing stays explicit; no ad hoc bridge construction exists outside this catalog
///
/// Failure modes:
/// - construction fails through `acceptance_registered_metal_lane` when the workload declaration
///   is not Metal-capable
#[derive(Clone, Copy, Debug)]
pub struct AcceptanceMetalBridgeCatalog<'a> {
    lane: AcceptanceMetalLane<'a>,
}

impl<'a> AcceptanceMetalBridgeCatalog<'a> {
    pub fn new(lane: AcceptanceMetalLane<'a>) -> Self {
        Self { lane }
    }

    pub fn workload_name(&self) -> &'static str {
        self.lane.workload_name()
    }

    pub fn framework<E: FrameworkEval>(
        self,
        component: &'a FrameworkComponent<E>,
    ) -> AcceptanceMetalFrameworkComponent<'a, E> {
        let _ = self.workload_name();
        AcceptanceMetalFrameworkComponent { inner: component }
    }

    pub fn simd(
        self,
        component: &'a dyn ComponentProver<SimdBackend>,
    ) -> AcceptanceMetalSimdComponent<'a> {
        let _ = self.workload_name();
        AcceptanceMetalSimdComponent { inner: component }
    }
}

pub fn acceptance_bridge_catalog(
    boundary: &MetalWorkloadBoundary,
) -> Result<AcceptanceMetalBridgeCatalog<'_>, AcceptanceMetalLaneError> {
    acceptance_registered_metal_lane(boundary).map(AcceptanceMetalBridgeCatalog::new)
}

#[derive(Clone, Copy)]
pub struct AcceptanceMetalFrameworkComponent<'a, E: FrameworkEval> {
    inner: &'a FrameworkComponent<E>,
}

#[derive(Clone, Copy)]
pub struct AcceptanceMetalSimdComponent<'a> {
    inner: &'a dyn ComponentProver<SimdBackend>,
}

impl<E: FrameworkEval> Component for AcceptanceMetalFrameworkComponent<'_, E> {
    fn n_constraints(&self) -> usize {
        self.inner.n_constraints()
    }

    fn max_constraint_log_degree_bound(&self) -> u32 {
        self.inner.max_constraint_log_degree_bound()
    }

    fn trace_log_degree_bounds(&self) -> TreeVec<ColumnVec<u32>> {
        self.inner.trace_log_degree_bounds()
    }

    fn mask_points(
        &self,
        point: CirclePoint<SecureField>,
        max_log_degree_bound: u32,
    ) -> TreeVec<ColumnVec<Vec<CirclePoint<SecureField>>>> {
        self.inner.mask_points(point, max_log_degree_bound)
    }

    fn preprocessed_column_indices(&self) -> ColumnVec<usize> {
        self.inner.preprocessed_column_indices().to_vec()
    }

    fn evaluate_constraint_quotients_at_point(
        &self,
        point: CirclePoint<SecureField>,
        mask: &TreeVec<ColumnVec<Vec<SecureField>>>,
        evaluation_accumulator: &mut PointEvaluationAccumulator,
        max_log_degree_bound: u32,
    ) {
        self.inner.evaluate_constraint_quotients_at_point(
            point,
            mask,
            evaluation_accumulator,
            max_log_degree_bound,
        );
    }
}

impl Component for AcceptanceMetalSimdComponent<'_> {
    fn n_constraints(&self) -> usize {
        self.inner.n_constraints()
    }

    fn max_constraint_log_degree_bound(&self) -> u32 {
        self.inner.max_constraint_log_degree_bound()
    }

    fn trace_log_degree_bounds(&self) -> TreeVec<ColumnVec<u32>> {
        self.inner.trace_log_degree_bounds()
    }

    fn mask_points(
        &self,
        point: CirclePoint<SecureField>,
        max_log_degree_bound: u32,
    ) -> TreeVec<ColumnVec<Vec<CirclePoint<SecureField>>>> {
        self.inner.mask_points(point, max_log_degree_bound)
    }

    fn preprocessed_column_indices(&self) -> ColumnVec<usize> {
        self.inner.preprocessed_column_indices()
    }

    fn evaluate_constraint_quotients_at_point(
        &self,
        point: CirclePoint<SecureField>,
        mask: &TreeVec<ColumnVec<Vec<SecureField>>>,
        evaluation_accumulator: &mut PointEvaluationAccumulator,
        max_log_degree_bound: u32,
    ) {
        self.inner.evaluate_constraint_quotients_at_point(
            point,
            mask,
            evaluation_accumulator,
            max_log_degree_bound,
        );
    }
}

impl<E: FrameworkEval + Sync> ComponentProver<MetalBackend>
    for AcceptanceMetalFrameworkComponent<'_, E>
{
    fn evaluate_constraint_quotients_on_domain(
        &self,
        trace: &Trace<'_, MetalBackend>,
        evaluation_accumulator: &mut DomainEvaluationAccumulator<MetalBackend>,
    ) {
        let n_constraints = self.inner.n_constraints();
        if n_constraints == 0 {
            return;
        }

        if self.inner.is_disabled() {
            evaluation_accumulator.skip_coeffs(n_constraints);
            return;
        }

        let eval_domain =
            CanonicCoset::new(self.inner.max_constraint_log_degree_bound()).circle_domain();
        let trace_domain = CanonicCoset::new(self.inner.log_size());

        let mut component_polys = trace.polys.sub_tree(self.inner.trace_locations());
        component_polys[PREPROCESSED_TRACE_IDX] = self
            .inner
            .preprocessed_column_indices()
            .iter()
            .map(|idx| &trace.polys[PREPROCESSED_TRACE_IDX][*idx])
            .collect();

        let need_to_extend = component_polys
            .iter()
            .flatten()
            .any(|poly| poly.evals.domain.log_size() != eval_domain.log_size());
        let trace: TreeVec<
            Vec<Cow<'_, CircleEvaluation<CpuBackend, BaseField, BitReversedOrder>>>,
        > = if need_to_extend {
            let twiddles = MetalBackend::precompute_twiddles(eval_domain.half_coset);
            component_polys.as_cols_ref().map_cols(|poly| {
                Cow::Owned(metal_eval_to_cpu(
                    poly.get_evaluation_on_domain(eval_domain, &twiddles),
                ))
            })
        } else {
            component_polys.map_cols(|poly| Cow::Owned(metal_eval_to_cpu(poly.evals.clone())))
        };

        let log_expand = eval_domain.log_size() - trace_domain.log_size();
        let mut denom_inv = (0..1 << log_expand)
            .map(|index| {
                stwo::core::constraints::coset_vanishing(
                    trace_domain.coset(),
                    eval_domain.at(index),
                )
                .inverse()
            })
            .collect::<Vec<_>>();
        bit_reverse(&mut denom_inv);

        let [mut accum] = evaluation_accumulator.columns([(eval_domain.log_size(), n_constraints)]);
        accum.random_coeff_powers.reverse();

        let trace_cols = trace.as_cols_ref().map_cols(|eval| eval.as_ref());
        *accum.col = metal_secure_column_from_cpu(accumulate_pointwise_cpu(
            self.inner,
            trace_cols,
            eval_domain.log_size(),
            trace_domain.log_size(),
            denom_inv,
            &accum.random_coeff_powers,
            &accum.col.to_cpu(),
        ));
    }
}

impl ComponentProver<MetalBackend> for AcceptanceMetalSimdComponent<'_> {
    fn evaluate_constraint_quotients_on_domain(
        &self,
        trace: &Trace<'_, MetalBackend>,
        evaluation_accumulator: &mut DomainEvaluationAccumulator<MetalBackend>,
    ) {
        let n_constraints = self.inner.n_constraints();
        if n_constraints == 0 {
            return;
        }

        let global_log_size = evaluation_accumulator.log_size();
        let [mut outer_accumulator] =
            evaluation_accumulator.columns([(global_log_size, n_constraints)]);
        outer_accumulator.random_coeff_powers.reverse();

        let local_alpha = match outer_accumulator.random_coeff_powers.as_slice() {
            [single] => *single,
            coeffs => {
                let low = *coeffs.last().expect("component must reserve coefficients");
                let next = coeffs[coeffs.len() - 2];
                next * low.inverse()
            }
        };
        let global_scale = *outer_accumulator
            .random_coeff_powers
            .last()
            .expect("component must reserve coefficients");

        let simd_trace = metal_trace_to_simd(trace);
        let mut simd_accumulator = DomainEvaluationAccumulator::<SimdBackend>::new(
            local_alpha,
            self.inner.max_constraint_log_degree_bound(),
            n_constraints,
        );
        self.inner
            .evaluate_constraint_quotients_on_domain(&simd_trace, &mut simd_accumulator);

        let local_poly = simd_accumulator.finalize();
        let global_domain = CanonicCoset::new(global_log_size).circle_domain();
        let twiddles = SimdBackend::precompute_twiddles(global_domain.half_coset);
        let local_eval = local_poly.evaluate_with_twiddles(global_domain, &twiddles);

        let scaled = scale_secure_column_cpu(local_eval.to_cpu().values, global_scale);
        let scaled_metal = SecureColumnByCoords {
            columns: scaled.columns.map(MetalBaseFieldVec::from_vec),
        };
        MetalBackend::accumulate(outer_accumulator.col, &scaled_metal);
    }
}

fn metal_eval_to_cpu(
    eval: CircleEvaluation<MetalBackend, BaseField, BitReversedOrder>,
) -> CircleEvaluation<CpuBackend, BaseField, BitReversedOrder> {
    let coeffs_domain = eval.domain;
    CircleEvaluation::new(coeffs_domain, eval.values.to_cpu())
}

fn metal_trace_to_simd<'a>(trace: &Trace<'a, MetalBackend>) -> Trace<'a, SimdBackend> {
    Trace {
        polys: trace.polys.as_cols_ref().map_cols(|poly| {
            let coeffs = poly.coeffs.as_ref().map(metal_coeffs_to_simd);
            let evals = CircleEvaluation::new(
                poly.evals.domain,
                Col::<SimdBackend, BaseField>::from_iter(poly.evals.values.to_cpu()),
            );
            &*Box::leak(Box::new(Poly::new(coeffs, evals)))
        }),
    }
}

fn metal_coeffs_to_simd(
    coeffs: &CircleCoefficients<MetalBackend>,
) -> CircleCoefficients<SimdBackend> {
    CircleCoefficients::new(Col::<SimdBackend, BaseField>::from_iter(
        coeffs.coeffs.to_cpu(),
    ))
}

fn metal_secure_column_from_cpu(
    column: SecureColumnByCoords<CpuBackend>,
) -> SecureColumnByCoords<MetalBackend> {
    SecureColumnByCoords {
        columns: column.columns.map(MetalBaseFieldVec::from_vec),
    }
}

fn scale_secure_column_cpu(
    column: SecureColumnByCoords<CpuBackend>,
    scale: SecureField,
) -> SecureColumnByCoords<CpuBackend> {
    (0..column.len())
        .map(|index| column.at(index) * scale)
        .collect()
}

fn accumulate_pointwise_cpu<E: FrameworkEval>(
    component: &FrameworkComponent<E>,
    trace_cols: TreeVec<Vec<&CircleEvaluation<CpuBackend, BaseField, BitReversedOrder>>>,
    eval_log_size: u32,
    trace_log_size: u32,
    denom_inv: Vec<BaseField>,
    random_coeff_powers: &[SecureField],
    accum: &SecureColumnByCoords<CpuBackend>,
) -> SecureColumnByCoords<CpuBackend> {
    let mut result = SecureColumnByCoords::zeros(1 << eval_log_size);
    for row in 0..(1 << eval_log_size) {
        let eval = CpuDomainEvaluator::new(
            &trace_cols,
            row,
            random_coeff_powers,
            trace_log_size,
            eval_log_size,
            component.log_size(),
            component.claimed_sum(),
        );
        let row_res = component.evaluate(eval).row_res;
        let row_denom_inv = denom_inv[row >> trace_log_size];
        result.set(row, accum.at(row) + row_res * row_denom_inv);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::{acceptance_registered_metal_lane, AcceptanceMetalLaneError};
    use stwo_metal::{
        declare_exemplar_metal_workload_boundary, MetalExecutionIntent, MetalExecutionPlan,
    };

    #[test]
    fn registered_lane_accepts_metal_capable_boundary() {
        let boundary = declare_exemplar_metal_workload_boundary(
            MetalExecutionIntent::PreferMetal,
            "fibonacci_example",
        )
        .unwrap();

        let lane = acceptance_registered_metal_lane(&boundary).unwrap();

        assert_eq!(lane.workload_name(), "fibonacci_example");
        assert_eq!(lane.boundary().plan(), MetalExecutionPlan::MetalFriHybrid);
    }

    #[test]
    fn registered_lane_rejects_cpu_only_boundary() {
        let boundary = declare_exemplar_metal_workload_boundary(
            MetalExecutionIntent::ForceCpu,
            "fibonacci_example",
        )
        .unwrap();

        let error = acceptance_registered_metal_lane(&boundary).unwrap_err();

        assert_eq!(
            error,
            AcceptanceMetalLaneError::PlanNotMetalCapable {
                workload_name: "fibonacci_example",
                plan: MetalExecutionPlan::CpuOnly,
            }
        );
    }
}
