#[cfg(test)]
mod tests {
    use stwo::core::fields::m31::BaseField;
    use stwo::core::pcs::quotients::ColumnSampleBatch;
    use stwo::prover::poly::circle::CircleEvaluation;
    use stwo::prover::poly::BitReversedOrder;
    use stwo::prover::{AccumulatedNumerators, QuotientOps};
    use stwo_metal::CudaBackend;

    #[test]
    fn quotient_probe_accumulate_numerators_surface_typechecks() {
        let _accumulate_numerators: fn(
            &[&CircleEvaluation<CudaBackend, BaseField, BitReversedOrder>],
            &[ColumnSampleBatch],
            &mut Vec<AccumulatedNumerators<CudaBackend>>,
        ) = <CudaBackend as QuotientOps>::accumulate_numerators;
    }

    #[test]
    fn quotient_probe_compute_quotients_and_combine_surface_typechecks() {
        let _compute_quotients_and_combine: fn(
            Vec<AccumulatedNumerators<CudaBackend>>,
            u32,
        ) -> stwo::prover::poly::circle::SecureEvaluation<CudaBackend, BitReversedOrder> =
            <CudaBackend as QuotientOps>::compute_quotients_and_combine;
    }
}
