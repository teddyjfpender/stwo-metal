use super::*;

#[cfg(test)]
pub(super) fn eval_poseidon_constraints_mode_with_logup_finalization<E: EvalAtRow>(
    eval: &mut E,
    lookup_elements: &PoseidonElements,
    mode: PoseidonEvalMode,
    logup_finalization_mode: LogupFinalizationMode,
) {
    for _ in 0..N_INSTANCES_PER_ROW {
        let mut state: [_; N_STATE] = std::array::from_fn(|_| eval.next_trace_mask());
        let initial_state = state.clone();

        (0..N_HALF_FULL_ROUNDS).for_each(|round| {
            (0..N_STATE).for_each(|index| {
                state[index] += EXTERNAL_ROUND_CONSTS[round][index];
            });
            apply_external_round_matrix(&mut state);
            state = std::array::from_fn(|index| pow5(state[index].clone()));
            state.iter_mut().enumerate().for_each(|(index, value)| {
                let mask = eval.next_trace_mask();
                if mode.includes_first_half_full_cell(round, index) {
                    eval.add_constraint(value.clone() - mask.clone());
                }
                *value = mask;
            });
        });

        (0..N_PARTIAL_ROUNDS).for_each(|round| {
            state[0] += INTERNAL_ROUND_CONSTS[round];
            apply_internal_round_matrix(&mut state);
            state[0] = pow5(state[0].clone());
            let mask = eval.next_trace_mask();
            if mode.includes_partial_round(round) {
                eval.add_constraint(state[0].clone() - mask.clone());
            }
            state[0] = mask;
        });

        (0..N_HALF_FULL_ROUNDS).for_each(|round| {
            (0..N_STATE).for_each(|index| {
                state[index] += EXTERNAL_ROUND_CONSTS[round + N_HALF_FULL_ROUNDS][index];
            });
            apply_external_round_matrix(&mut state);
            state = std::array::from_fn(|index| pow5(state[index].clone()));
            state.iter_mut().enumerate().for_each(|(index, value)| {
                let mask = eval.next_trace_mask();
                if mode.includes_second_half_full_cell(round, index) {
                    eval.add_constraint(value.clone() - mask.clone());
                }
                *value = mask;
            });
        });

        if mode.includes_logup() {
            eval.add_to_relation(RelationEntry::new(lookup_elements, E::EF::one(), &initial_state));
            eval.add_to_relation(RelationEntry::new(lookup_elements, -E::EF::one(), &state));
        }
    }

    if mode.includes_logup() {
        match logup_finalization_mode {
            LogupFinalizationMode::Sequential => eval.finalize_logup(),
            LogupFinalizationMode::InPairs => eval.finalize_logup_in_pairs(),
            LogupFinalizationMode::Skip => {}
        }
    }
}

#[cfg(test)]
mod host_reference_tests {
    use super::*;

    fn synthetic_cpu_trace(
        log_size: u32,
        n_columns: usize,
    ) -> Vec<CircleEvaluation<CpuBackend, BaseField, BitReversedOrder>> {
        let domain = CanonicCoset::new(log_size).circle_domain();
        (0..n_columns)
            .map(|column_index| {
                let values = (0..(1 << log_size))
                    .map(|row_index| {
                        BaseField::from_u32_unchecked(
                            (row_index as u32) + 17 * (column_index as u32 + 1),
                        )
                    })
                    .collect();
                CircleEvaluation::new(domain, values)
            })
            .collect()
    }

    fn cpu_poseidon_direct_and_proved_sampled_values(
        log_n_rows: u32,
        oods_point: stwo::core::circle::CirclePoint<SecureField>,
        random_coeff: SecureField,
    ) -> (
        TreeVec<Vec<Vec<SecureField>>>,
        TreeVec<Vec<Vec<SecureField>>>,
    ) {
        let mut draw_channel = Blake2sChannel::default();
        let lookup_elements = PoseidonElements::draw(&mut draw_channel);
        let claimed_sum = SecureField::zero();
        let component = FrameworkComponent::new(
            &mut TraceLocationAllocator::default(),
            PoseidonEval {
                log_n_rows,
                lookup_elements,
                mode: PoseidonEvalMode::Full,
            },
            claimed_sum,
        );

        let trace_log_degree_bounds = component.trace_log_degree_bounds();
        let main_trace =
            synthetic_cpu_trace(log_n_rows, trace_log_degree_bounds[MAIN_TRACE_IDX].len());
        let interaction_trace =
            synthetic_cpu_trace(log_n_rows, trace_log_degree_bounds[INTERACTION_TRACE_IDX].len());

        let config = PcsConfig::default();
        let twiddles = CpuBackend::precompute_twiddles(
            CanonicCoset::new(log_n_rows + LOG_EXPAND + config.fri_config.log_blowup_factor)
                .circle_domain()
                .half_coset,
        );
        let prover_channel = &mut Blake2sChannel::default();
        let mut commitment_scheme =
            CommitmentSchemeProver::<CpuBackend, Blake2sMerkleChannel>::new(config, &twiddles);
        let mut tree_builder = commitment_scheme.tree_builder();
        tree_builder.extend_evals(vec![]);
        tree_builder.commit(prover_channel);
        maybe_enable_poseidon_store_polynomials_coefficients(&mut commitment_scheme);

        commit_trace_cpu(
            &mut commitment_scheme,
            prover_channel,
            main_trace,
            &twiddles,
        );
        commit_trace_cpu(
            &mut commitment_scheme,
            prover_channel,
            interaction_trace,
            &twiddles,
        );

        let n_preprocessed_columns = commitment_scheme.trees[PREPROCESSED_TRACE_IDX]
            .polynomials
            .len();
        let component_provers = ComponentProvers {
            components: vec![&component],
            n_preprocessed_columns,
        };
        let trace = commitment_scheme.trace();
        let composition_poly =
            component_provers.compute_composition_polynomial(random_coeff, &trace);

        let mut tree_builder = commitment_scheme.tree_builder();
        let (left_comp_poly_half, right_comp_poly_half) = composition_poly.split_at_mid();
        tree_builder.extend_polys(left_comp_poly_half.into_coordinate_polys());
        tree_builder.extend_polys(right_comp_poly_half.into_coordinate_polys());
        tree_builder.commit(prover_channel);

        let split_composition_log_size = commitment_scheme
            .trees
            .last()
            .expect("composition tree should exist")
            .commitment
            .layers
            .len() as u32
            - 1;
        let lifting_log_size =
            get_lifting_log_size(&commitment_scheme.config, split_composition_log_size);
        let max_log_degree_bound =
            lifting_log_size - commitment_scheme.config.fri_config.log_blowup_factor;

        let mut sample_points = component_provers.components().mask_points(
            oods_point,
            max_log_degree_bound,
            false,
        );
        let direct_trace_mask_values =
            direct_trace_mask_values_from_committed_trees_with_prove_values_lifting(
                &commitment_scheme,
                &sample_points,
            )
            .expect("CPU direct prove_values-lifted trace values should be available");

        sample_points.push(vec![vec![oods_point]; 2 * SECURE_EXTENSION_DEGREE]);
        let proof = StarkProof(commitment_scheme.prove_values(sample_points, prover_channel).proof);

        (direct_trace_mask_values, proof.sampled_values.clone())
    }

    #[test]
    fn cpu_sampled_trace_mask_values_follow_prove_values_global_lifting() {
        let config = PcsConfig::default();
        let max_trace_log_size = 5;
        let twiddles = CpuBackend::precompute_twiddles(
            CanonicCoset::new(max_trace_log_size + config.fri_config.log_blowup_factor)
                .circle_domain()
                .half_coset,
        );
        let channel = &mut Blake2sChannel::default();
        let mut commitment_scheme =
            CommitmentSchemeProver::<CpuBackend, Blake2sMerkleChannel>::new(config, &twiddles);
        let mut tree_builder = commitment_scheme.tree_builder();
        tree_builder.extend_evals(vec![]);
        tree_builder.commit(channel);
        maybe_enable_poseidon_store_polynomials_coefficients(&mut commitment_scheme);

        commit_trace_cpu(
            &mut commitment_scheme,
            channel,
            synthetic_cpu_trace(3, 2),
            &twiddles,
        );
        commit_trace_cpu(
            &mut commitment_scheme,
            channel,
            synthetic_cpu_trace(max_trace_log_size, 1),
            &twiddles,
        );

        let sample_point = stwo::core::circle::CirclePoint::<SecureField>::get_point(12_345);
        let sample_points = TreeVec::new(vec![
            vec![],
            vec![vec![sample_point], vec![sample_point]],
            vec![vec![sample_point]],
        ]);

        let tree_local_values =
            direct_trace_mask_values_from_committed_trees(&commitment_scheme, &sample_points)
                .expect("tree-local sampled values should be available");
        let prove_values_contract_values =
            direct_trace_mask_values_from_committed_trees_with_prove_values_lifting(
                &commitment_scheme,
                &sample_points,
            )
            .expect("prove_values sampled values should be available");

        let proof = commitment_scheme.prove_values(sample_points, channel);

        assert!(
            !sampled_trace_mask_values_match_direct(&proof.proof.sampled_values, &tree_local_values),
            "tree-local lifting should not be treated as the prove_values sampled-value contract"
        );
        assert!(
            sampled_trace_mask_values_match_direct(
                &proof.proof.sampled_values,
                &prove_values_contract_values,
            ),
            "prove_values sampled-value contract should use the global lifting of the last tree"
        );
    }

    #[test]
    fn cpu_poseidon_direct_lifted_trace_values_match_cpu_prove_values() {
        const LOG_N_ROWS: u32 = 7;
        let oods_point = stwo::core::circle::CirclePoint::<SecureField>::get_point(12_345_678);
        let random_coeff = SecureField::one();

        let (direct_trace_mask_values, sampled_values) =
            cpu_poseidon_direct_and_proved_sampled_values(LOG_N_ROWS, oods_point, random_coeff);

        let main_mismatches = count_trace_mask_value_mismatches(
            &TreeVec(vec![sampled_values[MAIN_TRACE_IDX].clone()]),
            &TreeVec(vec![direct_trace_mask_values[MAIN_TRACE_IDX].clone()]),
        );
        let interaction_mismatches = count_trace_mask_value_mismatches(
            &TreeVec(vec![sampled_values[INTERACTION_TRACE_IDX].clone()]),
            &TreeVec(vec![direct_trace_mask_values[INTERACTION_TRACE_IDX].clone()]),
        );

        eprintln!(
            "cpu_poseidon_direct_lifted_sampled_parity main_mismatches={} interaction_mismatches={}",
            main_mismatches, interaction_mismatches,
        );

        assert!(
            sampled_trace_mask_values_match_direct(&sampled_values, &direct_trace_mask_values),
            "CPU direct lifted helper drifted from actual CPU prove_values sampled values: main_mismatches={} interaction_mismatches={}",
            main_mismatches,
            interaction_mismatches,
        );
    }
}

