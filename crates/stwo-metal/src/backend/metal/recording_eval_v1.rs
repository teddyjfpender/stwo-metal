//! Recording evaluator for lowering `FrameworkEval` constraint trees to V1
//! bytecode.
//!
//! The [`RecordingEvaluator`] implements the [`EvalAtRow`] trait from the stwo
//! constraint framework.  Instead of computing concrete field values it records
//! symbolic operations as V1 base/ext instructions and constraint roots.

use std::cell::RefCell;
use std::fmt;
use std::ops::{Add, AddAssign, Mul, MulAssign, Neg, Sub};
use std::rc::Rc;
use std::vec::Vec;

use ark_std::{One, Zero};
use stwo::core::fields::m31::BaseField;
use stwo::core::fields::qm31::SecureField;
use stwo::core::fields::FieldExpOps;
use stwo::core::Fraction;
use stwo_constraint_framework::logup::LogupAtRow;
use stwo_constraint_framework::EvalAtRow;

use super::eval_program_v1::{
    MetalEvaluationProgramBaseInstV1, MetalEvaluationProgramBaseOpcodeV1,
    MetalEvaluationProgramExtInstV1, MetalEvaluationProgramExtOpcodeV1,
};

// =========================================================================
// Shared recording state
// =========================================================================

/// Accumulated recording state shared (via `Rc<RefCell<..>>`) between the
/// evaluator and every symbolic value it produces.
#[allow(dead_code)]
pub(crate) struct RecordingState {
    pub base_insts: Vec<MetalEvaluationProgramBaseInstV1>,
    pub ext_insts: Vec<MetalEvaluationProgramExtInstV1>,
    pub constraint_roots: Vec<u32>,
    next_base_reg: u16,
    next_ext_reg: u16,
    max_interaction: usize,
    columns_per_interaction: Vec<u32>,
    n_base_params: u32,
    n_ext_params: u32,
}

impl RecordingState {
    fn new() -> Self {
        Self {
            base_insts: Vec::new(),
            ext_insts: Vec::new(),
            constraint_roots: Vec::new(),
            next_base_reg: 0,
            next_ext_reg: 0,
            max_interaction: 0,
            columns_per_interaction: Vec::new(),
            n_base_params: 0,
            n_ext_params: 0,
        }
    }

    fn alloc_base_reg(&mut self) -> u16 {
        let reg = self.next_base_reg;
        self.next_base_reg = reg.checked_add(1).expect("base register overflow");
        reg
    }

    fn alloc_ext_reg(&mut self) -> u16 {
        let reg = self.next_ext_reg;
        self.next_ext_reg = reg.checked_add(1).expect("ext register overflow");
        reg
    }

    fn tracker_next_column(&mut self, interaction: usize) -> u32 {
        if interaction >= self.columns_per_interaction.len() {
            self.columns_per_interaction.resize(interaction + 1, 0);
        }
        let col = self.columns_per_interaction[interaction];
        self.columns_per_interaction[interaction] = col + 1;
        col
    }

    /// Ensure there is a `Const(0)` base instruction and return its register.
    fn ensure_zero_const(&mut self) -> u16 {
        for inst in &self.base_insts {
            if inst.op == MetalEvaluationProgramBaseOpcodeV1::Const as u8 && inst.a == 0 {
                return inst.dst;
            }
        }
        let dst = self.alloc_base_reg();
        self.base_insts
            .push(MetalEvaluationProgramBaseInstV1::const_value(dst, 0));
        dst
    }

    /// Emit an ext `Const` instruction for a `SecureField` value and return
    /// the allocated ext register.
    fn emit_ext_const(&mut self, value: SecureField) -> u16 {
        let limbs = value.to_m31_array();
        let dst = self.alloc_ext_reg();
        self.ext_insts.push(MetalEvaluationProgramExtInstV1 {
            op: MetalEvaluationProgramExtOpcodeV1::Const as u8,
            reserved0: 0,
            dst,
            a: limbs[0].0,
            b: limbs[1].0,
            c: limbs[2].0,
            d: limbs[3].0,
        });
        dst
    }

    /// Promote a base register to an ext register using SecureCol with zero
    /// padding for the upper 3 limbs.
    fn promote_base_to_ext(&mut self, base_reg: u16) -> u16 {
        let zero_reg = self.ensure_zero_const();
        let ext_dst = self.alloc_ext_reg();
        self.ext_insts
            .push(MetalEvaluationProgramExtInstV1::secure_col(
                ext_dst,
                base_reg as u32,
                zero_reg as u32,
                zero_reg as u32,
                zero_reg as u32,
            ));
        ext_dst
    }

    /// Emit an ext binary instruction (Add, Sub, Mul).
    fn emit_ext_binary(
        &mut self,
        op: MetalEvaluationProgramExtOpcodeV1,
        src1: u16,
        src2: u16,
    ) -> u16 {
        let dst = self.alloc_ext_reg();
        self.ext_insts.push(MetalEvaluationProgramExtInstV1 {
            op: op as u8,
            reserved0: 0,
            dst,
            a: src1 as u32,
            b: src2 as u32,
            c: 0,
            d: 0,
        });
        dst
    }

    /// Emit an ext Neg instruction.
    fn emit_ext_neg(&mut self, src: u16) -> u16 {
        let dst = self.alloc_ext_reg();
        self.ext_insts.push(MetalEvaluationProgramExtInstV1 {
            op: MetalEvaluationProgramExtOpcodeV1::Neg as u8,
            reserved0: 0,
            dst,
            a: src as u32,
            b: 0,
            c: 0,
            d: 0,
        });
        dst
    }

    /// Number of interactions (1-indexed count).
    #[allow(dead_code)]
    pub fn n_interactions(&self) -> u32 {
        if self.columns_per_interaction.is_empty() && self.n_base_params == 0 {
            0
        } else {
            (self.max_interaction as u32) + 1
        }
    }

    #[allow(dead_code)]
    pub fn n_base_params(&self) -> u32 {
        self.n_base_params
    }

    #[allow(dead_code)]
    pub fn n_ext_params(&self) -> u32 {
        self.n_ext_params
    }

    pub fn max_base_regs(&self) -> u32 {
        self.next_base_reg as u32
    }

    pub fn max_ext_regs(&self) -> u32 {
        self.next_ext_reg as u32
    }
}

type SharedState = Rc<RefCell<RecordingState>>;

// =========================================================================
// RecordedBaseValue  (EvalAtRow::F)
// =========================================================================

/// Symbolic base-field value.  Carries a shared reference to the recording
/// state so that arithmetic operators can emit V1 instructions.
#[derive(Clone)]
pub(crate) struct RecordedBaseValue {
    reg: u16,
    state: SharedState,
    /// When `Some(v)`, this value represents a constant that has not yet been
    /// emitted as a `Const` instruction.  It will be materialized lazily when
    /// the value participates in arithmetic with another (real) value.
    pending_const: Option<u32>,
}

impl RecordedBaseValue {
    fn new(reg: u16, state: &SharedState) -> Self {
        Self {
            reg,
            state: Rc::clone(state),
            pending_const: None,
        }
    }

    /// Create a detached value used for trait defaults that must not be used in
    /// constraint-producing arithmetic.
    fn detached() -> Self {
        Self {
            reg: u16::MAX,
            state: Rc::new(RefCell::new(RecordingState::new())),
            pending_const: None,
        }
    }

    /// If this value has a pending constant, materialize it by emitting a
    /// `Const` instruction into `target_state`, allocating a register there,
    /// and updating `self` to point at that register / state.
    fn materialize_if_pending(&mut self, target_state: &SharedState) {
        if let Some(val) = self.pending_const.take() {
            let mut st = target_state.borrow_mut();
            let dst = st.alloc_base_reg();
            st.base_insts
                .push(MetalEvaluationProgramBaseInstV1::const_value(dst, val));
            drop(st);
            self.reg = dst;
            self.state = Rc::clone(target_state);
        }
    }

