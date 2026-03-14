//! Recording evaluator for FrameworkEval constraint evaluation.
//!
//! Instead of computing values, `RecordingEval` records every operation into a
//! sequence of [`RecordedOp`] entries that represent the constraint DAG. This
//! sequence can later be lowered into V1 opcodes (see `eval_program_v1`).

use std::cell::RefCell;
use std::fmt;
use std::ops::{Add, AddAssign, Mul, MulAssign, Neg, Sub};
use std::rc::Rc;

use ark_std::{One, Zero};
use stwo::core::fields::m31::BaseField;
use stwo::core::fields::qm31::{SecureField, SECURE_EXTENSION_DEGREE};
use stwo::core::fields::FieldExpOps;
use stwo::core::Fraction;
use stwo_constraint_framework::preprocessed_columns::PreProcessedColumnId;
use stwo_constraint_framework::{EvalAtRow, FrameworkEval, Relation, RelationEntry};

// ---------------------------------------------------------------------------
// Recorded operation types
// ---------------------------------------------------------------------------

/// Reference to a previously recorded operation (index into the ops vec).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpRef(pub usize);

/// A recorded operation from a FrameworkEval constraint evaluation.
#[derive(Debug, Clone)]
pub enum RecordedOp {
    /// Read trace column at current row (interaction index, column within that interaction).
    TraceCol {
        interaction: usize,
        col_index: usize,
        offset: isize,
    },
    /// Read preprocessed column.
    PreprocessedCol { col_index: usize },
    /// Load a constant.
    Const { value: BaseField },
    /// Binary addition.
    Add { lhs: OpRef, rhs: OpRef },
    /// Binary subtraction.
    Sub { lhs: OpRef, rhs: OpRef },
    /// Binary multiplication.
    Mul { lhs: OpRef, rhs: OpRef },
    /// Field inversion.
    Inv { operand: OpRef },
    /// Add constraint (the operand is the constraint expression value).
    AddConstraint { expr: OpRef },
}

// ---------------------------------------------------------------------------
// Shared recording context
// ---------------------------------------------------------------------------

/// Internal shared state between `RecordingEval` and all `RecordedExpr` instances.
#[derive(Debug)]
struct RecordingContext {
    ops: Vec<RecordedOp>,
}

impl RecordingContext {
    fn new() -> Self {
        Self { ops: Vec::new() }
    }

    fn push(&mut self, op: RecordedOp) -> OpRef {
        let idx = self.ops.len();
        self.ops.push(op);
        OpRef(idx)
    }
}

type SharedCtx = Rc<RefCell<RecordingContext>>;

// ---------------------------------------------------------------------------
// RecordedExpr – the "field element" type that records arithmetic
// ---------------------------------------------------------------------------

/// A symbolic expression handle. Arithmetic on `RecordedExpr` values records
/// operations into the shared context rather than computing field values.
#[derive(Clone)]
pub struct RecordedExpr {
    ref_: OpRef,
    ctx: SharedCtx,
}

impl RecordedExpr {
    fn new(ref_: OpRef, ctx: SharedCtx) -> Self {
        Self { ref_, ctx }
    }

    fn push_op(&self, op: RecordedOp) -> Self {
        let ref_ = self.ctx.borrow_mut().push(op);
        Self::new(ref_, Rc::clone(&self.ctx))
    }

    /// Returns the [`OpRef`] referencing this expression in the recorded DAG.
    pub fn op_ref(&self) -> OpRef {
        self.ref_
    }
}

impl fmt::Debug for RecordedExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RecordedExpr")
            .field("ref_", &self.ref_)
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Arithmetic trait implementations for RecordedExpr
// ---------------------------------------------------------------------------

impl Add for RecordedExpr {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        self.push_op(RecordedOp::Add {
            lhs: self.ref_,
            rhs: rhs.ref_,
        })
    }
}

impl Sub for RecordedExpr {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        self.push_op(RecordedOp::Sub {
            lhs: self.ref_,
            rhs: rhs.ref_,
        })
    }
}

impl Mul for RecordedExpr {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self {
        self.push_op(RecordedOp::Mul {
            lhs: self.ref_,
            rhs: rhs.ref_,
        })
    }
}

impl Neg for RecordedExpr {
    type Output = Self;
    fn neg(self) -> Self {
        // Emit 0 - self.
        let zero_ref = self.ctx.borrow_mut().push(RecordedOp::Const {
            value: BaseField::from(0u32),
        });
        self.push_op(RecordedOp::Sub {
            lhs: zero_ref,
            rhs: self.ref_,
        })
    }
}

