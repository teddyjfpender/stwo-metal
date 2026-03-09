use core::ffi::c_void;

use stwo::core::fields::m31::BaseField;
use stwo::core::fields::qm31::SecureField;

pub const STWO_CUDA_ABI_VERSION: u32 = 1;

#[repr(u32)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum StwoCudaStatusV1 {
    Ok = 0,
    InvalidArgument = 1,
    AbiMismatch = 2,
    UnsupportedComponent = 3,
    LaunchFailure = 4,
}

impl StwoCudaStatusV1 {
    pub fn expect_ok(self, context: &str) {
        assert_eq!(self, Self::Ok, "{context} failed with status {:?}", self);
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct StwoCudaM31AbiV1 {
    pub value: u32,
}

impl From<BaseField> for StwoCudaM31AbiV1 {
    fn from(value: BaseField) -> Self {
        Self { value: value.0 }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct StwoCudaCm31AbiV1 {
    pub a: StwoCudaM31AbiV1,
    pub b: StwoCudaM31AbiV1,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct StwoCudaQm31AbiV1 {
    pub a: StwoCudaCm31AbiV1,
    pub b: StwoCudaCm31AbiV1,
}

impl From<SecureField> for StwoCudaQm31AbiV1 {
    fn from(value: SecureField) -> Self {
        Self {
            a: StwoCudaCm31AbiV1 {
                a: value.0 .0.into(),
                b: value.0 .1.into(),
            },
            b: StwoCudaCm31AbiV1 {
                a: value.1 .0.into(),
                b: value.1 .1.into(),
            },
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct StwoCudaLookupElements16AbiV1 {
    pub z: StwoCudaQm31AbiV1,
    pub alpha: StwoCudaQm31AbiV1,
    pub alpha_powers: [StwoCudaQm31AbiV1; 16],
}

impl StwoCudaLookupElements16AbiV1 {
    pub fn new(z: SecureField, alpha: SecureField, alpha_powers: [SecureField; 16]) -> Self {
        Self {
            z: z.into(),
            alpha: alpha.into(),
            alpha_powers: alpha_powers.map(Into::into),
        }
    }
}

pub trait CudaPoseidonLookupElementsAbiV1 {
    fn to_cuda_poseidon_lookup_elements_abi_v1(&self) -> StwoCudaLookupElements16AbiV1;
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct StwoCudaCommonEvalAbiV1 {
    pub eval_id: u32,
    pub log_n_rows: u32,
}

impl StwoCudaCommonEvalAbiV1 {
    pub const fn new(eval_id: u32, log_n_rows: u32) -> Self {
        Self {
            eval_id,
            log_n_rows,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct StwoCudaWideFibonacciEvalAbiV1 {
    pub common: StwoCudaCommonEvalAbiV1,
}

impl StwoCudaWideFibonacciEvalAbiV1 {
    pub const fn new(eval_id: u32, log_n_rows: u32) -> Self {
        Self {
            common: StwoCudaCommonEvalAbiV1::new(eval_id, log_n_rows),
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct StwoCudaPoseidonEvalAbiV1 {
    pub common: StwoCudaCommonEvalAbiV1,
    pub lookup_elements: StwoCudaLookupElements16AbiV1,
    pub claimed_sum: StwoCudaQm31AbiV1,
}

impl StwoCudaPoseidonEvalAbiV1 {
    pub fn new(
        eval_id: u32,
        log_n_rows: u32,
        lookup_elements: StwoCudaLookupElements16AbiV1,
        claimed_sum: SecureField,
    ) -> Self {
        Self {
            common: StwoCudaCommonEvalAbiV1::new(eval_id, log_n_rows),
            lookup_elements,
            claimed_sum: claimed_sum.into(),
        }
    }
}

pub enum OwnedConstraintEvalAbiV1 {
    WideFibonacci(StwoCudaWideFibonacciEvalAbiV1),
    Poseidon(StwoCudaPoseidonEvalAbiV1),
}

impl OwnedConstraintEvalAbiV1 {
    pub fn as_ptr(&self) -> *const c_void {
        match self {
            Self::WideFibonacci(value) => {
                value as *const StwoCudaWideFibonacciEvalAbiV1 as *const c_void
            }
            Self::Poseidon(value) => value as *const StwoCudaPoseidonEvalAbiV1 as *const c_void,
        }
    }
}

pub trait CudaConstraintEvalAbiV1 {
    fn to_cuda_constraint_eval_abi_v1(&self) -> Option<OwnedConstraintEvalAbiV1> {
        None
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct StwoCudaConstraintEvalRequestV1 {
    pub quotient_columns: [*const u32; 4],
    pub trace0_evaluations: *const *const u32,
    pub trace0_evaluations_len: u32,
    pub trace1_evaluations: *const *const u32,
    pub trace1_evaluations_len: u32,
    pub trace2_evaluations: *const *const u32,
    pub trace2_evaluations_len: u32,
    pub random_coeff_powers: *const u32,
    pub denominator_inverses: *const u32,
    pub domain_log_size: u32,
    pub eval_domain_log_size: u32,
    pub number_of_columns: u32,
    pub logup_counts: u32,
    pub eval: *const c_void,
    pub cumsum_shift: StwoCudaQm31AbiV1,
    pub should_accumulate: bool,
    pub use_assert_evaluator: bool,
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct StwoCudaWideFibonacciTraceRequestV1 {
    pub input_a: *const u32,
    pub input_b: *const u32,
    pub input_len: u32,
    pub trace_columns: *const *const u32,
    pub trace_columns_len: u32,
    pub n_columns: u32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct StwoCudaPoseidonTraceRequestV1 {
    pub trace_columns: *const *const u32,
    pub trace_columns_len: u32,
    pub lookup_init: *const *const u32,
    pub lookup_init_len: u32,
    pub lookup_final: *const *const u32,
    pub lookup_final_len: u32,
    pub trace_log_size: u32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct StwoCudaPoseidonInteractionTraceRequestV1 {
    pub lookup_elements: StwoCudaLookupElements16AbiV1,
    pub lookup_init: *const *const u32,
    pub lookup_init_len: u32,
    pub lookup_final: *const *const u32,
    pub lookup_final_len: u32,
    pub log_size: u32,
    pub interaction_traces: *const *const u32,
    pub interaction_traces_len: u32,
    pub claimed_sum: *mut u32,
}

#[cfg(test)]
mod tests {
    use core::mem::{align_of, offset_of, size_of};

    use super::{
        StwoCudaCommonEvalAbiV1, StwoCudaConstraintEvalRequestV1, StwoCudaLookupElements16AbiV1,
        StwoCudaPoseidonEvalAbiV1, StwoCudaPoseidonInteractionTraceRequestV1,
        StwoCudaPoseidonTraceRequestV1, StwoCudaQm31AbiV1, StwoCudaWideFibonacciEvalAbiV1,
        StwoCudaWideFibonacciTraceRequestV1,
    };

    #[test]
    fn abi_v1_layouts_match_exemplar_contract() {
        assert_eq!(size_of::<StwoCudaQm31AbiV1>(), 16);
        assert_eq!(align_of::<StwoCudaQm31AbiV1>(), 4);
        assert_eq!(size_of::<StwoCudaLookupElements16AbiV1>(), 288);
        assert_eq!(align_of::<StwoCudaLookupElements16AbiV1>(), 4);
        assert_eq!(size_of::<StwoCudaCommonEvalAbiV1>(), 8);
        assert_eq!(size_of::<StwoCudaWideFibonacciEvalAbiV1>(), 8);
        assert_eq!(size_of::<StwoCudaPoseidonEvalAbiV1>(), 312);
        assert_eq!(size_of::<StwoCudaConstraintEvalRequestV1>(), 144);
        assert_eq!(align_of::<StwoCudaConstraintEvalRequestV1>(), 8);
        assert_eq!(size_of::<StwoCudaWideFibonacciTraceRequestV1>(), 40);
        assert_eq!(size_of::<StwoCudaPoseidonTraceRequestV1>(), 48);
        assert_eq!(size_of::<StwoCudaPoseidonInteractionTraceRequestV1>(), 344);
    }

    #[test]
    fn abi_v1_offsets_match_exemplar_contract() {
        assert_eq!(offset_of!(StwoCudaPoseidonEvalAbiV1, common), 0);
        assert_eq!(offset_of!(StwoCudaPoseidonEvalAbiV1, lookup_elements), 8);
        assert_eq!(offset_of!(StwoCudaPoseidonEvalAbiV1, claimed_sum), 296);
        assert_eq!(offset_of!(StwoCudaConstraintEvalRequestV1, eval), 112);
        assert_eq!(
            offset_of!(StwoCudaConstraintEvalRequestV1, cumsum_shift),
            120
        );
        assert_eq!(
            offset_of!(StwoCudaConstraintEvalRequestV1, should_accumulate),
            136
        );
        assert_eq!(
            offset_of!(StwoCudaPoseidonInteractionTraceRequestV1, claimed_sum),
            336
        );
    }
}
