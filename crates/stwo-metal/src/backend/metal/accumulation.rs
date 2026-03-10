use std::array;

use ark_std::One;
#[cfg(feature = "parallel")]
use rayon::prelude::*;
use stwo::core::fields::m31::BaseField;
use stwo::core::fields::qm31::SecureField;
use stwo::prover::secure_column::SecureColumnByCoords;
use stwo::prover::AccumulationOps;

use super::MetalBackend;
use crate::stwo_metal::base_field_vec::BaseFieldVec;

fn lifted_index(index: usize, log_ratio: u32) -> usize {
    (index >> (log_ratio + 1) << 1) + (index & 1)
}

fn metal_secure_column_from_coordinate_columns(
    columns: [Vec<BaseField>; 4],
) -> SecureColumnByCoords<MetalBackend> {
    SecureColumnByCoords {
        columns: columns.map(BaseFieldVec::from_vec),
    }
}

pub(crate) fn metal_secure_column_from_values(
    values: Vec<SecureField>,
) -> SecureColumnByCoords<MetalBackend> {
    let len = values.len();
    let mut columns = array::from_fn(|_| BaseFieldVec::new_uninitialized(len));
    for (index, value) in values.into_iter().enumerate() {
        for (column, coord) in columns.iter_mut().zip(value.to_m31_array()) {
            column.set_data(index, coord);
        }
    }
    SecureColumnByCoords { columns }
}

impl AccumulationOps for MetalBackend {
    fn accumulate(column: &mut SecureColumnByCoords<Self>, other: &SecureColumnByCoords<Self>) {
        let updated = array::from_fn(|coord| {
            let lhs = column.columns[coord].host_slice();
            let rhs = other.columns[coord].host_slice();
            #[cfg(not(feature = "parallel"))]
            let values = lhs
                .iter()
                .zip(rhs.iter())
                .map(|(lhs, rhs)| *lhs + *rhs)
                .collect();
            #[cfg(feature = "parallel")]
            let values = (0..lhs.len())
                .into_par_iter()
                .map(|index| lhs[index] + rhs[index])
                .collect();
            values
        });
        *column = metal_secure_column_from_coordinate_columns(updated);
    }

    fn generate_secure_powers(felt: SecureField, n_powers: usize) -> Vec<SecureField> {
        (0..n_powers)
            .scan(SecureField::one(), |acc, _| {
                let res = *acc;
                *acc *= felt;
                Some(res)
            })
            .collect()
    }

    fn lift_and_accumulate(
        cols: Vec<SecureColumnByCoords<Self>>,
    ) -> Option<SecureColumnByCoords<Self>> {
        let mut cols_iter = cols.into_iter();
        let first = cols_iter.next()?;
        const INITIAL_SIZE: usize = 2;
        assert!(
            first.len() >= INITIAL_SIZE,
            "A column must be of length at least {INITIAL_SIZE}."
        );

        let mut curr = SecureColumnByCoords::zeros(INITIAL_SIZE);
        for col in std::iter::once(first).chain(cols_iter) {
            let log_ratio = col.len().ilog2() - curr.len().ilog2();
            let updated = array::from_fn(|coord| {
                let lifted_col: &BaseFieldVec = &curr.columns[coord];
                let current_col: &BaseFieldVec = &col.columns[coord];
                let lifted: &[BaseField] = lifted_col.host_slice();
                let current: &[BaseField] = current_col.host_slice();
                #[cfg(not(feature = "parallel"))]
                let values = (0..col.len())
                    .map(|index| current[index] + lifted[lifted_index(index, log_ratio)])
                    .collect();
                #[cfg(feature = "parallel")]
                let values = (0..col.len())
                    .into_par_iter()
                    .map(|index| current[index] + lifted[lifted_index(index, log_ratio)])
                    .collect();
                values
            });

            curr = metal_secure_column_from_coordinate_columns(updated);
        }
        Some(curr)
    }
}
