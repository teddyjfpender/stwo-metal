#![cfg(all(feature = "prover", target_os = "macos", target_arch = "aarch64"))]

use ark_std::Zero;
use stwo::core::fields::m31::BaseField;
use stwo_metal::{metal_runtime_support, MetalBaseFieldVec, MetalRuntimeSupport};

fn require_metal_runtime() {
    assert_eq!(
        metal_runtime_support(),
        MetalRuntimeSupport::Available,
        "Metal runtime must be available for the native Apple Silicon parity test"
    );
}

fn base_values(count: usize, seed: u32) -> Vec<BaseField> {
    (0..count)
        .map(|i| BaseField::from_u32_unchecked(seed + i as u32))
        .collect()
}

fn bit_reverse_index(index: usize, log_len: u32) -> usize {
    let mut reversed = 0usize;
    for bit in 0..log_len {
        reversed = (reversed << 1) | ((index >> bit) & 1usize);
    }
    reversed
}

fn bit_reverse(values: &mut [BaseField]) {
    let log_len = values.len().ilog2();
    let input = values.to_vec();
    for (index, value) in values.iter_mut().enumerate() {
        *value = input[bit_reverse_index(index, log_len)];
    }
}

fn cpu_prefix_sum_bit_rev_circle_domain(mut values: Vec<BaseField>) -> Vec<BaseField> {
    let len = values.len();
    bit_reverse(&mut values);

    let mut coset = vec![BaseField::zero(); len];
    for i in 0..(len / 2) {
        coset[2 * i] = values[i];
        coset[2 * i + 1] = values[len - 1 - i];
    }

    for i in 1..len {
        let prefix = coset[i - 1];
        coset[i] += prefix;
    }

    let mut circle = vec![BaseField::zero(); len];
    let half_len = len / 2;
    for i in 0..half_len {
        circle[i] = coset[i * 2];
    }
    for i in half_len..len {
        let j = i - half_len;
        circle[i] = coset[len - 1 - (j * 2)];
    }

    bit_reverse(&mut circle);
    circle
}

#[test]
fn metal_prefix_sum_native_matches_cpu_reference() {
    require_metal_runtime();

    for log_len in [1u32, 3u32, 6u32] {
        let values = base_values(1usize << log_len, 17);
        let expected = cpu_prefix_sum_bit_rev_circle_domain(values.clone());

        let mut actual = MetalBaseFieldVec::from_vec(values);
        actual.inclusive_prefix_sum_bit_rev_circle_domain();

        assert_eq!(actual.to_vec(), expected);
    }
}
