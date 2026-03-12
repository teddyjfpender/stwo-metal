use std::sync::OnceLock;
use std::vec::Vec;

use ark_std::Zero;
use stwo::core::fields::m31::BaseField;
use stwo::core::fields::qm31::SecureField;
use stwo_metal_sys::metal::{metal_runtime_support, MetalError, MetalRuntimeSupport, U32Buffer};

pub const STWO_METAL_EVAL_PROGRAM_MAGIC_V1: u32 = u32::from_le_bytes(*b"STP1");
pub const STWO_METAL_EVAL_PROGRAM_ABI_MAJOR_V1: u16 = 1;
pub const STWO_METAL_EVAL_PROGRAM_ABI_MINOR_V1: u16 = 0;

pub const STWO_METAL_EVAL_PROGRAM_FLAG_PREFINALIZED_LOGUP_V1: u32 = 1 << 0;
pub const STWO_METAL_EVAL_PROGRAM_FLAG_DEBUG_PRESENT_V1: u32 = 1 << 1;

pub const STWO_METAL_EVAL_PROGRAM_CAP_BASE_INV_V1: u64 = 1 << 0;
pub const STWO_METAL_EVAL_PROGRAM_CAP_EXT_MUL_V1: u64 = 1 << 1;
pub const STWO_METAL_EVAL_PROGRAM_CAP_PREFINALIZED_LOGUP_V1: u64 = 1 << 2;

pub const STWO_METAL_EVAL_PROGRAM_SECURE_EXT_DEGREE_V1: u32 = 4;

#[repr(u8)]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum MetalEvaluationProgramBaseOpcodeV1 {
    TraceCol = 0,
    PreprocessedCol = 1,
    Param = 2,
    Const = 3,
    Add = 4,
    Sub = 5,
    Mul = 6,
    Neg = 7,
    Inv = 8,
}

impl MetalEvaluationProgramBaseOpcodeV1 {
    pub const fn from_raw(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::TraceCol),
            1 => Some(Self::PreprocessedCol),
            2 => Some(Self::Param),
            3 => Some(Self::Const),
            4 => Some(Self::Add),
            5 => Some(Self::Sub),
            6 => Some(Self::Mul),
            7 => Some(Self::Neg),
            8 => Some(Self::Inv),
            _ => None,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct MetalEvaluationProgramBaseInstV1 {
    pub op: u8,
    pub interaction: u8,
    pub dst: u16,
    pub a: u32,
    pub b: u32,
    pub imm: i32,
}

impl MetalEvaluationProgramBaseInstV1 {
    pub const fn trace_col(dst: u16, interaction: u8, column: u32, offset: i32) -> Self {
        Self {
            op: MetalEvaluationProgramBaseOpcodeV1::TraceCol as u8,
            interaction,
            dst,
            a: column,
            b: 0,
            imm: offset,
        }
    }

    pub const fn const_value(dst: u16, value: u32) -> Self {
        Self {
            op: MetalEvaluationProgramBaseOpcodeV1::Const as u8,
            interaction: 0,
            dst,
            a: value,
            b: 0,
            imm: 0,
        }
    }

    pub const fn binary(op: MetalEvaluationProgramBaseOpcodeV1, dst: u16, a: u32, b: u32) -> Self {
        Self {
            op: op as u8,
            interaction: 0,
            dst,
            a,
            b,
            imm: 0,
        }
    }
}

#[repr(u8)]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum MetalEvaluationProgramExtOpcodeV1 {
    SecureCol = 0,
    Param = 1,
    Const = 2,
    Add = 3,
    Sub = 4,
    Mul = 5,
    Neg = 6,
}

impl MetalEvaluationProgramExtOpcodeV1 {
    pub const fn from_raw(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::SecureCol),
            1 => Some(Self::Param),
            2 => Some(Self::Const),
            3 => Some(Self::Add),
            4 => Some(Self::Sub),
            5 => Some(Self::Mul),
            6 => Some(Self::Neg),
            _ => None,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct MetalEvaluationProgramExtInstV1 {
    pub op: u8,
    pub reserved0: u8,
    pub dst: u16,
    pub a: u32,
    pub b: u32,
    pub c: u32,
    pub d: u32,
}

impl MetalEvaluationProgramExtInstV1 {
    pub const fn secure_col(dst: u16, a: u32, b: u32, c: u32, d: u32) -> Self {
        Self {
            op: MetalEvaluationProgramExtOpcodeV1::SecureCol as u8,
            reserved0: 0,
            dst,
            a,
            b,
            c,
            d,
        }
    }
}

#[repr(u32)]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum MetalEvaluationProgramSectionKindV1 {
    BaseConsts = 1,
    ExtConsts = 2,
    BaseInsts = 3,
    ExtInsts = 4,
    ConstraintRoots = 5,
    DebugStrings = 6,
    ParamDebugMap = 7,
    NodeDebugMap = 8,
}

impl MetalEvaluationProgramSectionKindV1 {
    pub const REQUIRED: [Self; 5] = [
        Self::BaseConsts,
        Self::ExtConsts,
        Self::BaseInsts,
        Self::ExtInsts,
        Self::ConstraintRoots,
    ];

