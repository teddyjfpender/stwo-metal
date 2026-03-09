#[cfg(test)]
mod tests {
    use itertools::Itertools;
    use rand::rngs::SmallRng;
    use rand::{Rng, SeedableRng};
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
    use stwo_metal::{BaseFieldVec, CudaBackend};

    fn cpu_secure_columns(values: &[SecureField]) -> [Vec<BaseField>; 4] {
        let mut columns: [Vec<BaseField>; 4] = [vec![], vec![], vec![], vec![]];
        values.iter().for_each(|value| {
            columns[0].push(BaseField::from_u32_unchecked(value.0 .0 .0));
            columns[1].push(BaseField::from_u32_unchecked(value.0 .1 .0));
            columns[2].push(BaseField::from_u32_unchecked(value.1 .0 .0));
            columns[3].push(BaseField::from_u32_unchecked(value.1 .1 .0));
        });
        columns
    }

    fn cuda_secure_columns(values: &[SecureField]) -> [BaseFieldVec; 4] {
        let cpu_columns = cpu_secure_columns(values);
        cpu_columns.map(BaseFieldVec::from_vec)
    }

    #[test]
    fn fri_probe_fold_line_and_fold_circle_match_cpu() {
        const LOG_SIZE: u32 = 10;
        const FOLD_STEP: u32 = 3;

        let mut rng = SmallRng::seed_from_u64(0);
        let values: Vec<SecureField> = (0..1 << LOG_SIZE).map(|_| rng.gen()).collect_vec();
        let alpha = SecureField::from_u32_unchecked(1, 3, 5, 7);

        let line_domain = LineDomain::new(CanonicCoset::new(LOG_SIZE + 1).half_coset());
        let cpu_line_eval = LineEvaluation::new(
            line_domain,
            SecureColumnByCoords::<CpuBackend> {
                columns: cpu_secure_columns(&values),
            },
        );
        let gpu_line_eval = LineEvaluation::new(
            line_domain,
            SecureColumnByCoords::<CudaBackend> {
                columns: cuda_secure_columns(&values),
            },
        );

        let cpu_line_fold = CpuBackend::fold_line(
            &cpu_line_eval,
            alpha,
            &CpuBackend::precompute_twiddles(line_domain.coset()),
            FOLD_STEP,
        );
        let gpu_line_fold = CudaBackend::fold_line(
            &gpu_line_eval,
            alpha,
            &CudaBackend::precompute_twiddles(line_domain.coset()),
            FOLD_STEP,
        );
        assert_eq!(cpu_line_fold.values.to_vec(), gpu_line_fold.values.to_cpu().to_vec());

        let circle_domain = CanonicCoset::new(LOG_SIZE).circle_domain();
        let circle_line_domain = LineDomain::new(circle_domain.half_coset);
        let cpu_src = SecureEvaluation::<CpuBackend, BitReversedOrder>::new(
            circle_domain,
            SecureColumnByCoords::<CpuBackend> {
                columns: cpu_secure_columns(&values),
            },
        );
        let gpu_src = SecureEvaluation::<CudaBackend, BitReversedOrder>::new(
            circle_domain,
            SecureColumnByCoords::<CudaBackend> {
                columns: cuda_secure_columns(&values),
            },
        );

        let mut cpu_dst = LineEvaluation::new(
            circle_line_domain,
            SecureColumnByCoords::<CpuBackend>::zeros(1 << (LOG_SIZE - 1)),
        );
        let mut gpu_dst = LineEvaluation::new(
            circle_line_domain,
            SecureColumnByCoords::<CudaBackend>::zeros(1 << (LOG_SIZE - 1)),
        );

        CpuBackend::fold_circle_into_line(
            &mut cpu_dst,
            &cpu_src,
            alpha,
            &CpuBackend::precompute_twiddles(circle_line_domain.coset()),
        );
        CudaBackend::fold_circle_into_line(
            &mut gpu_dst,
            &gpu_src,
            alpha,
            &CudaBackend::precompute_twiddles(circle_line_domain.coset()),
        );

        assert_eq!(cpu_dst.values.to_vec(), gpu_dst.values.to_cpu().to_vec());
    }
}