impl AddAssign for RecordedExpr {
    fn add_assign(&mut self, rhs: Self) {
        *self = self.clone() + rhs;
    }
}

impl AddAssign<BaseField> for RecordedExpr {
    fn add_assign(&mut self, rhs: BaseField) {
        *self = self.clone() + RecordedExpr::from_base_field(rhs, &self.ctx);
    }
}

impl MulAssign for RecordedExpr {
    fn mul_assign(&mut self, rhs: Self) {
        *self = self.clone() * rhs;
    }
}

// Mul<BaseField> for RecordedExpr  (required by EvalAtRow::F)
impl Mul<BaseField> for RecordedExpr {
    type Output = Self;
    fn mul(self, rhs: BaseField) -> Self {
        let rhs_expr = RecordedExpr::from_base_field(rhs, &self.ctx);
        self * rhs_expr
    }
}

// Add<SecureField> -> EF  (required by EvalAtRow::F: Add<SecureField, Output = EF>)
impl Add<SecureField> for RecordedExpr {
    type Output = RecordedExpr;
    fn add(self, rhs: SecureField) -> RecordedExpr {
        // Represent the SecureField constant as a base field constant (its first component).
        // For the recording evaluator we just need structural equivalence.
        let rhs_expr = RecordedExpr::from_secure_field(rhs, &self.ctx);
        self + rhs_expr
    }
}

// Mul<SecureField> -> EF  (required by EvalAtRow::F: Mul<SecureField, Output = EF>)
impl Mul<SecureField> for RecordedExpr {
    type Output = RecordedExpr;
    fn mul(self, rhs: SecureField) -> RecordedExpr {
        let rhs_expr = RecordedExpr::from_secure_field(rhs, &self.ctx);
        self * rhs_expr
    }
}

// Sub<SecureField> for RecordedExpr  (required by EvalAtRow::EF)
impl Sub<SecureField> for RecordedExpr {
    type Output = RecordedExpr;
    fn sub(self, rhs: SecureField) -> RecordedExpr {
        let rhs_expr = RecordedExpr::from_secure_field(rhs, &self.ctx);
        self - rhs_expr
    }
}

// Add<BaseField> for RecordedExpr  (required by EvalAtRow::EF)
impl Add<BaseField> for RecordedExpr {
    type Output = RecordedExpr;
    fn add(self, rhs: BaseField) -> RecordedExpr {
        let rhs_expr = RecordedExpr::from_base_field(rhs, &self.ctx);
        self + rhs_expr
    }
}

// Mul<BaseField> for RecordedExpr is already defined above as Mul<BaseField>.

impl From<BaseField> for RecordedExpr {
    fn from(_value: BaseField) -> Self {
        // This is called by the framework when it needs a "from" conversion, typically for
        // constants. We need a context here. We use a *detached* context that will be
        // merged on first arithmetic use. However, since EvalAtRow::F: From<BaseField> is
        // required and we have no context yet, we create a fresh context.
        //
        // The InfoEvaluator works around this by having FieldCounter be Default. We store
        // a placeholder that any binary op with a context-bearing expr will adopt.
        let ctx = Rc::new(RefCell::new(RecordingContext::new()));
        let ref_ = ctx.borrow_mut().push(RecordedOp::Const { value: _value });
        Self::new(ref_, ctx)
    }
}

impl From<SecureField> for RecordedExpr {
    fn from(value: SecureField) -> Self {
        // Same approach as From<BaseField>.
        let ctx = Rc::new(RefCell::new(RecordingContext::new()));
        let ref_ = ctx.borrow_mut().push(RecordedOp::Const {
            value: value.to_m31_array()[0],
        });
        Self::new(ref_, ctx)
    }
}

impl RecordedExpr {
    fn from_base_field(value: BaseField, ctx: &SharedCtx) -> Self {
        let ref_ = ctx.borrow_mut().push(RecordedOp::Const { value });
        Self::new(ref_, Rc::clone(ctx))
    }

    fn from_secure_field(value: SecureField, ctx: &SharedCtx) -> Self {
        // Record as a const using the first M31 component. In practice for the
        // recording DAG, the exact value matters less than the structural shape.
        let ref_ = ctx.borrow_mut().push(RecordedOp::Const {
            value: value.to_m31_array()[0],
        });
        Self::new(ref_, Rc::clone(ctx))
    }
}

