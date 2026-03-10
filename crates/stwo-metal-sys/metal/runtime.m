#import <Foundation/Foundation.h>
#import <Metal/Metal.h>
#import <dispatch/dispatch.h>

#include <stdbool.h>
#include <stdint.h>
#include <string.h>

@interface StwoMetalRuntimeBox : NSObject
@property(nonatomic, strong) id<MTLDevice> device;
@property(nonatomic, strong) id<MTLCommandQueue> queue;
@property(nonatomic, strong) id<MTLLibrary> library;
@property(nonatomic, strong) NSMutableDictionary<NSString *, id<MTLComputePipelineState>> *pipelines;
@end

@implementation StwoMetalRuntimeBox
@end

@interface StwoMetalBufferBox : NSObject
@property(nonatomic, strong) id<MTLBuffer> buffer;
@property(nonatomic, assign) NSUInteger len;
@end

@implementation StwoMetalBufferBox
@end

typedef struct {
    uint32_t initial_x;
    uint32_t initial_y;
    uint32_t step_x;
    uint32_t step_y;
    uint32_t offset;
    uint32_t level_log_size;
} StwoMetalTwiddleLevelParams;

static void stwo_metal_write_error(char *dst, size_t dst_len, NSString *message) {
    if (dst == NULL || dst_len == 0) {
        return;
    }

    const char *utf8 = message.UTF8String;
    if (utf8 == NULL) {
        utf8 = "unknown Metal runtime error";
    }

    size_t copy_len = strnlen(utf8, dst_len - 1);
    memcpy(dst, utf8, copy_len);
    dst[copy_len] = '\0';
}

static StwoMetalRuntimeBox *stwo_metal_runtime_box(void *runtime) {
    return (__bridge StwoMetalRuntimeBox *)runtime;
}

static StwoMetalBufferBox *stwo_metal_buffer_box(void *buffer) {
    return (__bridge StwoMetalBufferBox *)buffer;
}

static id<MTLComputePipelineState> stwo_metal_pipeline(
    StwoMetalRuntimeBox *runtime,
    NSString *name,
    char *error_message,
    size_t error_message_len
) {
    @synchronized(runtime) {
        id<MTLComputePipelineState> pipeline = runtime.pipelines[name];
        if (pipeline != nil) {
            return pipeline;
        }

        id<MTLFunction> function = [runtime.library newFunctionWithName:name];
        if (function == nil) {
            stwo_metal_write_error(error_message, error_message_len, [NSString stringWithFormat:@"Missing Metal kernel '%@'.", name]);
            return nil;
        }

        NSError *error = nil;
        pipeline = [runtime.device newComputePipelineStateWithFunction:function error:&error];
        if (pipeline == nil) {
            stwo_metal_write_error(error_message, error_message_len, error.localizedDescription ?: @"Failed to compile Metal pipeline state.");
            return nil;
        }

        runtime.pipelines[name] = pipeline;
        return pipeline;
    }
}

static NSUInteger stwo_metal_threads_per_group(id<MTLComputePipelineState> pipeline) {
    NSUInteger threadgroup_width = pipeline.threadExecutionWidth > 0 ? pipeline.threadExecutionWidth : 1;
    NSUInteger max_threads = pipeline.maxTotalThreadsPerThreadgroup > 0 ? pipeline.maxTotalThreadsPerThreadgroup : 1;
    return MIN((NSUInteger)256, MAX(threadgroup_width, max_threads));
}