    /// Returns `true` when this value carries a pending constant that has not
    /// yet been emitted.
    fn is_pending(&self) -> bool {
        self.pending_const.is_some()
    }

    fn emit_binary(
        mut self,
        op: MetalEvaluationProgramBaseOpcodeV1,
        mut rhs: Self,
    ) -> Self {
        // Materialize any pending constants before emitting the binary op.
        // Use the other operand's state when one side is pending.
        if self.is_pending() && rhs.is_pending() {
            // Both pending — fold at record time to avoid creating a
            // detached-state value.  This prevents cross-state register
            // references when the result is later combined with main-state
            // values.
            let a = self.pending_const.take().unwrap();
            let b = rhs.pending_const.take().unwrap();
            let result = match op {
                MetalEvaluationProgramBaseOpcodeV1::Add => {
                    BaseField::from_u32_unchecked(a) + BaseField::from_u32_unchecked(b)
                }
                MetalEvaluationProgramBaseOpcodeV1::Sub => {
                    BaseField::from_u32_unchecked(a) - BaseField::from_u32_unchecked(b)
                }
                MetalEvaluationProgramBaseOpcodeV1::Mul => {
                    BaseField::from_u32_unchecked(a) * BaseField::from_u32_unchecked(b)
                }
                _ => {
                    // Fallback: materialize into self's state for unhandled ops.
                    self.pending_const = Some(a);
                    rhs.pending_const = Some(b);
                    self.materialize_if_pending(&self.state.clone());
                    rhs.materialize_if_pending(&self.state);
                    // Fall through to emit_binary below.
                    let mut st = self.state.borrow_mut();
                    let dst = st.alloc_base_reg();
                    st.base_insts
                        .push(MetalEvaluationProgramBaseInstV1::binary(
                            op,
                            dst,
                            self.reg as u32,
                            rhs.reg as u32,
                        ));
                    drop(st);
                    return RecordedBaseValue::new(dst, &self.state);
                }
            };
            return RecordedBaseValue {
                reg: u16::MAX,
                state: self.state,
                pending_const: Some(result.0),
            };
        } else if self.is_pending() {
            self.materialize_if_pending(&rhs.state);
        } else if rhs.is_pending() {
            rhs.materialize_if_pending(&self.state);
        }

        let mut st = self.state.borrow_mut();
        let dst = st.alloc_base_reg();
        st.base_insts
            .push(MetalEvaluationProgramBaseInstV1::binary(
                op,
                dst,
                self.reg as u32,
                rhs.reg as u32,
            ));
        drop(st);
        RecordedBaseValue::new(dst, &self.state)
    }
}

impl fmt::Debug for RecordedBaseValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RecordedBaseValue(r{})", self.reg)
    }
}

// --- Arithmetic trait impls ---

impl Add for RecordedBaseValue {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        self.emit_binary(MetalEvaluationProgramBaseOpcodeV1::Add, rhs)
    }
}

impl Sub for RecordedBaseValue {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        self.emit_binary(MetalEvaluationProgramBaseOpcodeV1::Sub, rhs)
    }
}

impl Mul for RecordedBaseValue {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self {
        self.emit_binary(MetalEvaluationProgramBaseOpcodeV1::Mul, rhs)
    }
}

impl MulAssign for RecordedBaseValue {
    fn mul_assign(&mut self, rhs: Self) {
        *self = self.clone() * rhs;
    }
}

impl Neg for RecordedBaseValue {
    type Output = Self;
    fn neg(mut self) -> Self {
        // Fold pending constant at record time to avoid detached state.
        if self.is_pending() {
            let val = self.pending_const.take().unwrap();
            let neg_val = -BaseField::from_u32_unchecked(val);
            return RecordedBaseValue {
                reg: u16::MAX,
                state: self.state,
                pending_const: Some(neg_val.0),
            };
        }
        let mut st = self.state.borrow_mut();
        let dst = st.alloc_base_reg();
        st.base_insts.push(MetalEvaluationProgramBaseInstV1 {
            op: MetalEvaluationProgramBaseOpcodeV1::Neg as u8,
            interaction: 0,
            dst,
            a: self.reg as u32,
            b: 0,
            imm: 0,
        });
        drop(st);
        RecordedBaseValue::new(dst, &self.state)
    }
}

impl AddAssign for RecordedBaseValue {
    fn add_assign(&mut self, rhs: Self) {
        *self = self.clone() + rhs;
    }
}

impl AddAssign<BaseField> for RecordedBaseValue {
    fn add_assign(&mut self, rhs: BaseField) {
        let rhs_val = {
            let mut st = self.state.borrow_mut();
            let dst = st.alloc_base_reg();
            st.base_insts
                .push(MetalEvaluationProgramBaseInstV1::const_value(dst, rhs.0));
            RecordedBaseValue::new(dst, &self.state)
        };
        *self = self.clone() + rhs_val;
    }
}

impl Mul<BaseField> for RecordedBaseValue {
    type Output = Self;
    fn mul(self, rhs: BaseField) -> Self {
        let rhs_val = {
            let mut st = self.state.borrow_mut();
            let dst = st.alloc_base_reg();
            st.base_insts
                .push(MetalEvaluationProgramBaseInstV1::const_value(dst, rhs.0));
            RecordedBaseValue::new(dst, &self.state)
        };
        self * rhs_val
    }
}

impl Add<SecureField> for RecordedBaseValue {
    type Output = RecordedExtValue;
    fn add(mut self, rhs: SecureField) -> RecordedExtValue {
        // Materialize pending constant if needed.
        if self.is_pending() {
            let st = self.state.clone();
            self.materialize_if_pending(&st);
        }
        // Promote base to ext, emit const for the SecureField, then add.
        let mut st = self.state.borrow_mut();
        let promoted = st.promote_base_to_ext(self.reg);
        let rhs_reg = st.emit_ext_const(rhs);
        let dst = st.emit_ext_binary(MetalEvaluationProgramExtOpcodeV1::Add, promoted, rhs_reg);
        drop(st);
        RecordedExtValue::realized(dst, &self.state)
    }
}

impl Mul<SecureField> for RecordedBaseValue {
    type Output = RecordedExtValue;
    fn mul(mut self, rhs: SecureField) -> RecordedExtValue {
        // Materialize pending constant if needed.
        if self.is_pending() {
            let st = self.state.clone();
            self.materialize_if_pending(&st);
        }
        // Promote base to ext, emit const for the SecureField, then mul.
        let mut st = self.state.borrow_mut();
        let promoted = st.promote_base_to_ext(self.reg);
        let rhs_reg = st.emit_ext_const(rhs);
        let dst = st.emit_ext_binary(MetalEvaluationProgramExtOpcodeV1::Mul, promoted, rhs_reg);
        drop(st);
        RecordedExtValue::realized(dst, &self.state)
    }
}

impl From<BaseField> for RecordedBaseValue {
    fn from(value: BaseField) -> Self {
        let mut v = RecordedBaseValue::detached();
        v.pending_const = Some(value.0);
        v
    }
}

impl FieldExpOps for RecordedBaseValue {
    fn inverse(&self) -> Self {
        // If pending, materialise into a clone so we have a valid register.
        let src = if self.is_pending() {
            let mut clone = self.clone();
            let st = clone.state.clone();
            clone.materialize_if_pending(&st);
            clone
        } else {
            self.clone()
        };
        let mut st = src.state.borrow_mut();
        let dst = st.alloc_base_reg();
        st.base_insts.push(MetalEvaluationProgramBaseInstV1 {
            op: MetalEvaluationProgramBaseOpcodeV1::Inv as u8,
            interaction: 0,
            dst,
            a: src.reg as u32,
            b: 0,
            imm: 0,
        });
        drop(st);
        RecordedBaseValue::new(dst, &src.state)
    }
}