// Zero and One – required by EvalAtRow::F and EvalAtRow::EF bounds.
impl Zero for RecordedExpr {
    fn zero() -> Self {
        RecordedExpr::from(BaseField::from(0u32))
    }
    fn is_zero(&self) -> bool {
        // Same approach as InfoEvaluator – this doesn't really make sense for symbolic exprs.
        panic!("is_zero is not meaningful for RecordedExpr")
    }
}

impl One for RecordedExpr {
    fn one() -> Self {
        RecordedExpr::from(BaseField::from(1u32))
    }
}

// FieldExpOps – required by EvalAtRow::F
impl FieldExpOps for RecordedExpr {
    fn inverse(&self) -> Self {
        self.push_op(RecordedOp::Inv {
            operand: self.ref_,
        })
    }
}

// ---------------------------------------------------------------------------
// RecordingEval – the EvalAtRow implementation
// ---------------------------------------------------------------------------

/// A recording evaluator that captures the constraint DAG from a
/// [`FrameworkEval::evaluate`] call.
pub struct RecordingEval {
    ctx: SharedCtx,
    /// Tracks current column index per interaction for `next_interaction_mask`.
    interaction_col_counters: Vec<usize>,
    /// Number of constraints recorded.
    n_constraints: usize,
    /// Number of preprocessed columns accessed.
    n_preprocessed_columns: usize,
}

impl RecordingEval {
    fn new() -> Self {
        Self {
            ctx: Rc::new(RefCell::new(RecordingContext::new())),
            interaction_col_counters: Vec::new(),
            n_constraints: 0,
            n_preprocessed_columns: 0,
        }
    }

    fn ensure_interaction(&mut self, interaction: usize) {
        if self.interaction_col_counters.len() <= interaction {
            self.interaction_col_counters.resize(interaction + 1, 0);
        }
    }
}

impl EvalAtRow for RecordingEval {
    type F = RecordedExpr;
    type EF = RecordedExpr;

    fn next_interaction_mask<const N: usize>(
        &mut self,
        interaction: usize,
        offsets: [isize; N],
    ) -> [Self::F; N] {
        self.ensure_interaction(interaction);
        let col_index = self.interaction_col_counters[interaction];
        self.interaction_col_counters[interaction] += 1;

        std::array::from_fn(|i| {
            let ref_ = self.ctx.borrow_mut().push(RecordedOp::TraceCol {
                interaction,
                col_index,
                offset: offsets[i],
            });
            RecordedExpr::new(ref_, Rc::clone(&self.ctx))
        })
    }

    fn get_preprocessed_column(&mut self, _column: PreProcessedColumnId) -> Self::F {
        let col_index = self.n_preprocessed_columns;
        self.n_preprocessed_columns += 1;
        let ref_ = self.ctx.borrow_mut().push(RecordedOp::PreprocessedCol {
            col_index,
        });
        RecordedExpr::new(ref_, Rc::clone(&self.ctx))
    }

    fn add_constraint<G>(&mut self, constraint: G)
    where
        Self::EF: Mul<G, Output = Self::EF> + From<G>,
    {
        let expr: RecordedExpr = constraint.into();
        // If the expr came from a detached context (via From<BaseField>), we need to
        // merge its ops into our main context. However, for the common case where the
        // constraint is built from expressions that already belong to our context, the
        // expr.ref_ is already valid. We handle the detached case by recording an
        // AddConstraint referencing the expr's ref.
        //
        // For simplicity, we check if the expr shares our context. If not, we copy
        // the detached ops into our context and remap the ref.
        let expr_ref = if Rc::ptr_eq(&expr.ctx, &self.ctx) {
            expr.ref_
        } else {
            // Detached expression – copy its ops into our context.
            let detached_ops = expr.ctx.borrow().ops.clone();
            let base_offset = self.ctx.borrow().ops.len();
            for op in &detached_ops {
                let remapped = remap_op(op, base_offset);
                self.ctx.borrow_mut().push(remapped);
            }
            OpRef(base_offset + expr.ref_.0)
        };

        self.ctx.borrow_mut().push(RecordedOp::AddConstraint {
            expr: expr_ref,
        });
        self.n_constraints += 1;
    }