static bool stwo_metal_dispatch_unary_u32_kernel(
    StwoMetalRuntimeBox *runtime,
    NSString *kernel_name,
    StwoMetalBufferBox *buffer,
    uint32_t log_len,
    char *error_message,
    size_t error_message_len
) {
    id<MTLComputePipelineState> pipeline = stwo_metal_pipeline(runtime, kernel_name, error_message, error_message_len);
    if (pipeline == nil) {
        return false;
    }

    id<MTLCommandBuffer> command_buffer = [runtime.queue commandBuffer];
    if (command_buffer == nil) {
        stwo_metal_write_error(error_message, error_message_len, @"Failed to create Metal command buffer.");
        return false;
    }

    id<MTLComputeCommandEncoder> encoder = [command_buffer computeCommandEncoder];
    if (encoder == nil) {
        stwo_metal_write_error(error_message, error_message_len, @"Failed to create Metal compute encoder.");
        return false;
    }

    NSUInteger len = ((NSUInteger)1) << log_len;
    [encoder setComputePipelineState:pipeline];
    [encoder setBuffer:buffer.buffer offset:0 atIndex:0];
    [encoder setBytes:&log_len length:sizeof(log_len) atIndex:1];

    MTLSize grid_size = MTLSizeMake(len, 1, 1);
    MTLSize threadgroup_size = MTLSizeMake(stwo_metal_threads_per_group(pipeline), 1, 1);
    [encoder dispatchThreads:grid_size threadsPerThreadgroup:threadgroup_size];
    [encoder endEncoding];

    [command_buffer commit];
    [command_buffer waitUntilCompleted];

    if (command_buffer.status == MTLCommandBufferStatusError) {
        stwo_metal_write_error(error_message, error_message_len, command_buffer.error.localizedDescription ?: @"Metal kernel execution failed.");
        return false;
    }

    return true;
}

void *stwo_metal_runtime_create(
    const uint8_t *metallib_bytes,
    size_t metallib_len,
    char *error_message,
    size_t error_message_len
) {
    @autoreleasepool {
        id<MTLDevice> device = MTLCreateSystemDefaultDevice();
        if (device == nil) {
            stwo_metal_write_error(error_message, error_message_len, @"No Metal device is available.");
            return NULL;
        }

        dispatch_data_t library_data =
            dispatch_data_create(metallib_bytes, metallib_len, dispatch_get_main_queue(), ^{
            });
        NSError *error = nil;
        id<MTLLibrary> library = [device newLibraryWithData:library_data error:&error];
        if (library == nil) {
            stwo_metal_write_error(error_message, error_message_len, error.localizedDescription ?: @"Failed to load embedded Metal library.");
            return NULL;
        }

        id<MTLCommandQueue> queue = [device newCommandQueue];
        if (queue == nil) {
            stwo_metal_write_error(error_message, error_message_len, @"Failed to create Metal command queue.");
            return NULL;
        }

        StwoMetalRuntimeBox *runtime = [StwoMetalRuntimeBox new];
        runtime.device = device;
        runtime.queue = queue;
        runtime.library = library;
        runtime.pipelines = [NSMutableDictionary dictionary];
        return (__bridge_retained void *)runtime;
    }
}

void stwo_metal_runtime_destroy(void *runtime) {
    if (runtime == NULL) {
        return;
    }
    @autoreleasepool {
        __unused id released = (__bridge_transfer id)runtime;
    }
}

void *stwo_metal_u32_buffer_from_host(
    void *runtime_ptr,
    const uint32_t *host_ptr,
    size_t len,
    char *error_message,
    size_t error_message_len
) {
    @autoreleasepool {
        StwoMetalRuntimeBox *runtime = stwo_metal_runtime_box(runtime_ptr);
        NSUInteger bytes = len * sizeof(uint32_t);
        id<MTLBuffer> buffer = [runtime.device newBufferWithLength:bytes options:MTLResourceStorageModeShared];
        if (buffer == nil) {
            stwo_metal_write_error(error_message, error_message_len, @"Failed to allocate Metal buffer.");
            return NULL;
        }

        if (bytes > 0) {
            memcpy(buffer.contents, host_ptr, bytes);
        }

        StwoMetalBufferBox *box = [StwoMetalBufferBox new];
        box.buffer = buffer;
        box.len = len;
        return (__bridge_retained void *)box;
    }
}

void *stwo_metal_u32_buffer_alloc_zeroed(
    void *runtime_ptr,
    size_t len,
    char *error_message,
    size_t error_message_len
) {
    @autoreleasepool {
        StwoMetalRuntimeBox *runtime = stwo_metal_runtime_box(runtime_ptr);
        NSUInteger bytes = len * sizeof(uint32_t);
        id<MTLBuffer> buffer = [runtime.device newBufferWithLength:bytes options:MTLResourceStorageModeShared];
        if (buffer == nil) {
            stwo_metal_write_error(error_message, error_message_len, @"Failed to allocate Metal buffer.");
            return NULL;
        }

        if (bytes > 0) {
            memset(buffer.contents, 0, bytes);
        }

        StwoMetalBufferBox *box = [StwoMetalBufferBox new];
        box.buffer = buffer;
        box.len = len;
        return (__bridge_retained void *)box;
    }
}