impl Zero for RecordedBaseValue {
    fn zero() -> Self {
        let mut v = RecordedBaseValue::detached();
        v.pending_const = Some(0);
        v
    }
    fn is_zero(&self) -> bool {
        false
    }
}

impl One for RecordedBaseValue {
    fn one() -> Self {
        let mut v = RecordedBaseValue::detached();
        v.pending_const = Some(1);
        v
    }
}

// =========================================================================
// RecordedExtValue  (EvalAtRow::EF)
// =========================================================================

/// The kind of a symbolic extension-field value.
///
/// Values start as detached (Zero, LazyConst, PromotedBase) and become
/// "Realized" (owning an ext register in the shared recording state) when they
/// participate in arithmetic with another Realized value.
#[derive(Clone)]
enum RecordedExtValueKind {
    /// Has a register allocation in the shared recording state.
    Realized { reg: u16, state: SharedState },
    /// A base-field value promoted to the ext type.  Holds the base register
    /// index and shared state.  Will emit a `SecureCol` when realized.
    PromotedBase { base_reg: u16, state: SharedState },
    /// A lazy SecureField constant.  Will emit an ext `Const` when realized.
    LazyConst(SecureField),
    /// Detached zero — placeholder for `Zero::zero()` / `One::one()`.
    Zero,
}

/// Symbolic extension-field value.
#[derive(Clone)]
pub(crate) struct RecordedExtValue {
    kind: RecordedExtValueKind,
}

impl RecordedExtValue {
    /// Create a realized ext value with an already-allocated ext register.
    fn realized(reg: u16, state: &SharedState) -> Self {
        Self {
            kind: RecordedExtValueKind::Realized {
                reg,
                state: Rc::clone(state),
            },
        }
    }

    /// Create a detached zero placeholder.
    fn detached() -> Self {
        Self {
            kind: RecordedExtValueKind::Zero,
        }
    }

    /// Resolve this value to a (register, SharedState) pair, emitting any
    /// pending instructions into `target_state`.  If `self` already has state,
    /// its own state is used; otherwise `target_state` provides it.
    fn realize(&self, target_state: &SharedState) -> (u16, SharedState) {
        match &self.kind {
            RecordedExtValueKind::Realized { reg, state } => (*reg, Rc::clone(state)),
            RecordedExtValueKind::PromotedBase { base_reg, state } => {
                let mut st = state.borrow_mut();
                let reg = st.promote_base_to_ext(*base_reg);
                drop(st);
                (reg, Rc::clone(state))
            }
            RecordedExtValueKind::LazyConst(value) => {
                let mut st = target_state.borrow_mut();
                let reg = st.emit_ext_const(*value);
                drop(st);
                (reg, Rc::clone(target_state))
            }
            RecordedExtValueKind::Zero => {
                // Emit a const 0 ext value.
                let mut st = target_state.borrow_mut();
                let reg = st.emit_ext_const(SecureField::zero());
                drop(st);
                (reg, Rc::clone(target_state))
            }
        }
    }

    /// Get the shared state from this value, if it has one.
    fn state(&self) -> Option<&SharedState> {
        match &self.kind {
            RecordedExtValueKind::Realized { state, .. } => Some(state),
            RecordedExtValueKind::PromotedBase { state, .. } => Some(state),
            RecordedExtValueKind::LazyConst(_) => None,
            RecordedExtValueKind::Zero => None,
        }
    }

    /// Pick the "real" shared state from either `self` or `other`.
    /// Panics if neither has state.
    fn pick_state<'a>(&'a self, other: &'a Self) -> &'a SharedState {
        self.state()
            .or_else(|| other.state())
            .expect("at least one ext value must have recording state for arithmetic")
    }

    /// Emit a binary ext operation between `self` and `rhs`.
    fn emit_ext_binary_op(
        &self,
        op: MetalEvaluationProgramExtOpcodeV1,
        rhs: &Self,
    ) -> Self {
        let target_state = self.pick_state(rhs);
        let (lhs_reg, _) = self.realize(target_state);
        let (rhs_reg, state) = rhs.realize(target_state);
        let mut st = state.borrow_mut();
        let dst = st.emit_ext_binary(op, lhs_reg, rhs_reg);
        drop(st);
        RecordedExtValue::realized(dst, &state)
    }
}

impl fmt::Debug for RecordedExtValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            RecordedExtValueKind::Realized { reg, .. } => {
                write!(f, "RecordedExtValue(r{})", reg)
            }
            RecordedExtValueKind::PromotedBase { base_reg, .. } => {
                write!(f, "RecordedExtValue(promoted_base r{})", base_reg)
            }
            RecordedExtValueKind::LazyConst(v) => {
                write!(f, "RecordedExtValue(lazy_const {:?})", v)
            }
            RecordedExtValueKind::Zero => {
                write!(f, "RecordedExtValue(zero)")
            }
        }
    }
}

// --- Ext arithmetic trait impls ---

impl Add for RecordedExtValue {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        // Optimization: adding zero is identity.
        if matches!(self.kind, RecordedExtValueKind::Zero) {
            return rhs;
        }
        if matches!(rhs.kind, RecordedExtValueKind::Zero) {
            return self;
        }
        self.emit_ext_binary_op(MetalEvaluationProgramExtOpcodeV1::Add, &rhs)
    }
}

impl Sub for RecordedExtValue {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        if matches!(rhs.kind, RecordedExtValueKind::Zero) {
            return self;
        }
        self.emit_ext_binary_op(MetalEvaluationProgramExtOpcodeV1::Sub, &rhs)
    }
}

impl Mul for RecordedExtValue {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self {
        // Optimization: multiplying by one is identity.
        // (One::one() creates a Zero-kind, but real multiplications use
        // concrete values, so this optimization only helps detached ones.)
        self.emit_ext_binary_op(MetalEvaluationProgramExtOpcodeV1::Mul, &rhs)
    }
}

impl MulAssign for RecordedExtValue {
    fn mul_assign(&mut self, rhs: Self) {
        *self = self.clone() * rhs;
    }
}

impl Neg for RecordedExtValue {
    type Output = Self;
    fn neg(self) -> Self {
        match self.kind {
            RecordedExtValueKind::Zero => RecordedExtValue { kind: RecordedExtValueKind::Zero },
            RecordedExtValueKind::LazyConst(v) => RecordedExtValue {
                kind: RecordedExtValueKind::LazyConst(-v),
            },
            _ => {
                let state = self.state().unwrap().clone();
                let (src_reg, state) = self.realize(&state);
                let mut st = state.borrow_mut();
                let dst = st.emit_ext_neg(src_reg);
                drop(st);
                RecordedExtValue::realized(dst, &state)
            }
        }
    }
}

impl AddAssign for RecordedExtValue {
    fn add_assign(&mut self, rhs: Self) {
        *self = self.clone() + rhs;
    }
}

impl Add<BaseField> for RecordedExtValue {
    type Output = Self;
    fn add(self, rhs: BaseField) -> Self {
        // Emit a base const, promote it to ext, then add.
        let state = self.state().expect(
            "RecordedExtValue + BaseField: ext value must have recording state",
        );
        let state = Rc::clone(state);
        let rhs_ext = {
            let mut st = state.borrow_mut();
            let base_dst = st.alloc_base_reg();
            st.base_insts
                .push(MetalEvaluationProgramBaseInstV1::const_value(base_dst, rhs.0));
            let ext_dst = st.promote_base_to_ext(base_dst);
            ext_dst
        };
        let (self_reg, _) = self.realize(&state);
        let mut st = state.borrow_mut();
        let dst = st.emit_ext_binary(MetalEvaluationProgramExtOpcodeV1::Add, self_reg, rhs_ext);
        drop(st);
        RecordedExtValue::realized(dst, &state)
    }
}