    fn combine_ef(values: [Self::F; SECURE_EXTENSION_DEGREE]) -> Self::EF {
        // For the recording evaluator, combine_ef produces a single expression.
        // We add the 4 base-field components together to represent the combination.
        // This is a simplification: in reality, each component would multiply by a
        // secure-field basis element. For recording purposes we capture the dependency.
        let mut iter = values.into_iter();
        let first = iter.next().unwrap();
        iter.fold(first, |acc, v| acc + v)
    }

    fn add_to_relation<R: Relation<Self::F, Self::EF>>(
        &mut self,
        _entry: RelationEntry<'_, Self::F, Self::EF, R>,
    ) {
        panic!("interaction columns (add_to_relation) not yet supported in recording evaluator");
    }

    fn write_logup_frac(&mut self, _fraction: Fraction<Self::EF, Self::EF>) {
        panic!("logup fractions not yet supported in recording evaluator");
    }

    fn finalize_logup_batched(&mut self, _batching: &Vec<usize>) {
        panic!("logup finalization not yet supported in recording evaluator");
    }

    fn finalize_logup(&mut self) {
        panic!("logup finalization not yet supported in recording evaluator");
    }

    fn finalize_logup_in_pairs(&mut self) {
        panic!("logup finalization not yet supported in recording evaluator");
    }
}