#[cfg(all(test, feature = "cuda-runtime"))]
mod tests {
    use super::*;
    use stwo::prover::backend::cpu::CpuBackend;
    use stwo::prover::backend::Column;
    use stwo_constraint_framework::CpuDomainEvaluator;

    fn synthetic_cpu_trace(
        log_size: u32,
        n_columns: usize,
    ) -> Vec<CircleEvaluation<CpuBackend, BaseField, BitReversedOrder>> {
        let domain = CanonicCoset::new(log_size).circle_domain();
        (0..n_columns)
            .map(|column_index| {
                let values = (0..(1 << log_size))
                    .map(|row_index| {
                        BaseField::from_u32_unchecked(
                            (row_index as u32) + 17 * (column_index as u32 + 1),
                        )
                    })
                    .collect();
                CircleEvaluation::new(domain, values)
            })
            .collect()
    }

    fn cpu_poseidon_direct_and_proved_sampled_values(
        log_n_rows: u32,
        oods_point: stwo::core::circle::CirclePoint<SecureField>,
        random_coeff: SecureField,
    ) -> (
        TreeVec<Vec<Vec<SecureField>>>,
        TreeVec<Vec<Vec<SecureField>>>,
    ) {
        let mut draw_channel = Blake2sChannel::default();
        let lookup_elements = PoseidonElements::draw(&mut draw_channel);
        let claimed_sum = SecureField::zero();
        let component = FrameworkComponent::new(
            &mut TraceLocationAllocator::default(),
            PoseidonEval {
                log_n_rows,
                lookup_elements,
                mode: PoseidonEvalMode::Full,
            },
            claimed_sum,
        );

        let trace_log_degree_bounds = component.trace_log_degree_bounds();
        let main_trace =
            synthetic_cpu_trace(log_n_rows, trace_log_degree_bounds[MAIN_TRACE_IDX].len());
        let interaction_trace =
            synthetic_cpu_trace(log_n_rows, trace_log_degree_bounds[INTERACTION_TRACE_IDX].len());

        let config = PcsConfig::default();
        let twiddles = CpuBackend::precompute_twiddles(
            CanonicCoset::new(log_n_rows + LOG_EXPAND + config.fri_config.log_blowup_factor)
                .circle_domain()
                .half_coset,
        );
        let prover_channel = &mut Blake2sChannel::default();
        let mut commitment_scheme =
            CommitmentSchemeProver::<CpuBackend, Blake2sMerkleChannel>::new(config, &twiddles);
        let mut tree_builder = commitment_scheme.tree_builder();
        tree_builder.extend_evals(vec![]);
        tree_builder.commit(prover_channel);
        maybe_enable_poseidon_store_polynomials_coefficients(&mut commitment_scheme);

        commit_trace_cpu(
            &mut commitment_scheme,
            prover_channel,
            main_trace,
            &twiddles,
        );
        commit_trace_cpu(
            &mut commitment_scheme,
            prover_channel,
            interaction_trace,
            &twiddles,
        );

        let n_preprocessed_columns = commitment_scheme.trees[PREPROCESSED_TRACE_IDX]
            .polynomials
            .len();
        let component_provers = ComponentProvers {
            components: vec![&component],
            n_preprocessed_columns,
        };
        let trace = commitment_scheme.trace();
        let composition_poly =
            component_provers.compute_composition_polynomial(random_coeff, &trace);

        let mut tree_builder = commitment_scheme.tree_builder();
        let (left_comp_poly_half, right_comp_poly_half) = composition_poly.split_at_mid();
        tree_builder.extend_polys(left_comp_poly_half.into_coordinate_polys());
        tree_builder.extend_polys(right_comp_poly_half.into_coordinate_polys());
        tree_builder.commit(prover_channel);

        let split_composition_log_size = commitment_scheme
            .trees
            .last()
            .expect("composition tree should exist")
            .commitment
            .layers
            .len() as u32
            - 1;
        let lifting_log_size =
            get_lifting_log_size(&commitment_scheme.config, split_composition_log_size);
        let max_log_degree_bound =
            lifting_log_size - commitment_scheme.config.fri_config.log_blowup_factor;

        let mut sample_points = component_provers.components().mask_points(
            oods_point,
            max_log_degree_bound,
            false,
        );
        let direct_trace_mask_values =
            direct_trace_mask_values_from_committed_trees_with_prove_values_lifting(
                &commitment_scheme,
                &sample_points,
            )
            .expect("CPU direct prove_values-lifted trace values should be available");

        sample_points.push(vec![vec![oods_point]; 2 * SECURE_EXTENSION_DEGREE]);
        let proof = StarkProof(commitment_scheme.prove_values(sample_points, prover_channel).proof);

        (direct_trace_mask_values, proof.sampled_values.clone())
    }

    #[test]
    fn cpu_sampled_trace_mask_values_follow_prove_values_global_lifting() {
        let config = PcsConfig::default();
        let max_trace_log_size = 5;
        let twiddles = CpuBackend::precompute_twiddles(
            CanonicCoset::new(max_trace_log_size + config.fri_config.log_blowup_factor)
                .circle_domain()
                .half_coset,
        );
        let channel = &mut Blake2sChannel::default();
        let mut commitment_scheme =
            CommitmentSchemeProver::<CpuBackend, Blake2sMerkleChannel>::new(config, &twiddles);
        let mut tree_builder = commitment_scheme.tree_builder();
        tree_builder.extend_evals(vec![]);
        tree_builder.commit(channel);
        maybe_enable_poseidon_store_polynomials_coefficients(&mut commitment_scheme);

        commit_trace_cpu(
            &mut commitment_scheme,
            channel,
            synthetic_cpu_trace(3, 2),
            &twiddles,
        );
        commit_trace_cpu(
            &mut commitment_scheme,
            channel,
            synthetic_cpu_trace(max_trace_log_size, 1),
            &twiddles,
        );

        let sample_point = stwo::core::circle::CirclePoint::<SecureField>::get_point(12_345);
        let sample_points = TreeVec::new(vec![
            vec![],
            vec![vec![sample_point], vec![sample_point]],
            vec![vec![sample_point]],
        ]);

        let tree_local_values =
            direct_trace_mask_values_from_committed_trees(&commitment_scheme, &sample_points)
                .expect("tree-local sampled values should be available");
        let prove_values_contract_values =
            direct_trace_mask_values_from_committed_trees_with_prove_values_lifting(
                &commitment_scheme,
                &sample_points,
            )
            .expect("prove_values sampled values should be available");

        let proof = commitment_scheme.prove_values(sample_points, channel);

        assert!(
            !sampled_trace_mask_values_match_direct(&proof.proof.sampled_values, &tree_local_values),
            "tree-local lifting should not be treated as the prove_values sampled-value contract"
        );
        assert!(
            sampled_trace_mask_values_match_direct(
                &proof.proof.sampled_values,
                &prove_values_contract_values,
            ),
            "prove_values sampled-value contract should use the global lifting of the last tree"
        );
    }

    #[test]
    fn cpu_poseidon_direct_lifted_trace_values_match_cpu_prove_values() {
        const LOG_N_ROWS: u32 = 7;
        let oods_point = stwo::core::circle::CirclePoint::<SecureField>::get_point(12_345_678);
        let random_coeff = SecureField::one();

        let (direct_trace_mask_values, sampled_values) =
            cpu_poseidon_direct_and_proved_sampled_values(LOG_N_ROWS, oods_point, random_coeff);

        let main_mismatches = count_trace_mask_value_mismatches(
            &TreeVec(vec![sampled_values[MAIN_TRACE_IDX].clone()]),
            &TreeVec(vec![direct_trace_mask_values[MAIN_TRACE_IDX].clone()]),
        );
        let interaction_mismatches = count_trace_mask_value_mismatches(
            &TreeVec(vec![sampled_values[INTERACTION_TRACE_IDX].clone()]),
            &TreeVec(vec![direct_trace_mask_values[INTERACTION_TRACE_IDX].clone()]),
        );

        eprintln!(
            "cpu_poseidon_direct_lifted_sampled_parity main_mismatches={} interaction_mismatches={}",
            main_mismatches, interaction_mismatches,
        );

        assert!(
            sampled_trace_mask_values_match_direct(&sampled_values, &direct_trace_mask_values),
            "CPU direct lifted helper drifted from actual CPU prove_values sampled values: main_mismatches={} interaction_mismatches={}",
            main_mismatches,
            interaction_mismatches,
        );
    }

