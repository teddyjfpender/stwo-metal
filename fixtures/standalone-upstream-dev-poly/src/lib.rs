#[cfg(test)]
mod tests {
    use rand::rngs::SmallRng;
    use rand::{Rng, SeedableRng};
    use stwo::core::circle::CirclePoint;
    use stwo::core::fields::m31::BaseField;
    use stwo::core::fields::qm31::SecureField;
    use stwo::core::poly::circle::CanonicCoset;
    use stwo::prover::backend::{Col, Column, CpuBackend};
    use stwo::prover::poly::circle::{CircleCoefficients, PolyOps};
    use stwo::prover::poly::BitReversedOrder;
    use stwo_metal::{BaseFieldVec, CudaBackend};

    #[test]
    fn poly_probe_fold_split_and_evaluate_into_match_cpu() {
        const LOG_SIZE: u32 = 10;
        const EXTENDED_LOG_SIZE: u32 = LOG_SIZE + 1;

        let mut rng = SmallRng::seed_from_u64(0);
        let coeffs: Vec<BaseField> = (0..(1 << LOG_SIZE)).map(|_| rng.gen()).collect();

        let cpu_poly = CircleCoefficients::<CpuBackend>::new(coeffs.clone());
        let gpu_poly = CircleCoefficients::<CudaBackend>::new(BaseFieldVec::from_vec(coeffs));

        let domain = CanonicCoset::new(EXTENDED_LOG_SIZE).circle_domain();
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
        assert_eq!(cpu_eval.values, gpu_eval.values.to_cpu());

        let point = CirclePoint::get_point(21903);
        let cpu_fold = CpuBackend::eval_at_point_by_folding(&cpu_eval, point, &cpu_twiddles);
        let gpu_fold = CudaBackend::eval_at_point_by_folding(&gpu_eval, point, &gpu_twiddles);
        assert_eq!(cpu_fold, gpu_fold);

        let (cpu_left, cpu_right) = cpu_poly.clone().split_at_mid();
        let (gpu_left, gpu_right) = gpu_poly.clone().split_at_mid();
        assert_eq!(cpu_left.coeffs, gpu_left.coeffs.to_cpu());
        assert_eq!(cpu_right.coeffs, gpu_right.coeffs.to_cpu());
    }

    #[test]
    fn poly_probe_barycentric_weights_surface_typechecks() {
        let _barycentric_weights: fn(
            CanonicCoset,
            CirclePoint<SecureField>,
        ) -> Col<CudaBackend, SecureField> = <CudaBackend as PolyOps>::barycentric_weights;
    }

    #[test]
    fn poly_probe_barycentric_eval_surface_typechecks() {
        let _barycentric_eval_at_point: fn(
            &stwo::prover::poly::circle::CircleEvaluation<CudaBackend, BaseField, BitReversedOrder>,
            &Col<CudaBackend, SecureField>,
        ) -> SecureField = <CudaBackend as PolyOps>::barycentric_eval_at_point;
    }
}
