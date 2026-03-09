use stwo::core::fields::m31::BaseField;
use stwo::prover::backend::{Column, ColumnOps};

use super::MetalBackend;
use crate::stwo_metal::base_field_vec::BaseFieldVec;

#[cfg(feature = "vendored-upstream-bridge")]
fn split_host_backed_vec<T, V>(values: Vec<T>, from_vec: fn(Vec<T>) -> V) -> (V, V) {
    assert!(
        values.len() % 2 == 0,
        "column split_at_mid requires an even-length column"
    );
    let mid = values.len() / 2;
    let mut values = values;
    let right = values.split_off(mid);
    (from_vec(values), from_vec(right))
}

impl ColumnOps<BaseField> for MetalBackend {
    type Column = BaseFieldVec;

    fn bit_reverse_column(column: &mut Self::Column) {
        let size = column.len();
        assert!(size.is_power_of_two() && size < u32::MAX as usize);
        column.bit_reverse();
    }
}

impl Column<BaseField> for BaseFieldVec {
    fn zeros(len: usize) -> Self {
        Self::new_zeroes(len)
    }

    fn to_cpu(&self) -> Vec<BaseField> {
        self.to_vec()
    }

    fn len(&self) -> usize {
        self.len()
    }

    fn at(&self, index: usize) -> BaseField {
        self.get_data(index)
    }

    fn set(&mut self, index: usize, value: BaseField) {
        self.set_data(index, value);
    }

    #[cfg(feature = "vendored-upstream-bridge")]
    fn split_at_mid(self) -> (Self, Self) {
        split_host_backed_vec(self.to_vec(), BaseFieldVec::from_vec)
    }

    unsafe fn uninitialized(len: usize) -> Self {
        Self::new_uninitialized(len)
    }
}

impl FromIterator<BaseField> for BaseFieldVec {
    fn from_iter<T: IntoIterator<Item = BaseField>>(iter: T) -> Self {
        Self::from_vec(iter.into_iter().collect())
    }
}

impl IntoIterator for BaseFieldVec {
    type Item = BaseField;
    type IntoIter = std::vec::IntoIter<BaseField>;

    fn into_iter(self) -> Self::IntoIter {
        self.to_cpu().into_iter()
    }
}
