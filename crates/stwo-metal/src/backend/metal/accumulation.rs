use stwo::core::fields::qm31::SecureField;
use stwo::prover::backend::CpuBackend;
use stwo::prover::secure_column::SecureColumnByCoords;
use stwo::prover::AccumulationOps;

use super::MetalBackend;
use crate::stwo_metal::base_field_vec::BaseFieldVec;

pub(crate) fn metal_secure_column_from_cpu(
    column: SecureColumnByCoords<CpuBackend>,
) -> SecureColumnByCoords<MetalBackend> {
    SecureColumnByCoords {
        columns: column.columns.map(BaseFieldVec::from_vec),
    }
}

impl AccumulationOps for MetalBackend {
    fn accumulate(column: &mut SecureColumnByCoords<Self>, other: &SecureColumnByCoords<Self>) {
        let mut cpu_column = column.to_cpu();
        let cpu_other = other.to_cpu();
        CpuBackend::accumulate(&mut cpu_column, &cpu_other);
        *column = metal_secure_column_from_cpu(cpu_column);
    }

    fn generate_secure_powers(felt: SecureField, n_powers: usize) -> Vec<SecureField> {
        CpuBackend::generate_secure_powers(felt, n_powers)
    }

    fn lift_and_accumulate(
        cols: Vec<SecureColumnByCoords<Self>>,
    ) -> Option<SecureColumnByCoords<Self>> {
        let cpu_cols = cols.into_iter().map(|column| column.to_cpu()).collect();
        CpuBackend::lift_and_accumulate(cpu_cols).map(metal_secure_column_from_cpu)
    }
}
