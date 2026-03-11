use std::vec::Vec;

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
    UnsupportedComponent {
        component_name: &'static str,
    },
    InvalidWideFibonacciColumnCount {
        n_columns: u32,
    },
    RegisterBudgetOverflow,
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
    InvalidMagic { found: u32 },
    UnsupportedAbiMajor { found: u16 },
    UnsupportedAbiMinor { found: u16 },
    MissingPrefinalizedLogupFlag,
    MissingPrefinalizedLogupCapability,
    UnsupportedSecureExtDegree { found: u32 },
    SectionCountMismatch { header_count: u32, actual_count: usize },
    UnknownSectionKind { raw_kind: u32 },
    DuplicateSection { kind: MetalEvaluationProgramSectionKindV1 },
    MissingRequiredSection { kind: MetalEvaluationProgramSectionKindV1 },
    ZeroSizedSection { kind: MetalEvaluationProgramSectionKindV1 },
    SectionOutOfBounds {
        kind: MetalEvaluationProgramSectionKindV1,
        end_offset: u64,
        payload_len_bytes: u64,
    },
    BaseRegisterBudgetExceeded { required: u32, supported: u32 },
    ExtensionRegisterBudgetExceeded { required: u32, supported: u32 },
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
            return Err(
                MetalEvaluationProgramValidationError::MissingPrefinalizedLogupCapability,
            );
        }
        if self.header.secure_ext_degree != STWO_METAL_EVAL_PROGRAM_SECURE_EXT_DEGREE_V1 {
            return Err(
                MetalEvaluationProgramValidationError::UnsupportedSecureExtDegree {
                    found: self.header.secure_ext_degree,
                },
            );
        }
        if self.header.n_sections as usize != self.sections.len() {
            return Err(MetalEvaluationProgramValidationError::SectionCountMismatch {
                header_count: self.header.n_sections,
                actual_count: self.sections.len(),
            });
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
                return Err(MetalEvaluationProgramValidationError::MissingRequiredSection {
                    kind: required,
                });
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

pub fn lower_registered_metal_evaluation_program_v1(
    component_name: &'static str,
    specialization: MetalEvaluationProgramSpecializationV1,
) -> Result<OwnedMetalEvaluationProgramV1, MetalEvaluationProgramLoweringError> {
    match component_name {
        "fibonacci_example" => lower_wide_fibonacci_evaluation_program_v1(specialization),
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
    let base_regs_required = n_constraints
        .checked_mul(7)
        .ok_or(MetalEvaluationProgramLoweringError::RegisterBudgetOverflow)?;
    let ext_regs_required = n_constraints;

    let mut base_insts = Vec::with_capacity(base_regs_required as usize);
    let mut ext_insts = Vec::with_capacity(ext_regs_required as usize);
    let mut constraint_roots = Vec::with_capacity(n_constraints as usize);

    let mut next_base_reg = 0u16;
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
            0,
            0,
            0,
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

#[cfg(test)]
mod tests {
    use core::mem::{align_of, offset_of, size_of};

    use super::{
        lower_registered_metal_evaluation_program_v1, lower_wide_fibonacci_evaluation_program_v1,
        metal_evaluation_program_semantic_hash_v1, validate_metal_evaluation_program_v1,
        MetalEvaluationProgramBaseOpcodeV1, MetalEvaluationProgramBudgetV1,
        MetalEvaluationProgramHeaderV1, MetalEvaluationProgramLoweringError,
        MetalEvaluationProgramSectionDescV1, MetalEvaluationProgramSectionKindV1,
        MetalEvaluationProgramSpecializationV1,
        MetalEvaluationProgramValidationError, STWO_METAL_EVAL_PROGRAM_ABI_MAJOR_V1,
        STWO_METAL_EVAL_PROGRAM_CAP_BASE_INV_V1, STWO_METAL_EVAL_PROGRAM_CAP_EXT_MUL_V1,
        STWO_METAL_EVAL_PROGRAM_CAP_PREFINALIZED_LOGUP_V1,
        STWO_METAL_EVAL_PROGRAM_FLAG_PREFINALIZED_LOGUP_V1,
        STWO_METAL_EVAL_PROGRAM_MAGIC_V1, STWO_METAL_EVAL_PROGRAM_SECURE_EXT_DEGREE_V1,
    };

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
        assert_eq!(offset_of!(MetalEvaluationProgramHeaderV1, semantic_hash), 16);
        assert_eq!(offset_of!(MetalEvaluationProgramHeaderV1, max_base_regs), 48);
        assert_eq!(offset_of!(MetalEvaluationProgramHeaderV1, reserved), 60);
    }

    #[test]
    fn section_descriptor_layout_matches_v1_contract() {
        assert_eq!(size_of::<MetalEvaluationProgramSectionDescV1>(), 24);
        assert_eq!(align_of::<MetalEvaluationProgramSectionDescV1>(), 8);
        assert_eq!(offset_of!(MetalEvaluationProgramSectionDescV1, kind), 0);
        assert_eq!(offset_of!(MetalEvaluationProgramSectionDescV1, offset_bytes), 8);
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
        let program = lower_wide_fibonacci_evaluation_program_v1(
            MetalEvaluationProgramSpecializationV1 {
                log_n_rows: 20,
                n_columns: 100,
            },
        )
        .unwrap();

        assert_eq!(program.header().n_interactions, 2);
        assert_eq!(program.header().n_constraints, 98);
        assert_eq!(program.header().max_base_regs, 98 * 7);
        assert_eq!(program.header().max_ext_regs, 98);
        assert!(program.base_consts().is_empty());
        assert!(program.ext_consts().is_empty());
        assert_eq!(program.base_insts().len(), 98 * 7);
        assert_eq!(program.ext_insts().len(), 98);
        assert_eq!(program.constraint_roots().len(), 98);

        let first_trace = &program.base_insts()[0];
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
}