    pub const fn from_raw(value: u32) -> Option<Self> {
        match value {
            1 => Some(Self::BaseConsts),
            2 => Some(Self::ExtConsts),
            3 => Some(Self::BaseInsts),
            4 => Some(Self::ExtInsts),
            5 => Some(Self::ConstraintRoots),
            6 => Some(Self::DebugStrings),
            7 => Some(Self::ParamDebugMap),
            8 => Some(Self::NodeDebugMap),
            _ => None,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct MetalEvaluationProgramHeaderV1 {
    pub magic: u32,
    pub abi_major: u16,
    pub abi_minor: u16,
    pub n_sections: u32,
    pub flags: u32,
    pub semantic_hash: u64,
    pub capability_bits: u64,
    pub n_interactions: u32,
    pub n_base_params: u32,
    pub n_ext_params: u32,
    pub n_constraints: u32,
    pub max_base_regs: u32,
    pub max_ext_regs: u32,
    pub secure_ext_degree: u32,
    pub reserved: [u32; 8],
}

impl MetalEvaluationProgramHeaderV1 {
    pub const fn new(
        n_sections: u32,
        semantic_hash: u64,
        capability_bits: u64,
        n_interactions: u32,
        n_base_params: u32,
        n_ext_params: u32,
        n_constraints: u32,
        max_base_regs: u32,
        max_ext_regs: u32,
    ) -> Self {
        Self {
            magic: STWO_METAL_EVAL_PROGRAM_MAGIC_V1,
            abi_major: STWO_METAL_EVAL_PROGRAM_ABI_MAJOR_V1,
            abi_minor: STWO_METAL_EVAL_PROGRAM_ABI_MINOR_V1,
            n_sections,
            flags: STWO_METAL_EVAL_PROGRAM_FLAG_PREFINALIZED_LOGUP_V1,
            semantic_hash,
            capability_bits,
            n_interactions,
            n_base_params,
            n_ext_params,
            n_constraints,
            max_base_regs,
            max_ext_regs,
            secure_ext_degree: STWO_METAL_EVAL_PROGRAM_SECURE_EXT_DEGREE_V1,
            reserved: [0; 8],
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct MetalEvaluationProgramSectionDescV1 {
    pub kind: u32,
    pub elem_size: u32,
    pub offset_bytes: u64,
    pub count: u64,
}

impl MetalEvaluationProgramSectionDescV1 {
    pub const fn new(
        kind: MetalEvaluationProgramSectionKindV1,
        elem_size: u32,
        offset_bytes: u64,
        count: u64,
    ) -> Self {
        Self {
            kind: kind as u32,
            elem_size,
            offset_bytes,
            count,
        }
    }

    pub const fn section_kind(self) -> Option<MetalEvaluationProgramSectionKindV1> {
        MetalEvaluationProgramSectionKindV1::from_raw(self.kind)
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct MetalEvaluationProgramBudgetV1 {
    pub max_base_regs: u32,
    pub max_ext_regs: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OwnedMetalEvaluationProgramV1 {
    header: MetalEvaluationProgramHeaderV1,
    sections: Vec<MetalEvaluationProgramSectionDescV1>,
    base_consts: Vec<u32>,
    ext_consts: Vec<[u32; 4]>,
    base_insts: Vec<MetalEvaluationProgramBaseInstV1>,
    ext_insts: Vec<MetalEvaluationProgramExtInstV1>,
    constraint_roots: Vec<u32>,
}

impl OwnedMetalEvaluationProgramV1 {
    pub fn header(&self) -> MetalEvaluationProgramHeaderV1 {
        self.header
    }

    pub fn sections(&self) -> &[MetalEvaluationProgramSectionDescV1] {
        &self.sections
    }

    pub fn base_consts(&self) -> &[u32] {
        &self.base_consts
    }

    pub fn ext_consts(&self) -> &[[u32; 4]] {
        &self.ext_consts
    }

    pub fn base_insts(&self) -> &[MetalEvaluationProgramBaseInstV1] {
        &self.base_insts
    }

    pub fn ext_insts(&self) -> &[MetalEvaluationProgramExtInstV1] {
        &self.ext_insts
    }

    pub fn constraint_roots(&self) -> &[u32] {
        &self.constraint_roots
    }

    pub fn payload_len_bytes(&self) -> u64 {
        self.sections
            .iter()
            .map(|section| section.offset_bytes + section.count * section.elem_size as u64)
            .max()
            .unwrap_or(0)
    }

    pub fn validate(
        &self,
        budget: MetalEvaluationProgramBudgetV1,
    ) -> Result<(), MetalEvaluationProgramValidationError> {
        validate_metal_evaluation_program_v1(
            self.header,
            &self.sections,
            self.payload_len_bytes(),
            budget,
        )
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct MetalEvaluationProgramSpecializationV1 {
    pub log_n_rows: u32,
    pub n_columns: u32,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum MetalEvaluationProgramLoweringError {
    UnsupportedComponent { component_name: &'static str },
    InvalidWideFibonacciColumnCount { n_columns: u32 },
    InvalidVirtualSnosColumnCount { n_columns: u32 },
    RegisterBudgetOverflow,
    AbiLayoutMismatch {
        record: &'static str,
        expected_size: usize,
        actual_size: usize,
    },
    AbiAlignmentMismatch {
        record: &'static str,
        expected_align: usize,
        actual_align: usize,
    },
    AbiFieldOffsetMismatch {
        record: &'static str,
    },
}

#[derive(Copy, Clone, Debug)]
pub struct MetalEvaluationProgramTraceViewV1<'a> {
    pub trace_interactions: &'a [&'a [&'a [BaseField]]],
    pub preprocessed_columns: &'a [&'a [BaseField]],
}

#[derive(Copy, Clone, Debug)]
pub struct MetalEvaluationProgramRuntimeInputsV1<'a> {
    pub trace: MetalEvaluationProgramTraceViewV1<'a>,
    pub base_params: &'a [BaseField],
    pub ext_params: &'a [SecureField],
    pub random_coeff_powers: &'a [SecureField],
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum MetalEvaluationProgramInterpreterError {
    EmptyTrace,
    InconsistentTraceColumnLength {
        expected_len: usize,
        actual_len: usize,
    },
    InconsistentPreprocessedColumnLength {
        expected_len: usize,
        actual_len: usize,
    },
    TraceInteractionOutOfRange {
        interaction: usize,
        available: usize,
    },
    TraceColumnOutOfRange {
        interaction: usize,
        column: usize,
        available: usize,
    },
    PreprocessedColumnOutOfRange {
        column: usize,
        available: usize,
    },
    BaseParamOutOfRange {
        slot: usize,
        available: usize,
    },
    ExtParamOutOfRange {
        slot: usize,
        available: usize,
    },
    BaseRegisterOutOfRange {
        register: usize,
        available: usize,
    },
    ExtRegisterOutOfRange {
        register: usize,
        available: usize,
    },
    ConstraintRootOutOfRange {
        root: usize,
        available: usize,
    },
    RandomCoeffCountMismatch {
        expected: usize,
        actual: usize,
    },
    NonZeroTraceOffsetUnsupported {
        offset: i32,
    },
    UnsupportedBaseOpcode {
        opcode: u8,
    },
    UnsupportedExtOpcode {
        opcode: u8,
    },
}

#[derive(Debug, Eq, PartialEq)]
pub enum MetalEvaluationProgramExecutionError {
    RuntimeUnavailable(MetalRuntimeSupport),
    MetalRuntime {
        message: String,
    },
    EmptyTrace,
    InconsistentTraceColumnLength {
        expected_len: usize,
        actual_len: usize,
    },
    InconsistentPreprocessedColumnLength {
        expected_len: usize,
        actual_len: usize,
    },
    TraceInteractionCountMismatch {
        expected: usize,
        actual: usize,
    },
    RandomCoeffCountMismatch {
        expected: usize,
        actual: usize,
    },
    BaseParamCountMismatch {
        expected: usize,
        actual: usize,
    },
    ExtParamCountMismatch {
        expected: usize,
        actual: usize,
    },
    UnsupportedNonZeroTraceOffset {
        offset: i32,
    },
    RegisterBudgetExceeded {
        required_base: u32,
        required_ext: u32,
        supported_base: u32,
        supported_ext: u32,
    },
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct MetalEvaluationProgramCapabilityProfileV1 {
    pub runtime_support: MetalRuntimeSupport,
    pub capability_bits: u64,
    pub max_base_regs: u32,
    pub max_ext_regs: u32,
}

impl MetalEvaluationProgramCapabilityProfileV1 {
    pub const fn new(
        runtime_support: MetalRuntimeSupport,
        capability_bits: u64,
        max_base_regs: u32,
        max_ext_regs: u32,
    ) -> Self {
        Self {
            runtime_support,
            capability_bits,
            max_base_regs,
            max_ext_regs,
        }
    }

    pub fn current() -> Self {
        Self {
            runtime_support: metal_runtime_support(),
            capability_bits: STWO_METAL_EVAL_PROGRAM_CAP_BASE_INV_V1
                | STWO_METAL_EVAL_PROGRAM_CAP_EXT_MUL_V1
                | STWO_METAL_EVAL_PROGRAM_CAP_PREFINALIZED_LOGUP_V1,
            max_base_regs: METAL_EVAL_PROGRAM_V1_DEVICE_BUDGET.max_base_regs,
            max_ext_regs: METAL_EVAL_PROGRAM_V1_DEVICE_BUDGET.max_ext_regs,
        }
    }

    pub const fn budget(self) -> MetalEvaluationProgramBudgetV1 {
        MetalEvaluationProgramBudgetV1::new(self.max_base_regs, self.max_ext_regs)
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct MetalEvaluationProgramOverlayV1 {
    pub name: &'static str,
    pub semantic_hash: u64,
    pub required_capability_bits: u64,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum MetalEvaluationProgramDispatchKindV1 {
    GenericMetalInterpreter,
    GeneratedOverlay(MetalEvaluationProgramOverlayV1),
}

const WIDE_FIBONACCI_EVAL_OVERLAY_NAME_V1: &str = "wide_fibonacci_eval_v1";

pub fn metal_evaluation_program_overlays_v1() -> &'static [MetalEvaluationProgramOverlayV1] {
    static OVERLAYS: OnceLock<Vec<MetalEvaluationProgramOverlayV1>> = OnceLock::new();
    OVERLAYS
        .get_or_init(|| {
            let semantic_hash =
                lower_wide_fibonacci_evaluation_program_v1(MetalEvaluationProgramSpecializationV1 {
                    log_n_rows: 20,
                    n_columns: 100,
                })
                .expect("wide-fibonacci V1 overlay specialization should lower")
                .header()
                .semantic_hash;
            vec![MetalEvaluationProgramOverlayV1 {
                name: WIDE_FIBONACCI_EVAL_OVERLAY_NAME_V1,
                semantic_hash,
                required_capability_bits: STWO_METAL_EVAL_PROGRAM_CAP_BASE_INV_V1
                    | STWO_METAL_EVAL_PROGRAM_CAP_EXT_MUL_V1
                    | STWO_METAL_EVAL_PROGRAM_CAP_PREFINALIZED_LOGUP_V1,
            }]
        })
        .as_slice()
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Eq, PartialEq)]
enum MetalEvaluationProgramDeviceInterpreterError {
    RuntimeUnavailable(MetalRuntimeSupport),
    MetalRuntime(MetalError),
    EmptyTrace,
    InconsistentTraceColumnLength {
        expected_len: usize,
        actual_len: usize,
    },
    InconsistentPreprocessedColumnLength {
        expected_len: usize,
        actual_len: usize,
    },
    TraceInteractionCountMismatch {
        expected: usize,
        actual: usize,
    },
    RandomCoeffCountMismatch {
        expected: usize,
        actual: usize,
    },
    BaseParamCountMismatch {
        expected: usize,
        actual: usize,
    },
    ExtParamCountMismatch {
        expected: usize,
        actual: usize,
    },
    UnsupportedNonZeroTraceOffset {
        offset: i32,
    },
    RegisterBudgetExceeded {
        required_base: u32,
        required_ext: u32,
        supported_base: u32,
        supported_ext: u32,
    },
}

impl From<MetalError> for MetalEvaluationProgramDeviceInterpreterError {
    fn from(value: MetalError) -> Self {
        Self::MetalRuntime(value)
    }
}

impl From<MetalEvaluationProgramDeviceInterpreterError> for MetalEvaluationProgramExecutionError {
    fn from(value: MetalEvaluationProgramDeviceInterpreterError) -> Self {
        match value {
            MetalEvaluationProgramDeviceInterpreterError::RuntimeUnavailable(status) => {
                Self::RuntimeUnavailable(status)
            }
            MetalEvaluationProgramDeviceInterpreterError::MetalRuntime(error) => {
                Self::MetalRuntime {
                    message: error.message().to_string(),
                }
            }
            MetalEvaluationProgramDeviceInterpreterError::EmptyTrace => Self::EmptyTrace,
            MetalEvaluationProgramDeviceInterpreterError::InconsistentTraceColumnLength {
                expected_len,
                actual_len,
            } => Self::InconsistentTraceColumnLength {
                expected_len,
                actual_len,
            },
            MetalEvaluationProgramDeviceInterpreterError::InconsistentPreprocessedColumnLength {
                expected_len,
                actual_len,
            } => Self::InconsistentPreprocessedColumnLength {
                expected_len,
                actual_len,
            },
            MetalEvaluationProgramDeviceInterpreterError::TraceInteractionCountMismatch {
                expected,
                actual,
            } => Self::TraceInteractionCountMismatch { expected, actual },
            MetalEvaluationProgramDeviceInterpreterError::RandomCoeffCountMismatch {
                expected,
                actual,
            } => Self::RandomCoeffCountMismatch { expected, actual },
            MetalEvaluationProgramDeviceInterpreterError::BaseParamCountMismatch {
                expected,
                actual,
            } => Self::BaseParamCountMismatch { expected, actual },
            MetalEvaluationProgramDeviceInterpreterError::ExtParamCountMismatch {
                expected,
                actual,
            } => Self::ExtParamCountMismatch { expected, actual },
            MetalEvaluationProgramDeviceInterpreterError::UnsupportedNonZeroTraceOffset {
                offset,
            } => Self::UnsupportedNonZeroTraceOffset { offset },
            MetalEvaluationProgramDeviceInterpreterError::RegisterBudgetExceeded {
                required_base,
                required_ext,
                supported_base,
                supported_ext,
            } => Self::RegisterBudgetExceeded {
                required_base,
                required_ext,
                supported_base,
                supported_ext,
            },
        }
    }
}

impl MetalEvaluationProgramBudgetV1 {
    pub const fn new(max_base_regs: u32, max_ext_regs: u32) -> Self {
        Self {
            max_base_regs,
            max_ext_regs,
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct MetalEvaluationProgramV1<'a> {
    pub header: MetalEvaluationProgramHeaderV1,
    pub sections: &'a [MetalEvaluationProgramSectionDescV1],
    pub payload_len_bytes: u64,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum MetalEvaluationProgramValidationError {
    InvalidMagic {
        found: u32,
    },
    UnsupportedAbiMajor {
        found: u16,
    },
    UnsupportedAbiMinor {
        found: u16,
    },
    MissingPrefinalizedLogupFlag,
    MissingPrefinalizedLogupCapability,
    UnsupportedSecureExtDegree {
        found: u32,
    },
    SectionCountMismatch {
        header_count: u32,
        actual_count: usize,
    },
    UnknownSectionKind {
        raw_kind: u32,
    },
    DuplicateSection {
        kind: MetalEvaluationProgramSectionKindV1,
    },
    MissingRequiredSection {
        kind: MetalEvaluationProgramSectionKindV1,
    },
    ZeroSizedSection {
        kind: MetalEvaluationProgramSectionKindV1,
    },
    SectionOutOfBounds {
        kind: MetalEvaluationProgramSectionKindV1,
        end_offset: u64,
        payload_len_bytes: u64,
    },
    BaseRegisterBudgetExceeded {
        required: u32,
        supported: u32,
    },
    ExtensionRegisterBudgetExceeded {
        required: u32,
        supported: u32,
    },
}

impl<'a> MetalEvaluationProgramV1<'a> {
    pub fn validate(
        self,
        budget: MetalEvaluationProgramBudgetV1,
    ) -> Result<(), MetalEvaluationProgramValidationError> {
        if self.header.magic != STWO_METAL_EVAL_PROGRAM_MAGIC_V1 {
            return Err(MetalEvaluationProgramValidationError::InvalidMagic {
                found: self.header.magic,
            });
        }
        if self.header.abi_major != STWO_METAL_EVAL_PROGRAM_ABI_MAJOR_V1 {
            return Err(MetalEvaluationProgramValidationError::UnsupportedAbiMajor {
                found: self.header.abi_major,
            });
        }
        if self.header.abi_minor > STWO_METAL_EVAL_PROGRAM_ABI_MINOR_V1 {
            return Err(MetalEvaluationProgramValidationError::UnsupportedAbiMinor {
                found: self.header.abi_minor,
            });
        }
        if self.header.flags & STWO_METAL_EVAL_PROGRAM_FLAG_PREFINALIZED_LOGUP_V1 == 0 {
            return Err(MetalEvaluationProgramValidationError::MissingPrefinalizedLogupFlag);
        }
        if self.header.capability_bits & STWO_METAL_EVAL_PROGRAM_CAP_PREFINALIZED_LOGUP_V1 == 0 {
            return Err(MetalEvaluationProgramValidationError::MissingPrefinalizedLogupCapability);
        }
        if self.header.secure_ext_degree != STWO_METAL_EVAL_PROGRAM_SECURE_EXT_DEGREE_V1 {
            return Err(
                MetalEvaluationProgramValidationError::UnsupportedSecureExtDegree {
                    found: self.header.secure_ext_degree,
                },
            );
        }
        if self.header.n_sections as usize != self.sections.len() {
            return Err(
                MetalEvaluationProgramValidationError::SectionCountMismatch {
                    header_count: self.header.n_sections,
                    actual_count: self.sections.len(),
                },
            );
        }
        if self.header.max_base_regs > budget.max_base_regs {
            return Err(
                MetalEvaluationProgramValidationError::BaseRegisterBudgetExceeded {
                    required: self.header.max_base_regs,
                    supported: budget.max_base_regs,
                },
            );
        }
        if self.header.max_ext_regs > budget.max_ext_regs {
            return Err(
                MetalEvaluationProgramValidationError::ExtensionRegisterBudgetExceeded {
                    required: self.header.max_ext_regs,
                    supported: budget.max_ext_regs,
                },
            );
        }

        let mut seen_kinds = [false; 8];
        for section in self.sections {
            let kind = section.section_kind().ok_or(
                MetalEvaluationProgramValidationError::UnknownSectionKind {
                    raw_kind: section.kind,
                },
            )?;
            let kind_idx = (kind as usize) - 1;
            if seen_kinds[kind_idx] {
                return Err(MetalEvaluationProgramValidationError::DuplicateSection { kind });
            }
            seen_kinds[kind_idx] = true;

            if section.elem_size == 0 {
                return Err(MetalEvaluationProgramValidationError::ZeroSizedSection { kind });
            }

            let byte_len = section
                .count
                .checked_mul(section.elem_size as u64)
                .and_then(|len| section.offset_bytes.checked_add(len))
                .ok_or(MetalEvaluationProgramValidationError::SectionOutOfBounds {
                    kind,
                    end_offset: u64::MAX,
                    payload_len_bytes: self.payload_len_bytes,
                })?;
            if byte_len > self.payload_len_bytes {
                return Err(MetalEvaluationProgramValidationError::SectionOutOfBounds {
                    kind,
                    end_offset: byte_len,
                    payload_len_bytes: self.payload_len_bytes,
                });
            }
        }

        for required in MetalEvaluationProgramSectionKindV1::REQUIRED {
            if !seen_kinds[(required as usize) - 1] {
                return Err(
                    MetalEvaluationProgramValidationError::MissingRequiredSection {
                        kind: required,
                    },
                );
            }
        }

        Ok(())
    }
}

pub fn validate_metal_evaluation_program_v1(
    header: MetalEvaluationProgramHeaderV1,
    sections: &[MetalEvaluationProgramSectionDescV1],
    payload_len_bytes: u64,
    budget: MetalEvaluationProgramBudgetV1,
) -> Result<(), MetalEvaluationProgramValidationError> {
    MetalEvaluationProgramV1 {
        header,
        sections,
        payload_len_bytes,
    }
    .validate(budget)
}

pub fn metal_evaluation_program_semantic_hash_v1(chunks: &[&[u8]]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for chunk in chunks {
        for byte in *chunk {
            hash ^= *byte as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
    }
    hash
}

fn hash_u32s(values: &[u32]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect::<Vec<_>>()
}

fn hash_u32x4s(values: &[[u32; 4]]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.into_iter().flat_map(|limb| limb.to_le_bytes()))
        .collect::<Vec<_>>()
}

fn hash_base_insts(values: &[MetalEvaluationProgramBaseInstV1]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|inst| {
            let mut bytes = Vec::with_capacity(16);
            bytes.push(inst.op);
            bytes.push(inst.interaction);
            bytes.extend_from_slice(&inst.dst.to_le_bytes());
            bytes.extend_from_slice(&inst.a.to_le_bytes());
            bytes.extend_from_slice(&inst.b.to_le_bytes());
            bytes.extend_from_slice(&inst.imm.to_le_bytes());
            bytes
        })
        .collect()
}

fn hash_ext_insts(values: &[MetalEvaluationProgramExtInstV1]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|inst| {
            let mut bytes = Vec::with_capacity(20);
            bytes.push(inst.op);
            bytes.push(inst.reserved0);
            bytes.extend_from_slice(&inst.dst.to_le_bytes());
            bytes.extend_from_slice(&inst.a.to_le_bytes());
            bytes.extend_from_slice(&inst.b.to_le_bytes());
            bytes.extend_from_slice(&inst.c.to_le_bytes());
            bytes.extend_from_slice(&inst.d.to_le_bytes());
            bytes
        })
        .collect()
}

fn build_owned_program_v1(
    capability_bits: u64,
    n_interactions: u32,
    n_base_params: u32,
    n_ext_params: u32,
    max_base_regs: u32,
    max_ext_regs: u32,
    base_consts: Vec<u32>,
    ext_consts: Vec<[u32; 4]>,
    base_insts: Vec<MetalEvaluationProgramBaseInstV1>,
    ext_insts: Vec<MetalEvaluationProgramExtInstV1>,
    constraint_roots: Vec<u32>,
) -> OwnedMetalEvaluationProgramV1 {
    let base_consts_bytes = hash_u32s(&base_consts);
    let ext_consts_bytes = hash_u32x4s(&ext_consts);
    let base_insts_bytes = hash_base_insts(&base_insts);
    let ext_insts_bytes = hash_ext_insts(&ext_insts);
    let constraint_roots_bytes = hash_u32s(&constraint_roots);

    let semantic_hash = metal_evaluation_program_semantic_hash_v1(&[
        &base_consts_bytes,
        &ext_consts_bytes,
        &base_insts_bytes,
        &ext_insts_bytes,
        &constraint_roots_bytes,
    ]);

    let mut offset_bytes = 0u64;
    let mut next_section =
        |kind: MetalEvaluationProgramSectionKindV1, elem_size: u32, count: u64| {
            let section =
                MetalEvaluationProgramSectionDescV1::new(kind, elem_size, offset_bytes, count);
            offset_bytes += elem_size as u64 * count;
            section
        };

    let sections = vec![
        next_section(
            MetalEvaluationProgramSectionKindV1::BaseConsts,
            4,
            base_consts.len() as u64,
        ),
        next_section(
            MetalEvaluationProgramSectionKindV1::ExtConsts,
            16,
            ext_consts.len() as u64,
        ),
        next_section(
            MetalEvaluationProgramSectionKindV1::BaseInsts,
            size_of::<MetalEvaluationProgramBaseInstV1>() as u32,
            base_insts.len() as u64,
        ),
        next_section(
            MetalEvaluationProgramSectionKindV1::ExtInsts,
            size_of::<MetalEvaluationProgramExtInstV1>() as u32,
            ext_insts.len() as u64,
        ),
        next_section(
            MetalEvaluationProgramSectionKindV1::ConstraintRoots,
            4,
            constraint_roots.len() as u64,
        ),
    ];

    let header = MetalEvaluationProgramHeaderV1::new(
        sections.len() as u32,
        semantic_hash,
        capability_bits,
        n_interactions,
        n_base_params,
        n_ext_params,
        constraint_roots.len() as u32,
        max_base_regs,
        max_ext_regs,
    );

    OwnedMetalEvaluationProgramV1 {
        header,
        sections,
        base_consts,
        ext_consts,
        base_insts,
        ext_insts,
        constraint_roots,
    }
}

/// Validate ABI layout of all `#[repr(C)]` host/device boundary records used
/// by V1 evaluation programs. Returns `Ok(())` if all sizes, alignments, and
/// field offsets match the contract expected by Metal shader code.
///
/// This check is called at lowering time so that any compiler or platform
/// layout change is caught before a program is handed to the device.
pub fn validate_eval_program_abi_layout_v1() -> Result<(), MetalEvaluationProgramLoweringError> {
    use core::mem::{align_of, offset_of, size_of};

    // MetalEvaluationProgramHeaderV1: 96 bytes, 8-byte align
    if size_of::<MetalEvaluationProgramHeaderV1>() != 96 {
        return Err(MetalEvaluationProgramLoweringError::AbiLayoutMismatch {
            record: "MetalEvaluationProgramHeaderV1",
            expected_size: 96,
            actual_size: size_of::<MetalEvaluationProgramHeaderV1>(),
        });
    }
    if align_of::<MetalEvaluationProgramHeaderV1>() != 8 {
        return Err(MetalEvaluationProgramLoweringError::AbiAlignmentMismatch {
            record: "MetalEvaluationProgramHeaderV1",
            expected_align: 8,
            actual_align: align_of::<MetalEvaluationProgramHeaderV1>(),
        });
    }
    // Header field offsets
    if offset_of!(MetalEvaluationProgramHeaderV1, magic) != 0
        || offset_of!(MetalEvaluationProgramHeaderV1, abi_major) != 4
        || offset_of!(MetalEvaluationProgramHeaderV1, abi_minor) != 6
        || offset_of!(MetalEvaluationProgramHeaderV1, n_sections) != 8
        || offset_of!(MetalEvaluationProgramHeaderV1, flags) != 12
        || offset_of!(MetalEvaluationProgramHeaderV1, semantic_hash) != 16
        || offset_of!(MetalEvaluationProgramHeaderV1, capability_bits) != 24
        || offset_of!(MetalEvaluationProgramHeaderV1, n_interactions) != 32
        || offset_of!(MetalEvaluationProgramHeaderV1, n_base_params) != 36
        || offset_of!(MetalEvaluationProgramHeaderV1, n_ext_params) != 40
        || offset_of!(MetalEvaluationProgramHeaderV1, n_constraints) != 44
        || offset_of!(MetalEvaluationProgramHeaderV1, max_base_regs) != 48
        || offset_of!(MetalEvaluationProgramHeaderV1, max_ext_regs) != 52
        || offset_of!(MetalEvaluationProgramHeaderV1, secure_ext_degree) != 56
        || offset_of!(MetalEvaluationProgramHeaderV1, reserved) != 60
    {
        return Err(MetalEvaluationProgramLoweringError::AbiFieldOffsetMismatch {
            record: "MetalEvaluationProgramHeaderV1",
        });
    }

    // MetalEvaluationProgramSectionDescV1: 24 bytes, 8-byte align
    if size_of::<MetalEvaluationProgramSectionDescV1>() != 24 {
        return Err(MetalEvaluationProgramLoweringError::AbiLayoutMismatch {
            record: "MetalEvaluationProgramSectionDescV1",
            expected_size: 24,
            actual_size: size_of::<MetalEvaluationProgramSectionDescV1>(),
        });
    }
    if align_of::<MetalEvaluationProgramSectionDescV1>() != 8 {
        return Err(MetalEvaluationProgramLoweringError::AbiAlignmentMismatch {
            record: "MetalEvaluationProgramSectionDescV1",
            expected_align: 8,
            actual_align: align_of::<MetalEvaluationProgramSectionDescV1>(),
        });
    }
    if offset_of!(MetalEvaluationProgramSectionDescV1, kind) != 0
        || offset_of!(MetalEvaluationProgramSectionDescV1, elem_size) != 4
        || offset_of!(MetalEvaluationProgramSectionDescV1, offset_bytes) != 8
        || offset_of!(MetalEvaluationProgramSectionDescV1, count) != 16
    {
        return Err(MetalEvaluationProgramLoweringError::AbiFieldOffsetMismatch {
            record: "MetalEvaluationProgramSectionDescV1",
        });
    }

    // MetalEvaluationProgramBaseInstV1: 16 bytes, 4-byte align
    if size_of::<MetalEvaluationProgramBaseInstV1>() != 16 {
        return Err(MetalEvaluationProgramLoweringError::AbiLayoutMismatch {
            record: "MetalEvaluationProgramBaseInstV1",
            expected_size: 16,
            actual_size: size_of::<MetalEvaluationProgramBaseInstV1>(),
        });
    }
    if align_of::<MetalEvaluationProgramBaseInstV1>() != 4 {
        return Err(MetalEvaluationProgramLoweringError::AbiAlignmentMismatch {
            record: "MetalEvaluationProgramBaseInstV1",
            expected_align: 4,
            actual_align: align_of::<MetalEvaluationProgramBaseInstV1>(),
        });
    }
    if offset_of!(MetalEvaluationProgramBaseInstV1, op) != 0
        || offset_of!(MetalEvaluationProgramBaseInstV1, interaction) != 1
        || offset_of!(MetalEvaluationProgramBaseInstV1, dst) != 2
        || offset_of!(MetalEvaluationProgramBaseInstV1, a) != 4
        || offset_of!(MetalEvaluationProgramBaseInstV1, b) != 8
        || offset_of!(MetalEvaluationProgramBaseInstV1, imm) != 12
    {
        return Err(MetalEvaluationProgramLoweringError::AbiFieldOffsetMismatch {
            record: "MetalEvaluationProgramBaseInstV1",
        });
    }

    // MetalEvaluationProgramExtInstV1: 20 bytes, 4-byte align
    if size_of::<MetalEvaluationProgramExtInstV1>() != 20 {
        return Err(MetalEvaluationProgramLoweringError::AbiLayoutMismatch {
            record: "MetalEvaluationProgramExtInstV1",
            expected_size: 20,
            actual_size: size_of::<MetalEvaluationProgramExtInstV1>(),
        });
    }
    if align_of::<MetalEvaluationProgramExtInstV1>() != 4 {
        return Err(MetalEvaluationProgramLoweringError::AbiAlignmentMismatch {
            record: "MetalEvaluationProgramExtInstV1",
            expected_align: 4,
            actual_align: align_of::<MetalEvaluationProgramExtInstV1>(),
        });
    }
    if offset_of!(MetalEvaluationProgramExtInstV1, op) != 0
        || offset_of!(MetalEvaluationProgramExtInstV1, reserved0) != 1
        || offset_of!(MetalEvaluationProgramExtInstV1, dst) != 2
        || offset_of!(MetalEvaluationProgramExtInstV1, a) != 4
        || offset_of!(MetalEvaluationProgramExtInstV1, b) != 8
        || offset_of!(MetalEvaluationProgramExtInstV1, c) != 12
        || offset_of!(MetalEvaluationProgramExtInstV1, d) != 16
    {
        return Err(MetalEvaluationProgramLoweringError::AbiFieldOffsetMismatch {
            record: "MetalEvaluationProgramExtInstV1",
        });
    }

    Ok(())
}

pub fn lower_registered_metal_evaluation_program_v1(
    component_name: &'static str,
    specialization: MetalEvaluationProgramSpecializationV1,
) -> Result<OwnedMetalEvaluationProgramV1, MetalEvaluationProgramLoweringError> {
    validate_eval_program_abi_layout_v1()?;
    match component_name {
        "fibonacci_example" => lower_wide_fibonacci_evaluation_program_v1(specialization),
        "virtual_snos" => lower_virtual_snos_evaluation_program_v1(specialization),
        _ => Err(MetalEvaluationProgramLoweringError::UnsupportedComponent { component_name }),
    }
}

pub fn lower_wide_fibonacci_evaluation_program_v1(
    specialization: MetalEvaluationProgramSpecializationV1,
) -> Result<OwnedMetalEvaluationProgramV1, MetalEvaluationProgramLoweringError> {
    if specialization.n_columns < 3 {
        return Err(
            MetalEvaluationProgramLoweringError::InvalidWideFibonacciColumnCount {
                n_columns: specialization.n_columns,
            },
        );
    }

    let n_constraints = specialization.n_columns - 2;
    let base_regs_required = 1u32
        .checked_add(
            n_constraints
                .checked_mul(7)
                .ok_or(MetalEvaluationProgramLoweringError::RegisterBudgetOverflow)?,
        )
        .ok_or(MetalEvaluationProgramLoweringError::RegisterBudgetOverflow)?;
    let ext_regs_required = n_constraints;

    let mut base_insts = Vec::with_capacity(base_regs_required as usize);
    let mut ext_insts = Vec::with_capacity(ext_regs_required as usize);
    let mut constraint_roots = Vec::with_capacity(n_constraints as usize);

    let zero_reg = 0u16;
    base_insts.push(MetalEvaluationProgramBaseInstV1::const_value(zero_reg, 0));

    let mut next_base_reg = 1u16;
    let mut next_ext_reg = 0u16;
    for column in 2..specialization.n_columns {
        let reg_a = next_base_reg;
        next_base_reg = next_base_reg
            .checked_add(1)
            .ok_or(MetalEvaluationProgramLoweringError::RegisterBudgetOverflow)?;
        base_insts.push(MetalEvaluationProgramBaseInstV1::trace_col(
            reg_a,
            1,
            column - 2,
            0,
        ));

        let reg_b = next_base_reg;
        next_base_reg = next_base_reg
            .checked_add(1)
            .ok_or(MetalEvaluationProgramLoweringError::RegisterBudgetOverflow)?;
        base_insts.push(MetalEvaluationProgramBaseInstV1::trace_col(
            reg_b,
            1,
            column - 1,
            0,
        ));

        let reg_c = next_base_reg;
        next_base_reg = next_base_reg
            .checked_add(1)
            .ok_or(MetalEvaluationProgramLoweringError::RegisterBudgetOverflow)?;
        base_insts.push(MetalEvaluationProgramBaseInstV1::trace_col(
            reg_c, 1, column, 0,
        ));

        let reg_a_sq = next_base_reg;
        next_base_reg = next_base_reg
            .checked_add(1)
            .ok_or(MetalEvaluationProgramLoweringError::RegisterBudgetOverflow)?;
        base_insts.push(MetalEvaluationProgramBaseInstV1::binary(
            MetalEvaluationProgramBaseOpcodeV1::Mul,
            reg_a_sq,
            reg_a as u32,
            reg_a as u32,
        ));

        let reg_b_sq = next_base_reg;
        next_base_reg = next_base_reg
            .checked_add(1)
            .ok_or(MetalEvaluationProgramLoweringError::RegisterBudgetOverflow)?;
        base_insts.push(MetalEvaluationProgramBaseInstV1::binary(
            MetalEvaluationProgramBaseOpcodeV1::Mul,
            reg_b_sq,
            reg_b as u32,
            reg_b as u32,
        ));

        let reg_sum = next_base_reg;
        next_base_reg = next_base_reg
            .checked_add(1)
            .ok_or(MetalEvaluationProgramLoweringError::RegisterBudgetOverflow)?;
        base_insts.push(MetalEvaluationProgramBaseInstV1::binary(
            MetalEvaluationProgramBaseOpcodeV1::Add,
            reg_sum,
            reg_a_sq as u32,
            reg_b_sq as u32,
        ));

        let reg_constraint = next_base_reg;
        next_base_reg = next_base_reg
            .checked_add(1)
            .ok_or(MetalEvaluationProgramLoweringError::RegisterBudgetOverflow)?;
        base_insts.push(MetalEvaluationProgramBaseInstV1::binary(
            MetalEvaluationProgramBaseOpcodeV1::Sub,
            reg_constraint,
            reg_c as u32,
            reg_sum as u32,
        ));

        let ext_reg = next_ext_reg;
        next_ext_reg = next_ext_reg
            .checked_add(1)
            .ok_or(MetalEvaluationProgramLoweringError::RegisterBudgetOverflow)?;
        ext_insts.push(MetalEvaluationProgramExtInstV1::secure_col(
            ext_reg,
            reg_constraint as u32,
            zero_reg as u32,
            zero_reg as u32,
            zero_reg as u32,
        ));
        constraint_roots.push(ext_reg as u32);
    }

    Ok(build_owned_program_v1(
        STWO_METAL_EVAL_PROGRAM_CAP_BASE_INV_V1
            | STWO_METAL_EVAL_PROGRAM_CAP_EXT_MUL_V1
            | STWO_METAL_EVAL_PROGRAM_CAP_PREFINALIZED_LOGUP_V1,
        2,
        0,
        0,
        base_regs_required,
        ext_regs_required,
        Vec::new(),
        Vec::new(),
        base_insts,
        ext_insts,
        constraint_roots,
    ))
}

/// Lower a synthetic `virtual_snos` constraint set representing the first
/// stwo-cairo downstream hardening target (G11 / DN-0011).
///
/// The constraint set models a parametric column-pair sum check that exercises
/// V1 contract capabilities beyond wide-fibonacci:
/// - Multi-interaction traces (interaction 0 = preprocessed, interaction 1 = main)
/// - Base parameter access (Param opcode)
/// - Column-pair arithmetic (Add, Sub)
///
/// For each consecutive column pair (col_i, col_{i+1}) in interaction 1:
///   constraint_k = col_i + col_{i+1} - base_param_0
///
/// Requires n_columns >= 2 (at least one column pair).
pub fn lower_virtual_snos_evaluation_program_v1(
    specialization: MetalEvaluationProgramSpecializationV1,
) -> Result<OwnedMetalEvaluationProgramV1, MetalEvaluationProgramLoweringError> {
    if specialization.n_columns < 2 {
        return Err(
            MetalEvaluationProgramLoweringError::InvalidVirtualSnosColumnCount {
                n_columns: specialization.n_columns,
            },
        );
    }

    let n_constraints = specialization.n_columns - 1;
    // Per constraint: param(1) + trace_col_a(1) + trace_col_b(1) + add(1) + sub(1) = 5 regs
    let base_regs_required = n_constraints
        .checked_mul(5)
        .ok_or(MetalEvaluationProgramLoweringError::RegisterBudgetOverflow)?;
    let ext_regs_required = n_constraints;

    let mut base_insts = Vec::with_capacity(base_regs_required as usize);
    let mut ext_insts = Vec::with_capacity(ext_regs_required as usize);
    let mut constraint_roots = Vec::with_capacity(n_constraints as usize);

    let mut next_base_reg = 0u16;
    let mut next_ext_reg = 0u16;

    for col in 0..n_constraints {
        // Load base_param_0 into a register
        let reg_param = next_base_reg;
        next_base_reg = next_base_reg
            .checked_add(1)
            .ok_or(MetalEvaluationProgramLoweringError::RegisterBudgetOverflow)?;
        base_insts.push(MetalEvaluationProgramBaseInstV1 {
            op: MetalEvaluationProgramBaseOpcodeV1::Param as u8,
            interaction: 0,
            dst: reg_param,
            a: 0, // param slot 0
            b: 0,
            imm: 0,
        });

        // Load col_i from interaction 1
        let reg_a = next_base_reg;
        next_base_reg = next_base_reg
            .checked_add(1)
            .ok_or(MetalEvaluationProgramLoweringError::RegisterBudgetOverflow)?;
        base_insts.push(MetalEvaluationProgramBaseInstV1::trace_col(
            reg_a,
            1,
            col,
            0,
        ));

        // Load col_{i+1} from interaction 1
        let reg_b = next_base_reg;
        next_base_reg = next_base_reg
            .checked_add(1)
            .ok_or(MetalEvaluationProgramLoweringError::RegisterBudgetOverflow)?;
        base_insts.push(MetalEvaluationProgramBaseInstV1::trace_col(
            reg_b,
            1,
            col + 1,
            0,
        ));

        // sum = col_i + col_{i+1}
        let reg_sum = next_base_reg;
        next_base_reg = next_base_reg
            .checked_add(1)
            .ok_or(MetalEvaluationProgramLoweringError::RegisterBudgetOverflow)?;
        base_insts.push(MetalEvaluationProgramBaseInstV1::binary(
            MetalEvaluationProgramBaseOpcodeV1::Add,
            reg_sum,
            reg_a as u32,
            reg_b as u32,
        ));

        // constraint = sum - param_0
        let reg_constraint = next_base_reg;
        next_base_reg = next_base_reg
            .checked_add(1)
            .ok_or(MetalEvaluationProgramLoweringError::RegisterBudgetOverflow)?;
        base_insts.push(MetalEvaluationProgramBaseInstV1::binary(
            MetalEvaluationProgramBaseOpcodeV1::Sub,
            reg_constraint,
            reg_sum as u32,
            reg_param as u32,
        ));

        // Lift to secure field for constraint root
        let ext_reg = next_ext_reg;
        next_ext_reg = next_ext_reg
            .checked_add(1)
            .ok_or(MetalEvaluationProgramLoweringError::RegisterBudgetOverflow)?;
        ext_insts.push(MetalEvaluationProgramExtInstV1::secure_col(
            ext_reg,
            reg_constraint as u32,
            0, // pad with zero-valued base registers (unused)
            0,
            0,
        ));
        constraint_roots.push(ext_reg as u32);
    }

    Ok(build_owned_program_v1(
        STWO_METAL_EVAL_PROGRAM_CAP_BASE_INV_V1
            | STWO_METAL_EVAL_PROGRAM_CAP_EXT_MUL_V1
            | STWO_METAL_EVAL_PROGRAM_CAP_PREFINALIZED_LOGUP_V1,
        2, // n_interactions: preprocessed + main
        1, // n_base_params: one parameter (the expected sum)
        0, // n_ext_params
        base_regs_required,
        ext_regs_required,
        Vec::new(),
        Vec::new(),
        base_insts,
        ext_insts,
        constraint_roots,
    ))
}

pub fn interpret_metal_evaluation_program_v1(
    program: &OwnedMetalEvaluationProgramV1,
    runtime: MetalEvaluationProgramRuntimeInputsV1<'_>,
) -> Result<Vec<SecureField>, MetalEvaluationProgramInterpreterError> {
    let n_rows = runtime
        .trace
        .trace_interactions
        .first()
        .and_then(|interaction| interaction.first().map(|column| column.len()))
        .or_else(|| {
            runtime
                .trace
                .trace_interactions
                .iter()
                .find_map(|interaction| interaction.first().map(|column| column.len()))
        })
        .ok_or(MetalEvaluationProgramInterpreterError::EmptyTrace)?;
    for interaction in runtime.trace.trace_interactions {
        for column in *interaction {
            if column.len() != n_rows {
                return Err(
                    MetalEvaluationProgramInterpreterError::InconsistentTraceColumnLength {
                        expected_len: n_rows,
                        actual_len: column.len(),
                    },
                );
            }
        }
    }
    for column in runtime.trace.preprocessed_columns {
        if column.len() != n_rows {
            return Err(
                MetalEvaluationProgramInterpreterError::InconsistentPreprocessedColumnLength {
                    expected_len: n_rows,
                    actual_len: column.len(),
                },
            );
        }
    }
    if runtime.random_coeff_powers.len() != program.constraint_roots().len() {
        return Err(
            MetalEvaluationProgramInterpreterError::RandomCoeffCountMismatch {
                expected: program.constraint_roots().len(),
                actual: runtime.random_coeff_powers.len(),
            },
        );
    }

    let interactions = runtime.trace.trace_interactions;
    if interactions.len() != program.header().n_interactions as usize {
        return Err(
            MetalEvaluationProgramInterpreterError::TraceInteractionOutOfRange {
                interaction: program.header().n_interactions.saturating_sub(1) as usize,
                available: interactions.len(),
            },
        );
    }
    let mut row_res = Vec::with_capacity(n_rows);
    let max_base_regs = program.header().max_base_regs as usize;
    let max_ext_regs = program.header().max_ext_regs as usize;
    for row in 0..n_rows {
        let mut base_regs = vec![BaseField::zero(); max_base_regs];
        let mut ext_regs = vec![SecureField::zero(); max_ext_regs];

        for inst in program.base_insts() {
            let dst = inst.dst as usize;
            if dst >= max_base_regs {
                return Err(
                    MetalEvaluationProgramInterpreterError::BaseRegisterOutOfRange {
                        register: dst,
                        available: max_base_regs,
                    },
                );
            }
            base_regs[dst] = interpret_base_inst(
                *inst,
                row,
                interactions,
                runtime.trace.preprocessed_columns,
                runtime.base_params,
                &base_regs,
            )?;
        }

        for inst in program.ext_insts() {
            let dst = inst.dst as usize;
            if dst >= max_ext_regs {
                return Err(
                    MetalEvaluationProgramInterpreterError::ExtRegisterOutOfRange {
                        register: dst,
                        available: max_ext_regs,
                    },
                );
            }
            ext_regs[dst] = interpret_ext_inst(*inst, runtime.ext_params, &base_regs, &ext_regs)?;
        }

        let mut acc = SecureField::zero();
        for (constraint_index, root) in program.constraint_roots().iter().enumerate() {
            let root = *root as usize;
            if root >= max_ext_regs {
                return Err(
                    MetalEvaluationProgramInterpreterError::ConstraintRootOutOfRange {
                        root,
                        available: max_ext_regs,
                    },
                );
            }
            acc += runtime.random_coeff_powers[constraint_index] * ext_regs[root];
        }
        row_res.push(acc);
    }

    Ok(row_res)
}

pub fn execute_metal_evaluation_program_v1_on_metal(
    program: &OwnedMetalEvaluationProgramV1,
    runtime: MetalEvaluationProgramRuntimeInputsV1<'_>,
) -> Result<Vec<SecureField>, MetalEvaluationProgramExecutionError> {
    let (row_res, _) = execute_selected_metal_evaluation_program_v1_on_metal(
        program,
        runtime,
        MetalEvaluationProgramCapabilityProfileV1::current(),
    )?;
    Ok(row_res)
}

pub fn lookup_metal_evaluation_program_overlay_v1(
    program: &OwnedMetalEvaluationProgramV1,
    profile: MetalEvaluationProgramCapabilityProfileV1,
) -> Option<MetalEvaluationProgramOverlayV1> {
    metal_evaluation_program_overlays_v1()
        .iter()
        .copied()
        .find(|overlay| {
            overlay.semantic_hash == program.header().semantic_hash
                && profile.capability_bits & overlay.required_capability_bits
                    == overlay.required_capability_bits
        })
}

pub fn select_metal_evaluation_program_dispatch_v1(
    program: &OwnedMetalEvaluationProgramV1,
    profile: MetalEvaluationProgramCapabilityProfileV1,
) -> Result<MetalEvaluationProgramDispatchKindV1, MetalEvaluationProgramExecutionError> {
    if profile.runtime_support != MetalRuntimeSupport::Available {
        return Err(MetalEvaluationProgramExecutionError::RuntimeUnavailable(
            profile.runtime_support,
        ));
    }

    program
        .validate(profile.budget())
        .map_err(
            |_| MetalEvaluationProgramExecutionError::RegisterBudgetExceeded {
                required_base: program.header().max_base_regs,
                required_ext: program.header().max_ext_regs,
                supported_base: profile.max_base_regs,
                supported_ext: profile.max_ext_regs,
            },
        )?;

    Ok(
        lookup_metal_evaluation_program_overlay_v1(program, profile)
            .map(MetalEvaluationProgramDispatchKindV1::GeneratedOverlay)
            .unwrap_or(MetalEvaluationProgramDispatchKindV1::GenericMetalInterpreter),
    )
}

pub fn execute_selected_metal_evaluation_program_v1_on_metal(
    program: &OwnedMetalEvaluationProgramV1,
    runtime: MetalEvaluationProgramRuntimeInputsV1<'_>,
    profile: MetalEvaluationProgramCapabilityProfileV1,
) -> Result<(Vec<SecureField>, MetalEvaluationProgramDispatchKindV1), MetalEvaluationProgramExecutionError>
{
    let dispatch = select_metal_evaluation_program_dispatch_v1(program, profile)?;
    let row_res = match dispatch {
        MetalEvaluationProgramDispatchKindV1::GenericMetalInterpreter => {
            interpret_metal_evaluation_program_v1_on_metal(program, runtime)
                .map_err(MetalEvaluationProgramExecutionError::from)?
        }
        MetalEvaluationProgramDispatchKindV1::GeneratedOverlay(overlay) => {
            match overlay.name {
                WIDE_FIBONACCI_EVAL_OVERLAY_NAME_V1 => {
                    execute_wide_fibonacci_overlay_v1_on_metal(program, runtime)
                        .map_err(MetalEvaluationProgramExecutionError::from)?
                }
                _ => {
                    interpret_metal_evaluation_program_v1_on_metal(program, runtime)
                        .map_err(MetalEvaluationProgramExecutionError::from)?
                }
            }
        }
    };
    Ok((row_res, dispatch))
}

#[cfg_attr(not(test), allow(dead_code))]
const METAL_EVAL_PROGRAM_V1_DEVICE_BUDGET: MetalEvaluationProgramBudgetV1 =
    MetalEvaluationProgramBudgetV1::new(1024, 256);

#[cfg_attr(not(test), allow(dead_code))]
fn interpret_metal_evaluation_program_v1_on_metal(
    program: &OwnedMetalEvaluationProgramV1,
    runtime: MetalEvaluationProgramRuntimeInputsV1<'_>,
) -> Result<Vec<SecureField>, MetalEvaluationProgramDeviceInterpreterError> {
    let support = metal_runtime_support();
    if support != MetalRuntimeSupport::Available {
        return Err(MetalEvaluationProgramDeviceInterpreterError::RuntimeUnavailable(support));
    }

    program
        .validate(METAL_EVAL_PROGRAM_V1_DEVICE_BUDGET)
        .map_err(
            |_| MetalEvaluationProgramDeviceInterpreterError::RegisterBudgetExceeded {
                required_base: program.header().max_base_regs,
                required_ext: program.header().max_ext_regs,
                supported_base: METAL_EVAL_PROGRAM_V1_DEVICE_BUDGET.max_base_regs,
                supported_ext: METAL_EVAL_PROGRAM_V1_DEVICE_BUDGET.max_ext_regs,
            },
        )?;

    let n_rows = runtime
        .trace
        .trace_interactions
        .first()
        .and_then(|interaction| interaction.first().map(|column| column.len()))
        .or_else(|| {
            runtime
                .trace
                .trace_interactions
                .iter()
                .find_map(|interaction| interaction.first().map(|column| column.len()))
        })
        .ok_or(MetalEvaluationProgramDeviceInterpreterError::EmptyTrace)?;

    if runtime.trace.trace_interactions.len() != program.header().n_interactions as usize {
        return Err(
            MetalEvaluationProgramDeviceInterpreterError::TraceInteractionCountMismatch {
                expected: program.header().n_interactions as usize,
                actual: runtime.trace.trace_interactions.len(),
            },
        );
    }
    for interaction in runtime.trace.trace_interactions {
        for column in *interaction {
            if column.len() != n_rows {
                return Err(
                    MetalEvaluationProgramDeviceInterpreterError::InconsistentTraceColumnLength {
                        expected_len: n_rows,
                        actual_len: column.len(),
                    },
                );
            }
        }
    }
    for column in runtime.trace.preprocessed_columns {
        if column.len() != n_rows {
            return Err(
                MetalEvaluationProgramDeviceInterpreterError::InconsistentPreprocessedColumnLength {
                    expected_len: n_rows,
                    actual_len: column.len(),
                },
            );
        }
    }
    if runtime.base_params.len() != program.header().n_base_params as usize {
        return Err(
            MetalEvaluationProgramDeviceInterpreterError::BaseParamCountMismatch {
                expected: program.header().n_base_params as usize,
                actual: runtime.base_params.len(),
            },
        );
    }
    if runtime.ext_params.len() != program.header().n_ext_params as usize {
        return Err(
            MetalEvaluationProgramDeviceInterpreterError::ExtParamCountMismatch {
                expected: program.header().n_ext_params as usize,
                actual: runtime.ext_params.len(),
            },
        );
    }
    if runtime.random_coeff_powers.len() != program.constraint_roots().len() {
        return Err(
            MetalEvaluationProgramDeviceInterpreterError::RandomCoeffCountMismatch {
                expected: program.constraint_roots().len(),
                actual: runtime.random_coeff_powers.len(),
            },
        );
    }
    for inst in program.base_insts() {
        if MetalEvaluationProgramBaseOpcodeV1::from_raw(inst.op)
            == Some(MetalEvaluationProgramBaseOpcodeV1::TraceCol)
            && inst.imm != 0
        {
            return Err(
                MetalEvaluationProgramDeviceInterpreterError::UnsupportedNonZeroTraceOffset {
                    offset: inst.imm,
                },
            );
        }
    }

    let (flat_trace, interaction_offsets) =
        flatten_trace_interactions(runtime.trace.trace_interactions, n_rows);
    let flat_preprocessed = runtime
        .trace
        .preprocessed_columns
        .iter()
        .flat_map(|column| column.iter().map(|value| value.0))
        .collect::<Vec<_>>();
    let base_params = runtime
        .base_params
        .iter()
        .map(|value| value.0)
        .collect::<Vec<_>>();
    let ext_params = runtime
        .ext_params
        .iter()
        .flat_map(|value| value.to_m31_array().map(|limb| limb.0))
        .collect::<Vec<_>>();
    let random_coeff_powers = runtime
        .random_coeff_powers
        .iter()
        .flat_map(|value| value.to_m31_array().map(|limb| limb.0))
        .collect::<Vec<_>>();
    let base_insts = pack_base_insts(program.base_insts());
    let ext_insts = pack_ext_insts(program.ext_insts());
    let constraint_roots = program.constraint_roots().to_vec();

    let trace_values = U32Buffer::from_slice(&flat_trace)?;
    let interaction_offsets = U32Buffer::from_slice(&interaction_offsets)?;
    let preprocessed_values = optional_u32_buffer(&flat_preprocessed)?;
    let base_params = optional_u32_buffer(&base_params)?;
    let ext_params = optional_u32_buffer(&ext_params)?;
    let random_coeff_powers = U32Buffer::from_slice(&random_coeff_powers)?;
    let base_insts = U32Buffer::from_slice(&base_insts)?;
    let ext_insts = U32Buffer::from_slice(&ext_insts)?;
    let constraint_roots = U32Buffer::from_slice(&constraint_roots)?;

    let dst = U32Buffer::eval_program_v1_reference_u32x4(
        &trace_values,
        &interaction_offsets,
        &preprocessed_values,
        &base_params,
        &ext_params,
        &random_coeff_powers,
        &base_insts,
        &ext_insts,
        &constraint_roots,
        n_rows,
        runtime.trace.trace_interactions.len() as u32,
        runtime.trace.preprocessed_columns.len() as u32,
        runtime.base_params.len() as u32,
        runtime.ext_params.len() as u32,
        program.base_insts().len() as u32,
        program.ext_insts().len() as u32,
        program.constraint_roots().len() as u32,
    )?;

    let raw = dst.to_vec()?;
    Ok(raw
        .chunks_exact(4)
        .map(|limbs| SecureField::from_u32_unchecked(limbs[0], limbs[1], limbs[2], limbs[3]))
        .collect())
}

fn execute_wide_fibonacci_overlay_v1_on_metal(
    program: &OwnedMetalEvaluationProgramV1,
    runtime: MetalEvaluationProgramRuntimeInputsV1<'_>,
) -> Result<Vec<SecureField>, MetalEvaluationProgramDeviceInterpreterError> {
    let support = metal_runtime_support();
    if support != MetalRuntimeSupport::Available {
        return Err(MetalEvaluationProgramDeviceInterpreterError::RuntimeUnavailable(support));
    }

    program
        .validate(METAL_EVAL_PROGRAM_V1_DEVICE_BUDGET)
        .map_err(
            |_| MetalEvaluationProgramDeviceInterpreterError::RegisterBudgetExceeded {
                required_base: program.header().max_base_regs,
                required_ext: program.header().max_ext_regs,
                supported_base: METAL_EVAL_PROGRAM_V1_DEVICE_BUDGET.max_base_regs,
                supported_ext: METAL_EVAL_PROGRAM_V1_DEVICE_BUDGET.max_ext_regs,
            },
        )?;

    let n_rows = runtime
        .trace
        .trace_interactions
        .first()
        .and_then(|interaction| interaction.first().map(|column| column.len()))
        .or_else(|| {
            runtime
                .trace
                .trace_interactions
                .iter()
                .find_map(|interaction| interaction.first().map(|column| column.len()))
        })
        .ok_or(MetalEvaluationProgramDeviceInterpreterError::EmptyTrace)?;

    if runtime.trace.trace_interactions.len() != program.header().n_interactions as usize {
        return Err(
            MetalEvaluationProgramDeviceInterpreterError::TraceInteractionCountMismatch {
                expected: program.header().n_interactions as usize,
                actual: runtime.trace.trace_interactions.len(),
            },
        );
    }
    for interaction in runtime.trace.trace_interactions {
        for column in *interaction {
            if column.len() != n_rows {
                return Err(
                    MetalEvaluationProgramDeviceInterpreterError::InconsistentTraceColumnLength {
                        expected_len: n_rows,
                        actual_len: column.len(),
                    },
                );
            }
        }
    }
    if runtime.random_coeff_powers.len() != program.constraint_roots().len() {
        return Err(
            MetalEvaluationProgramDeviceInterpreterError::RandomCoeffCountMismatch {
                expected: program.constraint_roots().len(),
                actual: runtime.random_coeff_powers.len(),
            },
        );
    }
    if runtime.base_params.len() != program.header().n_base_params as usize {
        return Err(
            MetalEvaluationProgramDeviceInterpreterError::BaseParamCountMismatch {
                expected: program.header().n_base_params as usize,
                actual: runtime.base_params.len(),
            },
        );
    }
    if runtime.ext_params.len() != program.header().n_ext_params as usize {
        return Err(
            MetalEvaluationProgramDeviceInterpreterError::ExtParamCountMismatch {
                expected: program.header().n_ext_params as usize,
                actual: runtime.ext_params.len(),
            },
        );
    }
    for inst in program.base_insts() {
        if MetalEvaluationProgramBaseOpcodeV1::from_raw(inst.op)
            == Some(MetalEvaluationProgramBaseOpcodeV1::TraceCol)
            && inst.imm != 0
        {
            return Err(
                MetalEvaluationProgramDeviceInterpreterError::UnsupportedNonZeroTraceOffset {
                    offset: inst.imm,
                },
            );
        }
    }

    let (flat_trace, interaction_offsets) =
        flatten_trace_interactions(runtime.trace.trace_interactions, n_rows);
    let random_coeff_powers = runtime
        .random_coeff_powers
        .iter()
        .flat_map(|value| value.to_m31_array().map(|limb| limb.0))
        .collect::<Vec<_>>();

    let trace_values = U32Buffer::from_slice(&flat_trace)?;
    let interaction_offsets = U32Buffer::from_slice(&interaction_offsets)?;
    let random_coeff_powers = U32Buffer::from_slice(&random_coeff_powers)?;
    let dst = U32Buffer::eval_program_v1_wide_fibonacci_u32x4(
        &trace_values,
        &interaction_offsets,
        &random_coeff_powers,
        n_rows,
        runtime.trace.trace_interactions.len() as u32,
        program.constraint_roots().len() as u32,
    )?;

    let raw = dst.to_vec()?;
    Ok(raw
        .chunks_exact(4)
        .map(|limbs| SecureField::from_u32_unchecked(limbs[0], limbs[1], limbs[2], limbs[3]))
        .collect())
}

#[cfg_attr(not(test), allow(dead_code))]
fn flatten_trace_interactions(
    interactions: &[&[&[BaseField]]],
    n_rows: usize,
) -> (Vec<u32>, Vec<u32>) {
    let mut flat_trace = Vec::new();
    let mut interaction_offsets = Vec::with_capacity(interactions.len() + 1);
    let mut running_columns = 0u32;
    interaction_offsets.push(running_columns);
    for interaction in interactions {
        for column in *interaction {
            debug_assert_eq!(column.len(), n_rows);
            flat_trace.extend(column.iter().map(|value| value.0));
        }
        running_columns += interaction.len() as u32;
        interaction_offsets.push(running_columns);
    }
    (flat_trace, interaction_offsets)
}

#[cfg_attr(not(test), allow(dead_code))]
fn pack_base_insts(values: &[MetalEvaluationProgramBaseInstV1]) -> Vec<u32> {
    values
        .iter()
        .flat_map(|inst| {
            [
                (inst.op as u32) | ((inst.interaction as u32) << 8) | ((inst.dst as u32) << 16),
                inst.a,
                inst.b,
                inst.imm as u32,
            ]
        })
        .collect()
}

#[cfg_attr(not(test), allow(dead_code))]
fn pack_ext_insts(values: &[MetalEvaluationProgramExtInstV1]) -> Vec<u32> {
    values
        .iter()
        .flat_map(|inst| {
            [
                (inst.op as u32) | ((inst.reserved0 as u32) << 8) | ((inst.dst as u32) << 16),
                inst.a,
                inst.b,
                inst.c,
                inst.d,
            ]
        })
        .collect()
}

#[cfg_attr(not(test), allow(dead_code))]
fn optional_u32_buffer(values: &[u32]) -> Result<U32Buffer, MetalError> {
    if values.is_empty() {
        U32Buffer::zeroed(1)
    } else {
        U32Buffer::from_slice(values)
    }
}
fn interpret_base_inst(
    inst: MetalEvaluationProgramBaseInstV1,
    row: usize,
    interactions: &[&[&[BaseField]]],
    preprocessed_columns: &[&[BaseField]],
    base_params: &[BaseField],
    base_regs: &[BaseField],
) -> Result<BaseField, MetalEvaluationProgramInterpreterError> {
    Ok(
        match MetalEvaluationProgramBaseOpcodeV1::from_raw(inst.op).ok_or(
            MetalEvaluationProgramInterpreterError::UnsupportedBaseOpcode { opcode: inst.op },
        )? {
            MetalEvaluationProgramBaseOpcodeV1::TraceCol => {
                if inst.imm != 0 {
                    return Err(
                        MetalEvaluationProgramInterpreterError::NonZeroTraceOffsetUnsupported {
                            offset: inst.imm,
                        },
                    );
                }
                let interaction = inst.interaction as usize;
                let columns = interactions.get(interaction).ok_or(
                    MetalEvaluationProgramInterpreterError::TraceInteractionOutOfRange {
                        interaction,
                        available: interactions.len(),
                    },
                )?;
                let column = inst.a as usize;
                let values = columns.get(column).ok_or(
                    MetalEvaluationProgramInterpreterError::TraceColumnOutOfRange {
                        interaction,
                        column,
                        available: columns.len(),
                    },
                )?;
                values[row]
            }
            MetalEvaluationProgramBaseOpcodeV1::PreprocessedCol => {
                let column = inst.a as usize;
                let values = preprocessed_columns.get(column).ok_or(
                    MetalEvaluationProgramInterpreterError::PreprocessedColumnOutOfRange {
                        column,
                        available: preprocessed_columns.len(),
                    },
                )?;
                values[row]
            }
            MetalEvaluationProgramBaseOpcodeV1::Param => {
                let slot = inst.a as usize;
                *base_params.get(slot).ok_or(
                    MetalEvaluationProgramInterpreterError::BaseParamOutOfRange {
                        slot,
                        available: base_params.len(),
                    },
                )?
            }
            MetalEvaluationProgramBaseOpcodeV1::Const => BaseField::from_u32_unchecked(inst.a),
            MetalEvaluationProgramBaseOpcodeV1::Add => {
                read_base_reg(base_regs, inst.a as usize)?
                    + read_base_reg(base_regs, inst.b as usize)?
            }
            MetalEvaluationProgramBaseOpcodeV1::Sub => {
                read_base_reg(base_regs, inst.a as usize)?
                    - read_base_reg(base_regs, inst.b as usize)?
            }
            MetalEvaluationProgramBaseOpcodeV1::Mul => {
                read_base_reg(base_regs, inst.a as usize)?
                    * read_base_reg(base_regs, inst.b as usize)?
            }
            MetalEvaluationProgramBaseOpcodeV1::Neg => -read_base_reg(base_regs, inst.a as usize)?,
            MetalEvaluationProgramBaseOpcodeV1::Inv => {
                read_base_reg(base_regs, inst.a as usize)?.inverse()
            }
        },
    )
}

fn interpret_ext_inst(
    inst: MetalEvaluationProgramExtInstV1,
    ext_params: &[SecureField],
    base_regs: &[BaseField],
    ext_regs: &[SecureField],
) -> Result<SecureField, MetalEvaluationProgramInterpreterError> {
    Ok(
        match MetalEvaluationProgramExtOpcodeV1::from_raw(inst.op).ok_or(
            MetalEvaluationProgramInterpreterError::UnsupportedExtOpcode { opcode: inst.op },
        )? {
            MetalEvaluationProgramExtOpcodeV1::SecureCol => SecureField::from_partial_evals([
                SecureField::from(read_base_reg(base_regs, inst.a as usize)?),
                SecureField::from(read_base_reg(base_regs, inst.b as usize)?),
                SecureField::from(read_base_reg(base_regs, inst.c as usize)?),
                SecureField::from(read_base_reg(base_regs, inst.d as usize)?),
            ]),
            MetalEvaluationProgramExtOpcodeV1::Param => {
                let slot = inst.a as usize;
                *ext_params.get(slot).ok_or(
                    MetalEvaluationProgramInterpreterError::ExtParamOutOfRange {
                        slot,
                        available: ext_params.len(),
                    },
                )?
            }
            MetalEvaluationProgramExtOpcodeV1::Const => {
                SecureField::from_u32_unchecked(inst.a, inst.b, inst.c, inst.d)
            }
            MetalEvaluationProgramExtOpcodeV1::Add => {
                read_ext_reg(ext_regs, inst.a as usize)? + read_ext_reg(ext_regs, inst.b as usize)?
            }
            MetalEvaluationProgramExtOpcodeV1::Sub => {
                read_ext_reg(ext_regs, inst.a as usize)? - read_ext_reg(ext_regs, inst.b as usize)?
            }
            MetalEvaluationProgramExtOpcodeV1::Mul => {
                read_ext_reg(ext_regs, inst.a as usize)? * read_ext_reg(ext_regs, inst.b as usize)?
            }
            MetalEvaluationProgramExtOpcodeV1::Neg => -read_ext_reg(ext_regs, inst.a as usize)?,
        },
    )
}

fn read_base_reg(
    base_regs: &[BaseField],
    register: usize,
) -> Result<BaseField, MetalEvaluationProgramInterpreterError> {
    base_regs.get(register).copied().ok_or(
        MetalEvaluationProgramInterpreterError::BaseRegisterOutOfRange {
            register,
            available: base_regs.len(),
        },
    )
}

fn read_ext_reg(
    ext_regs: &[SecureField],
    register: usize,
) -> Result<SecureField, MetalEvaluationProgramInterpreterError> {
    ext_regs.get(register).copied().ok_or(
        MetalEvaluationProgramInterpreterError::ExtRegisterOutOfRange {
            register,
            available: ext_regs.len(),
        },
    )
}

#[cfg(test)]
mod tests {
    use core::mem::{align_of, offset_of, size_of};

    use ark_std::Zero;
    use stwo::core::fields::m31::BaseField;
    use stwo::core::fields::qm31::SecureField;
    use stwo::core::fields::FieldExpOps;

    use super::{
        execute_selected_metal_evaluation_program_v1_on_metal,
        interpret_metal_evaluation_program_v1, interpret_metal_evaluation_program_v1_on_metal,
        lower_registered_metal_evaluation_program_v1, lower_wide_fibonacci_evaluation_program_v1,
        lookup_metal_evaluation_program_overlay_v1, metal_evaluation_program_semantic_hash_v1,
        metal_runtime_support, select_metal_evaluation_program_dispatch_v1,
        validate_metal_evaluation_program_v1, MetalEvaluationProgramBaseOpcodeV1,
        MetalEvaluationProgramBudgetV1, MetalEvaluationProgramCapabilityProfileV1,
        MetalEvaluationProgramDeviceInterpreterError, MetalEvaluationProgramDispatchKindV1,
        MetalEvaluationProgramHeaderV1, MetalEvaluationProgramInterpreterError,
        MetalEvaluationProgramLoweringError, MetalEvaluationProgramRuntimeInputsV1,
        MetalEvaluationProgramSectionDescV1, MetalEvaluationProgramSectionKindV1,
        MetalEvaluationProgramSpecializationV1, MetalEvaluationProgramTraceViewV1,
        MetalEvaluationProgramValidationError, MetalRuntimeSupport,
        STWO_METAL_EVAL_PROGRAM_ABI_MAJOR_V1, STWO_METAL_EVAL_PROGRAM_CAP_BASE_INV_V1,
        STWO_METAL_EVAL_PROGRAM_CAP_EXT_MUL_V1, STWO_METAL_EVAL_PROGRAM_CAP_PREFINALIZED_LOGUP_V1,
        STWO_METAL_EVAL_PROGRAM_FLAG_PREFINALIZED_LOGUP_V1, STWO_METAL_EVAL_PROGRAM_MAGIC_V1,
        STWO_METAL_EVAL_PROGRAM_SECURE_EXT_DEGREE_V1,
    };

    fn wide_fibonacci_trace(n_rows: usize, n_columns: usize) -> Vec<Vec<BaseField>> {
        let mut columns = vec![vec![BaseField::zero(); n_rows]; n_columns];
        for row in 0..n_rows {
            let mut a = BaseField::from_u32_unchecked(1);
            let mut b = BaseField::from_u32_unchecked((row as u32) + 2);
            columns[0][row] = a;
            columns[1][row] = b;
            for column in columns.iter_mut().skip(2) {
                let next = a.square() + b.square();
                column[row] = next;
                a = b;
                b = next;
            }
        }
        columns
    }

    fn wide_fibonacci_row_residues(
        columns: &[Vec<BaseField>],
        random_coeff_powers: &[SecureField],
    ) -> Vec<SecureField> {
        let n_rows = columns[0].len();
        let n_constraints = columns.len() - 2;
        (0..n_rows)
            .map(|row| {
                let mut acc = SecureField::zero();
                for constraint in 0..n_constraints {
                    let a = columns[constraint][row];
                    let b = columns[constraint + 1][row];
                    let c = columns[constraint + 2][row];
                    let residue = c - (a.square() + b.square());
                    acc += SecureField::from_partial_evals([
                        SecureField::from(residue),
                        SecureField::zero(),
                        SecureField::zero(),
                        SecureField::zero(),
                    ]) * random_coeff_powers[constraint];
                }
                acc
            })
            .collect()
    }

    fn valid_header() -> MetalEvaluationProgramHeaderV1 {
        let semantic_hash = metal_evaluation_program_semantic_hash_v1(&[b"base", b"ext"]);
        MetalEvaluationProgramHeaderV1 {
            magic: STWO_METAL_EVAL_PROGRAM_MAGIC_V1,
            abi_major: STWO_METAL_EVAL_PROGRAM_ABI_MAJOR_V1,
            abi_minor: 0,
            n_sections: 5,
            flags: STWO_METAL_EVAL_PROGRAM_FLAG_PREFINALIZED_LOGUP_V1,
            semantic_hash,
            capability_bits: STWO_METAL_EVAL_PROGRAM_CAP_BASE_INV_V1
                | STWO_METAL_EVAL_PROGRAM_CAP_EXT_MUL_V1
                | STWO_METAL_EVAL_PROGRAM_CAP_PREFINALIZED_LOGUP_V1,
            n_interactions: 2,
            n_base_params: 3,
            n_ext_params: 1,
            n_constraints: 4,
            max_base_regs: 16,
            max_ext_regs: 8,
            secure_ext_degree: STWO_METAL_EVAL_PROGRAM_SECURE_EXT_DEGREE_V1,
            reserved: [0; 8],
        }
    }

    fn valid_sections() -> [MetalEvaluationProgramSectionDescV1; 5] {
        [
            MetalEvaluationProgramSectionDescV1::new(
                MetalEvaluationProgramSectionKindV1::BaseConsts,
                4,
                0,
                8,
            ),
            MetalEvaluationProgramSectionDescV1::new(
                MetalEvaluationProgramSectionKindV1::ExtConsts,
                16,
                32,
                2,
            ),
            MetalEvaluationProgramSectionDescV1::new(
                MetalEvaluationProgramSectionKindV1::BaseInsts,
                16,
                64,
                6,
            ),
            MetalEvaluationProgramSectionDescV1::new(
                MetalEvaluationProgramSectionKindV1::ExtInsts,
                20,
                160,
                3,
            ),
            MetalEvaluationProgramSectionDescV1::new(
                MetalEvaluationProgramSectionKindV1::ConstraintRoots,
                4,
                220,
                4,
            ),
        ]
    }

    #[test]
    fn header_layout_matches_v1_contract() {
        assert_eq!(size_of::<MetalEvaluationProgramHeaderV1>(), 96);
        assert_eq!(align_of::<MetalEvaluationProgramHeaderV1>(), 8);
        assert_eq!(offset_of!(MetalEvaluationProgramHeaderV1, magic), 0);
        assert_eq!(offset_of!(MetalEvaluationProgramHeaderV1, abi_major), 4);
        assert_eq!(
            offset_of!(MetalEvaluationProgramHeaderV1, semantic_hash),
            16
        );
        assert_eq!(
            offset_of!(MetalEvaluationProgramHeaderV1, max_base_regs),
            48
        );
        assert_eq!(offset_of!(MetalEvaluationProgramHeaderV1, reserved), 60);
    }

    #[test]
    fn section_descriptor_layout_matches_v1_contract() {
        assert_eq!(size_of::<MetalEvaluationProgramSectionDescV1>(), 24);
        assert_eq!(align_of::<MetalEvaluationProgramSectionDescV1>(), 8);
        assert_eq!(offset_of!(MetalEvaluationProgramSectionDescV1, kind), 0);
        assert_eq!(
            offset_of!(MetalEvaluationProgramSectionDescV1, offset_bytes),
            8
        );
        assert_eq!(offset_of!(MetalEvaluationProgramSectionDescV1, count), 16);
    }

    #[test]
    fn semantic_hash_is_deterministic_for_identical_semantic_chunks() {
        let a = metal_evaluation_program_semantic_hash_v1(&[b"base", b"ext", &[1, 2, 3]]);
        let b = metal_evaluation_program_semantic_hash_v1(&[b"base", b"ext", &[1, 2, 3]]);
        let c = metal_evaluation_program_semantic_hash_v1(&[b"base", b"ext", &[1, 2, 4]]);

        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn validator_accepts_minimal_valid_program() {
        validate_metal_evaluation_program_v1(
            valid_header(),
            &valid_sections(),
            236,
            MetalEvaluationProgramBudgetV1::new(32, 16),
        )
        .unwrap();
    }

    #[test]
    fn validator_rejects_missing_prefinalized_flag() {
        let mut header = valid_header();
        header.flags = 0;

        let error = validate_metal_evaluation_program_v1(
            header,
            &valid_sections(),
            236,
            MetalEvaluationProgramBudgetV1::new(32, 16),
        )
        .unwrap_err();

        assert_eq!(
            error,
            MetalEvaluationProgramValidationError::MissingPrefinalizedLogupFlag
        );
    }

    #[test]
    fn validator_rejects_missing_required_section() {
        let mut header = valid_header();
        header.n_sections = 4;
        let sections = [
            MetalEvaluationProgramSectionDescV1::new(
                MetalEvaluationProgramSectionKindV1::BaseConsts,
                4,
                0,
                8,
            ),
            MetalEvaluationProgramSectionDescV1::new(
                MetalEvaluationProgramSectionKindV1::ExtConsts,
                16,
                32,
                2,
            ),
            MetalEvaluationProgramSectionDescV1::new(
                MetalEvaluationProgramSectionKindV1::BaseInsts,
                16,
                64,
                6,
            ),
            MetalEvaluationProgramSectionDescV1::new(
                MetalEvaluationProgramSectionKindV1::ExtInsts,
                20,
                160,
                3,
            ),
        ];

        let error = validate_metal_evaluation_program_v1(
            header,
            &sections,
            236,
            MetalEvaluationProgramBudgetV1::new(32, 16),
        )
        .unwrap_err();

        assert_eq!(
            error,
            MetalEvaluationProgramValidationError::MissingRequiredSection {
                kind: MetalEvaluationProgramSectionKindV1::ConstraintRoots,
            }
        );
    }

    #[test]
    fn validator_rejects_out_of_bounds_section() {
        let mut sections = valid_sections();
        sections[3] = MetalEvaluationProgramSectionDescV1::new(
            MetalEvaluationProgramSectionKindV1::ExtInsts,
            20,
            220,
            3,
        );

        let error = validate_metal_evaluation_program_v1(
            valid_header(),
            &sections,
            236,
            MetalEvaluationProgramBudgetV1::new(32, 16),
        )
        .unwrap_err();

        assert_eq!(
            error,
            MetalEvaluationProgramValidationError::SectionOutOfBounds {
                kind: MetalEvaluationProgramSectionKindV1::ExtInsts,
                end_offset: 280,
                payload_len_bytes: 236,
            }
        );
    }

    #[test]
    fn validator_rejects_register_budget_exhaustion() {
        let error = validate_metal_evaluation_program_v1(
            valid_header(),
            &valid_sections(),
            236,
            MetalEvaluationProgramBudgetV1::new(8, 16),
        )
        .unwrap_err();

        assert_eq!(
            error,
            MetalEvaluationProgramValidationError::BaseRegisterBudgetExceeded {
                required: 16,
                supported: 8,
            }
        );
    }

    #[test]
    fn wide_fibonacci_lowering_builds_dense_v1_program() {
        let program =
            lower_wide_fibonacci_evaluation_program_v1(MetalEvaluationProgramSpecializationV1 {
                log_n_rows: 20,
                n_columns: 100,
            })
            .unwrap();

        assert_eq!(program.header().n_interactions, 2);
        assert_eq!(program.header().n_constraints, 98);
        assert_eq!(program.header().max_base_regs, 1 + 98 * 7);
        assert_eq!(program.header().max_ext_regs, 98);
        assert!(program.base_consts().is_empty());
        assert!(program.ext_consts().is_empty());
        assert_eq!(program.base_insts().len(), 1 + 98 * 7);
        assert_eq!(program.ext_insts().len(), 98);
        assert_eq!(program.constraint_roots().len(), 98);

        let first_const = &program.base_insts()[0];
        assert_eq!(
            first_const.op,
            MetalEvaluationProgramBaseOpcodeV1::Const as u8
        );
        assert_eq!(first_const.a, 0);

        let first_trace = &program.base_insts()[1];
        assert_eq!(
            first_trace.op,
            MetalEvaluationProgramBaseOpcodeV1::TraceCol as u8
        );
        assert_eq!(first_trace.interaction, 1);
        assert_eq!(first_trace.a, 0);
        assert_eq!(first_trace.imm, 0);

        program
            .validate(MetalEvaluationProgramBudgetV1::new(2048, 512))
            .unwrap();
    }

    #[test]
    fn wide_fibonacci_lowering_rejects_too_few_columns() {
        assert_eq!(
            lower_wide_fibonacci_evaluation_program_v1(MetalEvaluationProgramSpecializationV1 {
                log_n_rows: 4,
                n_columns: 2,
            }),
            Err(
                MetalEvaluationProgramLoweringError::InvalidWideFibonacciColumnCount {
                    n_columns: 2,
                }
            )
        );
    }

    #[test]
    fn registered_lowering_rejects_unsupported_component() {
        assert_eq!(
            lower_registered_metal_evaluation_program_v1(
                "poseidon_example",
                MetalEvaluationProgramSpecializationV1 {
                    log_n_rows: 8,
                    n_columns: 16,
                },
            ),
            Err(MetalEvaluationProgramLoweringError::UnsupportedComponent {
                component_name: "poseidon_example",
            })
        );
    }

    #[test]
    fn virtual_snos_lowering_succeeds_with_valid_columns() {
        let program = lower_registered_metal_evaluation_program_v1(
            "virtual_snos",
            MetalEvaluationProgramSpecializationV1 {
                log_n_rows: 4,
                n_columns: 4,
            },
        )
        .unwrap();

        let header = program.header();
        assert_eq!(header.n_interactions, 2);
        assert_eq!(header.n_base_params, 1);
        assert_eq!(header.n_ext_params, 0);
        assert_eq!(header.n_constraints, 3); // n_columns - 1 = 3
    }

    #[test]
    fn virtual_snos_lowering_rejects_single_column() {
        assert_eq!(
            lower_virtual_snos_evaluation_program_v1(MetalEvaluationProgramSpecializationV1 {
                log_n_rows: 4,
                n_columns: 1,
            }),
            Err(MetalEvaluationProgramLoweringError::InvalidVirtualSnosColumnCount {
                n_columns: 1,
            })
        );
    }

    #[test]
    fn virtual_snos_interpreter_produces_zero_for_satisfied_constraints() {
        let n_rows = 8usize;
        let n_columns = 4u32;
        let param_value = BaseField::from_u32_unchecked(5);
        // Build columns where col_i + col_{i+1} = param_value for all rows
        let columns: Vec<Vec<BaseField>> = (0..n_columns)
            .map(|i| {
                (0..n_rows)
                    .map(|_| {
                        if i % 2 == 0 {
                            BaseField::from_u32_unchecked(2)
                        } else {
                            BaseField::from_u32_unchecked(3) // 2 + 3 = 5
                        }
                    })
                    .collect()
            })
            .collect();
        let trace_columns = columns.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let trace_interactions = [&[][..], trace_columns.as_slice()];
        let n_constraints = (n_columns - 1) as usize;
        let random_coeff_powers: Vec<SecureField> = (0..n_constraints)
            .map(|i| SecureField::from(BaseField::from_u32_unchecked((i + 1) as u32)))
            .collect();
        let program =
            lower_virtual_snos_evaluation_program_v1(MetalEvaluationProgramSpecializationV1 {
                log_n_rows: 3,
                n_columns,
            })
            .unwrap();

        let interpreted = interpret_metal_evaluation_program_v1(
            &program,
            MetalEvaluationProgramRuntimeInputsV1 {
                trace: MetalEvaluationProgramTraceViewV1 {
                    trace_interactions: &trace_interactions,
                    preprocessed_columns: &[],
                },
                base_params: &[param_value],
                ext_params: &[],
                random_coeff_powers: &random_coeff_powers,
            },
        )
        .unwrap();

        // All constraints satisfied → all residues should be zero
        for (i, residue) in interpreted.iter().enumerate() {
            assert_eq!(
                *residue,
                SecureField::from_u32_unchecked(0, 0, 0, 0),
                "residue at row {i} should be zero for satisfied constraints"
            );
        }
    }

    #[test]
    fn wide_fibonacci_interpreter_matches_manual_row_residues() {
        let columns = wide_fibonacci_trace(8, 6);
        let trace_columns = columns.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let trace_interactions = [&[][..], trace_columns.as_slice()];
        let random_coeff_powers = (0..4)
            .map(|i| SecureField::from(BaseField::from_u32_unchecked((i + 1) as u32)))
            .collect::<Vec<_>>();
        let program =
            lower_wide_fibonacci_evaluation_program_v1(MetalEvaluationProgramSpecializationV1 {
                log_n_rows: 3,
                n_columns: 6,
            })
            .unwrap();

        let interpreted = interpret_metal_evaluation_program_v1(
            &program,
            MetalEvaluationProgramRuntimeInputsV1 {
                trace: MetalEvaluationProgramTraceViewV1 {
                    trace_interactions: &trace_interactions,
                    preprocessed_columns: &[],
                },
                base_params: &[],
                ext_params: &[],
                random_coeff_powers: &random_coeff_powers,
            },
        )
        .unwrap();

        assert_eq!(
            interpreted,
            wide_fibonacci_row_residues(&columns, &random_coeff_powers)
        );
        assert!(interpreted.iter().all(SecureField::is_zero));
    }

    #[test]
    fn wide_fibonacci_interpreter_detects_tampered_trace_rows() {
        let mut columns = wide_fibonacci_trace(8, 6);
        columns[4][3] += BaseField::from_u32_unchecked(7);
        let trace_columns = columns.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let trace_interactions = [&[][..], trace_columns.as_slice()];
        let random_coeff_powers = (0..4)
            .map(|i| SecureField::from(BaseField::from_u32_unchecked((i + 3) as u32)))
            .collect::<Vec<_>>();
        let program =
            lower_wide_fibonacci_evaluation_program_v1(MetalEvaluationProgramSpecializationV1 {
                log_n_rows: 3,
                n_columns: 6,
            })
            .unwrap();

        let interpreted = interpret_metal_evaluation_program_v1(
            &program,
            MetalEvaluationProgramRuntimeInputsV1 {
                trace: MetalEvaluationProgramTraceViewV1 {
                    trace_interactions: &trace_interactions,
                    preprocessed_columns: &[],
                },
                base_params: &[],
                ext_params: &[],
                random_coeff_powers: &random_coeff_powers,
            },
        )
        .unwrap();

        assert_eq!(
            interpreted,
            wide_fibonacci_row_residues(&columns, &random_coeff_powers)
        );
        assert!(interpreted.iter().any(|value| !value.is_zero()));
    }

    #[test]
    fn interpreter_rejects_non_zero_trace_offsets_for_v1_reference_lane() {
        let mut program =
            lower_wide_fibonacci_evaluation_program_v1(MetalEvaluationProgramSpecializationV1 {
                log_n_rows: 3,
                n_columns: 3,
            })
            .unwrap();
        // Slot 0 is the explicit zero constant for SecureCol padding. Mutate
        // the first trace read so the interpreter must reject nonzero offsets.
        program.base_insts[1].imm = 1;
        let columns = wide_fibonacci_trace(8, 3);
        let trace_columns = columns.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let trace_interactions = [&[][..], trace_columns.as_slice()];
        let error = interpret_metal_evaluation_program_v1(
            &program,
            MetalEvaluationProgramRuntimeInputsV1 {
                trace: MetalEvaluationProgramTraceViewV1 {
                    trace_interactions: &trace_interactions,
                    preprocessed_columns: &[],
                },
                base_params: &[],
                ext_params: &[],
                random_coeff_powers: &[SecureField::from(BaseField::from_u32_unchecked(1))],
            },
        )
        .unwrap_err();

        assert_eq!(
            error,
            MetalEvaluationProgramInterpreterError::NonZeroTraceOffsetUnsupported { offset: 1 }
        );
    }

    #[test]
    fn wide_fibonacci_device_interpreter_matches_reference_lane() {
        if !matches!(metal_runtime_support(), MetalRuntimeSupport::Available) {
            return;
        }

        let program =
            lower_wide_fibonacci_evaluation_program_v1(MetalEvaluationProgramSpecializationV1 {
                log_n_rows: 3,
                n_columns: 5,
            })
            .unwrap();
        let columns = wide_fibonacci_trace(8, 5);
        let trace_columns = columns.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let trace_interactions = [&[][..], trace_columns.as_slice()];
        let random_coeff_powers = [
            SecureField::from_u32_unchecked(3, 5, 7, 11),
            SecureField::from_u32_unchecked(13, 17, 19, 23),
            SecureField::from_u32_unchecked(29, 31, 37, 41),
        ];

        let runtime = MetalEvaluationProgramRuntimeInputsV1 {
            trace: MetalEvaluationProgramTraceViewV1 {
                trace_interactions: &trace_interactions,
                preprocessed_columns: &[],
            },
            base_params: &[],
            ext_params: &[],
            random_coeff_powers: &random_coeff_powers,
        };
        let interpreted = interpret_metal_evaluation_program_v1(&program, runtime).unwrap();
        let device_interpreted =
            interpret_metal_evaluation_program_v1_on_metal(&program, runtime).unwrap();

        assert_eq!(device_interpreted, interpreted);
    }

    #[test]
    fn selection_defaults_to_generic_metal_interpreter_when_no_overlay_is_registered() {
        let program =
            lower_wide_fibonacci_evaluation_program_v1(MetalEvaluationProgramSpecializationV1 {
                log_n_rows: 3,
                n_columns: 5,
            })
            .unwrap();
        let profile = MetalEvaluationProgramCapabilityProfileV1::new(
            MetalRuntimeSupport::Available,
            STWO_METAL_EVAL_PROGRAM_CAP_BASE_INV_V1
                | STWO_METAL_EVAL_PROGRAM_CAP_EXT_MUL_V1
                | STWO_METAL_EVAL_PROGRAM_CAP_PREFINALIZED_LOGUP_V1,
            1024,
            256,
        );

        assert_eq!(lookup_metal_evaluation_program_overlay_v1(&program, profile), None);
        assert_eq!(
            select_metal_evaluation_program_dispatch_v1(&program, profile).unwrap(),
            MetalEvaluationProgramDispatchKindV1::GenericMetalInterpreter
        );
    }

    #[test]
    fn selection_fails_closed_when_runtime_is_unavailable() {
        let program =
            lower_wide_fibonacci_evaluation_program_v1(MetalEvaluationProgramSpecializationV1 {
                log_n_rows: 3,
                n_columns: 5,
            })
            .unwrap();
        let profile = MetalEvaluationProgramCapabilityProfileV1::new(
            MetalRuntimeSupport::DisabledByConfiguration,
            STWO_METAL_EVAL_PROGRAM_CAP_BASE_INV_V1
                | STWO_METAL_EVAL_PROGRAM_CAP_EXT_MUL_V1
                | STWO_METAL_EVAL_PROGRAM_CAP_PREFINALIZED_LOGUP_V1,
            1024,
            256,
        );

        let error = select_metal_evaluation_program_dispatch_v1(&program, profile).unwrap_err();

        assert_eq!(
            error,
            super::MetalEvaluationProgramExecutionError::RuntimeUnavailable(
                MetalRuntimeSupport::DisabledByConfiguration
            )
        );
    }

    #[test]
    fn selected_execution_reports_dispatch_kind() {
        if !matches!(metal_runtime_support(), MetalRuntimeSupport::Available) {
            return;
        }

        let program =
            lower_wide_fibonacci_evaluation_program_v1(MetalEvaluationProgramSpecializationV1 {
                log_n_rows: 3,
                n_columns: 5,
            })
            .unwrap();
        let columns = wide_fibonacci_trace(8, 5);
        let trace_columns = columns.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let trace_interactions = [&[][..], trace_columns.as_slice()];
        let random_coeff_powers = [
            SecureField::from_u32_unchecked(3, 5, 7, 11),
            SecureField::from_u32_unchecked(13, 17, 19, 23),
            SecureField::from_u32_unchecked(29, 31, 37, 41),
        ];
        let runtime = MetalEvaluationProgramRuntimeInputsV1 {
            trace: MetalEvaluationProgramTraceViewV1 {
                trace_interactions: &trace_interactions,
                preprocessed_columns: &[],
            },
            base_params: &[],
            ext_params: &[],
            random_coeff_powers: &random_coeff_powers,
        };

        let (_, dispatch) = execute_selected_metal_evaluation_program_v1_on_metal(
            &program,
            runtime,
            MetalEvaluationProgramCapabilityProfileV1::current(),
        )
        .unwrap();

        assert_eq!(dispatch, MetalEvaluationProgramDispatchKindV1::GenericMetalInterpreter);
    }

    #[test]
    fn device_interpreter_rejects_non_zero_trace_offsets_for_v1_reference_lane() {
        if !matches!(metal_runtime_support(), MetalRuntimeSupport::Available) {
            return;
        }

        let mut program =
            lower_wide_fibonacci_evaluation_program_v1(MetalEvaluationProgramSpecializationV1 {
                log_n_rows: 3,
                n_columns: 3,
            })
            .unwrap();
        program.base_insts[1].imm = 1;
        let columns = wide_fibonacci_trace(8, 3);
        let trace_columns = columns.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let trace_interactions = [&[][..], trace_columns.as_slice()];

        let error = interpret_metal_evaluation_program_v1_on_metal(
            &program,
            MetalEvaluationProgramRuntimeInputsV1 {
                trace: MetalEvaluationProgramTraceViewV1 {
                    trace_interactions: &trace_interactions,
                    preprocessed_columns: &[],
                },
                base_params: &[],
                ext_params: &[],
                random_coeff_powers: &[SecureField::from(BaseField::from_u32_unchecked(1))],
            },
        )
        .unwrap_err();

        assert_eq!(
            error,
            MetalEvaluationProgramDeviceInterpreterError::UnsupportedNonZeroTraceOffset {
                offset: 1
            }
        );
    }

    #[test]
    fn eval_program_abi_layout_v1_passes() {
        validate_eval_program_abi_layout_v1().unwrap();
    }
}
