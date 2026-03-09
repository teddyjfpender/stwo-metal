#![cfg(all(stwo_cuda_link, feature = "vendored-upstream-bridge"))]

use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};
use stwo::core::circle::CirclePoint;
use stwo::core::fields::m31::BaseField;
use stwo::core::fields::qm31::SecureField;
use stwo::core::poly::circle::CanonicCoset;
use stwo::prover::backend::CpuBackend;
use stwo::prover::poly::circle::{CircleCoefficients, PolyOps};
use stwo_metal::{BaseFieldVec, CudaBackend};

#[test]
fn test_barycentric_weights_and_eval_compared_with_cpu() {
    const LOG_SIZE: u32 = 6;

    let mut rng = SmallRng::seed_from_u64(7);
    let coeffs: Vec<BaseField> = (0..(1 << LOG_SIZE)).map(|_| rng.gen()).collect();
    let cpu_poly = CircleCoefficients::<CpuBackend>::new(coeffs.clone());
    let gpu_poly = CircleCoefficients::<CudaBackend>::new(BaseFieldVec::from_vec(coeffs));

    let coset = CanonicCoset::new(LOG_SIZE);
    let domain = coset.circle_domain();
    let cpu_twiddles = CpuBackend::precompute_twiddles(domain.half_coset);
    let gpu_twiddles = CudaBackend::precompute_twiddles(domain.half_coset);

    let cpu_eval = CpuBackend::evaluate_into(
        &cpu_poly,
        domain,
        &cpu_twiddles,
        vec![BaseField::default(); domain.size()],
    );
    let gpu_eval = CudaBackend::evaluate_into(
        &gpu_poly,
        domain,
        &gpu_twiddles,
        BaseFieldVec::new_zeroes(domain.size()),
    );

    let point = CirclePoint::get_point(1 << 18).into_ef::<SecureField>();
    let cpu_weights = CpuBackend::barycentric_weights(coset, point);
    let gpu_weights = CudaBackend::barycentric_weights(coset, point);

    assert_eq!(gpu_weights.to_vec(), cpu_weights.to_vec());

    let cpu_value = CpuBackend::barycentric_eval_at_point(&cpu_eval, &cpu_weights);
    let gpu_value = CudaBackend::barycentric_eval_at_point(&gpu_eval, &gpu_weights);

    assert_eq!(gpu_value, cpu_value);
}