impl Mul<BaseField> for RecordedExtValue {
    type Output = Self;
    fn mul(self, rhs: BaseField) -> Self {
        // Emit a base const, promote it to ext, then mul.
        let state = self.state().expect(
            "RecordedExtValue * BaseField: ext value must have recording state",
        );
        let state = Rc::clone(state);
        let rhs_ext = {
            let mut st = state.borrow_mut();
            let base_dst = st.alloc_base_reg();
            st.base_insts
                .push(MetalEvaluationProgramBaseInstV1::const_value(base_dst, rhs.0));
            let ext_dst = st.promote_base_to_ext(base_dst);
            ext_dst
        };
        let (self_reg, _) = self.realize(&state);
        let mut st = state.borrow_mut();
        let dst = st.emit_ext_binary(MetalEvaluationProgramExtOpcodeV1::Mul, self_reg, rhs_ext);
        drop(st);
        RecordedExtValue::realized(dst, &state)
    }
}

impl Add<SecureField> for RecordedExtValue {
    type Output = Self;
    fn add(self, rhs: SecureField) -> Self {
        let rhs_ext = RecordedExtValue {
            kind: RecordedExtValueKind::LazyConst(rhs),
        };
        self + rhs_ext
    }
}

impl Sub<SecureField> for RecordedExtValue {
    type Output = Self;
    fn sub(self, rhs: SecureField) -> Self {
        let rhs_ext = RecordedExtValue {
            kind: RecordedExtValueKind::LazyConst(rhs),
        };
        self - rhs_ext
    }
}

impl Mul<SecureField> for RecordedExtValue {
    type Output = Self;
    fn mul(self, rhs: SecureField) -> Self {
        let rhs_ext = RecordedExtValue {
            kind: RecordedExtValueKind::LazyConst(rhs),
        };
        self * rhs_ext
    }
}

impl Add<RecordedBaseValue> for RecordedExtValue {
    type Output = Self;
    fn add(self, mut rhs: RecordedBaseValue) -> Self {
        // Fold Zero/LazyConst + pending-base at record time.
        if rhs.is_pending() {
            match &self.kind {
                RecordedExtValueKind::Zero => {
                    let base_val = BaseField::from_u32_unchecked(rhs.pending_const.unwrap());
                    return RecordedExtValue {
                        kind: RecordedExtValueKind::LazyConst(SecureField::from(base_val)),
                    };
                }
                RecordedExtValueKind::LazyConst(ext_val) => {
                    let base_val = BaseField::from_u32_unchecked(rhs.pending_const.unwrap());
                    return RecordedExtValue {
                        kind: RecordedExtValueKind::LazyConst(
                            *ext_val + SecureField::from(base_val),
                        ),
                    };
                }
                _ => {
                    let target = self
                        .state()
                        .cloned()
                        .unwrap_or_else(|| rhs.state.clone());
                    rhs.materialize_if_pending(&target);
                }
            }
        }
        let rhs_ext = RecordedExtValue {
            kind: RecordedExtValueKind::PromotedBase {
                base_reg: rhs.reg,
                state: rhs.state,
            },
        };
        self + rhs_ext
    }
}

impl Mul<RecordedBaseValue> for RecordedExtValue {
    type Output = Self;
    fn mul(self, mut rhs: RecordedBaseValue) -> Self {
        // Fold LazyConst × pending-base at record time to avoid creating a
        // detached-state ext value that would later cause cross-state
        // register references.
        if rhs.is_pending() {
            if let RecordedExtValueKind::LazyConst(ext_val) = &self.kind {
                let base_val = BaseField::from_u32_unchecked(rhs.pending_const.unwrap());
                return RecordedExtValue {
                    kind: RecordedExtValueKind::LazyConst(*ext_val * base_val),
                };
            }
            let target = self
                .state()
                .cloned()
                .unwrap_or_else(|| rhs.state.clone());
            rhs.materialize_if_pending(&target);
        }
        let rhs_ext = RecordedExtValue {
            kind: RecordedExtValueKind::PromotedBase {
                base_reg: rhs.reg,
                state: rhs.state,
            },
        };
        self * rhs_ext
    }
}

impl From<SecureField> for RecordedExtValue {
    fn from(value: SecureField) -> Self {
        RecordedExtValue {
            kind: RecordedExtValueKind::LazyConst(value),
        }
    }
}

impl From<RecordedBaseValue> for RecordedExtValue {
    fn from(v: RecordedBaseValue) -> Self {
        if v.is_pending() {
            // Convert pending base constant directly to a LazyConst ext
            // value to avoid creating a detached-state PromotedBase that
            // would cause cross-state register references.
            let base_val = BaseField::from_u32_unchecked(v.pending_const.unwrap());
            // Mark logup as finalized before dropping v (it carries a
            // detached LogupAtRow via its state, but we don't need it).
            return RecordedExtValue {
                kind: RecordedExtValueKind::LazyConst(SecureField::from(base_val)),
            };
        }
        RecordedExtValue {
            kind: RecordedExtValueKind::PromotedBase {
                base_reg: v.reg,
                state: v.state,
            },
        }
    }
}

impl Zero for RecordedExtValue {
    fn zero() -> Self {
        RecordedExtValue::detached()
    }
    fn is_zero(&self) -> bool {
        false
    }
}

impl One for RecordedExtValue {
    fn one() -> Self {
        RecordedExtValue {
            kind: RecordedExtValueKind::LazyConst(SecureField::one()),
        }
    }
}

// =========================================================================
// RecordingEvaluator  (implements EvalAtRow)
// =========================================================================

/// A recording evaluator that implements [`EvalAtRow`] by emitting V1 bytecode
/// into a shared [`RecordingState`].
pub(crate) struct RecordingEvaluator {
    state: SharedState,
    pub logup: LogupAtRow<Self>,
}

impl RecordingEvaluator {
    pub fn new() -> Self {
        Self {
            state: Rc::new(RefCell::new(RecordingState::new())),
            logup: LogupAtRow::dummy(),
        }
    }

    /// Inject a base-parameter read.  Returns a symbolic value that maps to a
    /// `Param` instruction.  Must be called *before* passing this evaluator to
    /// `FrameworkEval::evaluate`.
    pub fn inject_base_param(&self, interaction: u8, slot: u32) -> RecordedBaseValue {
        let mut st = self.state.borrow_mut();
        let dst = st.alloc_base_reg();
        st.base_insts.push(MetalEvaluationProgramBaseInstV1 {
            op: MetalEvaluationProgramBaseOpcodeV1::Param as u8,
            interaction,
            dst,
            a: slot,
            b: 0,
            imm: 0,
        });
        st.n_base_params += 1;
        if interaction as usize > st.max_interaction {
            st.max_interaction = interaction as usize;
        }
        RecordedBaseValue::new(dst, &self.state)
    }

    /// Consume the evaluator and return the accumulated recording state.
    ///
    /// Panics if any symbolic values still hold references to the shared state.
    pub fn finish(self) -> RecordingState {
        // Explicitly drop logup first so its Rc references are released
        // before we try to unwrap.
        drop(self.logup);
        match Rc::try_unwrap(self.state) {
            Ok(cell) => cell.into_inner(),
            Err(_) => panic!("RecordingEvaluator::finish: dangling symbolic value references"),
        }
    }
}

impl EvalAtRow for RecordingEvaluator {
    type F = RecordedBaseValue;
    type EF = RecordedExtValue;

    fn next_interaction_mask<const N: usize>(
        &mut self,
        interaction: usize,
        offsets: [isize; N],
    ) -> [Self::F; N] {
        // Allocate ONE column index for all offsets — each offset reads the
        // same column at a different row.  The previous implementation
        // incorrectly called tracker_next_column once per offset, inflating
        // the column count and producing out-of-range accesses.
        let col = {
            let mut st = self.state.borrow_mut();
            if interaction > st.max_interaction {
                st.max_interaction = interaction;
            }
            st.tracker_next_column(interaction)
        };
        std::array::from_fn(|i| {
            let mut st = self.state.borrow_mut();
            let dst = st.alloc_base_reg();
            st.base_insts
                .push(MetalEvaluationProgramBaseInstV1::trace_col(
                    dst,
                    interaction as u8,
                    col,
                    offsets[i] as i32,
                ));
            RecordedBaseValue::new(dst, &self.state)
        })
    }

