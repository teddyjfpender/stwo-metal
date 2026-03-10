#![cfg(feature = "prover")]

use stwo::core::fields::m31::BaseField;
use stwo::core::fields::qm31::SecureField;
use stwo::core::fields::FieldExpOps;
use stwo_metal::{
    accumulate_wide_fibonacci_quotients, metal_runtime_support, MetalBaseFieldVec,
    MetalRuntimeSupport, MetalWideFibonacciQuotientError, MetalWideFibonacciQuotientRequest,
};

fn cpu_wide_fibonacci_quotients(
    trace_evaluations: &[&[BaseField]],
    random_coeff_powers: &[SecureField],
    denominator_inverses: &[BaseField],
    domain_log_size: u32,
) -> Vec<SecureField> {
    let eval_domain_size = trace_evaluations[0].len();
    (0..eval_domain_size)
        .map(|row_index| {
            let mut a = trace_evaluations[0][row_index];
            let mut b = trace_evaluations[1][row_index];
            let mut acc = SecureField::default();
            for (constraint_index, column) in trace_evaluations.iter().skip(2).enumerate() {
                let c = column[row_index];
                let constraint = c - (a.square() + b.square());
                acc += random_coeff_powers[constraint_index] * constraint;
                a = b;
                b = c;
            }
            acc * denominator_inverses[row_index >> domain_log_size]
        })
        .collect()
}

#[test]
fn quotient_request_validates_shape() {
    let trace0 = vec![BaseField::from_u32_unchecked(1); 8];
    let trace1 = vec![BaseField::from_u32_unchecked(2); 8];
    let trace2 = vec![BaseField::from_u32_unchecked(3); 4];
    let trace0 = MetalBaseFieldVec::from_vec(trace0);
    let trace1 = MetalBaseFieldVec::from_vec(trace1);
    let trace2 = MetalBaseFieldVec::from_vec(trace2);
    let refs = vec![&trace0, &trace1, &trace2];

    let error = accumulate_wide_fibonacci_quotients(MetalWideFibonacciQuotientRequest {
        trace_evaluations: &refs,
        random_coeff_powers: &[SecureField::from_u32_unchecked(1, 0, 0, 0)],
        denominator_inverses: &[BaseField::from_u32_unchecked(1)],
        domain_log_size: 2,
        eval_domain_log_size: 3,
    })
    .unwrap_err();

    assert_eq!(
        error,
        MetalWideFibonacciQuotientError::InconsistentTraceColumnLength {
            expected_len: 8,
            actual_len: 4,
            column_index: 2,
        }
    );
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn native_wide_fibonacci_quotients_match_cpu_oracle() {
    assert_eq!(
        metal_runtime_support(),
        MetalRuntimeSupport::Available,
        "Metal runtime must be available for the native Apple Silicon parity test"
    );

    let eval_domain_log_size = 4u32;
    let domain_log_size = 3u32;
    let eval_domain_size = 1usize << eval_domain_log_size;
    let trace0 = (0..eval_domain_size)
        .map(|i| BaseField::from_u32_unchecked((i as u32 % 29) + 3))
        .collect::<Vec<_>>();
    let trace1 = (0..eval_domain_size)
        .map(|i| BaseField::from_u32_unchecked(((i as u32 * 5) % 31) + 4))
        .collect::<Vec<_>>();
    let trace2 = (0..eval_domain_size)
        .map(|i| BaseField::from_u32_unchecked(((i as u32 * 7) % 37) + 5))
        .collect::<Vec<_>>();
    let trace3 = (0..eval_domain_size)
        .map(|i| BaseField::from_u32_unchecked(((i as u32 * 11) % 41) + 6))
        .collect::<Vec<_>>();
    let trace4 = (0..eval_domain_size)
        .map(|i| BaseField::from_u32_unchecked(((i as u32 * 13) % 43) + 7))
        .collect::<Vec<_>>();
    let trace_refs = vec![
        trace0.as_slice(),
        trace1.as_slice(),
        trace2.as_slice(),
        trace3.as_slice(),
        trace4.as_slice(),
    ];
    let metal_trace0 = MetalBaseFieldVec::from_vec(trace0.clone());
    let metal_trace1 = MetalBaseFieldVec::from_vec(trace1.clone());
    let metal_trace2 = MetalBaseFieldVec::from_vec(trace2.clone());
    let metal_trace3 = MetalBaseFieldVec::from_vec(trace3.clone());
    let metal_trace4 = MetalBaseFieldVec::from_vec(trace4.clone());
    let metal_trace_refs = vec![
        &metal_trace0,
        &metal_trace1,
        &metal_trace2,
        &metal_trace3,
        &metal_trace4,
    ];
    let random_coeff_powers = vec![
        SecureField::from_u32_unchecked(3, 5, 7, 11),
        SecureField::from_u32_unchecked(13, 17, 19, 23),
        SecureField::from_u32_unchecked(29, 31, 37, 41),
    ];
    let denominator_inverses = vec![
        BaseField::from_u32_unchecked(5),
        BaseField::from_u32_unchecked(9),
    ];

    let native = accumulate_wide_fibonacci_quotients(MetalWideFibonacciQuotientRequest {
        trace_evaluations: &metal_trace_refs,
        random_coeff_powers: &random_coeff_powers,
        denominator_inverses: &denominator_inverses,
        domain_log_size,
        eval_domain_log_size,
    })
    .expect("wide-fibonacci quotient accumulation should succeed");
    let cpu = cpu_wide_fibonacci_quotients(
        &trace_refs,
        &random_coeff_powers,
        &denominator_inverses,
        domain_log_size,
    );

    assert_eq!(native.len(), eval_domain_size);
    assert_eq!(native.to_vec(), cpu);

    let coordinate_columns = native.to_coordinate_columns();
    for (row_index, expected) in cpu.iter().enumerate() {
        let expected_limbs = expected.to_m31_array();
        for (column, limb) in coordinate_columns.iter().zip(expected_limbs) {
            assert_eq!(column[row_index], limb);
        }
    }
}
