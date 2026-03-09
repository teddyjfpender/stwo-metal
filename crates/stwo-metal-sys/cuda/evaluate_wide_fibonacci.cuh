#ifndef EVALUATE_WIDE_FIBONACCI_H
#define EVALUATE_WIDE_FIBONACCI_H

#include "fields.cuh"
#include "utils.cuh"
#include "component_abi_v1_generated.cuh"

using WideFibEval = StwoCudaWideFibonacciEvalAbiV1;

extern "C"
void evaluate_wide_fibonacci_constraint_quotients_on_domain(
    m31 *quotients_0, m31 *quotients_1, m31 *quotients_2, m31 *quotients_3,
    const m31 *const *trace0_evaluations,
    unsigned trace0_evaluations_len,
    const m31 *const *trace1_evaluations,
    unsigned trace1_evaluations_len,
    const qm31 *random_coeff_powers,
    const m31 *denominator_inverses,
    unsigned int domain_log_size,
    unsigned int eval_domain_log_size,
    unsigned int number_of_columns,
    unsigned int logup_counts
);

extern "C"
void stwo_cuda_dispatch_constraint_eval_fibonacci_example_v1(
    const StwoCudaConstraintEvalRequestV1 *request,
    cudaStream_t stream
);

extern "C"
void generate_wide_fibonacci_trace(
    m31 *input_a,
    m31 *input_b,
    unsigned input_len,
    m31 **traces,
    unsigned trace_len,
    unsigned n_columns
);

#endif // WIDE_FIBONACCI_H
