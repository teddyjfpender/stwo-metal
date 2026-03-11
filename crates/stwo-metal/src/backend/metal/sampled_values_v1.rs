use stwo::core::air::Components;
use stwo::core::circle::CirclePoint;
use stwo::core::fields::qm31::SecureField;
use stwo::core::pcs::TreeVec;

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
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum MetalSampledValuesLoweringError {
    TreeCountOverflow,
    ColumnCountOverflow,
    ValueCountOverflow,
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

pub fn lower_metal_sampled_values_v1(
    sampled_values: &TreeVec<Vec<Vec<SecureField>>>,
) -> Result<OwnedMetalSampledValuesV1, MetalSampledValuesLoweringError> {
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

#[cfg(test)]
mod tests {
    use stwo::core::fields::qm31::SecureField;
    use stwo::core::fields::m31::BaseField;
    use stwo::core::pcs::TreeVec;

    use super::{
        lower_metal_sampled_values_v1,
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
}