void *stwo_metal_u32_buffer_alloc_uninitialized(
    void *runtime_ptr,
    size_t len,
    char *error_message,
    size_t error_message_len
) {
    @autoreleasepool {
        StwoMetalRuntimeBox *runtime = stwo_metal_runtime_box(runtime_ptr);
        NSUInteger bytes = len * sizeof(uint32_t);
        id<MTLBuffer> buffer = [runtime.device newBufferWithLength:bytes options:MTLResourceStorageModeShared];
        if (buffer == nil) {
            stwo_metal_write_error(error_message, error_message_len, @"Failed to allocate Metal buffer.");
            return NULL;
        }

        StwoMetalBufferBox *box = [StwoMetalBufferBox new];
        box.buffer = buffer;
        box.len = len;
        return (__bridge_retained void *)box;
    }
}

void stwo_metal_u32_buffer_destroy(void *buffer) {
    if (buffer == NULL) {
        return;
    }
    @autoreleasepool {
        __unused id released = (__bridge_transfer id)buffer;
    }
}

bool stwo_metal_u32_buffer_read(
    void *runtime_ptr,
    void *buffer_ptr,
    uint32_t *host_ptr,
    size_t len,
    char *error_message,
    size_t error_message_len
) {
    @autoreleasepool {
        (void)runtime_ptr;
        StwoMetalBufferBox *buffer = stwo_metal_buffer_box(buffer_ptr);
        if (len > buffer.len) {
            stwo_metal_write_error(error_message, error_message_len, @"Requested read exceeds Metal buffer length.");
            return false;
        }

        if (len > 0) {
            memcpy(host_ptr, buffer.buffer.contents, len * sizeof(uint32_t));
        }
        return true;
    }
}

uint32_t stwo_metal_u32_buffer_get(void *buffer_ptr, size_t index) {
    @autoreleasepool {
        StwoMetalBufferBox *buffer = stwo_metal_buffer_box(buffer_ptr);
        return ((uint32_t *)buffer.buffer.contents)[index];
    }
}

void stwo_metal_u32_buffer_set(void *buffer_ptr, size_t index, uint32_t value) {
    @autoreleasepool {
        StwoMetalBufferBox *buffer = stwo_metal_buffer_box(buffer_ptr);
        ((uint32_t *)buffer.buffer.contents)[index] = value;
    }
}

bool stwo_metal_u32_buffer_copy(
    void *src_ptr,
    void *dst_ptr,
    size_t len,
    size_t dst_offset,
    char *error_message,
    size_t error_message_len
) {
    @autoreleasepool {
        StwoMetalBufferBox *src = stwo_metal_buffer_box(src_ptr);
        StwoMetalBufferBox *dst = stwo_metal_buffer_box(dst_ptr);
        if (len > src.len || dst_offset + len > dst.len) {
            stwo_metal_write_error(error_message, error_message_len, @"Metal buffer copy exceeds source or destination length.");
            return false;
        }

        memmove(
            ((uint32_t *)dst.buffer.contents) + dst_offset,
            ((uint32_t *)src.buffer.contents),
            len * sizeof(uint32_t)
        );
        return true;
    }
}

bool stwo_metal_bit_reverse_u32(
    void *runtime_ptr,
    void *buffer_ptr,
    uint32_t log_len,
    char *error_message,
    size_t error_message_len
) {
    @autoreleasepool {
        StwoMetalRuntimeBox *runtime = stwo_metal_runtime_box(runtime_ptr);
        StwoMetalBufferBox *buffer = stwo_metal_buffer_box(buffer_ptr);
        NSUInteger len = ((NSUInteger)1) << log_len;
        if (buffer.len != len) {
            stwo_metal_write_error(error_message, error_message_len, @"Bit-reverse kernel expected a power-of-two buffer length matching log_len.");
            return false;
        }

        return stwo_metal_dispatch_unary_u32_kernel(
            runtime,
            @"bit_reverse_u32",
            buffer,
            log_len,
            error_message,
            error_message_len
        );
    }
}

