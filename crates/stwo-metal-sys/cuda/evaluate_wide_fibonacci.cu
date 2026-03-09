#include "fields.cuh"
#include "evaluate_wide_fibonacci.cuh"
#include "evaluate_poseidon_constraint.cuh"
#include "timer.cuh"
#include <cstdlib>

namespace {
constexpr int WIDE_FIBONACCI_THREAD_COUNT_MAX = 256;

struct WideFibonacciLaunchGeometry {
    int block_dim;
    int num_blocks;
};

WideFibonacciLaunchGeometry compute_wide_fibonacci_launch_geometry(unsigned work_items) {
    const int block_dim = work_items < static_cast<unsigned>(WIDE_FIBONACCI_THREAD_COUNT_MAX)
        ? static_cast<int>(work_items)
        : WIDE_FIBONACCI_THREAD_COUNT_MAX;
    return {
        block_dim,
        block_dim == 0 ? 0 : static_cast<int>((work_items + static_cast<unsigned>(block_dim) - 1) / static_cast<unsigned>(block_dim)),
    };
}

bool wide_fibonacci_debug_enabled() {
    const char *value = std::getenv("STWO_CUDA_DEBUG_WIDE_FIBONACCI");
    return value != nullptr && value[0] != '\0' && value[0] != '0';
}

void log_wide_fibonacci_launch_state(
    unsigned trace0_evaluations_len,
    unsigned trace1_evaluations_len,
    unsigned domain_log_size,
    unsigned eval_domain_log_size,
    unsigned number_of_columns,
    unsigned logup_counts,
    int block_dim,
    int num_blocks,
    const m31 *const *trace0_evaluations,
    const m31 *const *trace1_evaluations,
    const qm31 *random_coeff_powers,
    const m31 *denominator_inverses,
    const m31 **device_trace0_evaluations,
    const m31 **device_trace1_evaluations,
    qm31 *numerators,
    Fraction *intermediate_fractions
) {
    if (!wide_fibonacci_debug_enabled()) {
        return;
    }

    const unsigned eval_domain_size = 1u << eval_domain_log_size;
    fprintf(
        stderr,
        "[wide_fib_debug] trace0_len=%u trace1_len=%u domain_log=%u eval_domain_log=%u eval_domain_size=%u number_of_columns=%u logup_counts=%u block_dim=%d num_blocks=%d\n",
        trace0_evaluations_len,
        trace1_evaluations_len,
        domain_log_size,
        eval_domain_log_size,
        eval_domain_size,
        number_of_columns,
        logup_counts,
        block_dim,
        num_blocks
    );
    fprintf(
        stderr,
        "[wide_fib_debug] host trace0=%p first0=%p host trace1=%p first1=%p random_coeff=%p denom=%p\n",
        static_cast<const void*>(trace0_evaluations),
        trace0_evaluations_len == 0 ? nullptr : static_cast<const void*>(trace0_evaluations[0]),
        static_cast<const void*>(trace1_evaluations),
        trace1_evaluations_len == 0 ? nullptr : static_cast<const void*>(trace1_evaluations[0]),
        static_cast<const void*>(random_coeff_powers),
        static_cast<const void*>(denominator_inverses)
    );
    fprintf(
        stderr,
        "[wide_fib_debug] device trace0=%p device trace1=%p numerators=%p fractions=%p prelaunch=%s\n",
        static_cast<void*>(device_trace0_evaluations),
        static_cast<void*>(device_trace1_evaluations),
        static_cast<void*>(numerators),
        static_cast<void*>(intermediate_fractions),
        cudaGetErrorName(cudaPeekAtLastError())
    );
}
}  // namespace