    fn assert_poseidon_constraint_quotients_match_cpu_reference(claimed_sum_shift: SecureField) {
        if std::env::var("STWO_CUDA_MODE")
            .map(|mode| mode == "no-cuda")
            .unwrap_or(false)
        {
            eprintln!("skipping CUDA-only Poseidon quotient validation on no-cuda host mode");
            return;
        }

        const LOG_N_ROWS: u32 = 4;

        let (trace, lookup_data, _) = generate_poseidon_trace_evaluations(LOG_N_ROWS);
        let mut channel = Blake2sChannel::default();
        let lookup_elements = PoseidonElements::draw(&mut channel);
        let (interaction_trace, claimed_sum, _) =
            generate_poseidon_interaction_trace_evaluations(LOG_N_ROWS, lookup_data, &lookup_elements);

        let main_polys = trace
            .into_iter()
            .map(|evaluation| evaluation.interpolate())
            .collect::<Vec<_>>();
        let interaction_polys = interaction_trace
            .into_iter()
            .map(|evaluation| evaluation.interpolate())
            .collect::<Vec<_>>();

        let eval_log_size = LOG_N_ROWS + LOG_EXPAND;
        let eval_domain = CanonicCoset::new(eval_log_size).circle_domain();
        let trace_evals = TreeVec::new(vec![
            vec![],
            main_polys
                .iter()
                .map(|poly| poly.evaluate(eval_domain))
                .collect::<Vec<_>>(),
            interaction_polys
                .iter()
                .map(|poly| poly.evaluate(eval_domain))
                .collect::<Vec<_>>(),
        ]);

        let log_expand = eval_log_size - LOG_N_ROWS;
        let mut denominator_inverses = (0..(1 << log_expand))
            .map(|index| coset_vanishing(CanonicCoset::new(LOG_N_ROWS).coset(), eval_domain.at(index)).inverse())
            .collect::<Vec<_>>();
        bit_reverse(&mut denominator_inverses);
        let denominator_inverses_cpu = denominator_inverses.clone();
        let denominator_inverses = BaseFieldVec::from_vec(denominator_inverses);

        let quotient_size = 1usize << eval_log_size;
        let quotient_columns = [
            BaseFieldVec::new_zeroes(quotient_size),
            BaseFieldVec::new_zeroes(quotient_size),
            BaseFieldVec::new_zeroes(quotient_size),
            BaseFieldVec::new_zeroes(quotient_size),
        ];

        let component = PoseidonBenchmarkComponent::new(LOG_N_ROWS, lookup_elements.clone(), claimed_sum);
        let random_coeff_powers =
            SecureFieldVec::from_vec(vec![SecureField::one(); component.n_constraints()]);

        let trace0_evaluations = trace_evals[PREPROCESSED_TRACE_IDX]
            .iter()
            .map(|column| column.values.device_ptr)
            .collect::<Vec<_>>();
        let trace1_evaluations = trace_evals[MAIN_TRACE_IDX]
            .iter()
            .map(|column| column.values.device_ptr)
            .collect::<Vec<_>>();
        let trace2_evaluations = trace_evals[INTERACTION_TRACE_IDX]
            .iter()
            .map(|column| column.values.device_ptr)
            .collect::<Vec<_>>();

        launch_constraint_quotients_on_domain(ConstraintQuotientEvalRequest {
            quotient_columns: &quotient_columns,
            trace0_evaluations: &trace0_evaluations,
            trace1_evaluations: &trace1_evaluations,
            trace2_evaluations: &trace2_evaluations,
            random_coeff_powers: &random_coeff_powers,
            denominator_inverses: &denominator_inverses,
            domain_log_size: LOG_N_ROWS,
            eval_domain_log_size: eval_log_size,
            number_of_columns: component.n_constraints() as u32,
            logup_counts: 2,
            eval: opaque_eval_ptr(&component.eval_abi),
            claimed_sum_shift,
            should_accumulate: true,
        });

        let cuda_q0 = quotient_columns[0].to_cpu();
        let cuda_q1 = quotient_columns[1].to_cpu();
        let cuda_q2 = quotient_columns[2].to_cpu();
        let cuda_q3 = quotient_columns[3].to_cpu();

        let cpu_main_evals = trace_evals[MAIN_TRACE_IDX]
            .iter()
            .map(|evaluation| CircleEvaluation::<CpuBackend, BaseField, BitReversedOrder>::new(
                eval_domain,
                evaluation.values.to_cpu(),
            ))
            .collect::<Vec<_>>();
        let cpu_interaction_evals = trace_evals[INTERACTION_TRACE_IDX]
            .iter()
            .map(|evaluation| CircleEvaluation::<CpuBackend, BaseField, BitReversedOrder>::new(
                eval_domain,
                evaluation.values.to_cpu(),
            ))
            .collect::<Vec<_>>();
        let cpu_trace_evals = TreeVec::new(vec![
            vec![],
            cpu_main_evals.iter().collect::<Vec<_>>(),
            cpu_interaction_evals.iter().collect::<Vec<_>>(),
        ]);

        let poseidon_eval = PoseidonEval {
            log_n_rows: LOG_N_ROWS,
            lookup_elements,
            mode: PoseidonEvalMode::Full,
        };
        let random_coeffs = vec![SecureField::one(); component.n_constraints()];
        let mut mismatches = 0usize;

        for row in 0..quotient_size {
            let eval = CpuDomainEvaluator::new(
                &cpu_trace_evals,
                row,
                &random_coeffs,
                LOG_N_ROWS,
                eval_log_size,
                LOG_N_ROWS,
                claimed_sum,
            );
            let cpu_constraint_eval = poseidon_eval.evaluate(eval).row_res;
            let cpu_quotient = cpu_constraint_eval * denominator_inverses_cpu[row >> LOG_N_ROWS];
            let cuda_quotient = SecureField::from_m31_array([
                cuda_q0[row],
                cuda_q1[row],
                cuda_q2[row],
                cuda_q3[row],
            ]);

            if cpu_quotient != cuda_quotient {
                mismatches += 1;
                if mismatches <= 4 {
                    eprintln!(
                        "row {} mismatch: cpu={:?} cuda={:?}",
                        row, cpu_quotient, cuda_quotient
                    );
                }
            }
        }

        assert_eq!(
            mismatches, 0,
            "supported Poseidon CUDA quotients drifted on {} rows",
            mismatches
        );
    }