bool stwo_metal_bit_reverse_u32x4(
    void *runtime_ptr,
    void *buffer_ptr,
    uint32_t log_len,
    char *error_message,
    size_t error_message_len
) {
    @autoreleasepool {
        StwoMetalRuntimeBox *runtime = stwo_metal_runtime_box(runtime_ptr);
        StwoMetalBufferBox *buffer = stwo_metal_buffer_box(buffer_ptr);
        NSUInteger len = ((NSUInteger)1) << log_len;
        if (buffer.len != len * 4) {
            stwo_metal_write_error(error_message, error_message_len, @"u32x4 bit-reverse kernel expected four limbs per logical element.");
            return false;
        }

        return stwo_metal_dispatch_unary_u32_kernel(
            runtime,
            @"bit_reverse_u32x4",
            buffer,
            log_len,
            error_message,
            error_message_len
        );
    }
}

bool stwo_metal_invert_m31_values_u32(
    void *runtime_ptr,
    void *buffer_ptr,
    uint32_t len,
    char *error_message,
    size_t error_message_len
) {
    @autoreleasepool {
        StwoMetalRuntimeBox *runtime = stwo_metal_runtime_box(runtime_ptr);
        StwoMetalBufferBox *buffer = stwo_metal_buffer_box(buffer_ptr);
        if (buffer.len != (NSUInteger)len) {
            stwo_metal_write_error(error_message, error_message_len, @"M31 inversion expects a destination length matching len.");
            return false;
        }

        id<MTLComputePipelineState> pipeline =
            stwo_metal_pipeline(runtime, @"invert_m31_values_u32", error_message, error_message_len);
        if (pipeline == nil) {
            return false;
        }

        id<MTLCommandBuffer> command_buffer = [runtime.queue commandBuffer];
        if (command_buffer == nil) {
            stwo_metal_write_error(error_message, error_message_len, @"Failed to create Metal command buffer.");
            return false;
        }

        id<MTLComputeCommandEncoder> encoder = [command_buffer computeCommandEncoder];
        if (encoder == nil) {
            stwo_metal_write_error(error_message, error_message_len, @"Failed to create Metal compute encoder.");
            return false;
        }

        [encoder setComputePipelineState:pipeline];
        [encoder setBuffer:buffer.buffer offset:0 atIndex:0];
        [encoder setBytes:&len length:sizeof(len) atIndex:1];

        MTLSize grid_size = MTLSizeMake((NSUInteger)len, 1, 1);
        MTLSize threadgroup_size = MTLSizeMake(stwo_metal_threads_per_group(pipeline), 1, 1);
        [encoder dispatchThreads:grid_size threadsPerThreadgroup:threadgroup_size];
        [encoder endEncoding];

        [command_buffer commit];
        [command_buffer waitUntilCompleted];

        if (command_buffer.status == MTLCommandBufferStatusError) {
            stwo_metal_write_error(error_message, error_message_len, command_buffer.error.localizedDescription ?: @"Metal kernel execution failed.");
            return false;
        }

        return true;
    }
}