__launch_bounds__(256, 2)
__global__ void evaluate_wide_fibonacci_constraint_quotients_kernel(
    m31 *quotients_0, m31 *quotients_1, m31 *quotients_2, m31 *quotients_3,
    const m31 *const *trace0_evaluations,
    const m31 *const *trace1_evaluations,
    qm31 *numerators,
    const qm31 *random_coeff_powers,
    const m31 *denominator_inverses,
    unsigned domain_log_size,
    unsigned int eval_domain_log_size,
    unsigned int number_of_columns,
    Fraction *intermediate_fractions,
    unsigned logup_counts
) {
    const unsigned eval_domain_size = 1u << eval_domain_log_size;
    const unsigned row = threadIdx.x + blockDim.x * blockIdx.x;
    if (row >= eval_domain_size) return;

    CudaEvaluator cuda_evaluator(
        trace1_evaluations,
        random_coeff_powers,
        0,
        row,
        {0},
        0,
        {0},
        domain_log_size,
        eval_domain_log_size,
        intermediate_fractions,
        logup_counts
    );

    if (row < eval_domain_size) {
        m31 a = cuda_evaluator.next_trace_mask();
        m31 b = cuda_evaluator.next_trace_mask();

        for (unsigned instance_index = 2; instance_index < number_of_columns + 2; instance_index++) {
            m31 c = cuda_evaluator.next_trace_mask();

            cuda_evaluator.add_constraint(sub(c, add(square(a), square(b))));
            a = b;
            b = c;
        }
        numerators[row] = cuda_evaluator.row_res;

        m31 denom_inv = denominator_inverses[row >> domain_log_size];
        qm31 constraint_quotient = mul(
            denom_inv,
            numerators[row]
        );

        quotients_0[row] = constraint_quotient.a.a;
        quotients_1[row] = constraint_quotient.a.b;
        quotients_2[row] = constraint_quotient.b.a;
        quotients_3[row] = constraint_quotient.b.b;

    }
}

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
) {
    unsigned eval_domain_size = 1 << (eval_domain_log_size);
    const m31 **device_trace0_evaluations = cuda_proving_clone_to_device<const m31*>(trace0_evaluations, trace0_evaluations_len);
    const m31 **device_trace1_evaluations = cuda_proving_clone_to_device<const m31*>(trace1_evaluations, trace1_evaluations_len);
    qm31 *numerators = reinterpret_cast<qm31*>(cuda_proving_alloc_zeroes_u32_words(4 * eval_domain_size));

    const auto launch_geometry = compute_wide_fibonacci_launch_geometry(eval_domain_size);
    int block_dim = launch_geometry.block_dim;
    int num_blocks = launch_geometry.num_blocks;

    // Wide Fibonacci currently has no logup interactions, so a zero count means there is no
    // intermediate fraction buffer to materialize for this launch.
    Fraction *d_intermediate_fractions =
        logup_counts == 0 ? nullptr : cuda_proving_malloc<Fraction>(eval_domain_size * logup_counts);

    log_wide_fibonacci_launch_state(
        trace0_evaluations_len,
        trace1_evaluations_len,
        domain_log_size,
        eval_domain_log_size,
        number_of_columns,
        logup_counts,
        block_dim,
        num_blocks,
        trace0_evaluations,
        trace1_evaluations,
        random_coeff_powers,
        denominator_inverses,
        device_trace0_evaluations,
        device_trace1_evaluations,
        numerators,
        d_intermediate_fractions
    );

    timer global_timer;
    // global_timer.start("evaluate_wide_fibonacci_constraint_quotients_on_domain");
    evaluate_wide_fibonacci_constraint_quotients_kernel<<<num_blocks, block_dim>>>(
        quotients_0, quotients_1, quotients_2, quotients_3,
        device_trace0_evaluations,
        device_trace1_evaluations,
        numerators,
        random_coeff_powers,
        denominator_inverses,
        domain_log_size,
        eval_domain_log_size,
        number_of_columns,
        d_intermediate_fractions,
        logup_counts
    );
    cudaError_t launch_err = cudaPeekAtLastError();
    if (wide_fibonacci_debug_enabled()) {
        fprintf(stderr, "[wide_fib_debug] launch_err=%s\n", cudaGetErrorName(launch_err));
    }
    cudaError_t sync_err = cudaDeviceSynchronize();
    if (wide_fibonacci_debug_enabled()) {
        fprintf(stderr, "[wide_fib_debug] sync_err=%s\n", cudaGetErrorName(sync_err));
    }
    ASSERT_CUDA_SUCCESS(sync_err);
    cudaError_t post_sync_err = cudaGetLastError();
    if (wide_fibonacci_debug_enabled()) {
        fprintf(stderr, "[wide_fib_debug] post_sync_err=%s\n", cudaGetErrorName(post_sync_err));
    }
    ASSERT_CUDA_SUCCESS(post_sync_err);
    // global_timer.end("evaluate_wide_fibonacci_constraint_quotients_on_domain");

    cuda_proving_free(device_trace0_evaluations);
    cuda_proving_free(device_trace1_evaluations);
    cuda_proving_free(numerators);
    if (d_intermediate_fractions != nullptr) {
        cuda_proving_free(d_intermediate_fractions);
    }
}

extern "C"
void stwo_cuda_dispatch_constraint_eval_fibonacci_example_v1(
    const StwoCudaConstraintEvalRequestV1 *request,
    cudaStream_t stream
) {
    (void) stream;

    evaluate_wide_fibonacci_constraint_quotients_on_domain(
        request->quotient_columns[0],
        request->quotient_columns[1],
        request->quotient_columns[2],
        request->quotient_columns[3],
        request->trace0_evaluations,
        request->trace0_evaluations_len,
        request->trace1_evaluations,
        request->trace1_evaluations_len,
        request->random_coeff_powers,
        request->denominator_inverses,
        request->domain_log_size,
        request->eval_domain_log_size,
        request->number_of_columns,
        request->logup_counts
    );
}

__launch_bounds__(256, 2)
__global__ void generate_wide_fibonacci_trace_kernel(
    m31 *input_a,
    m31 *input_b,
    unsigned input_len,
    m31 **device_trace,
    unsigned columns
) {
    int row_index = blockIdx.x * blockDim.x + threadIdx.x;

    if (row_index < input_len) {
        device_trace[0][row_index] = input_a[row_index];
        device_trace[1][row_index] = input_b[row_index];
        for (int i = 2; i < columns; i++) {
            device_trace[i][row_index] = add(square(device_trace[i - 2][row_index]), square(device_trace[i - 1][row_index]));
        }
    }
}

void generate_wide_fibonacci_trace(
    m31 *input_a,
    m31 *input_b,
    unsigned input_len,
    m31 **traces,
    unsigned traces_len,
    unsigned n_columns
) {
    m31 **device_trace = cuda_proving_clone_to_device<m31*>(traces, traces_len);

    const auto launch_geometry = compute_wide_fibonacci_launch_geometry(input_len);
    int block_dim = launch_geometry.block_dim;
    int num_blocks = launch_geometry.num_blocks;

    generate_wide_fibonacci_trace_kernel<<<num_blocks, block_dim>>>(
        input_a,
        input_b,
        input_len,
        device_trace,
        n_columns
    );

    ASSERT_CUDA_SUCCESS(cudaDeviceSynchronize());
    ASSERT_CUDA_SUCCESS(cudaGetLastError());

    cuda_proving_free(device_trace);
}