    fn collect_supported_poseidon_point_contract_snapshot(
        log_n_instances: u32,
    ) -> (
        ProveCoreSanitySnapshot,
        PointContractVariantSnapshot,
        PointContributionSplitSnapshot,
        LogupContractVariantSnapshot,
        LogupOnlyVariantSnapshot,
        InPairsLogupBoundarySnapshot,
        InPairsCumsumVariantSnapshot,
        InPairsFractionAssemblyVariantSnapshot,
        InPairsDenominatorVariantSnapshot,
        AlgebraicFamilySplitSnapshot,
        AlgebraicSequenceLawSnapshot,
        AlgebraicPerRoundSnapshot,
        AlgebraicPerCellSnapshot,
    ) {
        assert!(log_n_instances >= N_LOG_INSTANCES_PER_ROW as u32);
        let config = PcsConfig::default();
        let log_n_rows = log_n_instances - N_LOG_INSTANCES_PER_ROW as u32;

        let twiddles = CudaBackend::precompute_twiddles(
            CanonicCoset::new(log_n_rows + LOG_EXPAND + config.fri_config.log_blowup_factor)
                .circle_domain()
                .half_coset,
        );
        let prover_channel = &mut Blake2sChannel::default();
        let mut commitment_scheme =
            CommitmentSchemeProver::<CudaBackend, Blake2sMerkleChannel>::new(config, &twiddles);
        let mut tree_builder = commitment_scheme.tree_builder();
        tree_builder.extend_evals(vec![]);
        tree_builder.commit(prover_channel);
        maybe_enable_poseidon_store_polynomials_coefficients(&mut commitment_scheme);

        let (trace, lookup_data, _) = generate_poseidon_trace_evaluations(log_n_rows);
        let cpu_trace = copy_trace_to_cpu(&trace);
        let _ = commit_trace_with_breakdown(&mut commitment_scheme, prover_channel, trace, &twiddles);

        let lookup_elements = PoseidonElements::draw(prover_channel);
        let (interaction_trace, claimed_sum, _) =
            generate_poseidon_interaction_trace_evaluations(log_n_rows, lookup_data, &lookup_elements);
        let cpu_interaction_trace = copy_trace_to_cpu(&interaction_trace);
        let _ = commit_trace_with_breakdown(
            &mut commitment_scheme,
            prover_channel,
            interaction_trace,
            &twiddles,
        );

        let component =
            PoseidonBenchmarkComponent::new(log_n_rows, lookup_elements.clone(), claimed_sum);
        let n_preprocessed_columns = commitment_scheme.trees[PREPROCESSED_TRACE_IDX]
            .polynomials
            .len();
        let component_provers = ComponentProvers {
            components: vec![&component],
            n_preprocessed_columns,
        };
        let trace = commitment_scheme.trace();
        let random_coeff = prover_channel.draw_secure_felt();
        let composition_poly =
            component_provers.compute_composition_polynomial(random_coeff, &trace);

        let mut tree_builder = commitment_scheme.tree_builder();
        let (left_comp_poly_half, right_comp_poly_half) = composition_poly.split_at_mid();
        tree_builder.extend_polys(left_comp_poly_half.into_coordinate_polys());
        tree_builder.extend_polys(right_comp_poly_half.into_coordinate_polys());
        tree_builder.commit(prover_channel);

        let oods_point = stwo::core::circle::CirclePoint::<SecureField>::get_random_point(
            prover_channel,
        );
        let split_composition_log_size = commitment_scheme
            .trees
            .last()
            .expect("composition tree should exist")
            .commitment
            .layers
            .len() as u32
            - 1;
        let lifting_log_size =
            get_lifting_log_size(&commitment_scheme.config, split_composition_log_size);
        let max_log_degree_bound =
            lifting_log_size - commitment_scheme.config.fri_config.log_blowup_factor;

        let direct_composition_oods_eval =
            direct_composition_oods_eval_from_committed_tree(
                &commitment_scheme,
                oods_point,
                max_log_degree_bound,
            )
            .expect("direct composition OODS eval should be available");
        eprintln!("snapshot_phase=direct_composition_oods_eval");
        let cpu_trace_composition_oods_eval = cpu_composition_oods_eval_from_generated_traces(
            config,
            log_n_rows,
            lookup_elements.clone(),
            claimed_sum,
            random_coeff,
            oods_point,
            cpu_trace.clone(),
            cpu_interaction_trace.clone(),
            PoseidonEvalMode::Full,
        );
        eprintln!("snapshot_phase=cpu_trace_composition_oods_eval");

        let mut sample_points = component_provers
            .components()
            .mask_points(oods_point, max_log_degree_bound, false);
        let direct_trace_mask_values =
            direct_trace_mask_values_from_committed_trees_with_prove_values_lifting(
                &commitment_scheme,
                &sample_points,
            )
            .expect("direct prove_values trace mask values should be available");
        eprintln!("snapshot_phase=direct_trace_mask_values");
        let trace_points = component.point_mask_points(oods_point, component.log_n_rows);
        let trace_point_values =
            direct_trace_mask_values_from_committed_trees(&commitment_scheme, &trace_points)
                .expect("trace-point values should be available");
        eprintln!("snapshot_phase=trace_point_values");
        let lifted_trace_point_values =
            direct_trace_mask_values_from_committed_trees_with_prove_values_lifting(
                &commitment_scheme,
                &trace_points,
            )
            .expect("lifted trace-point values should be available");
        eprintln!("snapshot_phase=lifted_trace_point_values");
        let (
            cpu_sampled_values,
            cpu_trace_point_values,
            main_committed_coefficient_mismatches,
            interaction_committed_coefficient_mismatches,
        ) = with_cpu_poseidon_commitment_scheme(
            config,
            log_n_rows,
            cpu_trace.clone(),
            cpu_interaction_trace.clone(),
            |cpu_commitment_scheme| {
                let cpu_sampled_values =
                    direct_trace_mask_values_from_committed_trees_with_prove_values_lifting(
                        cpu_commitment_scheme,
                        &sample_points,
                    )
                    .expect("CPU prove_values-lifted trace-point values should be available");
                let cpu_trace_point_values =
                    direct_trace_mask_values_from_committed_trees(
                        cpu_commitment_scheme,
                        &trace_points,
                    )
                    .expect("CPU committed trace-point values should be available");
                let main_committed_coefficient_mismatches = commitment_scheme.trees
                    [MAIN_TRACE_IDX]
                    .polynomials
                    .iter()
                    .zip(
                        cpu_commitment_scheme.trees[MAIN_TRACE_IDX]
                            .polynomials
                            .iter(),
                    )
                    .filter(|(gpu_poly, cpu_poly)| {
                        gpu_poly
                            .coeffs
                            .as_ref()
                            .map(|coeffs| coeffs.coeffs.to_cpu())
                            != cpu_poly.coeffs.as_ref().map(|coeffs| coeffs.coeffs.clone())
                    })
                    .count();
                let interaction_committed_coefficient_mismatches = commitment_scheme.trees
                    [INTERACTION_TRACE_IDX]
                    .polynomials
                    .iter()
                    .zip(
                        cpu_commitment_scheme.trees[INTERACTION_TRACE_IDX]
                            .polynomials
                            .iter(),
                    )
                    .filter(|(gpu_poly, cpu_poly)| {
                        gpu_poly
                            .coeffs
                            .as_ref()
                            .map(|coeffs| coeffs.coeffs.to_cpu())
                            != cpu_poly.coeffs.as_ref().map(|coeffs| coeffs.coeffs.clone())
                    })
                    .count();

                (
                    cpu_sampled_values,
                    cpu_trace_point_values,
                    main_committed_coefficient_mismatches,
                    interaction_committed_coefficient_mismatches,
                )
            },
        );
        eprintln!("snapshot_phase=cpu_sampled_values");
        eprintln!("snapshot_phase=cpu_trace_point_values");
        let main_sampled_value_mismatches = count_trace_mask_value_mismatches(
            &TreeVec(vec![direct_trace_mask_values[MAIN_TRACE_IDX].clone()]),
            &TreeVec(vec![cpu_sampled_values[MAIN_TRACE_IDX].clone()]),
        );
        let interaction_sampled_value_mismatches = count_trace_mask_value_mismatches(
            &TreeVec(vec![direct_trace_mask_values[INTERACTION_TRACE_IDX].clone()]),
            &TreeVec(vec![cpu_sampled_values[INTERACTION_TRACE_IDX].clone()]),
        );
        let main_trace_point_value_mismatches = count_trace_mask_value_mismatches(
            &TreeVec(vec![trace_point_values[MAIN_TRACE_IDX].clone()]),
            &TreeVec(vec![cpu_trace_point_values[MAIN_TRACE_IDX].clone()]),
        );
        let interaction_trace_point_value_mismatches = count_trace_mask_value_mismatches(
            &TreeVec(vec![trace_point_values[INTERACTION_TRACE_IDX].clone()]),
            &TreeVec(vec![cpu_trace_point_values[INTERACTION_TRACE_IDX].clone()]),
        );
        sample_points.push(vec![vec![oods_point]; 2 * SECURE_EXTENSION_DEGREE]);

        let commitment_scheme_proof = commitment_scheme.prove_values(sample_points, prover_channel);
        eprintln!("snapshot_phase=prove_values");
        let proof = StarkProof(commitment_scheme_proof.proof);

        let sampled_composition_oods_eval =
            extract_composition_oods_eval(&proof, oods_point, max_log_degree_bound)
                .expect("sampled composition OODS eval should be available");
        eprintln!("snapshot_phase=sampled_composition_oods_eval");
        let point_composition_oods_eval =
            component_provers.components().eval_composition_polynomial_at_point(
                oods_point,
                &proof.sampled_values,
                random_coeff,
                max_log_degree_bound,
            );
        eprintln!("snapshot_phase=point_composition_oods_eval");
        let direct_trace_mask_point_composition_oods_eval =
            component_provers.components().eval_composition_polynomial_at_point(
                oods_point,
                &direct_trace_mask_values,
                random_coeff,
                max_log_degree_bound,
            );
        eprintln!("snapshot_phase=direct_trace_mask_point_composition_oods_eval");
        let point_contract_variants = characterize_point_contract_variants(
            &component,
            &proof.sampled_values,
            &trace_point_values,
            &lifted_trace_point_values,
            oods_point,
            random_coeff,
            max_log_degree_bound,
        )
        .expect("point contract variants should be available");
        eprintln!("snapshot_phase=point_contract_variants");
        let point_contribution_split = characterize_point_contribution_split(
            &component,
            &direct_trace_mask_values,
            &trace_point_values,
            oods_point,
            random_coeff,
            max_log_degree_bound,
            config,
            lookup_elements.clone(),
            claimed_sum,
            cpu_trace.clone(),
            cpu_interaction_trace.clone(),
        );
        eprintln!("snapshot_phase=point_contribution_split");
        let logup_contract_variants = characterize_logup_contract_variants(
            &component,
            &trace_point_values,
            oods_point,
            random_coeff,
            point_contribution_split,
        );
        eprintln!("snapshot_phase=logup_contract_variants");
        let logup_only_variants = characterize_logup_only_variants(
            &component,
            &proof.sampled_values,
            &trace_point_values,
            oods_point,
            random_coeff,
            max_log_degree_bound,
            config,
            lookup_elements.clone(),
            claimed_sum,
            cpu_trace.clone(),
            cpu_interaction_trace.clone(),
        );
        eprintln!("snapshot_phase=logup_only_variants");
        let in_pairs_logup_boundary = characterize_in_pairs_logup_boundary(
            &component,
            &trace_point_values,
            oods_point,
            random_coeff,
        );
        eprintln!("snapshot_phase=in_pairs_logup_boundary");
        let in_pairs_cumsum_variants = characterize_in_pairs_cumsum_variants(
            &in_pairs_logup_boundary,
            logup_only_variants.cpu_trace_eval,
        );
        eprintln!("snapshot_phase=in_pairs_cumsum_variants");
        let in_pairs_fraction_assembly_variants =
            characterize_in_pairs_fraction_assembly_variants(
                &in_pairs_logup_boundary,
                logup_only_variants.cpu_trace_eval,
            );
        eprintln!("snapshot_phase=in_pairs_fraction_assembly_variants");
        let in_pairs_denominator_variants = characterize_in_pairs_denominator_variants(
            &in_pairs_logup_boundary,
            &component.inner.lookup_elements,
            logup_only_variants.cpu_trace_eval,
        );
        eprintln!("snapshot_phase=in_pairs_denominator_variants");
        let algebraic_family_split = characterize_algebraic_family_split(
            &component,
            &direct_trace_mask_values,
            oods_point,
            random_coeff,
            max_log_degree_bound,
            config,
            lookup_elements.clone(),
            claimed_sum,
            cpu_trace.clone(),
            cpu_interaction_trace.clone(),
        );
        eprintln!("snapshot_phase=algebraic_family_split");
        let algebraic_sequence_law = characterize_algebraic_sequence_law(
            &component,
            &direct_trace_mask_values,
            oods_point,
            random_coeff,
            max_log_degree_bound,
            lookup_elements.clone(),
            claimed_sum,
            point_contribution_split.cpu_trace_algebraic_only_eval,
        );
        eprintln!("snapshot_phase=algebraic_sequence_law");
        let algebraic_per_round_split = characterize_algebraic_per_round_split(
            &component,
            &direct_trace_mask_values,
            oods_point,
            random_coeff,
            max_log_degree_bound,
            config,
            lookup_elements.clone(),
            claimed_sum,
            cpu_trace.clone(),
            cpu_interaction_trace.clone(),
        );
        eprintln!("snapshot_phase=algebraic_per_round_split");
        let algebraic_per_cell_split = characterize_algebraic_per_cell_split(
            &component,
            &direct_trace_mask_values,
            oods_point,
            random_coeff,
            max_log_degree_bound,
            config,
            lookup_elements.clone(),
            claimed_sum,
            cpu_trace.clone(),
            cpu_interaction_trace.clone(),
        );
        eprintln!("snapshot_phase=algebraic_per_cell_split");

        (
            ProveCoreSanitySnapshot {
                sampled_composition_oods_eval,
                direct_composition_oods_eval,
                point_composition_oods_eval,
                cpu_trace_composition_oods_eval: Some(cpu_trace_composition_oods_eval),
                direct_trace_mask_point_composition_oods_eval: Some(
                    direct_trace_mask_point_composition_oods_eval,
                ),
                sampled_trace_mask_values_match_direct: Some(
                    sampled_trace_mask_values_match_direct(
                        &proof.sampled_values,
                        &direct_trace_mask_values,
                    ),
                ),
                sampled_point_matches_direct_trace_mask_point: Some(
                    direct_trace_mask_point_composition_oods_eval == point_composition_oods_eval,
                ),
                cpu_trace_matches_direct: Some(
                    cpu_trace_composition_oods_eval == direct_composition_oods_eval,
                ),
                cpu_trace_matches_point: Some(
                    cpu_trace_composition_oods_eval == point_composition_oods_eval,
                ),
                trace_point_values_match_cpu_committed: Some(
                    trace_mask_values_equal(&trace_point_values, &cpu_trace_point_values),
                ),
                main_trace_point_values_match_cpu_committed: Some(
                    main_trace_point_value_mismatches == 0,
                ),
                interaction_trace_point_values_match_cpu_committed: Some(
                    interaction_trace_point_value_mismatches == 0,
                ),
                main_trace_point_value_mismatches: Some(main_trace_point_value_mismatches),
                interaction_trace_point_value_mismatches: Some(
                    interaction_trace_point_value_mismatches,
                ),
                sampled_values_match_cpu_committed: Some(
                    trace_mask_values_equal(&direct_trace_mask_values, &cpu_sampled_values),
                ),
                main_sampled_values_match_cpu_committed: Some(main_sampled_value_mismatches == 0),
                interaction_sampled_values_match_cpu_committed: Some(
                    interaction_sampled_value_mismatches == 0,
                ),
                main_sampled_value_mismatches: Some(main_sampled_value_mismatches),
                interaction_sampled_value_mismatches: Some(interaction_sampled_value_mismatches),
                committed_coefficients_match_cpu: Some(
                    main_committed_coefficient_mismatches == 0
                        && interaction_committed_coefficient_mismatches == 0,
                ),
                main_committed_coefficients_match_cpu: Some(
                    main_committed_coefficient_mismatches == 0,
                ),
                interaction_committed_coefficients_match_cpu: Some(
                    interaction_committed_coefficient_mismatches == 0,
                ),
                main_committed_coefficient_mismatches: Some(
                    main_committed_coefficient_mismatches,
                ),
                interaction_committed_coefficient_mismatches: Some(
                    interaction_committed_coefficient_mismatches,
                ),
            },
            point_contract_variants,
            point_contribution_split,
            logup_contract_variants,
            logup_only_variants,
            in_pairs_logup_boundary,
            in_pairs_cumsum_variants,
            in_pairs_fraction_assembly_variants,
            in_pairs_denominator_variants,
            algebraic_family_split,
            algebraic_sequence_law,
            algebraic_per_round_split,
            algebraic_per_cell_split,
        )
    }