bool stwo_metal_precompute_twiddle_level_u32(
    void *runtime_ptr,
    void *dst_ptr,
    uint32_t offset,
    uint32_t initial_x,
    uint32_t initial_y,
    uint32_t step_x,
    uint32_t step_y,
    uint32_t level_log_size,
    char *error_message,
    size_t error_message_len
) {
    @autoreleasepool {
        StwoMetalRuntimeBox *runtime = stwo_metal_runtime_box(runtime_ptr);
        StwoMetalBufferBox *dst = stwo_metal_buffer_box(dst_ptr);
        if (level_log_size == 0) {
            stwo_metal_write_error(error_message, error_message_len, @"Twiddle precompute expects a level_log_size greater than zero.");
            return false;
        }

        NSUInteger level_len = ((NSUInteger)1) << (level_log_size - 1);
        if (((NSUInteger)offset) + level_len > dst.len) {
            stwo_metal_write_error(error_message, error_message_len, @"Twiddle precompute level exceeds the destination buffer length.");
            return false;
        }

        id<MTLComputePipelineState> pipeline =
            stwo_metal_pipeline(runtime, @"precompute_twiddle_level_u32", error_message, error_message_len);
        if (pipeline == nil) {
            return false;
        }

        id<MTLCommandBuffer> command_buffer = [runtime.queue commandBuffer];
        if (command_buffer == nil) {
            stwo_metal_write_error(error_message, error_message_len, @"Failed to create Metal command buffer.");
            return false;
        }

        id<MTLComputeCommandEncoder> encoder = [command_buffer computeCommandEncoder];
        if (encoder == nil) {
            stwo_metal_write_error(error_message, error_message_len, @"Failed to create Metal compute encoder.");
            return false;
        }

        StwoMetalTwiddleLevelParams params = {
            .initial_x = initial_x,
            .initial_y = initial_y,
            .step_x = step_x,
            .step_y = step_y,
            .offset = offset,
            .level_log_size = level_log_size,
        };

        [encoder setComputePipelineState:pipeline];
        [encoder setBuffer:dst.buffer offset:0 atIndex:0];
        [encoder setBytes:&params length:sizeof(params) atIndex:1];

        MTLSize grid_size = MTLSizeMake(level_len, 1, 1);
        MTLSize threadgroup_size = MTLSizeMake(stwo_metal_threads_per_group(pipeline), 1, 1);
        [encoder dispatchThreads:grid_size threadsPerThreadgroup:threadgroup_size];
        [encoder endEncoding];

        [command_buffer commit];
        [command_buffer waitUntilCompleted];

        if (command_buffer.status == MTLCommandBufferStatusError) {
            stwo_metal_write_error(error_message, error_message_len, command_buffer.error.localizedDescription ?: @"Metal kernel execution failed.");
            return false;
        }

        return true;
    }
}

bool stwo_metal_permute_coset_to_circle_domain_bit_reversed_u32(
    void *runtime_ptr,
    void *src_ptr,
    void *dst_ptr,
    uint32_t log_len,
    char *error_message,
    size_t error_message_len
) {
    @autoreleasepool {
        StwoMetalRuntimeBox *runtime = stwo_metal_runtime_box(runtime_ptr);
        StwoMetalBufferBox *src = stwo_metal_buffer_box(src_ptr);
        StwoMetalBufferBox *dst = stwo_metal_buffer_box(dst_ptr);
        NSUInteger len = ((NSUInteger)1) << log_len;
        if (src.len != len || dst.len != len) {
            stwo_metal_write_error(error_message, error_message_len, @"Coset permutation expects equal power-of-two source and destination lengths.");
            return false;
        }

        id<MTLComputePipelineState> pipeline = stwo_metal_pipeline(runtime, @"permute_coset_to_circle_domain_bit_reversed_u32", error_message, error_message_len);
        if (pipeline == nil) {
            return false;
        }

        id<MTLCommandBuffer> command_buffer = [runtime.queue commandBuffer];
        if (command_buffer == nil) {
            stwo_metal_write_error(error_message, error_message_len, @"Failed to create Metal command buffer.");
            return false;
        }

        id<MTLComputeCommandEncoder> encoder = [command_buffer computeCommandEncoder];
        if (encoder == nil) {
            stwo_metal_write_error(error_message, error_message_len, @"Failed to create Metal compute encoder.");
            return false;
        }

        [encoder setComputePipelineState:pipeline];
        [encoder setBuffer:src.buffer offset:0 atIndex:0];
        [encoder setBuffer:dst.buffer offset:0 atIndex:1];
        [encoder setBytes:&log_len length:sizeof(log_len) atIndex:2];

        MTLSize grid_size = MTLSizeMake(len, 1, 1);
        MTLSize threadgroup_size = MTLSizeMake(stwo_metal_threads_per_group(pipeline), 1, 1);
        [encoder dispatchThreads:grid_size threadsPerThreadgroup:threadgroup_size];
        [encoder endEncoding];

        [command_buffer commit];
        [command_buffer waitUntilCompleted];

        if (command_buffer.status == MTLCommandBufferStatusError) {
            stwo_metal_write_error(error_message, error_message_len, command_buffer.error.localizedDescription ?: @"Metal kernel execution failed.");
            return false;
        }

        return true;
    }
}