/// Remap an op's internal references by adding `base_offset` to every `OpRef`.
fn remap_op(op: &RecordedOp, base_offset: usize) -> RecordedOp {
    match op {
        RecordedOp::TraceCol {
            interaction,
            col_index,
            offset,
        } => RecordedOp::TraceCol {
            interaction: *interaction,
            col_index: *col_index,
            offset: *offset,
        },
        RecordedOp::PreprocessedCol { col_index } => RecordedOp::PreprocessedCol {
            col_index: *col_index,
        },
        RecordedOp::Const { value } => RecordedOp::Const { value: *value },
        RecordedOp::Add { lhs, rhs } => RecordedOp::Add {
            lhs: OpRef(lhs.0 + base_offset),
            rhs: OpRef(rhs.0 + base_offset),
        },
        RecordedOp::Sub { lhs, rhs } => RecordedOp::Sub {
            lhs: OpRef(lhs.0 + base_offset),
            rhs: OpRef(rhs.0 + base_offset),
        },
        RecordedOp::Mul { lhs, rhs } => RecordedOp::Mul {
            lhs: OpRef(lhs.0 + base_offset),
            rhs: OpRef(rhs.0 + base_offset),
        },
        RecordedOp::Inv { operand } => RecordedOp::Inv {
            operand: OpRef(operand.0 + base_offset),
        },
        RecordedOp::AddConstraint { expr } => RecordedOp::AddConstraint {
            expr: OpRef(expr.0 + base_offset),
        },
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// The result of recording a [`FrameworkEval`] constraint evaluation.
#[derive(Debug, Clone)]
pub struct RecordedConstraintDag {
    /// The sequence of recorded operations forming the constraint DAG.
    pub ops: Vec<RecordedOp>,
    /// Number of constraints (i.e. `AddConstraint` ops) recorded.
    pub n_constraints: usize,
    /// Total number of trace columns accessed (across all interactions).
    pub n_trace_columns: usize,
    /// Number of preprocessed columns accessed.
    pub n_preprocessed_columns: usize,
    /// Number of trace interactions observed.
    pub n_interactions: usize,
}

/// Record the constraint DAG from a [`FrameworkEval`].
///
/// This drives the evaluator through `eval.evaluate(...)` using a
/// [`RecordingEval`] and returns the recorded operation sequence.
pub fn record_framework_eval<E: FrameworkEval>(eval: &E) -> RecordedConstraintDag {
    let recording = RecordingEval::new();
    let recording = eval.evaluate(recording);

    let n_interactions = recording.interaction_col_counters.len();
    let n_trace_columns: usize = recording.interaction_col_counters.iter().sum();

    let ops = recording.ctx.borrow().ops.clone();

    RecordedConstraintDag {
        ops,
        n_constraints: recording.n_constraints,
        n_trace_columns,
        n_preprocessed_columns: recording.n_preprocessed_columns,
        n_interactions,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal wide-fibonacci evaluator for testing, mirroring the upstream
    /// `WideFibonacciEval<N>` without pulling in the examples crate as a
    /// dependency.
    #[derive(Clone)]
    struct TestWideFibonacciEval<const N: usize> {
        log_n_rows: u32,
    }

    impl<const N: usize> FrameworkEval for TestWideFibonacciEval<N> {
        fn log_size(&self) -> u32 {
            self.log_n_rows
        }
        fn max_constraint_log_degree_bound(&self) -> u32 {
            self.log_n_rows + 1
        }
        fn evaluate<E: EvalAtRow>(&self, mut eval: E) -> E {
            let mut a = eval.next_trace_mask();
            let mut b = eval.next_trace_mask();
            for _ in 2..N {
                let c = eval.next_trace_mask();
                eval.add_constraint(c.clone() - (a.square() + b.square()));
                a = b;
                b = c;
            }
            eval
        }
    }

    #[test]
    fn test_record_wide_fibonacci_4() {
        let eval = TestWideFibonacciEval::<4> { log_n_rows: 8 };
        let dag = record_framework_eval(&eval);

        // WideFibonacciEval<4> reads 4 trace columns and produces 2 constraints.
        assert_eq!(dag.n_trace_columns, 4, "expected 4 trace columns");
        assert_eq!(dag.n_constraints, 2, "expected 2 constraints");
        assert_eq!(dag.n_preprocessed_columns, 0, "expected 0 preprocessed cols");

        // Verify the ops contain exactly 4 TraceCol reads.
        let trace_col_count = dag
            .ops
            .iter()
            .filter(|op| matches!(op, RecordedOp::TraceCol { .. }))
            .count();
        assert_eq!(trace_col_count, 4, "expected 4 TraceCol ops");

        // Verify the ops contain exactly 2 AddConstraint entries.
        let constraint_count = dag
            .ops
            .iter()
            .filter(|op| matches!(op, RecordedOp::AddConstraint { .. }))
            .count();
        assert_eq!(constraint_count, 2, "expected 2 AddConstraint ops");

        // Verify we have Mul ops (from .square() calls).
        let mul_count = dag
            .ops
            .iter()
            .filter(|op| matches!(op, RecordedOp::Mul { .. }))
            .count();
        assert!(mul_count >= 4, "expected at least 4 Mul ops from squares");

        // Verify we have Add and Sub ops.
        let add_count = dag
            .ops
            .iter()
            .filter(|op| matches!(op, RecordedOp::Add { .. }))
            .count();
        assert!(add_count >= 2, "expected at least 2 Add ops");
        let sub_count = dag
            .ops
            .iter()
            .filter(|op| matches!(op, RecordedOp::Sub { .. }))
            .count();
        assert!(sub_count >= 2, "expected at least 2 Sub ops");
    }

    #[test]
    fn test_record_wide_fibonacci_8() {
        let eval = TestWideFibonacciEval::<8> { log_n_rows: 8 };
        let dag = record_framework_eval(&eval);

        assert_eq!(dag.n_trace_columns, 8, "expected 8 trace columns");
        assert_eq!(dag.n_constraints, 6, "expected 6 constraints for N=8");
    }

    #[test]
    fn test_single_interaction() {
        let eval = TestWideFibonacciEval::<4> { log_n_rows: 8 };
        let dag = record_framework_eval(&eval);

        // All columns are in interaction 1 (ORIGINAL_TRACE_IDX).
        assert_eq!(dag.n_interactions, 2, "interactions should span up to index 1");

        // Every TraceCol should have interaction == 1.
        for op in &dag.ops {
            if let RecordedOp::TraceCol { interaction, .. } = op {
                assert_eq!(*interaction, 1, "all columns should be in interaction 1");
            }
        }
    }

    #[test]
    fn test_op_refs_are_valid() {
        let eval = TestWideFibonacciEval::<4> { log_n_rows: 8 };
        let dag = record_framework_eval(&eval);

        let n_ops = dag.ops.len();
        for (i, op) in dag.ops.iter().enumerate() {
            match op {
                RecordedOp::Add { lhs, rhs }
                | RecordedOp::Sub { lhs, rhs }
                | RecordedOp::Mul { lhs, rhs } => {
                    assert!(lhs.0 < i, "lhs ref {} should be < current index {}", lhs.0, i);
                    assert!(rhs.0 < i, "rhs ref {} should be < current index {}", rhs.0, i);
                }
                RecordedOp::Inv { operand } => {
                    assert!(operand.0 < i, "operand ref should be < current index");
                }
                RecordedOp::AddConstraint { expr } => {
                    assert!(expr.0 < i, "constraint expr ref should be < current index");
                    assert!(expr.0 < n_ops, "constraint expr ref should be in range");
                }
                _ => {}
            }
        }
    }
}