    #[test]
    #[ignore = "M32 blocker characterization lane; run explicitly on a CUDA host"]
    fn supported_poseidon_constraint_quotients_match_cpu_reference() {
        const LOG_N_ROWS: u32 = 4;
        let (_, lookup_data, _) = generate_poseidon_trace_evaluations(LOG_N_ROWS);
        let mut channel = Blake2sChannel::default();
        let lookup_elements = PoseidonElements::draw(&mut channel);
        let (_, claimed_sum, _) =
            generate_poseidon_interaction_trace_evaluations(LOG_N_ROWS, lookup_data, &lookup_elements);
        let eval_log_size = LOG_N_ROWS + LOG_EXPAND;
        assert_poseidon_constraint_quotients_match_cpu_reference(
            claimed_sum / BaseField::from_u32_unchecked(1 << eval_log_size),
        );
    }

    #[test]
    #[ignore = "M32 blocker characterization lane; run explicitly on a CUDA host"]
    fn supported_poseidon_constraint_quotients_match_cpu_reference_trace_shift() {
        const LOG_N_ROWS: u32 = 4;
        let (_, lookup_data, _) = generate_poseidon_trace_evaluations(LOG_N_ROWS);
        let mut channel = Blake2sChannel::default();
        let lookup_elements = PoseidonElements::draw(&mut channel);
        let (_, claimed_sum, _) =
            generate_poseidon_interaction_trace_evaluations(LOG_N_ROWS, lookup_data, &lookup_elements);
        assert_poseidon_constraint_quotients_match_cpu_reference(
            claimed_sum / BaseField::from_u32_unchecked(1 << LOG_N_ROWS),
        );
    }

    #[test]
    #[ignore = "M45 characterization lane; run explicitly on a CUDA host"]
    fn supported_poseidon_interpolate_columns_match_cpu_reference() {
        if std::env::var("STWO_CUDA_MODE")
            .map(|mode| mode == "no-cuda")
            .unwrap_or(false)
        {
            eprintln!("skipping CUDA-only Poseidon interpolation parity on no-cuda host mode");
            return;
        }

        const LOG_N_ROWS: u32 = 7;

        let (main_trace, lookup_data, _) = generate_poseidon_trace_evaluations(LOG_N_ROWS);
        let cpu_main_trace = copy_trace_to_cpu(&main_trace);

        let mut channel = Blake2sChannel::default();
        let lookup_elements = PoseidonElements::draw(&mut channel);
        let (interaction_trace, _claimed_sum, _) =
            generate_poseidon_interaction_trace_evaluations(LOG_N_ROWS, lookup_data, &lookup_elements);
        let cpu_interaction_trace = copy_trace_to_cpu(&interaction_trace);

        let config = PcsConfig::default();
        let cuda_twiddles = CudaBackend::precompute_twiddles(
            CanonicCoset::new(LOG_N_ROWS + LOG_EXPAND + config.fri_config.log_blowup_factor)
                .circle_domain()
                .half_coset,
        );
        let cpu_twiddles = CpuBackend::precompute_twiddles(
            CanonicCoset::new(LOG_N_ROWS + LOG_EXPAND + config.fri_config.log_blowup_factor)
                .circle_domain()
                .half_coset,
        );

        let cuda_main_polys = CudaBackend::interpolate_columns(main_trace, &cuda_twiddles);
        let cpu_main_polys = CpuBackend::interpolate_columns(cpu_main_trace, &cpu_twiddles);
        let cuda_interaction_polys =
            CudaBackend::interpolate_columns(interaction_trace, &cuda_twiddles);
        let cpu_interaction_polys =
            CpuBackend::interpolate_columns(cpu_interaction_trace, &cpu_twiddles);

        let main_mismatches = cuda_main_polys
            .iter()
            .zip(cpu_main_polys.iter())
            .filter(|(cuda_poly, cpu_poly)| cuda_poly.coeffs.to_cpu() != cpu_poly.coeffs)
            .count();
        let interaction_mismatches = cuda_interaction_polys
            .iter()
            .zip(cpu_interaction_polys.iter())
            .filter(|(cuda_poly, cpu_poly)| cuda_poly.coeffs.to_cpu() != cpu_poly.coeffs)
            .count();

        eprintln!(
            "poseidon_interpolate_columns_parity main_mismatches={} interaction_mismatches={} main_columns={} interaction_columns={}",
            main_mismatches,
            interaction_mismatches,
            cuda_main_polys.len(),
            cuda_interaction_polys.len(),
        );

        assert_eq!(
            main_mismatches, 0,
            "supported Poseidon main-trace interpolation drifted on {} columns",
            main_mismatches
        );
        assert_eq!(
            interaction_mismatches, 0,
            "supported Poseidon interaction-trace interpolation drifted on {} columns",
            interaction_mismatches
        );
    }

    #[test]
    #[ignore = "M45 characterization lane; run explicitly on a CUDA host"]
    fn supported_poseidon_direct_trace_point_values_match_cpu_reference() {
        if std::env::var("STWO_CUDA_MODE")
            .map(|mode| mode == "no-cuda")
            .unwrap_or(false)
        {
            eprintln!("skipping CUDA-only Poseidon point-value parity on no-cuda host mode");
            return;
        }

        const LOG_N_INSTANCES: u32 = 7;

        let config = PcsConfig::default();
        let log_n_rows = LOG_N_INSTANCES - N_LOG_INSTANCES_PER_ROW as u32;

        let cuda_twiddles = CudaBackend::precompute_twiddles(
            CanonicCoset::new(log_n_rows + LOG_EXPAND + config.fri_config.log_blowup_factor)
                .circle_domain()
                .half_coset,
        );
        let cpu_twiddles = CpuBackend::precompute_twiddles(
            CanonicCoset::new(log_n_rows + LOG_EXPAND + config.fri_config.log_blowup_factor)
                .circle_domain()
                .half_coset,
        );
        let gpu_channel = &mut Blake2sChannel::default();
        let mut gpu_commitment_scheme =
            CommitmentSchemeProver::<CudaBackend, Blake2sMerkleChannel>::new(config, &cuda_twiddles);
        let mut gpu_tree_builder = gpu_commitment_scheme.tree_builder();
        gpu_tree_builder.extend_evals(vec![]);
        gpu_tree_builder.commit(gpu_channel);
        maybe_enable_poseidon_store_polynomials_coefficients(&mut gpu_commitment_scheme);

        let (main_trace, lookup_data, _) = generate_poseidon_trace_evaluations(log_n_rows);
        let cpu_main_trace = copy_trace_to_cpu(&main_trace);
        let _ = commit_trace_with_breakdown(
            &mut gpu_commitment_scheme,
            gpu_channel,
            main_trace,
            &cuda_twiddles,
        );

        let lookup_elements = PoseidonElements::draw(gpu_channel);
        let (interaction_trace, claimed_sum, _) =
            generate_poseidon_interaction_trace_evaluations(log_n_rows, lookup_data, &lookup_elements);
        let cpu_interaction_trace = copy_trace_to_cpu(&interaction_trace);
        let _ = commit_trace_with_breakdown(
            &mut gpu_commitment_scheme,
            gpu_channel,
            interaction_trace,
            &cuda_twiddles,
        );

        let cpu_channel = &mut Blake2sChannel::default();
        let mut cpu_commitment_scheme =
            CommitmentSchemeProver::<CpuBackend, Blake2sMerkleChannel>::new(config, &cpu_twiddles);
        let mut cpu_tree_builder = cpu_commitment_scheme.tree_builder();
        cpu_tree_builder.extend_evals(vec![]);
        cpu_tree_builder.commit(cpu_channel);
        maybe_enable_poseidon_store_polynomials_coefficients(&mut cpu_commitment_scheme);
        commit_trace_cpu(
            &mut cpu_commitment_scheme,
            cpu_channel,
            cpu_main_trace,
            &cpu_twiddles,
        );
        commit_trace_cpu(
            &mut cpu_commitment_scheme,
            cpu_channel,
            cpu_interaction_trace,
            &cpu_twiddles,
        );

        let component =
            PoseidonBenchmarkComponent::new(log_n_rows, lookup_elements.clone(), claimed_sum);
        let oods_point = stwo::core::circle::CirclePoint::<SecureField>::get_point(12_345_678);
        let trace_points = component.point_mask_points(oods_point, component.log_n_rows);

        let gpu_trace_point_values =
            direct_trace_mask_values_from_committed_trees(&gpu_commitment_scheme, &trace_points)
                .expect("GPU committed trace-point values should be available");
        let cpu_trace_point_values =
            direct_trace_mask_values_from_committed_trees(&cpu_commitment_scheme, &trace_points)
                .expect("CPU committed trace-point values should be available");

        let main_mismatches = count_trace_mask_value_mismatches(
            &TreeVec(vec![gpu_trace_point_values[MAIN_TRACE_IDX].clone()]),
            &TreeVec(vec![cpu_trace_point_values[MAIN_TRACE_IDX].clone()]),
        );
        let interaction_mismatches = count_trace_mask_value_mismatches(
            &TreeVec(vec![gpu_trace_point_values[INTERACTION_TRACE_IDX].clone()]),
            &TreeVec(vec![cpu_trace_point_values[INTERACTION_TRACE_IDX].clone()]),
        );

        eprintln!(
            "poseidon_direct_trace_point_value_parity main_mismatches={} interaction_mismatches={}",
            main_mismatches, interaction_mismatches,
        );

        assert_eq!(
            main_mismatches, 0,
            "supported Poseidon main-trace committed point values drifted on {} masks",
            main_mismatches
        );
        assert_eq!(
            interaction_mismatches, 0,
            "supported Poseidon interaction-trace committed point values drifted on {} masks",
            interaction_mismatches
        );
    }

