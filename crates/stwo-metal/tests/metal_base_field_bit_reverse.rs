#![cfg(all(feature = "prover", target_os = "macos", target_arch = "aarch64"))]

use stwo::core::fields::m31::BaseField;
use stwo::prover::backend::{Column, ColumnOps, CpuBackend};
use stwo_metal::{metal_runtime_support, MetalBackend, MetalBaseFieldVec, MetalRuntimeSupport};

#[test]
fn metal_base_field_bit_reverse_matches_cpu_backend() {
    assert_eq!(
        metal_runtime_support(),
        MetalRuntimeSupport::Available,
        "Metal runtime must be available for the native Apple Silicon parity test"
    );

    let size = 1 << 10;
    let values = (0..size as u32).map(BaseField::from).collect::<Vec<_>>();
    let mut expected = values.clone();
    CpuBackend::bit_reverse_column(&mut expected);

    let mut actual = MetalBaseFieldVec::from_vec(values);
    <MetalBackend as ColumnOps<BaseField>>::bit_reverse_column(&mut actual);

    assert_eq!(actual.to_cpu(), expected);
}

#[test]
fn metal_base_field_column_set_round_trips_host_values() {
    assert_eq!(
        metal_runtime_support(),
        MetalRuntimeSupport::Available,
        "Metal runtime must be available for the native Apple Silicon parity test"
    );

    let mut column = MetalBaseFieldVec::from_vec((0..32u32).map(BaseField::from).collect());
    column.set(7, BaseField::from(999u32));
    column.set(19, BaseField::from(12345u32));

    let values = column.to_cpu();
    assert_eq!(values[7], BaseField::from(999u32));
    assert_eq!(values[19], BaseField::from(12345u32));
}
