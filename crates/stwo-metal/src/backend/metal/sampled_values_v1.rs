use stwo::core::air::Components;
use stwo::core::circle::CirclePoint;
use stwo::core::fields::qm31::SecureField;
use stwo::core::pcs::TreeVec;
use stwo_metal_sys::metal::{metal_runtime_support, MetalError, MetalRuntimeSupport, U32Buffer};

pub const STWO_METAL_SAMPLED_VALUES_MAGIC_V1: u32 = u32::from_le_bytes(*b"SMS1");
pub const STWO_METAL_SAMPLED_VALUES_ABI_MAJOR_V1: u16 = 1;
pub const STWO_METAL_SAMPLED_VALUES_ABI_MINOR_V1: u16 = 0;

#[repr(C)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct MetalSampledValuesHeaderV1 {
    pub magic: u32,
    pub abi_major: u16,
    pub abi_minor: u16,
    pub n_trees: u32,
    pub n_columns: u32,
    pub n_values: u32,
    pub reserved: [u32; 4],
}

impl MetalSampledValuesHeaderV1 {
    pub const fn new(n_trees: u32, n_columns: u32, n_values: u32) -> Self {
        Self {
            magic: STWO_METAL_SAMPLED_VALUES_MAGIC_V1,
            abi_major: STWO_METAL_SAMPLED_VALUES_ABI_MAJOR_V1,
            abi_minor: STWO_METAL_SAMPLED_VALUES_ABI_MINOR_V1,
            n_trees,
            n_columns,
            n_values,
            reserved: [0; 4],
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct MetalSampledValuesTreeDescV1 {
    pub first_column: u32,
    pub n_columns: u32,
}

impl MetalSampledValuesTreeDescV1 {
    pub const fn new(first_column: u32, n_columns: u32) -> Self {
        Self {
            first_column,
            n_columns,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct MetalSampledValuesColumnDescV1 {
    pub first_value: u32,
    pub n_values: u32,
}

impl MetalSampledValuesColumnDescV1 {
    pub const fn new(first_value: u32, n_values: u32) -> Self {
        Self {
            first_value,
            n_values,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct MetalSecureFieldValueV1 {
    pub limbs: [u32; 4],
}

impl From<SecureField> for MetalSecureFieldValueV1 {
    fn from(value: SecureField) -> Self {
        Self {
            limbs: value.to_m31_array().map(|limb| limb.0),
        }
    }
}

impl From<MetalSecureFieldValueV1> for SecureField {
    fn from(value: MetalSecureFieldValueV1) -> Self {
        SecureField::from_u32_unchecked(
            value.limbs[0],
            value.limbs[1],
            value.limbs[2],
            value.limbs[3],
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OwnedMetalSampledValuesV1 {
    header: MetalSampledValuesHeaderV1,
    tree_descs: Vec<MetalSampledValuesTreeDescV1>,
    column_descs: Vec<MetalSampledValuesColumnDescV1>,
    values: Vec<MetalSecureFieldValueV1>,
}

impl OwnedMetalSampledValuesV1 {
    pub fn header(&self) -> MetalSampledValuesHeaderV1 {
        self.header
    }

    pub fn tree_descs(&self) -> &[MetalSampledValuesTreeDescV1] {
        &self.tree_descs
    }

    pub fn column_descs(&self) -> &[MetalSampledValuesColumnDescV1] {
        &self.column_descs
    }

    pub fn values(&self) -> &[MetalSecureFieldValueV1] {
        &self.values
    }

    pub fn validate(&self) -> Result<(), MetalSampledValuesValidationError> {
        if self.header.magic != STWO_METAL_SAMPLED_VALUES_MAGIC_V1 {
            return Err(MetalSampledValuesValidationError::MagicMismatch {
                actual: self.header.magic,
            });
        }
        if self.header.abi_major != STWO_METAL_SAMPLED_VALUES_ABI_MAJOR_V1 {
            return Err(MetalSampledValuesValidationError::UnsupportedAbiMajor {
                actual: self.header.abi_major,
            });
        }
        if self.tree_descs.len() != self.header.n_trees as usize {
            return Err(MetalSampledValuesValidationError::TreeCountMismatch {
                expected: self.header.n_trees as usize,
                actual: self.tree_descs.len(),
            });
        }
        if self.column_descs.len() != self.header.n_columns as usize {
            return Err(MetalSampledValuesValidationError::ColumnCountMismatch {
                expected: self.header.n_columns as usize,
                actual: self.column_descs.len(),
            });
        }
        if self.values.len() != self.header.n_values as usize {
            return Err(MetalSampledValuesValidationError::ValueCountMismatch {
                expected: self.header.n_values as usize,
                actual: self.values.len(),
            });
        }

        for (tree_index, tree) in self.tree_descs.iter().enumerate() {
            let end = tree
                .first_column
                .checked_add(tree.n_columns)
                .ok_or(MetalSampledValuesValidationError::TreeRangeOverflow { tree_index })?;
            if end as usize > self.column_descs.len() {
                return Err(MetalSampledValuesValidationError::TreeColumnRangeOutOfBounds {
                    tree_index,
                    end_column: end as usize,
                    n_columns: self.column_descs.len(),
                });
            }
        }

        for (column_index, column) in self.column_descs.iter().enumerate() {
            let end = column
                .first_value
                .checked_add(column.n_values)
                .ok_or(MetalSampledValuesValidationError::ColumnRangeOverflow {
                    column_index,
                })?;
            if end as usize > self.values.len() {
                return Err(
                    MetalSampledValuesValidationError::ColumnValueRangeOutOfBounds {
                        column_index,
                        end_value: end as usize,
                        n_values: self.values.len(),
                    },
                );
            }
        }

        Ok(())
    }

    pub fn into_tree_vec(self) -> Result<TreeVec<Vec<Vec<SecureField>>>, MetalSampledValuesValidationError> {
        self.validate()?;
        Ok(TreeVec(
            self.tree_descs
                .iter()
                .map(|tree| {
                    self.column_descs
                        [tree.first_column as usize..(tree.first_column + tree.n_columns) as usize]
                        .iter()
                        .map(|column| {
                            self.values[column.first_value as usize
                                ..(column.first_value + column.n_values) as usize]
                                .iter()
                                .copied()
                                .map(SecureField::from)
                                .collect()
                        })
                        .collect()
                })
                .collect(),
        ))
    }

    fn validate_wide_fibonacci_post_composition_shape(
        &self,
    ) -> Result<WideFibonacciPostCompositionShapeV1, MetalSampledValuesExecutionError> {
        self.validate()
            .map_err(|source| MetalSampledValuesExecutionError::Validation { source })?;

        let Some(last_tree) = self.tree_descs.last().copied() else {
            return Err(MetalSampledValuesExecutionError::UnsupportedShape {
                message: "sampled-values ABI requires at least one tree".to_owned(),
            });
        };
        if last_tree.n_columns != 8 {
            return Err(MetalSampledValuesExecutionError::UnsupportedShape {
                message: format!(
                    "wide-fibonacci post-composition lane expects 8 columns in the last tree, got {}",
                    last_tree.n_columns
                ),
            });
        }
        let first_column = last_tree.first_column as usize;
        for (offset, column) in self.column_descs[first_column..first_column + last_tree.n_columns as usize]
            .iter()
            .enumerate()
        {
            if column.n_values != 1 {
                return Err(MetalSampledValuesExecutionError::UnsupportedShape {
                    message: format!(
                        "wide-fibonacci post-composition lane expects exactly one sampled value per final-tree column; column {} had {}",
                        first_column + offset,
                        column.n_values
                    ),
                });
            }
        }

        Ok(WideFibonacciPostCompositionShapeV1 { first_column })
    }

    fn last_tree_wide_fibonacci_values(
        &self,
        shape: WideFibonacciPostCompositionShapeV1,
    ) -> [SecureField; 8] {
        let mut values = [SecureField::from_u32_unchecked(0, 0, 0, 0); 8];
        for (index, slot) in values.iter_mut().enumerate() {
            let desc = self.column_descs[shape.first_column + index];
            *slot = self.values[desc.first_value as usize].into();
        }
        values
    }

    fn tree_desc_words(&self) -> Vec<u32> {
        self.tree_descs
            .iter()
            .flat_map(|desc| [desc.first_column, desc.n_columns])
            .collect()
    }

    fn column_desc_words(&self) -> Vec<u32> {
        self.column_descs
            .iter()
            .flat_map(|desc| [desc.first_value, desc.n_values])
            .collect()
    }

    fn value_words(&self) -> Vec<u32> {
        self.values
            .iter()
            .flat_map(|value| value.limbs)
            .collect()
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum MetalSampledValuesLoweringError {
    TreeCountOverflow,
    ColumnCountOverflow,
    ValueCountOverflow,
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

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum MetalSampledValuesValidationError {
    MagicMismatch { actual: u32 },
    UnsupportedAbiMajor { actual: u16 },
    TreeCountMismatch { expected: usize, actual: usize },
    ColumnCountMismatch { expected: usize, actual: usize },
    ValueCountMismatch { expected: usize, actual: usize },
    TreeRangeOverflow { tree_index: usize },
    TreeColumnRangeOutOfBounds {
        tree_index: usize,
        end_column: usize,
        n_columns: usize,
    },
    ColumnRangeOverflow { column_index: usize },
    ColumnValueRangeOutOfBounds {
        column_index: usize,
        end_value: usize,
        n_values: usize,
    },
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum MetalSampledValuesInterpreterError {
    Validation { source: MetalSampledValuesValidationError },
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum MetalSampledValuesDispatchKindV1 {
    ReferenceInterpreter,
    GeneratedOverlayWideFibonacci,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MetalSampledValuesExecutionError {
    Validation { source: MetalSampledValuesValidationError },
    UnsupportedShape { message: String },
    MetalRuntimeUnavailable { support: MetalRuntimeSupport },
    Metal { source: MetalError },
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
struct WideFibonacciPostCompositionShapeV1 {
    first_column: usize,
}

/// Validate ABI layout of all `#[repr(C)]` host/device boundary records used
/// by V1 sampled-values. Returns `Ok(())` if all sizes, alignments, and field
/// offsets match the contract expected by Metal shader code.
pub fn validate_sampled_values_abi_layout_v1() -> Result<(), MetalSampledValuesLoweringError> {
    use core::mem::{align_of, offset_of, size_of};

    // MetalSampledValuesHeaderV1: 36 bytes, 4-byte align
    if size_of::<MetalSampledValuesHeaderV1>() != 36 {
        return Err(MetalSampledValuesLoweringError::AbiLayoutMismatch {
            record: "MetalSampledValuesHeaderV1",
            expected_size: 36,
            actual_size: size_of::<MetalSampledValuesHeaderV1>(),
        });
    }
    if align_of::<MetalSampledValuesHeaderV1>() != 4 {
        return Err(MetalSampledValuesLoweringError::AbiAlignmentMismatch {
            record: "MetalSampledValuesHeaderV1",
            expected_align: 4,
            actual_align: align_of::<MetalSampledValuesHeaderV1>(),
        });
    }
    if offset_of!(MetalSampledValuesHeaderV1, magic) != 0
        || offset_of!(MetalSampledValuesHeaderV1, abi_major) != 4
        || offset_of!(MetalSampledValuesHeaderV1, abi_minor) != 6
        || offset_of!(MetalSampledValuesHeaderV1, n_trees) != 8
        || offset_of!(MetalSampledValuesHeaderV1, n_columns) != 12
        || offset_of!(MetalSampledValuesHeaderV1, n_values) != 16
        || offset_of!(MetalSampledValuesHeaderV1, reserved) != 20
    {
        return Err(MetalSampledValuesLoweringError::AbiFieldOffsetMismatch {
            record: "MetalSampledValuesHeaderV1",
        });
    }

    // MetalSampledValuesTreeDescV1: 8 bytes, 4-byte align
    if size_of::<MetalSampledValuesTreeDescV1>() != 8 {
        return Err(MetalSampledValuesLoweringError::AbiLayoutMismatch {
            record: "MetalSampledValuesTreeDescV1",
            expected_size: 8,
            actual_size: size_of::<MetalSampledValuesTreeDescV1>(),
        });
    }
    if offset_of!(MetalSampledValuesTreeDescV1, first_column) != 0
        || offset_of!(MetalSampledValuesTreeDescV1, n_columns) != 4
    {
        return Err(MetalSampledValuesLoweringError::AbiFieldOffsetMismatch {
            record: "MetalSampledValuesTreeDescV1",
        });
    }

    // MetalSampledValuesColumnDescV1: 8 bytes, 4-byte align
    if size_of::<MetalSampledValuesColumnDescV1>() != 8 {
        return Err(MetalSampledValuesLoweringError::AbiLayoutMismatch {
            record: "MetalSampledValuesColumnDescV1",
            expected_size: 8,
            actual_size: size_of::<MetalSampledValuesColumnDescV1>(),
        });
    }
    if offset_of!(MetalSampledValuesColumnDescV1, first_value) != 0
        || offset_of!(MetalSampledValuesColumnDescV1, n_values) != 4
    {
        return Err(MetalSampledValuesLoweringError::AbiFieldOffsetMismatch {
            record: "MetalSampledValuesColumnDescV1",
        });
    }

    // MetalSecureFieldValueV1: 16 bytes, 4-byte align
    if size_of::<MetalSecureFieldValueV1>() != 16 {
        return Err(MetalSampledValuesLoweringError::AbiLayoutMismatch {
            record: "MetalSecureFieldValueV1",
            expected_size: 16,
            actual_size: size_of::<MetalSecureFieldValueV1>(),
        });
    }
    if offset_of!(MetalSecureFieldValueV1, limbs) != 0 {
        return Err(MetalSampledValuesLoweringError::AbiFieldOffsetMismatch {
            record: "MetalSecureFieldValueV1",
        });
    }

    Ok(())
}

pub fn lower_metal_sampled_values_v1(
    sampled_values: &TreeVec<Vec<Vec<SecureField>>>,
) -> Result<OwnedMetalSampledValuesV1, MetalSampledValuesLoweringError> {
    validate_sampled_values_abi_layout_v1()?;
    let mut tree_descs = Vec::with_capacity(sampled_values.0.len());
    let mut column_descs = Vec::new();
    let mut values = Vec::new();
    let mut running_columns = 0u32;
    let mut running_values = 0u32;

    for tree in &sampled_values.0 {
        let first_column = running_columns;
        let n_columns =
            u32::try_from(tree.len()).map_err(|_| MetalSampledValuesLoweringError::ColumnCountOverflow)?;
        for column in tree {
            let first_value = running_values;
            let n_values =
                u32::try_from(column.len()).map_err(|_| MetalSampledValuesLoweringError::ValueCountOverflow)?;
            values.extend(column.iter().copied().map(MetalSecureFieldValueV1::from));
            running_values = running_values
                .checked_add(n_values)
                .ok_or(MetalSampledValuesLoweringError::ValueCountOverflow)?;
            column_descs.push(MetalSampledValuesColumnDescV1::new(first_value, n_values));
        }
        running_columns = running_columns
            .checked_add(n_columns)
            .ok_or(MetalSampledValuesLoweringError::ColumnCountOverflow)?;
        tree_descs.push(MetalSampledValuesTreeDescV1::new(first_column, n_columns));
    }

    let n_trees =
        u32::try_from(tree_descs.len()).map_err(|_| MetalSampledValuesLoweringError::TreeCountOverflow)?;
    let n_columns = u32::try_from(column_descs.len())
        .map_err(|_| MetalSampledValuesLoweringError::ColumnCountOverflow)?;
    let n_values =
        u32::try_from(values.len()).map_err(|_| MetalSampledValuesLoweringError::ValueCountOverflow)?;

    Ok(OwnedMetalSampledValuesV1 {
        header: MetalSampledValuesHeaderV1::new(n_trees, n_columns, n_values),
        tree_descs,
        column_descs,
        values,
    })
}

pub fn interpret_metal_sampled_values_v1(
    components: &Components<'_>,
    point: CirclePoint<SecureField>,
    sampled_values: &OwnedMetalSampledValuesV1,
    random_coeff: SecureField,
    max_log_degree_bound: u32,
) -> Result<SecureField, MetalSampledValuesInterpreterError> {
    let sampled_values = sampled_values
        .clone()
        .into_tree_vec()
        .map_err(|source| MetalSampledValuesInterpreterError::Validation { source })?;
    Ok(components.eval_composition_polynomial_at_point(
        point,
        &sampled_values,
        random_coeff,
        max_log_degree_bound,
    ))
}

pub fn interpret_metal_sampled_values_v1_reference(
    point: CirclePoint<SecureField>,
    sampled_values: &OwnedMetalSampledValuesV1,
    max_log_degree_bound: u32,
) -> Result<SecureField, MetalSampledValuesExecutionError> {
    let shape = sampled_values.validate_wide_fibonacci_post_composition_shape()?;
    let values = sampled_values.last_tree_wide_fibonacci_values(shape);
    let (left_values, right_values) = values.split_at(4);
    let left =
        SecureField::from_partial_evals(left_values.try_into().expect("fixed-width split"));
    let right =
        SecureField::from_partial_evals(right_values.try_into().expect("fixed-width split"));
    let x = point.repeated_double(max_log_degree_bound - 1).x;
    Ok(left + x * right)
}

pub fn select_metal_sampled_values_dispatch_v1(
    sampled_values: &OwnedMetalSampledValuesV1,
) -> Result<MetalSampledValuesDispatchKindV1, MetalSampledValuesExecutionError> {
    sampled_values.validate_wide_fibonacci_post_composition_shape()?;
    match metal_runtime_support() {
        MetalRuntimeSupport::Available => {
            Ok(MetalSampledValuesDispatchKindV1::GeneratedOverlayWideFibonacci)
        }
        _ => Ok(MetalSampledValuesDispatchKindV1::ReferenceInterpreter),
    }
}

pub fn execute_selected_metal_sampled_values_v1(
    point: CirclePoint<SecureField>,
    sampled_values: &OwnedMetalSampledValuesV1,
    max_log_degree_bound: u32,
) -> Result<(SecureField, MetalSampledValuesDispatchKindV1), MetalSampledValuesExecutionError> {
    let dispatch = select_metal_sampled_values_dispatch_v1(sampled_values)?;
    let value = match dispatch {
        MetalSampledValuesDispatchKindV1::ReferenceInterpreter => {
            interpret_metal_sampled_values_v1_reference(point, sampled_values, max_log_degree_bound)?
        }
        MetalSampledValuesDispatchKindV1::GeneratedOverlayWideFibonacci => {
            execute_metal_sampled_values_v1_wide_fibonacci(point, sampled_values, max_log_degree_bound)?
        }
    };
    Ok((value, dispatch))
}

fn execute_metal_sampled_values_v1_wide_fibonacci(
    point: CirclePoint<SecureField>,
    sampled_values: &OwnedMetalSampledValuesV1,
    max_log_degree_bound: u32,
) -> Result<SecureField, MetalSampledValuesExecutionError> {
    sampled_values.validate_wide_fibonacci_post_composition_shape()?;
    let tree_descs = U32Buffer::from_slice(&sampled_values.tree_desc_words())
        .map_err(|source| MetalSampledValuesExecutionError::Metal { source })?;
    let column_descs = U32Buffer::from_slice(&sampled_values.column_desc_words())
        .map_err(|source| MetalSampledValuesExecutionError::Metal { source })?;
    let values = U32Buffer::from_slice(&sampled_values.value_words())
        .map_err(|source| MetalSampledValuesExecutionError::Metal { source })?;
    let point_x = point
        .repeated_double(max_log_degree_bound - 1)
        .x
        .to_m31_array()
        .map(|limb| limb.0);
    let point_x = U32Buffer::from_slice(&point_x)
        .map_err(|source| MetalSampledValuesExecutionError::Metal { source })?;
    let dst = U32Buffer::sampled_values_v1_wide_fibonacci_u32x4(
        &tree_descs,
        &column_descs,
        &values,
        sampled_values.header.n_trees,
        &point_x,
    )
    .map_err(|source| MetalSampledValuesExecutionError::Metal { source })?;
    let limbs: [u32; 4] = dst
        .to_vec()
        .map_err(|source| MetalSampledValuesExecutionError::Metal { source })?
        .try_into()
        .expect("sampled-values V1 wide-fibonacci lane must return one secure field");
    Ok(SecureField::from_u32_unchecked(
        limbs[0], limbs[1], limbs[2], limbs[3],
    ))
}

#[cfg(test)]
mod tests {
    use stwo::core::channel::Blake2sChannel;
    use stwo::core::fields::qm31::SecureField;
    use stwo::core::fields::m31::BaseField;
    use stwo::core::pcs::TreeVec;

    use super::{
        execute_selected_metal_sampled_values_v1, interpret_metal_sampled_values_v1_reference,
        lower_metal_sampled_values_v1, validate_sampled_values_abi_layout_v1,
        MetalRuntimeSupport, MetalSampledValuesDispatchKindV1,
    };

    #[test]
    fn sampled_values_v1_roundtrip_preserves_tree_order() {
        let sampled_values = TreeVec(vec![
            vec![vec![]],
            vec![
                vec![SecureField::from_u32_unchecked(1, 2, 3, 4)],
                vec![
                    SecureField::from_u32_unchecked(5, 6, 7, 8),
                    SecureField::from_u32_unchecked(9, 10, 11, 12),
                ],
            ],
            vec![vec![SecureField::from_u32_unchecked(13, 14, 15, 16)]],
        ]);

        let lowered = lower_metal_sampled_values_v1(&sampled_values).unwrap();
        let restored = lowered.clone().into_tree_vec().unwrap();

        assert_eq!(restored.0, sampled_values.0);
        assert_eq!(lowered.header().magic, super::STWO_METAL_SAMPLED_VALUES_MAGIC_V1);
    }

    #[test]
    fn sampled_values_v1_validation_rejects_count_drift() {
        let sampled_values = TreeVec(vec![vec![vec![SecureField::from(BaseField::from_u32_unchecked(
            1,
        ))]]]);
        let mut lowered = lower_metal_sampled_values_v1(&sampled_values).unwrap();
        lowered.header.n_values += 1;
        assert_eq!(
            lowered.validate(),
            Err(super::MetalSampledValuesValidationError::ValueCountMismatch {
                expected: 2,
                actual: 1
            })
        );
    }

    #[test]
    fn sampled_values_v1_reference_lane_matches_wide_fibonacci_post_composition_formula() {
        let sampled_values = TreeVec(vec![
            vec![vec![SecureField::from_u32_unchecked(99, 98, 97, 96)]],
            vec![
                vec![SecureField::from_u32_unchecked(1, 2, 3, 4)],
                vec![SecureField::from_u32_unchecked(5, 6, 7, 8)],
                vec![SecureField::from_u32_unchecked(9, 10, 11, 12)],
                vec![SecureField::from_u32_unchecked(13, 14, 15, 16)],
                vec![SecureField::from_u32_unchecked(17, 18, 19, 20)],
                vec![SecureField::from_u32_unchecked(21, 22, 23, 24)],
                vec![SecureField::from_u32_unchecked(25, 26, 27, 28)],
                vec![SecureField::from_u32_unchecked(29, 30, 31, 32)],
            ],
        ]);
        let lowered = lower_metal_sampled_values_v1(&sampled_values).unwrap();
        let point = stwo::core::circle::CirclePoint::get_random_point(&mut Blake2sChannel::default());
        let max_log_degree_bound = 6;

        let value =
            interpret_metal_sampled_values_v1_reference(point, &lowered, max_log_degree_bound)
                .unwrap();
        let expected = {
            let left = SecureField::from_partial_evals(
                [
                    SecureField::from_u32_unchecked(1, 2, 3, 4),
                    SecureField::from_u32_unchecked(5, 6, 7, 8),
                    SecureField::from_u32_unchecked(9, 10, 11, 12),
                    SecureField::from_u32_unchecked(13, 14, 15, 16),
                ]
            );
            let right = SecureField::from_partial_evals(
                [
                    SecureField::from_u32_unchecked(17, 18, 19, 20),
                    SecureField::from_u32_unchecked(21, 22, 23, 24),
                    SecureField::from_u32_unchecked(25, 26, 27, 28),
                    SecureField::from_u32_unchecked(29, 30, 31, 32),
                ]
            );
            left + point.repeated_double(max_log_degree_bound - 1).x * right
        };

        assert_eq!(value, expected);
    }

    #[test]
    fn sampled_values_v1_selected_lane_matches_reference() {
        let sampled_values = TreeVec(vec![
            vec![vec![SecureField::from_u32_unchecked(99, 98, 97, 96)]],
            vec![
                vec![SecureField::from_u32_unchecked(1, 2, 3, 4)],
                vec![SecureField::from_u32_unchecked(5, 6, 7, 8)],
                vec![SecureField::from_u32_unchecked(9, 10, 11, 12)],
                vec![SecureField::from_u32_unchecked(13, 14, 15, 16)],
                vec![SecureField::from_u32_unchecked(17, 18, 19, 20)],
                vec![SecureField::from_u32_unchecked(21, 22, 23, 24)],
                vec![SecureField::from_u32_unchecked(25, 26, 27, 28)],
                vec![SecureField::from_u32_unchecked(29, 30, 31, 32)],
            ],
        ]);
        let lowered = lower_metal_sampled_values_v1(&sampled_values).unwrap();
        let point = stwo::core::circle::CirclePoint::get_random_point(&mut Blake2sChannel::default());
        let max_log_degree_bound = 6;

        let reference =
            interpret_metal_sampled_values_v1_reference(point, &lowered, max_log_degree_bound)
                .unwrap();
        let (selected, dispatch) =
            execute_selected_metal_sampled_values_v1(point, &lowered, max_log_degree_bound)
                .unwrap();

        assert_eq!(selected, reference);
        match super::metal_runtime_support() {
            MetalRuntimeSupport::Available => assert_eq!(
                dispatch,
                MetalSampledValuesDispatchKindV1::GeneratedOverlayWideFibonacci
            ),
            _ => assert_eq!(dispatch, MetalSampledValuesDispatchKindV1::ReferenceInterpreter),
        }
    }

    #[test]
    fn sampled_values_abi_layout_v1_passes() {
        validate_sampled_values_abi_layout_v1().unwrap();
    }
}
