#![cfg(all(feature = "prover", target_os = "macos", target_arch = "aarch64"))]

use stwo::core::fields::m31::BaseField;
use stwo::prover::backend::Column;
use stwo_metal::{metal_runtime_support, MetalBaseFieldVec, MetalRuntimeSupport};

fn require_metal_runtime() {
    assert_eq!(
        metal_runtime_support(),
        MetalRuntimeSupport::Available,
        "Metal runtime must be available for the native Apple Silicon cache test"
    );
}

#[test]
fn metal_base_field_vec_host_cache_is_invalidated_by_mutation() {
    require_metal_runtime();

    let mut column = MetalBaseFieldVec::from_vec((0..8u32).map(BaseField::from).collect());
    assert_eq!(
        column.to_vec(),
        (0..8u32).map(BaseField::from).collect::<Vec<_>>()
    );

    column.set_data(3, BaseField::from(99u32));
    assert_eq!(column.to_vec()[3], BaseField::from(99u32));
    assert_eq!(
        column.batch_at(&[0, 3, 7]),
        vec![
            BaseField::from(0u32),
            BaseField::from(99u32),
            BaseField::from(7u32),
        ]
    );

    column.bit_reverse();
    assert_eq!(
        column.to_vec(),
        vec![
            BaseField::from(0u32),
            BaseField::from(4u32),
            BaseField::from(2u32),
            BaseField::from(6u32),
            BaseField::from(1u32),
            BaseField::from(5u32),
            BaseField::from(99u32),
            BaseField::from(7u32),
        ]
    );
}

#[test]
fn metal_base_field_vec_batch_at_without_cache_reads_only_requested_positions() {
    require_metal_runtime();

    let mut column = unsafe { MetalBaseFieldVec::uninitialized(32) };
    for index in 0..32usize {
        column.set(index, BaseField::from((index as u32) * 3));
    }

    assert_eq!(
        column.batch_at(&[1, 7, 18, 31]),
        vec![
            BaseField::from(3u32),
            BaseField::from(21u32),
            BaseField::from(54u32),
            BaseField::from(93u32),
        ]
    );
}