bool stwo_metal_fri_fold_circle_into_line_first_layer_u32x4(
    void *runtime_ptr,
    void *src_ptr,
    void *dst_ptr,
    void *inverse_y_ptr,
    uint32_t src_log_len,
    const uint32_t *alpha_limbs,
    char *error_message,
    size_t error_message_len
) {
    @autoreleasepool {
        StwoMetalRuntimeBox *runtime = stwo_metal_runtime_box(runtime_ptr);
        StwoMetalBufferBox *src = stwo_metal_buffer_box(src_ptr);
        StwoMetalBufferBox *dst = stwo_metal_buffer_box(dst_ptr);
        StwoMetalBufferBox *inverse_y = stwo_metal_buffer_box(inverse_y_ptr);
        NSUInteger src_len = ((NSUInteger)1) << src_log_len;
        NSUInteger dst_len = src_len >> 1;
        if (src.len != src_len * 4 || dst.len != dst_len * 4 || inverse_y.len != dst_len) {
            stwo_metal_write_error(error_message, error_message_len, @"FRI first-layer fold expects src u32x4 input, dst u32x4 output, and one inverse-y factor per output element.");
            return false;
        }

        id<MTLComputePipelineState> pipeline = stwo_metal_pipeline(runtime, @"fri_fold_circle_into_line_first_layer_u32x4", error_message, error_message_len);
        if (pipeline == nil) {
            return false;
        }

        id<MTLCommandBuffer> command_buffer = [runtime.queue commandBuffer];
        if (command_buffer == nil) {
            stwo_metal_write_error(error_message, error_message_len, @"Failed to create Metal command buffer.");
            return false;
        }

        id<MTLComputeCommandEncoder> encoder = [command_buffer computeCommandEncoder];
        if (encoder == nil) {
            stwo_metal_write_error(error_message, error_message_len, @"Failed to create Metal compute encoder.");
            return false;
        }

        [encoder setComputePipelineState:pipeline];
        [encoder setBuffer:src.buffer offset:0 atIndex:0];
        [encoder setBuffer:dst.buffer offset:0 atIndex:1];
        [encoder setBuffer:inverse_y.buffer offset:0 atIndex:2];
        [encoder setBytes:alpha_limbs length:sizeof(uint32_t) * 4 atIndex:3];

        MTLSize grid_size = MTLSizeMake(dst_len, 1, 1);
        MTLSize threadgroup_size = MTLSizeMake(stwo_metal_threads_per_group(pipeline), 1, 1);
        [encoder dispatchThreads:grid_size threadsPerThreadgroup:threadgroup_size];
        [encoder endEncoding];

        [command_buffer commit];
        [command_buffer waitUntilCompleted];

        if (command_buffer.status == MTLCommandBufferStatusError) {
            stwo_metal_write_error(error_message, error_message_len, command_buffer.error.localizedDescription ?: @"Metal kernel execution failed.");
            return false;
        }

        return true;
    }
}

bool stwo_metal_fri_fold_line_step_u32x4(
    void *runtime_ptr,
    void *src_ptr,
    void *dst_ptr,
    void *inverse_x_ptr,
    uint32_t src_log_len,
    const uint32_t *alpha_limbs,
    char *error_message,
    size_t error_message_len
) {
    @autoreleasepool {
        StwoMetalRuntimeBox *runtime = stwo_metal_runtime_box(runtime_ptr);
        StwoMetalBufferBox *src = stwo_metal_buffer_box(src_ptr);
        StwoMetalBufferBox *dst = stwo_metal_buffer_box(dst_ptr);
        StwoMetalBufferBox *inverse_x = stwo_metal_buffer_box(inverse_x_ptr);
        NSUInteger src_len = ((NSUInteger)1) << src_log_len;
        NSUInteger dst_len = src_len >> 1;
        if (src.len != src_len * 4 || dst.len != dst_len * 4 || inverse_x.len != dst_len) {
            stwo_metal_write_error(error_message, error_message_len, @"FRI line-fold step expects src u32x4 input, dst u32x4 output, and one inverse-x factor per output element.");
            return false;
        }

        id<MTLComputePipelineState> pipeline = stwo_metal_pipeline(runtime, @"fri_fold_line_step_u32x4", error_message, error_message_len);
        if (pipeline == nil) {
            return false;
        }

        id<MTLCommandBuffer> command_buffer = [runtime.queue commandBuffer];
        if (command_buffer == nil) {
            stwo_metal_write_error(error_message, error_message_len, @"Failed to create Metal command buffer.");
            return false;
        }

        id<MTLComputeCommandEncoder> encoder = [command_buffer computeCommandEncoder];
        if (encoder == nil) {
            stwo_metal_write_error(error_message, error_message_len, @"Failed to create Metal compute encoder.");
            return false;
        }

        [encoder setComputePipelineState:pipeline];
        [encoder setBuffer:src.buffer offset:0 atIndex:0];
        [encoder setBuffer:dst.buffer offset:0 atIndex:1];
        [encoder setBuffer:inverse_x.buffer offset:0 atIndex:2];
        [encoder setBytes:alpha_limbs length:sizeof(uint32_t) * 4 atIndex:3];

        MTLSize grid_size = MTLSizeMake(dst_len, 1, 1);
        MTLSize threadgroup_size = MTLSizeMake(stwo_metal_threads_per_group(pipeline), 1, 1);
        [encoder dispatchThreads:grid_size threadsPerThreadgroup:threadgroup_size];
        [encoder endEncoding];

        [command_buffer commit];
        [command_buffer waitUntilCompleted];

        if (command_buffer.status == MTLCommandBufferStatusError) {
            stwo_metal_write_error(error_message, error_message_len, command_buffer.error.localizedDescription ?: @"Metal kernel execution failed.");
            return false;
        }

        return true;
    }
}

