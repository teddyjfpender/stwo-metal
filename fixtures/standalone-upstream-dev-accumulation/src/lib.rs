#[cfg(test)]
mod tests {
    use itertools::Itertools;
    use rand::rngs::SmallRng;
    use rand::{Rng, SeedableRng};
    use stwo::core::fields::m31::M31;
    use stwo::prover::backend::CpuBackend;
    use stwo::prover::secure_column::SecureColumnByCoords;
    use stwo::prover::AccumulationOps;
    use stwo_metal::{BaseFieldVec, CudaBackend};

    #[test]
    fn accumulation_probe_lift_and_accumulate_matches_cpu() {
        let mut rng = SmallRng::seed_from_u64(0);
        let log_sizes = [3u32, 5u32, 6u32];
        let cpu_cols = log_sizes
            .into_iter()
            .map(|log_size| {
                let len = 1usize << log_size;
                let columns = std::array::from_fn(|_| {
                    (0..len)
                        .map(|_| M31::from(rng.gen::<u32>()))
                        .collect_vec()
                });
                SecureColumnByCoords::<CpuBackend> { columns }
            })
            .collect_vec();
        let gpu_cols = cpu_cols
            .iter()
            .map(|column| SecureColumnByCoords::<CudaBackend> {
                columns: column.columns.clone().map(BaseFieldVec::from_vec),
            })
            .collect_vec();

        let cpu_result = CpuBackend::lift_and_accumulate(cpu_cols).unwrap();
        let gpu_result = CudaBackend::lift_and_accumulate(gpu_cols).unwrap();

        assert_eq!(cpu_result.columns, gpu_result.to_cpu().columns);
    }
}
