pub const STWO_METAL_EVAL_PROGRAM_MAGIC_V1: u32 = u32::from_le_bytes(*b"STP1");
pub const STWO_METAL_EVAL_PROGRAM_ABI_MAJOR_V1: u16 = 1;
pub const STWO_METAL_EVAL_PROGRAM_ABI_MINOR_V1: u16 = 0;

pub const STWO_METAL_EVAL_PROGRAM_FLAG_PREFINALIZED_LOGUP_V1: u32 = 1 << 0;
pub const STWO_METAL_EVAL_PROGRAM_FLAG_DEBUG_PRESENT_V1: u32 = 1 << 1;

pub const STWO_METAL_EVAL_PROGRAM_CAP_BASE_INV_V1: u64 = 1 << 0;
pub const STWO_METAL_EVAL_PROGRAM_CAP_EXT_MUL_V1: u64 = 1 << 1;
pub const STWO_METAL_EVAL_PROGRAM_CAP_PREFINALIZED_LOGUP_V1: u64 = 1 << 2;

pub const STWO_METAL_EVAL_PROGRAM_SECURE_EXT_DEGREE_V1: u32 = 4;

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

#[cfg(test)]
mod tests {
    use core::mem::{align_of, offset_of, size_of};

    use super::{
        metal_evaluation_program_semantic_hash_v1, validate_metal_evaluation_program_v1,
        MetalEvaluationProgramBudgetV1, MetalEvaluationProgramHeaderV1,
        MetalEvaluationProgramSectionDescV1, MetalEvaluationProgramSectionKindV1,
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
}
