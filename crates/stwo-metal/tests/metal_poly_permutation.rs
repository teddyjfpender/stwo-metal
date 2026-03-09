#![cfg(all(feature = "prover", target_os = "macos", target_arch = "aarch64"))]

use stwo::core::fields::m31::BaseField;
use stwo::core::utils::bit_reverse_coset_to_circle_domain_order;
use stwo::prover::backend::Column;
use stwo_metal::{
    metal_runtime_support, permute_coset_to_circle_domain_bit_reversed, MetalBaseFieldVec,
    MetalRuntimeSupport,
};

#[test]
fn metal_coset_to_circle_domain_bit_reversed_matches_cpu_reference() {
    assert_eq!(
        metal_runtime_support(),
        MetalRuntimeSupport::Available,
        "Metal runtime must be available for the native Apple Silicon parity test"
    );

    let values = (0..1 << 10)
        .map(|i| BaseField::from(i as u32))
        .collect::<Vec<_>>();
    let mut expected = values.clone();
    bit_reverse_coset_to_circle_domain_order(&mut expected);

    let input = MetalBaseFieldVec::from_vec(values);
    let actual = permute_coset_to_circle_domain_bit_reversed(&input);

    assert_eq!(actual.to_cpu(), expected);
}
