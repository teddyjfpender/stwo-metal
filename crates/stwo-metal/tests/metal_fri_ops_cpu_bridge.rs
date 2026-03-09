#![cfg(all(feature = "prover", target_os = "macos", target_arch = "aarch64"))]

use std::array;

use stwo::core::circle::Coset;
use stwo::core::fields::m31::BaseField;
use stwo::core::fields::qm31::SecureField;
use stwo::core::poly::circle::CanonicCoset;
use stwo::core::poly::line::LineDomain;
use stwo::prover::backend::CpuBackend;
use stwo::prover::fri::FriOps;
use stwo::prover::line::LineEvaluation;
use stwo::prover::poly::circle::{PolyOps, SecureEvaluation};
use stwo::prover::poly::BitReversedOrder;
use stwo::prover::secure_column::SecureColumnByCoords;
use stwo_metal::{metal_runtime_support, MetalBackend, MetalBaseFieldVec, MetalRuntimeSupport};

fn require_metal_runtime() {
    assert_eq!(
        metal_runtime_support(),
        MetalRuntimeSupport::Available,
        "Metal runtime must be available for the native Apple Silicon parity test"
    );
}

fn sf(seed: u32) -> SecureField {
    SecureField::from_u32_unchecked(seed, seed + 1, seed + 2, seed + 3)
}

fn metal_secure_column(values: &[SecureField]) -> SecureColumnByCoords<MetalBackend> {
    let mut columns = array::from_fn(|_| Vec::<BaseField>::with_capacity(values.len()));
    for value in values {
        for (column, coord) in columns.iter_mut().zip(value.to_m31_array()) {
            column.push(coord);
        }
    }
    SecureColumnByCoords {
        columns: columns.map(MetalBaseFieldVec::from_vec),
    }
}

#[test]
fn metal_fri_ops_cpu_bridge_matches_cpu_for_fold_line() {
    require_metal_runtime();

    const LOG_SIZE: u32 = 8;
    let domain = LineDomain::new(Coset::half_odds(LOG_SIZE));
    let alpha = sf(11);
    let values = (0..(1 << LOG_SIZE))
        .map(|i| sf(10 * i as u32 + 1))
        .collect::<Vec<_>>();

    let cpu_eval = LineEvaluation::<CpuBackend>::new(domain, values.clone().into_iter().collect());
    let cpu_twiddles = CpuBackend::precompute_twiddles(domain.coset());
    let expected = CpuBackend::fold_line(&cpu_eval, alpha, &cpu_twiddles, 2);

    let metal_eval = LineEvaluation::<MetalBackend>::new(domain, metal_secure_column(&values));
    let metal_twiddles = MetalBackend::precompute_twiddles(domain.coset());
    let actual = MetalBackend::fold_line(&metal_eval, alpha, &metal_twiddles, 2);

    assert_eq!(actual.values.to_cpu().to_vec(), expected.values.to_vec());
}

#[test]
fn metal_fri_ops_cpu_bridge_matches_cpu_for_circle_fold_accumulation() {
    require_metal_runtime();

    const LOG_SIZE: u32 = 9;
    let domain = CanonicCoset::new(LOG_SIZE).circle_domain();
    let line_domain = LineDomain::new(domain.half_coset);
    let alpha = sf(19);
    let src_values = (0..(1 << LOG_SIZE))
        .map(|i| sf(20 * i as u32 + 3))
        .collect::<Vec<_>>();
    let dst_values = (0..line_domain.size())
        .map(|i| sf(30 * i as u32 + 7))
        .collect::<Vec<_>>();

    let src_cpu = SecureEvaluation::<CpuBackend, BitReversedOrder>::new(
        domain,
        src_values.clone().into_iter().collect(),
    );
    let mut expected =
        LineEvaluation::<CpuBackend>::new(line_domain, dst_values.clone().into_iter().collect());
    let cpu_twiddles = CpuBackend::precompute_twiddles(domain.half_coset);
    CpuBackend::fold_circle_into_line(&mut expected, &src_cpu, alpha, &cpu_twiddles);

    let src_metal = SecureEvaluation::<MetalBackend, BitReversedOrder>::new(
        domain,
        metal_secure_column(&src_values),
    );
    let mut actual =
        LineEvaluation::<MetalBackend>::new(line_domain, metal_secure_column(&dst_values));
    let metal_twiddles = MetalBackend::precompute_twiddles(domain.half_coset);
    MetalBackend::fold_circle_into_line(&mut actual, &src_metal, alpha, &metal_twiddles);

    assert_eq!(actual.values.to_cpu().to_vec(), expected.values.to_vec());
}

#[test]
fn metal_fri_ops_cpu_bridge_matches_cpu_for_decompose() {
    require_metal_runtime();

    const LOG_SIZE: u32 = 8;
    let domain = CanonicCoset::new(LOG_SIZE).circle_domain();
    let values = (0..(1 << LOG_SIZE))
        .map(|i| sf(40 * i as u32 + 5))
        .collect::<Vec<_>>();

    let cpu_eval = SecureEvaluation::<CpuBackend, BitReversedOrder>::new(
        domain,
        values.clone().into_iter().collect(),
    );
    let (expected_eval, expected_lambda) = CpuBackend::decompose(&cpu_eval);

    let metal_eval = SecureEvaluation::<MetalBackend, BitReversedOrder>::new(
        domain,
        metal_secure_column(&values),
    );
    let (actual_eval, actual_lambda) = MetalBackend::decompose(&metal_eval);

    assert_eq!(actual_lambda, expected_lambda);
    assert_eq!(
        actual_eval.values.to_cpu().to_vec(),
        expected_eval.values.to_vec()
    );
}