bool stwo_metal_accumulate_wide_fibonacci_quotients_u32x4(
    void *runtime_ptr,
    void *trace_ptr,
    void *random_coeff_ptr,
    void *denominator_ptr,
    void *dst_ptr,
    uint32_t domain_log_size,
    uint32_t eval_domain_log_size,
    uint32_t n_constraints,
    char *error_message,
    size_t error_message_len
) {
    @autoreleasepool {
        StwoMetalRuntimeBox *runtime = stwo_metal_runtime_box(runtime_ptr);
        StwoMetalBufferBox *trace = stwo_metal_buffer_box(trace_ptr);
        StwoMetalBufferBox *random_coeff = stwo_metal_buffer_box(random_coeff_ptr);
        StwoMetalBufferBox *denominator = stwo_metal_buffer_box(denominator_ptr);
        StwoMetalBufferBox *dst = stwo_metal_buffer_box(dst_ptr);
        NSUInteger eval_domain_size = ((NSUInteger)1) << eval_domain_log_size;
        uint32_t eval_domain_size_u32 = ((uint32_t)1) << eval_domain_log_size;
        NSUInteger denominator_len = ((NSUInteger)1) << (eval_domain_log_size - domain_log_size);
        NSUInteger trace_columns = (NSUInteger)n_constraints + 2;
        if (trace.len != trace_columns * eval_domain_size ||
            random_coeff.len != ((NSUInteger)n_constraints) * 4 ||
            denominator.len != denominator_len ||
            dst.len != eval_domain_size * 4) {
            stwo_metal_write_error(error_message, error_message_len, @"Wide-fibonacci quotient accumulation expects column-major trace evaluations, one qm31 random coefficient per constraint, one denominator inverse per coset, and a u32x4 destination.");
            return false;
        }

        id<MTLComputePipelineState> pipeline = stwo_metal_pipeline(runtime, @"accumulate_wide_fibonacci_quotients_u32x4", error_message, error_message_len);
        if (pipeline == nil) {
            return false;
        }

        id<MTLCommandBuffer> command_buffer = [runtime.queue commandBuffer];
        if (command_buffer == nil) {
            stwo_metal_write_error(error_message, error_message_len, @"Failed to create Metal command buffer.");
            return false;
        }

        id<MTLComputeCommandEncoder> encoder = [command_buffer computeCommandEncoder];
        if (encoder == nil) {
            stwo_metal_write_error(error_message, error_message_len, @"Failed to create Metal compute encoder.");
            return false;
        }

        [encoder setComputePipelineState:pipeline];
        [encoder setBuffer:trace.buffer offset:0 atIndex:0];
        [encoder setBuffer:random_coeff.buffer offset:0 atIndex:1];
        [encoder setBuffer:denominator.buffer offset:0 atIndex:2];
        [encoder setBuffer:dst.buffer offset:0 atIndex:3];
        [encoder setBytes:&eval_domain_size_u32 length:sizeof(eval_domain_size_u32) atIndex:4];
        [encoder setBytes:&n_constraints length:sizeof(n_constraints) atIndex:5];
        [encoder setBytes:&domain_log_size length:sizeof(domain_log_size) atIndex:6];

        MTLSize grid_size = MTLSizeMake(eval_domain_size, 1, 1);
        MTLSize threadgroup_size = MTLSizeMake(stwo_metal_threads_per_group(pipeline), 1, 1);
        [encoder dispatchThreads:grid_size threadsPerThreadgroup:threadgroup_size];
        [encoder endEncoding];

        [command_buffer commit];
        [command_buffer waitUntilCompleted];

        if (command_buffer.status == MTLCommandBufferStatusError) {
            stwo_metal_write_error(error_message, error_message_len, command_buffer.error.localizedDescription ?: @"Metal kernel execution failed.");
            return false;
        }

        return true;
    }
}

