#![cfg(all(feature = "prover", target_os = "macos", target_arch = "aarch64"))]

use stwo::core::fields::qm31::SecureField;
use stwo::prover::backend::{Column, ColumnOps, CpuBackend};
use stwo_metal::{metal_runtime_support, MetalBackend, MetalRuntimeSupport, MetalSecureFieldVec};

#[test]
fn metal_secure_field_bit_reverse_matches_cpu_backend() {
    assert_eq!(
        metal_runtime_support(),
        MetalRuntimeSupport::Available,
        "Metal runtime must be available for the native Apple Silicon parity test"
    );

    let size = 1 << 10;
    let values = (0..size as u32)
        .map(|i| SecureField::from_u32_unchecked(4 * i, 4 * i + 1, 4 * i + 2, 4 * i + 3))
        .collect::<Vec<_>>();
    let mut expected = values.clone();
    CpuBackend::bit_reverse_column(&mut expected);

    let mut actual = MetalSecureFieldVec::from_vec(values);
    <MetalBackend as ColumnOps<SecureField>>::bit_reverse_column(&mut actual);

    assert_eq!(actual.to_cpu(), expected);
}

#[test]
fn metal_secure_field_column_set_round_trips_host_values() {
    assert_eq!(
        metal_runtime_support(),
        MetalRuntimeSupport::Available,
        "Metal runtime must be available for the native Apple Silicon parity test"
    );

    let mut column = MetalSecureFieldVec::from_vec(
        (0..32u32)
            .map(|i| SecureField::from_u32_unchecked(4 * i, 4 * i + 1, 4 * i + 2, 4 * i + 3))
            .collect(),
    );
    let updated = SecureField::from_u32_unchecked(91, 92, 93, 94);
    column.set(7, updated);

    let values = column.to_cpu();
    assert_eq!(values[7], updated);
}