    fn add_constraint<G>(&mut self, constraint: G)
    where
        Self::EF: Mul<G, Output = Self::EF> + From<G>,
    {
        // Convert the constraint to an ext-field symbolic value.
        let ext_val: Self::EF = Self::EF::from(constraint);

        match &ext_val.kind {
            RecordedExtValueKind::Realized { reg, .. } => {
                // Already a native ext register — use it directly as a
                // constraint root.
                let mut st = self.state.borrow_mut();
                st.constraint_roots.push(*reg as u32);
            }
            RecordedExtValueKind::PromotedBase { base_reg, .. } => {
                // Base value promoted to ext — emit SecureCol to lift it to
                // a constraint root.
                let mut st = self.state.borrow_mut();
                let zero_reg = st.ensure_zero_const();
                let ext_dst = st.alloc_ext_reg();
                st.ext_insts
                    .push(MetalEvaluationProgramExtInstV1::secure_col(
                        ext_dst,
                        *base_reg as u32,
                        zero_reg as u32,
                        zero_reg as u32,
                        zero_reg as u32,
                    ));
                st.constraint_roots.push(ext_dst as u32);
            }
            RecordedExtValueKind::LazyConst(value) => {
                // A constant constraint — emit a Const ext instruction.
                let mut st = self.state.borrow_mut();
                let reg = st.emit_ext_const(*value);
                st.constraint_roots.push(reg as u32);
            }
            RecordedExtValueKind::Zero => {
                // Zero constraint — emit a zero ext constant.
                let mut st = self.state.borrow_mut();
                let reg = st.emit_ext_const(SecureField::zero());
                st.constraint_roots.push(reg as u32);
            }
        }
    }

    fn combine_ef(values: [Self::F; 4]) -> Self::EF {
        let state = &values[0].state;
        let mut st = state.borrow_mut();
        let ext_dst = st.alloc_ext_reg();
        st.ext_insts
            .push(MetalEvaluationProgramExtInstV1::secure_col(
                ext_dst,
                values[0].reg as u32,
                values[1].reg as u32,
                values[2].reg as u32,
                values[3].reg as u32,
            ));
        drop(st);
        RecordedExtValue::realized(ext_dst, state)
    }

    // --- Logup support (manual implementation of logup_proxy!()) ---

    fn write_logup_frac(&mut self, fraction: Fraction<Self::EF, Self::EF>) {
        if self.logup.fracs.is_empty() {
            self.logup.is_finalized = false;
        }
        self.logup.fracs.push(fraction.clone());
    }

    fn finalize_logup_batched(&mut self, batching: &Vec<usize>) {
        assert!(!self.logup.is_finalized, "LogupAtRow was already finalized");
        assert_eq!(
            batching.len(),
            self.logup.fracs.len(),
            "Batching must be of the same length as the number of entries"
        );

        let last_batch = *batching.iter().max().unwrap();

        let mut fracs_by_batch =
            hashbrown::HashMap::<usize, Vec<Fraction<Self::EF, Self::EF>>>::new();

        for (batch, frac) in batching.iter().zip(self.logup.fracs.iter()) {
            fracs_by_batch
                .entry(*batch)
                .or_insert_with(Vec::new)
                .push(frac.clone());
        }

        let keys_set: hashbrown::HashSet<_> = fracs_by_batch.keys().cloned().collect();
        let all_batches_set: hashbrown::HashSet<_> = (0..last_batch + 1).collect();

        assert_eq!(
            keys_set, all_batches_set,
            "Batching must contain all consecutive batches"
        );

        let mut prev_col_cumsum = <Self::EF as Zero>::zero();

        // All batches except the last are cumulatively summed in new
        // interaction columns.
        for batch_id in 0..last_batch {
            let cur_frac: Fraction<_, _> = fracs_by_batch[&batch_id].iter().cloned().sum();
            let [cur_cumsum] =
                self.next_extension_interaction_mask(self.logup.interaction, [0]);
            let diff = cur_cumsum.clone() - prev_col_cumsum.clone();
            prev_col_cumsum = cur_cumsum;
            self.add_constraint(diff * cur_frac.denominator - cur_frac.numerator);
        }

        let frac: Fraction<_, _> = fracs_by_batch[&last_batch].clone().into_iter().sum();
        let [prev_row_cumsum, cur_cumsum] =
            self.next_extension_interaction_mask(self.logup.interaction, [-1, 0]);

        let diff = cur_cumsum - prev_row_cumsum - prev_col_cumsum.clone();
        // Instead of checking diff = num / denom, check
        // diff = num / denom - cumsum_shift.  This makes
        // (num / denom - cumsum_shift) have sum zero, which makes the
        // constraint uniform — apply on all rows.
        let shifted_diff = diff + self.logup.cumsum_shift.clone();

        self.add_constraint(shifted_diff * frac.denominator - frac.numerator);

        self.logup.is_finalized = true;
    }

    fn finalize_logup(&mut self) {
        let batches = (0..self.logup.fracs.len()).collect();
        self.finalize_logup_batched(&batches)
    }

