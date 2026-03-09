#ifndef STWO_CUDA_COMPONENT_ABI_V1_GENERATED_H
#define STWO_CUDA_COMPONENT_ABI_V1_GENERATED_H

#include <cstddef>

#include "fields.cuh"
#include "logup.cuh"

struct StwoCudaCommonEvalAbiV1 {
    unsigned eval_id;
    unsigned log_n_rows;
};

struct StwoCudaWideFibonacciEvalAbiV1 {
    StwoCudaCommonEvalAbiV1 common;
};

struct StwoCudaPoseidonEvalAbiV1 {
    StwoCudaCommonEvalAbiV1 common;
    LookupElementsBasic<16> lookup_elements;
    qm31 claimed_sum;
};

struct StwoCudaConstraintEvalRequestV1 {
    m31 *quotient_columns[4];
    const m31 *const *trace0_evaluations;
    unsigned trace0_evaluations_len;
    const m31 *const *trace1_evaluations;
    unsigned trace1_evaluations_len;
    const m31 *const *trace2_evaluations;
    unsigned trace2_evaluations_len;
    const qm31 *random_coeff_powers;
    const m31 *denominator_inverses;
    unsigned domain_log_size;
    unsigned eval_domain_log_size;
    unsigned number_of_columns;
    unsigned logup_counts;
    const void *eval;
    qm31 cumsum_shift;
    bool should_accumulate;
    bool use_assert_evaluator;
};

static_assert(sizeof(StwoCudaCommonEvalAbiV1) == 8, "Unexpected StwoCudaCommonEvalAbiV1 size");
static_assert(sizeof(StwoCudaWideFibonacciEvalAbiV1) == 8, "Unexpected StwoCudaWideFibonacciEvalAbiV1 size");
static_assert(sizeof(StwoCudaPoseidonEvalAbiV1) == 312, "Unexpected StwoCudaPoseidonEvalAbiV1 size");
static_assert(sizeof(StwoCudaConstraintEvalRequestV1) == 144, "Unexpected StwoCudaConstraintEvalRequestV1 size");
static_assert(alignof(StwoCudaConstraintEvalRequestV1) == 8, "Unexpected StwoCudaConstraintEvalRequestV1 alignment");
static_assert(offsetof(StwoCudaConstraintEvalRequestV1, eval) == 112, "Unexpected StwoCudaConstraintEvalRequestV1 eval offset");
static_assert(offsetof(StwoCudaConstraintEvalRequestV1, cumsum_shift) == 120, "Unexpected StwoCudaConstraintEvalRequestV1 cumsum_shift offset");
static_assert(offsetof(StwoCudaConstraintEvalRequestV1, should_accumulate) == 136, "Unexpected StwoCudaConstraintEvalRequestV1 should_accumulate offset");

#endif // STWO_CUDA_COMPONENT_ABI_V1_GENERATED_H