    #[test]
    #[ignore = "M45 characterization lane; run explicitly on a CUDA host"]
    fn supported_poseidon_trace_point_values_survive_composition_commit() {
        if std::env::var("STWO_CUDA_MODE")
            .map(|mode| mode == "no-cuda")
            .unwrap_or(false)
        {
            eprintln!(
                "skipping CUDA-only Poseidon post-composition trace-point parity on no-cuda host mode"
            );
            return;
        }

        const LOG_N_INSTANCES: u32 = 7;

        let config = PcsConfig::default();
        let log_n_rows = LOG_N_INSTANCES - N_LOG_INSTANCES_PER_ROW as u32;

        let cuda_twiddles = CudaBackend::precompute_twiddles(
            CanonicCoset::new(log_n_rows + LOG_EXPAND + config.fri_config.log_blowup_factor)
                .circle_domain()
                .half_coset,
        );
        let gpu_channel = &mut Blake2sChannel::default();
        let mut gpu_commitment_scheme =
            CommitmentSchemeProver::<CudaBackend, Blake2sMerkleChannel>::new(config, &cuda_twiddles);
        let mut gpu_tree_builder = gpu_commitment_scheme.tree_builder();
        gpu_tree_builder.extend_evals(vec![]);
        gpu_tree_builder.commit(gpu_channel);
        maybe_enable_poseidon_store_polynomials_coefficients(&mut gpu_commitment_scheme);

        let (main_trace, lookup_data, _) = generate_poseidon_trace_evaluations(log_n_rows);
        let cpu_main_trace = copy_trace_to_cpu(&main_trace);
        let _ = commit_trace_with_breakdown(
            &mut gpu_commitment_scheme,
            gpu_channel,
            main_trace,
            &cuda_twiddles,
        );

        let lookup_elements = PoseidonElements::draw(gpu_channel);
        let (interaction_trace, claimed_sum, _) =
            generate_poseidon_interaction_trace_evaluations(log_n_rows, lookup_data, &lookup_elements);
        let cpu_interaction_trace = copy_trace_to_cpu(&interaction_trace);
        let _ = commit_trace_with_breakdown(
            &mut gpu_commitment_scheme,
            gpu_channel,
            interaction_trace,
            &cuda_twiddles,
        );

        let component =
            PoseidonBenchmarkComponent::new(log_n_rows, lookup_elements.clone(), claimed_sum);
        let n_preprocessed_columns = gpu_commitment_scheme.trees[PREPROCESSED_TRACE_IDX]
            .polynomials
            .len();
        let component_provers = ComponentProvers {
            components: vec![&component],
            n_preprocessed_columns,
        };
        let trace = gpu_commitment_scheme.trace();
        let random_coeff = gpu_channel.draw_secure_felt();
        let composition_poly =
            component_provers.compute_composition_polynomial(random_coeff, &trace);

        let mut gpu_tree_builder = gpu_commitment_scheme.tree_builder();
        let (left_comp_poly_half, right_comp_poly_half) = composition_poly.split_at_mid();
        gpu_tree_builder.extend_polys(left_comp_poly_half.into_coordinate_polys());
        gpu_tree_builder.extend_polys(right_comp_poly_half.into_coordinate_polys());
        gpu_tree_builder.commit(gpu_channel);

        let component = PoseidonBenchmarkComponent::new(log_n_rows, lookup_elements, claimed_sum);
        let oods_point =
            stwo::core::circle::CirclePoint::<SecureField>::get_random_point(gpu_channel);
        let trace_points = component.point_mask_points(oods_point, component.log_n_rows);

        let gpu_trace_point_values =
            direct_trace_mask_values_from_committed_trees(&gpu_commitment_scheme, &trace_points)
                .expect("GPU post-composition trace-point values should be available");
        let cpu_trace_point_values =
            cpu_trace_point_values_from_generated_traces(
                config,
                log_n_rows,
                &trace_points,
                cpu_main_trace,
                cpu_interaction_trace,
            );

        let main_mismatches = count_trace_mask_value_mismatches(
            &TreeVec(vec![gpu_trace_point_values[MAIN_TRACE_IDX].clone()]),
            &TreeVec(vec![cpu_trace_point_values[MAIN_TRACE_IDX].clone()]),
        );
        let interaction_mismatches = count_trace_mask_value_mismatches(
            &TreeVec(vec![gpu_trace_point_values[INTERACTION_TRACE_IDX].clone()]),
            &TreeVec(vec![cpu_trace_point_values[INTERACTION_TRACE_IDX].clone()]),
        );

        eprintln!(
            "poseidon_post_composition_trace_point_value_parity main_mismatches={} interaction_mismatches={}",
            main_mismatches, interaction_mismatches,
        );

        assert_eq!(
            main_mismatches, 0,
            "supported Poseidon main-trace committed point values drifted after composition commit on {} masks",
            main_mismatches
        );
        assert_eq!(
            interaction_mismatches, 0,
            "supported Poseidon interaction-trace committed point values drifted after composition commit on {} masks",
            interaction_mismatches
        );
    }