bool stwo_metal_generate_wide_fibonacci_trace_u32(
    void *runtime_ptr,
    void *input_a_ptr,
    void *input_b_ptr,
    void *trace_ptr,
    uint32_t input_log_len,
    uint32_t n_columns,
    char *error_message,
    size_t error_message_len
) {
    @autoreleasepool {
        StwoMetalRuntimeBox *runtime = stwo_metal_runtime_box(runtime_ptr);
        StwoMetalBufferBox *input_a = stwo_metal_buffer_box(input_a_ptr);
        StwoMetalBufferBox *input_b = stwo_metal_buffer_box(input_b_ptr);
        StwoMetalBufferBox *trace = stwo_metal_buffer_box(trace_ptr);
        uint32_t input_len_u32 = ((uint32_t)1) << input_log_len;
        NSUInteger input_len = (NSUInteger)input_len_u32;
        NSUInteger trace_len = input_len * (NSUInteger)n_columns;
        if (n_columns < 2u) {
            stwo_metal_write_error(error_message, error_message_len, @"Wide-fibonacci trace generation requires at least two columns.");
            return false;
        }
        if (input_a.len != input_len || input_b.len != input_len || trace.len != trace_len) {
            stwo_metal_write_error(error_message, error_message_len, @"Wide-fibonacci trace generation expects equal power-of-two inputs and a contiguous column-major output buffer.");
            return false;
        }

        id<MTLComputePipelineState> pipeline = stwo_metal_pipeline(
            runtime,
            @"generate_wide_fibonacci_trace_u32",
            error_message,
            error_message_len
        );
        if (pipeline == nil) {
            return false;
        }

        id<MTLCommandBuffer> command_buffer = [runtime.queue commandBuffer];
        if (command_buffer == nil) {
            stwo_metal_write_error(error_message, error_message_len, @"Failed to create Metal command buffer.");
            return false;
        }

        id<MTLComputeCommandEncoder> encoder = [command_buffer computeCommandEncoder];
        if (encoder == nil) {
            stwo_metal_write_error(error_message, error_message_len, @"Failed to create Metal compute encoder.");
            return false;
        }

        [encoder setComputePipelineState:pipeline];
        [encoder setBuffer:input_a.buffer offset:0 atIndex:0];
        [encoder setBuffer:input_b.buffer offset:0 atIndex:1];
        [encoder setBuffer:trace.buffer offset:0 atIndex:2];
        [encoder setBytes:&input_len_u32 length:sizeof(input_len_u32) atIndex:3];
        [encoder setBytes:&n_columns length:sizeof(n_columns) atIndex:4];

        MTLSize grid_size = MTLSizeMake(input_len, 1, 1);
        MTLSize threadgroup_size = MTLSizeMake(stwo_metal_threads_per_group(pipeline), 1, 1);
        [encoder dispatchThreads:grid_size threadsPerThreadgroup:threadgroup_size];
        [encoder endEncoding];

        [command_buffer commit];
        [command_buffer waitUntilCompleted];

        if (command_buffer.status == MTLCommandBufferStatusError) {
            stwo_metal_write_error(error_message, error_message_len, command_buffer.error.localizedDescription ?: @"Metal kernel execution failed.");
            return false;
        }

        return true;
    }
}