    fn finalize_logup_in_pairs(&mut self) {
        let batches = (0..self.logup.fracs.len()).map(|n| n / 2).collect();
        self.finalize_logup_batched(&batches)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recording_evaluator_emits_trace_col_for_next_trace_mask() {
        let mut eval = RecordingEvaluator::new();
        let [val] = eval.next_interaction_mask(1, [0]);
        drop(val);
        let state = eval.finish();
        assert_eq!(state.base_insts.len(), 1);
        assert_eq!(
            state.base_insts[0].op,
            MetalEvaluationProgramBaseOpcodeV1::TraceCol as u8
        );
        assert_eq!(state.base_insts[0].interaction, 1);
        assert_eq!(state.base_insts[0].a, 0); // column 0
    }

    #[test]
    fn recording_evaluator_emits_param_for_inject_base_param() {
        let eval = RecordingEvaluator::new();
        let val = eval.inject_base_param(0, 0);
        drop(val);
        let state = eval.finish();
        assert_eq!(state.base_insts.len(), 1);
        assert_eq!(
            state.base_insts[0].op,
            MetalEvaluationProgramBaseOpcodeV1::Param as u8
        );
        assert_eq!(state.n_base_params(), 1);
    }

    #[test]
    fn recorded_base_values_emit_binary_ops() {
        let mut eval = RecordingEvaluator::new();
        let [a] = eval.next_interaction_mask(1, [0]);
        let [b] = eval.next_interaction_mask(1, [0]);
        let c = a + b;
        drop(c);
        let state = eval.finish();
        // 2 trace cols + 1 add = 3 base instructions
        assert_eq!(state.base_insts.len(), 3);
        assert_eq!(
            state.base_insts[2].op,
            MetalEvaluationProgramBaseOpcodeV1::Add as u8
        );
    }

    #[test]
    fn ext_add_emits_ext_add_instruction() {
        let mut eval = RecordingEvaluator::new();
        // Read 4 base columns and combine into an ext value.
        let [a] = eval.next_interaction_mask(1, [0]);
        let [b] = eval.next_interaction_mask(1, [0]);
        let [c] = eval.next_interaction_mask(1, [0]);
        let [d] = eval.next_interaction_mask(1, [0]);
        let ext1 = RecordingEvaluator::combine_ef([a, b, c, d]);

        let [e] = eval.next_interaction_mask(1, [0]);
        let [f] = eval.next_interaction_mask(1, [0]);
        let [g] = eval.next_interaction_mask(1, [0]);
        let [h] = eval.next_interaction_mask(1, [0]);
        let ext2 = RecordingEvaluator::combine_ef([e, f, g, h]);

        let result = ext1 + ext2;
        drop(result);
        let state = eval.finish();

        // Should have 8 trace cols (base), 2 SecureCol + 1 Add (ext)
        assert_eq!(state.base_insts.len(), 8);
        assert_eq!(state.ext_insts.len(), 3);
        assert_eq!(
            state.ext_insts[2].op,
            MetalEvaluationProgramExtOpcodeV1::Add as u8
        );
    }

    #[test]
    fn ext_from_securefield_creates_lazy_const() {
        let sf = SecureField::from_u32_unchecked(1, 2, 3, 4);
        let ext = RecordedExtValue::from(sf);
        assert!(matches!(ext.kind, RecordedExtValueKind::LazyConst(_)));
    }

    #[test]
    fn ext_lazy_const_realizes_on_arithmetic() {
        let mut eval = RecordingEvaluator::new();
        let [a] = eval.next_interaction_mask(1, [0]);
        let [b] = eval.next_interaction_mask(1, [0]);
        let [c] = eval.next_interaction_mask(1, [0]);
        let [d] = eval.next_interaction_mask(1, [0]);
        let ext1 = RecordingEvaluator::combine_ef([a, b, c, d]);

        let sf = SecureField::from_u32_unchecked(5, 6, 7, 8);
        let result = ext1 + sf;
        drop(result);
        let state = eval.finish();

        // ext_insts: 1 SecureCol + 1 Const + 1 Add
        assert_eq!(state.ext_insts.len(), 3);
        assert_eq!(
            state.ext_insts[0].op,
            MetalEvaluationProgramExtOpcodeV1::SecureCol as u8,
        );
        assert_eq!(
            state.ext_insts[1].op,
            MetalEvaluationProgramExtOpcodeV1::Const as u8,
        );
        assert_eq!(
            state.ext_insts[2].op,
            MetalEvaluationProgramExtOpcodeV1::Add as u8,
        );
    }

    #[test]
    fn base_value_add_securefield() {
        let mut eval = RecordingEvaluator::new();
        let [a] = eval.next_interaction_mask(1, [0]);
        let sf = SecureField::from_u32_unchecked(1, 2, 3, 4);
        let result = a + sf;
        drop(result);
        let state = eval.finish();
        // base_insts: 1 trace col + 1 const(0) for zero padding
        // ext_insts: 1 SecureCol (promote) + 1 Const (sf) + 1 Add
        assert!(state.ext_insts.len() >= 3);
    }

    #[test]
    fn base_value_mul_securefield() {
        let mut eval = RecordingEvaluator::new();
        let [a] = eval.next_interaction_mask(1, [0]);
        let sf = SecureField::from_u32_unchecked(1, 2, 3, 4);
        let result = a * sf;
        drop(result);
        let state = eval.finish();
        // ext_insts should contain SecureCol (promote) + Const + Mul
        let mul_count = state
            .ext_insts
            .iter()
            .filter(|i| i.op == MetalEvaluationProgramExtOpcodeV1::Mul as u8)
            .count();
        assert_eq!(mul_count, 1);
    }

    #[test]
    fn add_constraint_with_ext_value_uses_direct_reg() {
        let mut eval = RecordingEvaluator::new();
        let [a] = eval.next_interaction_mask(1, [0]);
        let [b] = eval.next_interaction_mask(1, [0]);
        let [c] = eval.next_interaction_mask(1, [0]);
        let [d] = eval.next_interaction_mask(1, [0]);
        let ext = RecordingEvaluator::combine_ef([a, b, c, d]);

        // The ext value is Realized with reg 0 (first ext reg from SecureCol).
        eval.add_constraint(ext);
        let state = eval.finish();

        // Should have 1 SecureCol and the constraint root should reference
        // it directly (register 0).
        assert_eq!(state.constraint_roots.len(), 1);
        assert_eq!(state.constraint_roots[0], 0);
        // Only 1 ext instruction (the SecureCol from combine_ef).
        assert_eq!(state.ext_insts.len(), 1);
    }

    #[test]
    fn add_constraint_with_base_value_emits_secure_col() {
        let mut eval = RecordingEvaluator::new();
        let [a] = eval.next_interaction_mask(1, [0]);
        let [b] = eval.next_interaction_mask(1, [0]);
        let c = a - b;
        eval.add_constraint(c);
        let state = eval.finish();

        // Should emit a SecureCol for the base constraint.
        assert_eq!(state.constraint_roots.len(), 1);
        assert_eq!(state.ext_insts.len(), 1);
        assert_eq!(
            state.ext_insts[0].op,
            MetalEvaluationProgramExtOpcodeV1::SecureCol as u8,
        );
    }

    #[test]
    fn ext_zero_add_returns_rhs() {
        let mut eval = RecordingEvaluator::new();
        let [a] = eval.next_interaction_mask(1, [0]);
        let [b] = eval.next_interaction_mask(1, [0]);
        let [c] = eval.next_interaction_mask(1, [0]);
        let [d] = eval.next_interaction_mask(1, [0]);
        let ext = RecordingEvaluator::combine_ef([a, b, c, d]);

        let zero = RecordedExtValue::zero();
        let result = zero + ext;
        drop(result);
        let state = eval.finish();
        // Zero + ext should return ext without additional instructions.
        // Only the SecureCol from combine_ef.
        assert_eq!(state.ext_insts.len(), 1);
    }

    #[test]
    fn ext_neg_emits_neg_instruction() {
        let mut eval = RecordingEvaluator::new();
        let [a] = eval.next_interaction_mask(1, [0]);
        let [b] = eval.next_interaction_mask(1, [0]);
        let [c] = eval.next_interaction_mask(1, [0]);
        let [d] = eval.next_interaction_mask(1, [0]);
        let ext = RecordingEvaluator::combine_ef([a, b, c, d]);

        let result = -ext;
        drop(result);
        let state = eval.finish();

        // SecureCol + Neg
        assert_eq!(state.ext_insts.len(), 2);
        assert_eq!(
            state.ext_insts[1].op,
            MetalEvaluationProgramExtOpcodeV1::Neg as u8,
        );
    }

    #[test]
    fn ext_mul_base_emits_promote_and_mul() {
        let mut eval = RecordingEvaluator::new();
        let [a] = eval.next_interaction_mask(1, [0]);
        let [b] = eval.next_interaction_mask(1, [0]);
        let [c] = eval.next_interaction_mask(1, [0]);
        let [d] = eval.next_interaction_mask(1, [0]);
        let ext = RecordingEvaluator::combine_ef([a, b, c, d]);

        let [e] = eval.next_interaction_mask(1, [0]);
        let result = ext * e;
        drop(result);
        let state = eval.finish();

        // ext_insts: SecureCol(combine) + SecureCol(promote e) + Mul
        assert_eq!(state.ext_insts.len(), 3);
        assert_eq!(
            state.ext_insts[2].op,
            MetalEvaluationProgramExtOpcodeV1::Mul as u8,
        );
    }

    /// Multi-fraction logup test: 3 logup fractions with non-zero claimed_sum,
    /// using finalize_logup_in_pairs. This exercises the batched logup path
    /// used by real Cairo components.
    #[test]
    fn v1_matches_cpu_domain_evaluator_with_multiple_logup_fractions() {
        use stwo::core::fields::m31::BaseField;
        use stwo::core::fields::qm31::SecureField;
        use stwo::core::poly::circle::CanonicCoset;
        use stwo::core::Fraction;
        use stwo::prover::backend::CpuBackend;
        use stwo::prover::poly::circle::CircleEvaluation;
        use stwo::prover::poly::BitReversedOrder;
        use stwo::core::pcs::TreeVec;
        use stwo_constraint_framework::{
            CpuDomainEvaluator, EvalAtRow, FrameworkEval,
        };

        const LOG_SIZE: u32 = 4;

        /// Component with 3 logup fractions — closer to real Cairo components.
        struct TestMultiFracEval;

        impl FrameworkEval for TestMultiFracEval {
            fn log_size(&self) -> u32 {
                LOG_SIZE
            }
            fn max_constraint_log_degree_bound(&self) -> u32 {
                LOG_SIZE + 1
            }
            fn evaluate<E: EvalAtRow>(&self, mut eval: E) -> E {
                // 3 fractions, each with a base-field numerator and extension-field denominator.
                for _ in 0..3 {
                    let num = eval.next_trace_mask();
                    let d0 = eval.next_trace_mask();
                    let d1 = eval.next_trace_mask();
                    let d2 = eval.next_trace_mask();
                    let d3 = eval.next_trace_mask();
                    let denom = E::combine_ef([d0, d1, d2, d3]);
                    eval.write_logup_frac(Fraction {
                        numerator: num.into(),
                        denominator: denom,
                    });
                }
                eval.finalize_logup_in_pairs();
                eval
            }
        }

        let claimed_sum = SecureField::from_u32_unchecked(100, 200, 300, 400);
        let test_eval = TestMultiFracEval;
        let program = crate::backend::metal::eval_program_v1::lower_framework_eval_to_v1_with_logup(
            &test_eval,
            3,
            0,
            0,
            claimed_sum,
            LOG_SIZE,
        )
        .expect("lowering should succeed");

        let eval_domain = CanonicCoset::new(LOG_SIZE + 1).circle_domain();
        let n_rows = 1 << (LOG_SIZE + 1);

        // 3 fractions × 5 columns each = 15 main trace columns.
        let main_cols: Vec<Vec<BaseField>> = (0..15)
            .map(|col_idx| {
                (0..n_rows)
                    .map(|row| BaseField::from_u32_unchecked((row * 7 + col_idx * 13 + 1) as u32 % 97))
                    .collect()
            })
            .collect();
        let main_refs: Vec<&[BaseField]> = main_cols.iter().map(|c| c.as_slice()).collect();

        // Cumsum: 4 base columns per ext field. With 3 fractions batched in pairs,
        // the logup generates 2 prefix_sum slots (ceil(3/2) = 2), so 8 cumsum columns.
        // Actually, finalize_logup_in_pairs with 3 fracs uses:
        // - 2 batches: batch 0 has fracs[0..2], batch 1 has fracs[2..3]
        // - Each batch reads prev cumsum (4 cols) and writes next cumsum (4 cols)
        // - The logup accesses cumsum at offsets [0] and [-1] => 2 ext values per batch
        // - Total cumsum columns = 2 batches * 4 cols/ext * 2 reads = 16? No...
        // Let me count by looking at what next_extension_interaction_mask produces.
        // For finalize_logup_batched::<2>, with 3 fracs:
        //   batch 0: fracs[0,1] → 1 ext_col read at [0] + 1 ext_col read at [-1] = 8 base cols
        //   batch 1: frac[2] → 1 ext_col read at [0] + 1 ext_col read at [-1] = 8 base cols
        // Total cumsum: 16 base columns
        // Wait, no. Let me look at finalize_logup_batched more carefully.
        // Each call to next_extension_interaction_mask reads 4 base columns.
        // In finalize_logup_batched::<2> with 3 fracs:
        //   for batch in [0..2, 2..3]:
        //     let cur = self.next_extension_interaction_mask(..., [0])  → 4 base cols
        //     let prev = self.next_extension_interaction_mask(..., [-1]) → 4 base cols
        // So 2 batches × 2 reads × 4 cols = 16 columns.
        // No wait, next_extension_interaction_mask with offsets [0] reads 4 cols.
        // With offsets [0, -1] it reads 4 cols × 2 offsets... no, it reads 4 cols
        // and each col is read at both offsets. Actually let me look at the code.

        // The CpuDomainEvaluator's logup_proxy finalize_logup_batched calls
        // next_extension_interaction_mask(interaction, [0, -1]) which reads
        // 4 columns from the interaction trace. Each "prefix_sum_col" is
        // 1 call to next_extension_interaction_mask = 4 base columns.
        // With 3 fracs and batch_size 2:
        //   chunk 0 (fracs 0,1): prefix_sum_col 0 → 4 cols (read at offsets [0,-1])
        //   chunk 1 (frac 2): prefix_sum_col 1 → 4 cols (read at offsets [0,-1])
        // Total: 8 base columns in the interaction trace.
        let cumsum_cols: Vec<Vec<BaseField>> = (0..8)
            .map(|col_idx| {
                (0..n_rows)
                    .map(|row| {
                        BaseField::from_u32_unchecked(
                            (row * 11 + col_idx * 17 + 3) as u32 % 101,
                        )
                    })
                    .collect()
            })
            .collect();
        let cumsum_refs: Vec<&[BaseField]> = cumsum_cols.iter().map(|c| c.as_slice()).collect();

        let interaction0: Vec<&[BaseField]> = vec![];
        let interactions: Vec<&[&[BaseField]]> = vec![
            &interaction0,
            main_refs.as_slice(),
            cumsum_refs.as_slice(),
        ];

        let n_constraints = program.constraint_roots().len();
        let random_coeff_powers: Vec<SecureField> = (0..n_constraints)
            .map(|i| SecureField::from_u32_unchecked(i as u32 + 1, 0, 0, 0))
            .collect();

        let runtime = crate::backend::metal::eval_program_v1::MetalEvaluationProgramRuntimeInputsV1 {
            trace: crate::backend::metal::eval_program_v1::MetalEvaluationProgramTraceViewV1 {
                trace_interactions: &interactions.iter().map(|i| *i).collect::<Vec<_>>(),
                preprocessed_columns: &[],
            },
            base_params: &[],
            ext_params: &[],
            random_coeff_powers: &random_coeff_powers,
        };
        let v1_results = crate::backend::metal::eval_program_v1::interpret_metal_evaluation_program_v1(
            &program, runtime,
        )
        .expect("V1 interpretation should succeed");

        // Reference via CpuDomainEvaluator.
        let mut trace_evals_storage: Vec<Vec<CircleEvaluation<CpuBackend, BaseField, BitReversedOrder>>> =
            Vec::new();
        trace_evals_storage.push(vec![]);
        let mut main_evals = Vec::new();
        for col in &main_cols {
            main_evals.push(CircleEvaluation::new(eval_domain, col.clone()));
        }
        trace_evals_storage.push(main_evals);
        let mut cumsum_evals = Vec::new();
        for col in &cumsum_cols {
            cumsum_evals.push(CircleEvaluation::new(eval_domain, col.clone()));
        }
        trace_evals_storage.push(cumsum_evals);

        let trace_evals_refs: Vec<Vec<&CircleEvaluation<CpuBackend, BaseField, BitReversedOrder>>> =
            trace_evals_storage.iter().map(|v| v.iter().collect()).collect();
        let tree_vec = TreeVec(trace_evals_refs);

        let mut ref_results = Vec::with_capacity(n_rows);
        for row in 0..n_rows {
            let cpu_eval = CpuDomainEvaluator::new(
                &tree_vec,
                row,
                &random_coeff_powers,
                LOG_SIZE,
                LOG_SIZE + 1,
                LOG_SIZE,
                claimed_sum,
            );
            let result = test_eval.evaluate(cpu_eval);
            ref_results.push(result.row_res);
        }

        assert_eq!(v1_results.len(), ref_results.len());
        let mut mismatches = 0;
        for (row, (v1, cpu)) in v1_results.iter().zip(ref_results.iter()).enumerate() {
            if v1 != cpu {
                if mismatches < 5 {
                    eprintln!("  ROW {}: V1={:?}, CPU={:?}", row, v1, cpu);
                }
                mismatches += 1;
            }
        }
        assert_eq!(
            mismatches, 0,
            "{} out of {} rows mismatch between V1 and CpuDomainEvaluator",
            mismatches, n_rows,
        );
    }

    /// End-to-end test: lower a simple logup component with non-zero claimed_sum,
    /// interpret the V1 program, and compare against CpuDomainEvaluator reference.
    #[test]
    fn v1_matches_cpu_domain_evaluator_with_nonzero_claimed_sum() {
        use stwo::core::fields::m31::BaseField;
        use stwo::core::fields::qm31::SecureField;
        use stwo::core::poly::circle::CanonicCoset;
        use stwo::core::Fraction;
        use stwo::prover::backend::CpuBackend;
        use stwo::prover::poly::circle::CircleEvaluation;
        use stwo::prover::poly::BitReversedOrder;
        use stwo::core::pcs::TreeVec;
        use stwo_constraint_framework::{
            CpuDomainEvaluator, EvalAtRow, FrameworkEval,
        };

        const LOG_SIZE: u32 = 4;

        /// A minimal component with 1 logup fraction and a non-zero claimed_sum.
        struct TestLogupEval;

        impl FrameworkEval for TestLogupEval {
            fn log_size(&self) -> u32 {
                LOG_SIZE
            }

            fn max_constraint_log_degree_bound(&self) -> u32 {
                LOG_SIZE + 1
            }

            fn evaluate<E: EvalAtRow>(&self, mut eval: E) -> E {
                // Read a denominator from the main trace (interaction 1).
                let d0 = eval.next_trace_mask();
                let d1 = eval.next_trace_mask();
                let d2 = eval.next_trace_mask();
                let d3 = eval.next_trace_mask();
                let denom = E::combine_ef([d0, d1, d2, d3]);

                // Read a numerator from the main trace (interaction 1).
                let n0 = eval.next_trace_mask();
                let n1 = eval.next_trace_mask();
                let n2 = eval.next_trace_mask();
                let n3 = eval.next_trace_mask();
                let num = E::combine_ef([n0, n1, n2, n3]);

                eval.write_logup_frac(Fraction {
                    numerator: num,
                    denominator: denom,
                });
                eval.finalize_logup_in_pairs();
                eval
            }
        }

        // Choose a non-zero claimed_sum.
        let claimed_sum = SecureField::from_u32_unchecked(100, 200, 300, 400);
        let _cumsum_shift =
            claimed_sum / BaseField::from_u32_unchecked(1 << LOG_SIZE);

        // Step 1: Lower via RecordingEvaluator.
        let test_eval = TestLogupEval;
        let program = crate::backend::metal::eval_program_v1::lower_framework_eval_to_v1_with_logup(
            &test_eval,
            3, // n_interactions: preprocessed(0), main(1), interaction(2)
            0, // n_base_params
            0, // n_ext_params
            claimed_sum,
            LOG_SIZE,
        )
        .expect("lowering should succeed");

        // Step 2: Build trace data.
        let eval_domain = CanonicCoset::new(LOG_SIZE + 1).circle_domain();
        let n_rows = 1 << (LOG_SIZE + 1); // eval domain size

        // Interaction 0 (preprocessed): empty.
        let interaction0: Vec<&[BaseField]> = vec![];

        // Interaction 1 (main trace): 8 columns (4 for denom, 4 for num).
        // Fill with simple patterns.
        let main_cols: Vec<Vec<BaseField>> = (0..8)
            .map(|col_idx| {
                (0..n_rows)
                    .map(|row| BaseField::from_u32_unchecked((row * 7 + col_idx * 13 + 1) as u32 % 97))
                    .collect()
            })
            .collect();
        let main_refs: Vec<&[BaseField]> = main_cols.iter().map(|c| c.as_slice()).collect();

        // Interaction 2 (cumsum): 4 columns (1 ext field = 4 base columns).
        // Build valid cumulative sum values that satisfy the constraint.
        // For a single logup fraction: frac = num/denom.
        // cumsum[row] = cumsum[row-1] + num/denom - cumsum_shift.
        // We need to compute this correctly.
        // Actually, for the test, just use random-ish values. The constraint
        // won't be satisfied, but we can still compare V1 vs reference.
        let cumsum_cols: Vec<Vec<BaseField>> = (0..4)
            .map(|col_idx| {
                (0..n_rows)
                    .map(|row| {
                        BaseField::from_u32_unchecked(
                            (row * 11 + col_idx * 17 + 3) as u32 % 101,
                        )
                    })
                    .collect()
            })
            .collect();
        let cumsum_refs: Vec<&[BaseField]> = cumsum_cols.iter().map(|c| c.as_slice()).collect();

        // Build the interactions array for V1 interpreter.
        let interactions: Vec<&[&[BaseField]]> = vec![
            &interaction0,
            main_refs.as_slice(),
            cumsum_refs.as_slice(),
        ];

        // Random coeff powers.
        let n_constraints = program.constraint_roots().len();
        let random_coeff_powers: Vec<SecureField> = (0..n_constraints)
            .map(|i| SecureField::from_u32_unchecked(i as u32 + 1, 0, 0, 0))
            .collect();

        // Step 3: Interpret V1 program.
        let runtime = crate::backend::metal::eval_program_v1::MetalEvaluationProgramRuntimeInputsV1 {
            trace: crate::backend::metal::eval_program_v1::MetalEvaluationProgramTraceViewV1 {
                trace_interactions: &interactions.iter().map(|i| *i).collect::<Vec<_>>(),
                preprocessed_columns: &[],
            },
            base_params: &[],
            ext_params: &[],
            random_coeff_powers: &random_coeff_powers,
        };
        let v1_results = crate::backend::metal::eval_program_v1::interpret_metal_evaluation_program_v1(
            &program, runtime,
        )
        .expect("V1 interpretation should succeed");

        // Step 4: Reference via CpuDomainEvaluator.
        // Build CircleEvaluation objects for the trace data.
        let mut trace_evals_storage: Vec<Vec<CircleEvaluation<CpuBackend, BaseField, BitReversedOrder>>> =
            Vec::new();

        // Interaction 0: empty.
        trace_evals_storage.push(vec![]);

        // Interaction 1: main trace.
        let mut main_evals = Vec::new();
        for col in &main_cols {
            main_evals.push(CircleEvaluation::new(eval_domain, col.clone()));
        }
        trace_evals_storage.push(main_evals);

        // Interaction 2: cumsum.
        let mut cumsum_evals = Vec::new();
        for col in &cumsum_cols {
            cumsum_evals.push(CircleEvaluation::new(eval_domain, col.clone()));
        }
        trace_evals_storage.push(cumsum_evals);

        let trace_evals_refs: Vec<Vec<&CircleEvaluation<CpuBackend, BaseField, BitReversedOrder>>> =
            trace_evals_storage.iter().map(|v| v.iter().collect()).collect();
        let tree_vec = TreeVec(trace_evals_refs);

        // Evaluate reference per row.
        let mut ref_results = Vec::with_capacity(n_rows);
        for row in 0..n_rows {
            let cpu_eval = CpuDomainEvaluator::new(
                &tree_vec,
                row,
                &random_coeff_powers,
                LOG_SIZE,
                LOG_SIZE + 1,
                LOG_SIZE,
                claimed_sum,
            );
            let result = test_eval.evaluate(cpu_eval);
            ref_results.push(result.row_res);
        }

        // Step 5: Compare V1 vs reference.
        assert_eq!(
            v1_results.len(),
            ref_results.len(),
            "result lengths must match"
        );
        let mut mismatches = 0;
        for (row, (v1, cpu)) in v1_results.iter().zip(ref_results.iter()).enumerate() {
            if v1 != cpu {
                if mismatches < 5 {
                    eprintln!(
                        "  ROW {}: V1={:?}, CPU={:?}",
                        row, v1, cpu
                    );
                }
                mismatches += 1;
            }
        }
        assert_eq!(
            mismatches, 0,
            "{} out of {} rows mismatch between V1 and CpuDomainEvaluator",
            mismatches, n_rows,
        );
    }
}