    #[test]
    #[ignore = "M33 blocker characterization lane; run explicitly on a CUDA host"]
    fn supported_poseidon_prove_core_snapshot() {
        if std::env::var("STWO_CUDA_MODE")
            .map(|mode| mode == "no-cuda")
            .unwrap_or(false)
        {
            eprintln!("skipping CUDA-only Poseidon prove-core snapshot on no-cuda host mode");
            return;
        }

        const LOG_N_INSTANCES: u32 = 7;
        let (
            snapshot,
            point_contract_variants,
            point_contribution_split,
            logup_contract_variants,
            logup_only_variants,
            in_pairs_logup_boundary,
            in_pairs_cumsum_variants,
            in_pairs_fraction_assembly_variants,
            in_pairs_denominator_variants,
            algebraic_family_split,
            algebraic_sequence_law,
            algebraic_per_round_split,
            algebraic_per_cell_split,
        ) =
            collect_supported_poseidon_point_contract_snapshot(LOG_N_INSTANCES);

        let sampled_matches_direct =
            snapshot.sampled_composition_oods_eval == snapshot.direct_composition_oods_eval;
        let point_matches_direct =
            snapshot.point_composition_oods_eval == snapshot.direct_composition_oods_eval;
        let sampled_trace_mask_values_match_direct = snapshot
            .sampled_trace_mask_values_match_direct
            .expect("M34 diagnostics should populate direct trace mask values");
        let sampled_point_matches_direct_trace_mask_point = snapshot
            .sampled_point_matches_direct_trace_mask_point
            .expect("M34 diagnostics should populate direct trace mask point eval");
        let cpu_trace_matches_direct = snapshot
            .cpu_trace_matches_direct
            .expect("M38 diagnostics should populate CPU trace composition parity");
        let cpu_trace_matches_point = snapshot
            .cpu_trace_matches_point
            .expect("M38 diagnostics should populate CPU trace composition parity");
        let trace_point_values_match_cpu_committed = snapshot
            .trace_point_values_match_cpu_committed
            .expect("M45 diagnostics should populate committed point-value parity");
        let main_trace_point_values_match_cpu_committed = snapshot
            .main_trace_point_values_match_cpu_committed
            .expect("M45 diagnostics should populate main-trace point-value parity");
        let interaction_trace_point_values_match_cpu_committed = snapshot
            .interaction_trace_point_values_match_cpu_committed
            .expect("M45 diagnostics should populate interaction-trace point-value parity");
        let main_trace_point_value_mismatches = snapshot
            .main_trace_point_value_mismatches
            .expect("M45 diagnostics should populate main-trace mismatch counts");
        let interaction_trace_point_value_mismatches = snapshot
            .interaction_trace_point_value_mismatches
            .expect("M45 diagnostics should populate interaction-trace mismatch counts");
        let sampled_values_match_cpu_committed = snapshot
            .sampled_values_match_cpu_committed
            .expect("M54 diagnostics should populate sampled-value CPU parity");
        let main_sampled_values_match_cpu_committed = snapshot
            .main_sampled_values_match_cpu_committed
            .expect("M54 diagnostics should populate main sampled-value CPU parity");
        let interaction_sampled_values_match_cpu_committed = snapshot
            .interaction_sampled_values_match_cpu_committed
            .expect("M54 diagnostics should populate interaction sampled-value CPU parity");
        let main_sampled_value_mismatches = snapshot
            .main_sampled_value_mismatches
            .expect("M54 diagnostics should populate main sampled-value mismatch counts");
        let interaction_sampled_value_mismatches = snapshot
            .interaction_sampled_value_mismatches
            .expect("M54 diagnostics should populate interaction sampled-value mismatch counts");
        let committed_coefficients_match_cpu = snapshot
            .committed_coefficients_match_cpu
            .expect("M55 diagnostics should populate committed coefficient parity");
        let main_committed_coefficients_match_cpu = snapshot
            .main_committed_coefficients_match_cpu
            .expect("M55 diagnostics should populate main committed coefficient parity");
        let interaction_committed_coefficients_match_cpu = snapshot
            .interaction_committed_coefficients_match_cpu
            .expect("M55 diagnostics should populate interaction committed coefficient parity");
        let main_committed_coefficient_mismatches = snapshot
            .main_committed_coefficient_mismatches
            .expect("M55 diagnostics should populate main committed coefficient mismatch counts");
        let interaction_committed_coefficient_mismatches = snapshot
            .interaction_committed_coefficient_mismatches
            .expect(
                "M55 diagnostics should populate interaction committed coefficient mismatch counts",
            );
        let cpu_trace_composition_oods_eval = snapshot
            .cpu_trace_composition_oods_eval
            .expect("M38 diagnostics should populate CPU trace composition eval");

        eprintln!(
            "point_contract_variants current={:?} trace_denom={:?} trace_points_current={:?} trace_points_trace={:?} trace_points_trace_lifted={:?} cpu_trace={:?}",
            point_contract_variants.current_framework_eval,
            point_contract_variants.trace_denominator_eval,
            point_contract_variants.trace_points_current_denominator_eval,
            point_contract_variants.trace_points_trace_denominator_eval,
            point_contract_variants.trace_points_trace_lifted_denominator_eval,
            cpu_trace_composition_oods_eval,
        );
        eprintln!(
            "point_contribution_split point_full={:?} point_algebraic={:?} point_algebraic_trace_denom={:?} trace_points_algebraic={:?} trace_points_algebraic_trace_denom={:?} point_logup={:?} cpu_full={:?} cpu_algebraic={:?} cpu_logup={:?}",
            point_contribution_split.point_full_eval,
            point_contribution_split.point_algebraic_only_eval,
            point_contribution_split.point_algebraic_trace_denominator_eval,
            point_contribution_split.trace_points_algebraic_only_eval,
            point_contribution_split.trace_points_algebraic_trace_denominator_eval,
            point_contribution_split.point_logup_delta_eval,
            point_contribution_split.cpu_trace_full_eval,
            point_contribution_split.cpu_trace_algebraic_only_eval,
            point_contribution_split.cpu_trace_logup_delta_eval,
        );
        eprintln!(
            "logup_contract_variants trace_points_trace_in_pairs={:?} trace_points_trace_sequential={:?} trace_points_trace_in_pairs_logup={:?} trace_points_trace_sequential_logup={:?} trace_points_trace_sequential_supported={} trace_points_trace_sequential_first_expected_arity={:?} trace_points_trace_sequential_first_actual_arity={:?} cpu_logup={:?}",
            logup_contract_variants.trace_points_trace_in_pairs_eval,
            logup_contract_variants.trace_points_trace_sequential_eval,
            logup_contract_variants.trace_points_trace_in_pairs_logup_delta_eval,
            logup_contract_variants.trace_points_trace_sequential_logup_delta_eval,
            logup_contract_variants.trace_points_trace_sequential_supported,
            logup_contract_variants
                .trace_points_trace_sequential_first_expected_arity,
            logup_contract_variants
                .trace_points_trace_sequential_first_actual_arity,
            logup_contract_variants.cpu_trace_logup_delta_eval,
        );
        eprintln!(
            "logup_only_variants current={:?} trace_denom={:?} trace_points_current={:?} trace_points_trace_in_pairs={:?} cpu={:?}",
            logup_only_variants.current_framework_eval,
            logup_only_variants.trace_denominator_eval,
            logup_only_variants.trace_points_current_denominator_eval,
            logup_only_variants.trace_points_trace_in_pairs_eval,
            logup_only_variants.cpu_trace_eval,
        );
        eprintln!(
            "in_pairs_logup_boundary frac_count={} batch_count={} denom_inverse={:?} cumsum_shift={:?} first_fraction={:?} second_fraction={:?} batched_fraction={:?} prev_row_cumsum={:?} cur_cumsum={:?} shifted_diff={:?} cumsum_times_denominator={:?} numerator={:?} constraint_before_denom_inverse={:?} emitted_evaluation_count={} emitted_horner={:?}",
            in_pairs_logup_boundary.frac_count,
            in_pairs_logup_boundary.batch_count,
            in_pairs_logup_boundary.denom_inverse,
            in_pairs_logup_boundary.cumsum_shift,
            in_pairs_logup_boundary.first_fraction,
            in_pairs_logup_boundary.second_fraction,
            in_pairs_logup_boundary.batched_fraction,
            in_pairs_logup_boundary.prev_row_cumsum,
            in_pairs_logup_boundary.cur_cumsum,
            in_pairs_logup_boundary.shifted_diff,
            in_pairs_logup_boundary.cumsum_times_denominator,
            in_pairs_logup_boundary.numerator,
            in_pairs_logup_boundary.constraint_before_denom_inverse,
            in_pairs_logup_boundary.emitted_evaluation_count,
            in_pairs_logup_boundary.emitted_horner_eval,
        );
        eprintln!(
            "in_pairs_cumsum_variants current={:?} swapped_diff={:?} subtract_shift={:?} swapped_diff_subtract_shift={:?} no_shift={:?} cpu={:?}",
            in_pairs_cumsum_variants.current_eval,
            in_pairs_cumsum_variants.swapped_diff_eval,
            in_pairs_cumsum_variants.subtract_shift_eval,
            in_pairs_cumsum_variants.swapped_diff_subtract_shift_eval,
            in_pairs_cumsum_variants.no_shift_eval,
            in_pairs_cumsum_variants.cpu_trace_eval,
        );
        eprintln!(
            "in_pairs_fraction_assembly_variants current={:?} first_only={:?} second_only={:?} both_positive={:?} swapped_signs={:?} naive_pair_current={:?} naive_pair_positive={:?} cpu={:?}",
            in_pairs_fraction_assembly_variants.current_eval,
            in_pairs_fraction_assembly_variants.first_only_eval,
            in_pairs_fraction_assembly_variants.second_only_eval,
            in_pairs_fraction_assembly_variants.both_positive_eval,
            in_pairs_fraction_assembly_variants.swapped_signs_eval,
            in_pairs_fraction_assembly_variants.naive_pair_current_eval,
            in_pairs_fraction_assembly_variants.naive_pair_positive_eval,
            in_pairs_fraction_assembly_variants.cpu_trace_eval,
        );
        eprintln!(
            "in_pairs_denominator_variants current={:?} first_reversed={:?} second_reversed={:?} both_reversed={:?} cpu={:?}",
            in_pairs_denominator_variants.current_eval,
            in_pairs_denominator_variants.first_reversed_eval,
            in_pairs_denominator_variants.second_reversed_eval,
            in_pairs_denominator_variants.both_reversed_eval,
            in_pairs_denominator_variants.cpu_trace_eval,
        );
        eprintln!(
            "algebraic_family_split point_first_half_full={:?} point_partial_rounds={:?} point_second_half_full={:?} cpu_first_half_full={:?} cpu_partial_rounds={:?} cpu_second_half_full={:?}",
            algebraic_family_split.point_first_half_full_eval,
            algebraic_family_split.point_partial_rounds_eval,
            algebraic_family_split.point_second_half_full_eval,
            algebraic_family_split.cpu_first_half_full_eval,
            algebraic_family_split.cpu_partial_rounds_eval,
            algebraic_family_split.cpu_second_half_full_eval,
        );
        eprintln!(
            "algebraic_sequence_law evaluation_count={} current_horner={:?} reversed_horner={:?} point_algebraic={:?} cpu_algebraic={:?}",
            algebraic_sequence_law.evaluation_count,
            algebraic_sequence_law.current_horner_eval,
            algebraic_sequence_law.reversed_horner_eval,
            algebraic_sequence_law.point_algebraic_only_eval,
            algebraic_sequence_law.cpu_trace_algebraic_only_eval,
        );
        eprintln!(
            "algebraic_per_round_split point_first_half_full={:?} point_partial={:?} point_second_half_full={:?} cpu_first_half_full={:?} cpu_partial={:?} cpu_second_half_full={:?}",
            algebraic_per_round_split.point_first_half_full_round_evals,
            algebraic_per_round_split.point_partial_round_evals,
            algebraic_per_round_split.point_second_half_full_round_evals,
            algebraic_per_round_split.cpu_first_half_full_round_evals,
            algebraic_per_round_split.cpu_partial_round_evals,
            algebraic_per_round_split.cpu_second_half_full_round_evals,
        );
        eprintln!(
            "algebraic_per_cell_split point_first_half_full={:?} point_second_half_full={:?} cpu_first_half_full={:?} cpu_second_half_full={:?}",
            algebraic_per_cell_split.point_first_half_full_cell_evals,
            algebraic_per_cell_split.point_second_half_full_cell_evals,
            algebraic_per_cell_split.cpu_first_half_full_cell_evals,
            algebraic_per_cell_split.cpu_second_half_full_cell_evals,
        );
        eprintln!(
            "sampled_value_cpu_parity all={} main={} interaction={} main_mismatches={} interaction_mismatches={}",
            sampled_values_match_cpu_committed,
            main_sampled_values_match_cpu_committed,
            interaction_sampled_values_match_cpu_committed,
            main_sampled_value_mismatches,
            interaction_sampled_value_mismatches,
        );
        eprintln!(
            "trace_point_value_parity all={} main={} interaction={} main_mismatches={} interaction_mismatches={}",
            trace_point_values_match_cpu_committed,
            main_trace_point_values_match_cpu_committed,
            interaction_trace_point_values_match_cpu_committed,
            main_trace_point_value_mismatches,
            interaction_trace_point_value_mismatches,
        );
        eprintln!(
            "committed_coefficient_parity all={} main={} interaction={} main_mismatches={} interaction_mismatches={}",
            committed_coefficients_match_cpu,
            main_committed_coefficients_match_cpu,
            interaction_committed_coefficients_match_cpu,
            main_committed_coefficient_mismatches,
            interaction_committed_coefficient_mismatches,
        );

        assert!(
            sampled_matches_direct
                && point_matches_direct
                && sampled_trace_mask_values_match_direct
                && sampled_point_matches_direct_trace_mask_point
                && cpu_trace_matches_direct
                && cpu_trace_matches_point,
            "sampled_matches_direct={} point_matches_direct={} sampled_trace_mask_values_match_direct={} sampled_point_matches_direct_trace_mask_point={} cpu_trace_matches_direct={} cpu_trace_matches_point={} sampled_values_match_cpu_committed={} main_sampled_values_match_cpu_committed={} interaction_sampled_values_match_cpu_committed={} main_sampled_value_mismatches={} interaction_sampled_value_mismatches={} trace_point_values_match_cpu_committed={} main_trace_point_values_match_cpu_committed={} interaction_trace_point_values_match_cpu_committed={} main_trace_point_value_mismatches={} interaction_trace_point_value_mismatches={} sampled={:?} direct={:?} point={:?} cpu_trace={:?} direct_trace_mask_point={:?} variants={{current:{:?}, trace_denom:{:?}, trace_points_current:{:?}, trace_points_trace:{:?}, trace_points_trace_lifted:{:?}}} split={{point_full:{:?}, point_algebraic:{:?}, point_algebraic_trace_denom:{:?}, trace_points_algebraic:{:?}, trace_points_algebraic_trace_denom:{:?}, point_logup:{:?}, cpu_full:{:?}, cpu_algebraic:{:?}, cpu_logup:{:?}}} logup_variants={{trace_points_trace_in_pairs:{:?}, trace_points_trace_sequential:{:?}, trace_points_trace_in_pairs_logup:{:?}, trace_points_trace_sequential_logup:{:?}, trace_points_trace_sequential_supported:{}, trace_points_trace_sequential_first_expected_arity:{:?}, trace_points_trace_sequential_first_actual_arity:{:?}, cpu_logup:{:?}}} logup_only={{current:{:?}, trace_denom:{:?}, trace_points_current:{:?}, trace_points_trace_in_pairs:{:?}, cpu:{:?}}} in_pairs_logup_boundary={{frac_count:{}, batch_count:{}, denom_inverse:{:?}, cumsum_shift:{:?}, first_fraction:{:?}, second_fraction:{:?}, batched_fraction:{:?}, prev_row_cumsum:{:?}, cur_cumsum:{:?}, shifted_diff:{:?}, cumsum_times_denominator:{:?}, numerator:{:?}, constraint_before_denom_inverse:{:?}, emitted_evaluation_count:{}, emitted_horner:{:?}}} in_pairs_cumsum_variants={{current:{:?}, swapped_diff:{:?}, subtract_shift:{:?}, swapped_diff_subtract_shift:{:?}, no_shift:{:?}, cpu:{:?}}} in_pairs_fraction_assembly_variants={{current:{:?}, first_only:{:?}, second_only:{:?}, both_positive:{:?}, swapped_signs:{:?}, naive_pair_current:{:?}, naive_pair_positive:{:?}, cpu:{:?}}} in_pairs_denominator_variants={{current:{:?}, first_reversed:{:?}, second_reversed:{:?}, both_reversed:{:?}, cpu:{:?}}} family_split={{point_first_half_full:{:?}, point_partial_rounds:{:?}, point_second_half_full:{:?}, cpu_first_half_full:{:?}, cpu_partial_rounds:{:?}, cpu_second_half_full:{:?}}} sequence_law={{evaluation_count:{}, current_horner:{:?}, reversed_horner:{:?}, point_algebraic:{:?}, cpu_algebraic:{:?}}} per_round={{point_first_half_full:{:?}, point_partial:{:?}, point_second_half_full:{:?}, cpu_first_half_full:{:?}, cpu_partial:{:?}, cpu_second_half_full:{:?}}} per_cell={{point_first_half_full:{:?}, point_second_half_full:{:?}, cpu_first_half_full:{:?}, cpu_second_half_full:{:?}}}",
            sampled_matches_direct,
            point_matches_direct,
            sampled_trace_mask_values_match_direct,
            sampled_point_matches_direct_trace_mask_point,
            cpu_trace_matches_direct,
            cpu_trace_matches_point,
            sampled_values_match_cpu_committed,
            main_sampled_values_match_cpu_committed,
            interaction_sampled_values_match_cpu_committed,
            main_sampled_value_mismatches,
            interaction_sampled_value_mismatches,
            trace_point_values_match_cpu_committed,
            main_trace_point_values_match_cpu_committed,
            interaction_trace_point_values_match_cpu_committed,
            main_trace_point_value_mismatches,
            interaction_trace_point_value_mismatches,
            snapshot.sampled_composition_oods_eval,
            snapshot.direct_composition_oods_eval,
            snapshot.point_composition_oods_eval,
            cpu_trace_composition_oods_eval,
            snapshot.direct_trace_mask_point_composition_oods_eval,
            point_contract_variants.current_framework_eval,
            point_contract_variants.trace_denominator_eval,
            point_contract_variants.trace_points_current_denominator_eval,
            point_contract_variants.trace_points_trace_denominator_eval,
            point_contract_variants.trace_points_trace_lifted_denominator_eval,
            point_contribution_split.point_full_eval,
            point_contribution_split.point_algebraic_only_eval,
            point_contribution_split.point_algebraic_trace_denominator_eval,
            point_contribution_split.trace_points_algebraic_only_eval,
            point_contribution_split.trace_points_algebraic_trace_denominator_eval,
            point_contribution_split.point_logup_delta_eval,
            point_contribution_split.cpu_trace_full_eval,
            point_contribution_split.cpu_trace_algebraic_only_eval,
            point_contribution_split.cpu_trace_logup_delta_eval,
            logup_contract_variants.trace_points_trace_in_pairs_eval,
            logup_contract_variants.trace_points_trace_sequential_eval,
            logup_contract_variants.trace_points_trace_in_pairs_logup_delta_eval,
            logup_contract_variants.trace_points_trace_sequential_logup_delta_eval,
            logup_contract_variants.trace_points_trace_sequential_supported,
            logup_contract_variants
                .trace_points_trace_sequential_first_expected_arity,
            logup_contract_variants
                .trace_points_trace_sequential_first_actual_arity,
            logup_contract_variants.cpu_trace_logup_delta_eval,
            logup_only_variants.current_framework_eval,
            logup_only_variants.trace_denominator_eval,
            logup_only_variants.trace_points_current_denominator_eval,
            logup_only_variants.trace_points_trace_in_pairs_eval,
            logup_only_variants.cpu_trace_eval,
            in_pairs_logup_boundary.frac_count,
            in_pairs_logup_boundary.batch_count,
            in_pairs_logup_boundary.denom_inverse,
            in_pairs_logup_boundary.cumsum_shift,
            in_pairs_logup_boundary.first_fraction,
            in_pairs_logup_boundary.second_fraction,
            in_pairs_logup_boundary.batched_fraction,
            in_pairs_logup_boundary.prev_row_cumsum,
            in_pairs_logup_boundary.cur_cumsum,
            in_pairs_logup_boundary.shifted_diff,
            in_pairs_logup_boundary.cumsum_times_denominator,
            in_pairs_logup_boundary.numerator,
            in_pairs_logup_boundary.constraint_before_denom_inverse,
            in_pairs_logup_boundary.emitted_evaluation_count,
            in_pairs_logup_boundary.emitted_horner_eval,
            in_pairs_cumsum_variants.current_eval,
            in_pairs_cumsum_variants.swapped_diff_eval,
            in_pairs_cumsum_variants.subtract_shift_eval,
            in_pairs_cumsum_variants.swapped_diff_subtract_shift_eval,
            in_pairs_cumsum_variants.no_shift_eval,
            in_pairs_cumsum_variants.cpu_trace_eval,
            in_pairs_fraction_assembly_variants.current_eval,
            in_pairs_fraction_assembly_variants.first_only_eval,
            in_pairs_fraction_assembly_variants.second_only_eval,
            in_pairs_fraction_assembly_variants.both_positive_eval,
            in_pairs_fraction_assembly_variants.swapped_signs_eval,
            in_pairs_fraction_assembly_variants.naive_pair_current_eval,
            in_pairs_fraction_assembly_variants.naive_pair_positive_eval,
            in_pairs_fraction_assembly_variants.cpu_trace_eval,
            in_pairs_denominator_variants.current_eval,
            in_pairs_denominator_variants.first_reversed_eval,
            in_pairs_denominator_variants.second_reversed_eval,
            in_pairs_denominator_variants.both_reversed_eval,
            in_pairs_denominator_variants.cpu_trace_eval,
            algebraic_family_split.point_first_half_full_eval,
            algebraic_family_split.point_partial_rounds_eval,
            algebraic_family_split.point_second_half_full_eval,
            algebraic_family_split.cpu_first_half_full_eval,
            algebraic_family_split.cpu_partial_rounds_eval,
            algebraic_family_split.cpu_second_half_full_eval,
            algebraic_sequence_law.evaluation_count,
            algebraic_sequence_law.current_horner_eval,
            algebraic_sequence_law.reversed_horner_eval,
            algebraic_sequence_law.point_algebraic_only_eval,
            algebraic_sequence_law.cpu_trace_algebraic_only_eval,
            algebraic_per_round_split.point_first_half_full_round_evals,
            algebraic_per_round_split.point_partial_round_evals,
            algebraic_per_round_split.point_second_half_full_round_evals,
            algebraic_per_round_split.cpu_first_half_full_round_evals,
            algebraic_per_round_split.cpu_partial_round_evals,
            algebraic_per_round_split.cpu_second_half_full_round_evals,
            algebraic_per_cell_split.point_first_half_full_cell_evals,
            algebraic_per_cell_split.point_second_half_full_cell_evals,
            algebraic_per_cell_split.cpu_first_half_full_cell_evals,
            algebraic_per_cell_split.cpu_second_half_full_cell_evals,
        );
    }

    #[test]
    #[ignore = "M54 verifier-boundary characterization lane; run explicitly on a CUDA host"]
    fn supported_poseidon_proof_verifier_boundary_snapshot() {
        if std::env::var("STWO_CUDA_MODE")
            .map(|mode| mode == "no-cuda")
            .unwrap_or(false)
        {
            eprintln!("skipping CUDA-only Poseidon verifier-boundary snapshot on no-cuda host mode");
            return;
        }

        const LOG_N_INSTANCES: u32 = 7;
        let config = PcsConfig::default();
        let (component, proof, _sentinel, _breakdown, snapshot) =
            prove_poseidon_blake_snapshot(LOG_N_INSTANCES, config);

        eprintln!(
            "verifier_boundary sampled={:?} point={:?}",
            snapshot.sampled_composition_oods_eval,
            snapshot.point_composition_oods_eval,
        );

        verify_poseidon_blake(&component, &proof)
            .expect("supported Poseidon proof should verify across the verifier boundary");
    }
}
