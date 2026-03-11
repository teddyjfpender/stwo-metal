#![cfg(feature = "prover")]

use stwo::core::fields::m31::BaseField;
use stwo::core::fields::qm31::SecureField;
use stwo::core::fields::FieldExpOps;
use stwo::core::poly::circle::CanonicCoset;
use stwo::prover::poly::circle::CircleEvaluation;
use stwo::prover::poly::circle::PolyOps;
use stwo::prover::poly::BitReversedOrder;
use stwo_metal::{
    accumulate_wide_fibonacci_quotients, accumulate_wide_fibonacci_quotients_from_batch,
    evaluate_polys_on_domain_batch, metal_runtime_support, MetalBackend, MetalBaseFieldVec,
    MetalRuntimeSupport, MetalWideFibonacciBatchQuotientRequest, MetalWideFibonacciQuotientError,
    MetalWideFibonacciQuotientRequest,
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

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn batched_domain_evaluation_feeds_the_same_wide_fibonacci_quotients() {
    assert_eq!(
        metal_runtime_support(),
        MetalRuntimeSupport::Available,
        "Metal runtime must be available for the native Apple Silicon parity test"
    );

    let trace_domain = CanonicCoset::new(3).circle_domain();
    let eval_domain = CanonicCoset::new(4).circle_domain();
    let trace0 = (0..trace_domain.size())
        .map(|i| BaseField::from_u32_unchecked((i as u32 % 17) + 3))
        .collect::<Vec<_>>();
    let trace1 = (0..trace_domain.size())
        .map(|i| BaseField::from_u32_unchecked(((i as u32 * 3) % 19) + 5))
        .collect::<Vec<_>>();
    let trace2 = (0..trace_domain.size())
        .map(|i| trace0[i].square() + trace1[i].square())
        .collect::<Vec<_>>();
    let trace3 = (0..trace_domain.size())
        .map(|i| trace1[i].square() + trace2[i].square())
        .collect::<Vec<_>>();
    let trace4 = (0..trace_domain.size())
        .map(|i| trace2[i].square() + trace3[i].square())
        .collect::<Vec<_>>();
    let trace_evals = [trace0, trace1, trace2, trace3, trace4]
        .into_iter()
        .map(|column| {
            CircleEvaluation::<MetalBackend, BaseField, BitReversedOrder>::new(
                trace_domain,
                MetalBaseFieldVec::from_vec(column),
            )
        })
        .collect::<Vec<_>>();

    let trace_twiddles = MetalBackend::precompute_twiddles(trace_domain.half_coset);
    let coeffs = trace_evals
        .into_iter()
        .map(|eval| MetalBackend::interpolate(eval, &trace_twiddles))
        .collect::<Vec<_>>();
    let coeff_refs = coeffs.iter().collect::<Vec<_>>();
    let eval_twiddles = MetalBackend::precompute_twiddles(eval_domain.half_coset);
    let batch = evaluate_polys_on_domain_batch(&coeff_refs, eval_domain, &eval_twiddles)
        .expect("batched Metal domain evaluation should succeed");
    let individual_evals = coeffs
        .iter()
        .map(|poly| MetalBackend::evaluate(poly, eval_domain, &eval_twiddles))
        .collect::<Vec<_>>();
    let individual_refs = individual_evals
        .iter()
        .map(|column| &column.values)
        .collect::<Vec<_>>();

    let random_coeff_powers = vec![
        SecureField::from_u32_unchecked(3, 5, 7, 11),
        SecureField::from_u32_unchecked(13, 17, 19, 23),
        SecureField::from_u32_unchecked(29, 31, 37, 41),
    ];
    let denominator_inverses = vec![
        BaseField::from_u32_unchecked(5),
        BaseField::from_u32_unchecked(9),
    ];

    let individual = accumulate_wide_fibonacci_quotients(MetalWideFibonacciQuotientRequest {
        trace_evaluations: &individual_refs,
        random_coeff_powers: &random_coeff_powers,
        denominator_inverses: &denominator_inverses,
        domain_log_size: trace_domain.log_size(),
        eval_domain_log_size: eval_domain.log_size(),
    })
    .expect("individual quotient path should succeed");
    let batched =
        accumulate_wide_fibonacci_quotients_from_batch(MetalWideFibonacciBatchQuotientRequest {
            trace_evaluations: &batch,
            random_coeff_powers: &random_coeff_powers,
            denominator_inverses: &denominator_inverses,
            domain_log_size: trace_domain.log_size(),
            eval_domain_log_size: eval_domain.log_size(),
        })
        .expect("batched quotient path should succeed");

    assert_eq!(batched.to_vec(), individual.to_vec());
}
