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

static id<MTLBuffer> stwo_metal_encode_qm31_pair_reduction(
    id<MTLCommandBuffer> command_buffer,
    id<MTLComputePipelineState> pipeline,
    id<MTLBuffer> current,
    id<MTLBuffer> temp,
    uint32_t len,
    char *error_message,
    size_t error_message_len
) {
    uint32_t current_len = len;
    id<MTLBuffer> current_buffer = current;
    id<MTLBuffer> temp_buffer = temp;

    while (current_len > 1u) {
        uint32_t next_len = current_len >> 1u;
        id<MTLComputeCommandEncoder> encoder = [command_buffer computeCommandEncoder];
        if (encoder == nil) {
            stwo_metal_write_error(error_message, error_message_len, @"Failed to create Metal compute encoder.");
            return nil;
        }

        [encoder setComputePipelineState:pipeline];
        [encoder setBuffer:current_buffer offset:0 atIndex:0];
        [encoder setBuffer:temp_buffer offset:0 atIndex:1];
        [encoder setBytes:&next_len length:sizeof(next_len) atIndex:2];
        MTLSize grid_size = MTLSizeMake(next_len, 1, 1);
        MTLSize threadgroup_size = MTLSizeMake(stwo_metal_threads_per_group(pipeline), 1, 1);
        [encoder dispatchThreads:grid_size threadsPerThreadgroup:threadgroup_size];
        [encoder endEncoding];

        id<MTLBuffer> swap = current_buffer;
        current_buffer = temp_buffer;
        temp_buffer = swap;
        current_len = next_len;
    }

    return current_buffer;
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

const uint32_t *stwo_metal_u32_buffer_host_ptr(void *buffer_ptr) {
    @autoreleasepool {
        StwoMetalBufferBox *buffer = stwo_metal_buffer_box(buffer_ptr);
        return (const uint32_t *)buffer.buffer.contents;
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

bool stwo_metal_u32_buffer_copy_range(
    void *src_ptr,
    void *dst_ptr,
    size_t src_offset,
    size_t len,
    size_t dst_offset,
    char *error_message,
    size_t error_message_len
) {
    @autoreleasepool {
        StwoMetalBufferBox *src = stwo_metal_buffer_box(src_ptr);
        StwoMetalBufferBox *dst = stwo_metal_buffer_box(dst_ptr);
        if (src_offset + len > src.len || dst_offset + len > dst.len) {
            stwo_metal_write_error(error_message, error_message_len, @"Metal buffer range copy exceeds source or destination length.");
            return false;
        }

        memmove(
            ((uint32_t *)dst.buffer.contents) + dst_offset,
            ((uint32_t *)src.buffer.contents) + src_offset,
            len * sizeof(uint32_t)
        );
        return true;
    }
}

bool stwo_metal_u32_buffer_read_indices(
    void *runtime_ptr,
    void *buffer_ptr,
    const uint32_t *indices,
    size_t indices_len,
    uint32_t *host_ptr,
    char *error_message,
    size_t error_message_len
) {
    @autoreleasepool {
        (void)runtime_ptr;
        StwoMetalBufferBox *buffer = stwo_metal_buffer_box(buffer_ptr);
        if (indices_len > 0 && (indices == NULL || host_ptr == NULL)) {
            stwo_metal_write_error(error_message, error_message_len, @"Indexed Metal buffer read requires non-null indices and destination pointers.");
            return false;
        }

        const uint32_t *values = (const uint32_t *)buffer.buffer.contents;
        for (size_t i = 0; i < indices_len; ++i) {
            uint32_t index = indices[i];
            if ((NSUInteger)index >= buffer.len) {
                stwo_metal_write_error(error_message, error_message_len, @"Indexed Metal buffer read exceeded source bounds.");
                return false;
            }
            host_ptr[i] = values[index];
        }
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

bool stwo_metal_rfft_evaluate_u32(
    void *runtime_ptr,
    void *values_ptr,
    void *twiddles_ptr,
    uint32_t values_log_len,
    char *error_message,
    size_t error_message_len
) {
    @autoreleasepool {
        StwoMetalRuntimeBox *runtime = stwo_metal_runtime_box(runtime_ptr);
        StwoMetalBufferBox *values = stwo_metal_buffer_box(values_ptr);
        StwoMetalBufferBox *twiddles = stwo_metal_buffer_box(twiddles_ptr);
        uint32_t values_len = ((uint32_t)1) << values_log_len;
        uint32_t eval_domain_size = values_len >> 1;
        if (values.len != (NSUInteger)values_len || twiddles.len != (NSUInteger)eval_domain_size) {
            stwo_metal_write_error(error_message, error_message_len, @"RFFT evaluation expects a power-of-two value buffer and a twiddle slice of half that length.");
            return false;
        }

        id<MTLComputePipelineState> line_pipeline =
            stwo_metal_pipeline(runtime, @"rfft_line_part_u32", error_message, error_message_len);
        if (line_pipeline == nil) {
            return false;
        }
        id<MTLComputePipelineState> circle_pipeline =
            stwo_metal_pipeline(runtime, @"rfft_circle_part_u32", error_message, error_message_len);
        if (circle_pipeline == nil) {
            return false;
        }

        uint32_t pair_count = values_len >> 1;
        id<MTLCommandBuffer> command_buffer = [runtime.queue commandBuffer];
        if (command_buffer == nil) {
            stwo_metal_write_error(error_message, error_message_len, @"Failed to create Metal command buffer.");
            return false;
        }

        uint32_t layer_domain_size = 1u;
        uint32_t layer_domain_offset = pair_count - 2u;
        for (uint32_t layer = values_log_len - 1u; layer > 0u; --layer) {
            id<MTLComputeCommandEncoder> encoder = [command_buffer computeCommandEncoder];
            if (encoder == nil) {
                stwo_metal_write_error(error_message, error_message_len, @"Failed to create Metal compute encoder.");
                return false;
            }

            [encoder setComputePipelineState:line_pipeline];
            [encoder setBuffer:values.buffer offset:0 atIndex:0];
            [encoder setBuffer:twiddles.buffer offset:0 atIndex:1];
            [encoder setBytes:&values_log_len length:sizeof(values_log_len) atIndex:2];
            [encoder setBytes:&layer length:sizeof(layer) atIndex:3];
            [encoder setBytes:&layer_domain_offset length:sizeof(layer_domain_offset) atIndex:4];

            MTLSize grid_size = MTLSizeMake(pair_count, 1, 1);
            MTLSize threadgroup_size = MTLSizeMake(stwo_metal_threads_per_group(line_pipeline), 1, 1);
            [encoder dispatchThreads:grid_size threadsPerThreadgroup:threadgroup_size];
            [encoder endEncoding];

            layer_domain_size <<= 1u;
            layer_domain_offset -= layer_domain_size;
        }

        id<MTLComputeCommandEncoder> encoder = [command_buffer computeCommandEncoder];
        if (encoder == nil) {
            stwo_metal_write_error(error_message, error_message_len, @"Failed to create Metal compute encoder.");
            return false;
        }

        [encoder setComputePipelineState:circle_pipeline];
        [encoder setBuffer:values.buffer offset:0 atIndex:0];
        [encoder setBuffer:twiddles.buffer offset:0 atIndex:1];
        [encoder setBytes:&values_len length:sizeof(values_len) atIndex:2];

        MTLSize grid_size = MTLSizeMake(pair_count, 1, 1);
        MTLSize threadgroup_size = MTLSizeMake(stwo_metal_threads_per_group(circle_pipeline), 1, 1);
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

bool stwo_metal_rfft_evaluate_subbuffer_u32(
    void *runtime_ptr,
    void *values_ptr,
    size_t value_offset,
    uint32_t values_log_len,
    void *twiddles_ptr,
    char *error_message,
    size_t error_message_len
) {
    @autoreleasepool {
        StwoMetalRuntimeBox *runtime = stwo_metal_runtime_box(runtime_ptr);
        StwoMetalBufferBox *values = stwo_metal_buffer_box(values_ptr);
        StwoMetalBufferBox *twiddles = stwo_metal_buffer_box(twiddles_ptr);
        uint32_t values_len = ((uint32_t)1) << values_log_len;
        uint32_t eval_domain_size = values_len >> 1;
        if (value_offset + (size_t)values_len > values.len ||
            twiddles.len != (NSUInteger)eval_domain_size) {
            stwo_metal_write_error(error_message, error_message_len, @"RFFT subbuffer evaluation expects an in-bounds power-of-two value range and a twiddle slice of half that length.");
            return false;
        }

        id<MTLComputePipelineState> line_pipeline =
            stwo_metal_pipeline(runtime, @"rfft_line_part_u32", error_message, error_message_len);
        if (line_pipeline == nil) {
            return false;
        }
        id<MTLComputePipelineState> circle_pipeline =
            stwo_metal_pipeline(runtime, @"rfft_circle_part_u32", error_message, error_message_len);
        if (circle_pipeline == nil) {
            return false;
        }

        uint32_t pair_count = values_len >> 1;
        NSUInteger value_offset_bytes = value_offset * sizeof(uint32_t);
        id<MTLCommandBuffer> command_buffer = [runtime.queue commandBuffer];
        if (command_buffer == nil) {
            stwo_metal_write_error(error_message, error_message_len, @"Failed to create Metal command buffer.");
            return false;
        }

        uint32_t layer_domain_size = 1u;
        uint32_t layer_domain_offset = pair_count - 2u;
        for (uint32_t layer = values_log_len - 1u; layer > 0u; --layer) {
            id<MTLComputeCommandEncoder> encoder = [command_buffer computeCommandEncoder];
            if (encoder == nil) {
                stwo_metal_write_error(error_message, error_message_len, @"Failed to create Metal compute encoder.");
                return false;
            }

            [encoder setComputePipelineState:line_pipeline];
            [encoder setBuffer:values.buffer offset:value_offset_bytes atIndex:0];
            [encoder setBuffer:twiddles.buffer offset:0 atIndex:1];
            [encoder setBytes:&values_log_len length:sizeof(values_log_len) atIndex:2];
            [encoder setBytes:&layer length:sizeof(layer) atIndex:3];
            [encoder setBytes:&layer_domain_offset length:sizeof(layer_domain_offset) atIndex:4];

            MTLSize grid_size = MTLSizeMake(pair_count, 1, 1);
            MTLSize threadgroup_size = MTLSizeMake(stwo_metal_threads_per_group(line_pipeline), 1, 1);
            [encoder dispatchThreads:grid_size threadsPerThreadgroup:threadgroup_size];
            [encoder endEncoding];

            layer_domain_size <<= 1u;
            layer_domain_offset -= layer_domain_size;
        }

        id<MTLComputeCommandEncoder> encoder = [command_buffer computeCommandEncoder];
        if (encoder == nil) {
            stwo_metal_write_error(error_message, error_message_len, @"Failed to create Metal compute encoder.");
            return false;
        }

        [encoder setComputePipelineState:circle_pipeline];
        [encoder setBuffer:values.buffer offset:value_offset_bytes atIndex:0];
        [encoder setBuffer:twiddles.buffer offset:0 atIndex:1];
        [encoder setBytes:&values_len length:sizeof(values_len) atIndex:2];

        MTLSize grid_size = MTLSizeMake(pair_count, 1, 1);
        MTLSize threadgroup_size = MTLSizeMake(stwo_metal_threads_per_group(circle_pipeline), 1, 1);
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

bool stwo_metal_ifft_interpolate_u32(
    void *runtime_ptr,
    void *values_ptr,
    void *inverse_twiddles_ptr,
    uint32_t values_log_len,
    uint32_t scale_factor,
    char *error_message,
    size_t error_message_len
) {
    @autoreleasepool {
        StwoMetalRuntimeBox *runtime = stwo_metal_runtime_box(runtime_ptr);
        StwoMetalBufferBox *values = stwo_metal_buffer_box(values_ptr);
        StwoMetalBufferBox *inverse_twiddles = stwo_metal_buffer_box(inverse_twiddles_ptr);
        uint32_t values_len = ((uint32_t)1) << values_log_len;
        uint32_t eval_domain_size = values_len >> 1;
        if (values.len != (NSUInteger)values_len || inverse_twiddles.len != (NSUInteger)eval_domain_size) {
            stwo_metal_write_error(error_message, error_message_len, @"IFFT interpolation expects a power-of-two value buffer and an inverse-twiddle slice of half that length.");
            return false;
        }

        id<MTLComputePipelineState> circle_pipeline =
            stwo_metal_pipeline(runtime, @"ifft_circle_part_u32", error_message, error_message_len);
        if (circle_pipeline == nil) {
            return false;
        }
        id<MTLComputePipelineState> line_pipeline =
            stwo_metal_pipeline(runtime, @"ifft_line_part_u32", error_message, error_message_len);
        if (line_pipeline == nil) {
            return false;
        }
        id<MTLComputePipelineState> rescale_pipeline =
            stwo_metal_pipeline(runtime, @"rescale_m31_values_u32", error_message, error_message_len);
        if (rescale_pipeline == nil) {
            return false;
        }

        uint32_t pair_count = values_len >> 1;
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

        [encoder setComputePipelineState:circle_pipeline];
        [encoder setBuffer:values.buffer offset:0 atIndex:0];
        [encoder setBuffer:inverse_twiddles.buffer offset:0 atIndex:1];
        [encoder setBytes:&values_len length:sizeof(values_len) atIndex:2];
        MTLSize grid_size = MTLSizeMake(pair_count, 1, 1);
        MTLSize threadgroup_size = MTLSizeMake(stwo_metal_threads_per_group(circle_pipeline), 1, 1);
        [encoder dispatchThreads:grid_size threadsPerThreadgroup:threadgroup_size];
        [encoder endEncoding];

        uint32_t layer_domain_offset = 0u;
        uint32_t layer_domain_size = pair_count;
        for (uint32_t layer = 1u; layer < values_log_len; ++layer) {
            id<MTLComputeCommandEncoder> line_encoder = [command_buffer computeCommandEncoder];
            if (line_encoder == nil) {
                stwo_metal_write_error(error_message, error_message_len, @"Failed to create Metal compute encoder.");
                return false;
            }

            [line_encoder setComputePipelineState:line_pipeline];
            [line_encoder setBuffer:values.buffer offset:0 atIndex:0];
            [line_encoder setBuffer:inverse_twiddles.buffer
                                 offset:(NSUInteger)(layer_domain_offset * sizeof(uint32_t))
                                atIndex:1];
            [line_encoder setBytes:&values_log_len length:sizeof(values_log_len) atIndex:2];
            [line_encoder setBytes:&layer length:sizeof(layer) atIndex:3];

            MTLSize line_grid_size = MTLSizeMake(pair_count, 1, 1);
            MTLSize line_threadgroup_size = MTLSizeMake(stwo_metal_threads_per_group(line_pipeline), 1, 1);
            [line_encoder dispatchThreads:line_grid_size threadsPerThreadgroup:line_threadgroup_size];
            [line_encoder endEncoding];

            layer_domain_size >>= 1u;
            layer_domain_offset += layer_domain_size;
        }

        id<MTLComputeCommandEncoder> rescale_encoder = [command_buffer computeCommandEncoder];
        if (rescale_encoder == nil) {
            stwo_metal_write_error(error_message, error_message_len, @"Failed to create Metal compute encoder.");
            return false;
        }

        [rescale_encoder setComputePipelineState:rescale_pipeline];
        [rescale_encoder setBuffer:values.buffer offset:0 atIndex:0];
        [rescale_encoder setBytes:&values_len length:sizeof(values_len) atIndex:1];
        [rescale_encoder setBytes:&scale_factor length:sizeof(scale_factor) atIndex:2];
        MTLSize rescale_grid_size = MTLSizeMake(values_len, 1, 1);
        MTLSize rescale_threadgroup_size = MTLSizeMake(stwo_metal_threads_per_group(rescale_pipeline), 1, 1);
        [rescale_encoder dispatchThreads:rescale_grid_size threadsPerThreadgroup:rescale_threadgroup_size];
        [rescale_encoder endEncoding];

        [command_buffer commit];
        [command_buffer waitUntilCompleted];

        if (command_buffer.status == MTLCommandBufferStatusError) {
            stwo_metal_write_error(error_message, error_message_len, command_buffer.error.localizedDescription ?: @"Metal kernel execution failed.");
            return false;
        }

        return true;
    }
}

bool stwo_metal_ifft_line_interpolate_u32(
    void *runtime_ptr,
    void *values_ptr,
    void *inverse_line_twiddles_ptr,
    uint32_t values_log_len,
    uint32_t scale_factor,
    char *error_message,
    size_t error_message_len
) {
    @autoreleasepool {
        StwoMetalRuntimeBox *runtime = stwo_metal_runtime_box(runtime_ptr);
        StwoMetalBufferBox *values = stwo_metal_buffer_box(values_ptr);
        StwoMetalBufferBox *inverse_line_twiddles = stwo_metal_buffer_box(inverse_line_twiddles_ptr);
        uint32_t values_len = ((uint32_t)1) << values_log_len;
        uint32_t expected_twiddle_len = values_len > 1 ? values_len - 1u : 0u;
        if (values.len != (NSUInteger)values_len || inverse_line_twiddles.len != (NSUInteger)expected_twiddle_len) {
            stwo_metal_write_error(error_message, error_message_len, @"Line IFFT interpolation expects a power-of-two value buffer and a stage-twiddle slice of len(values)-1.");
            return false;
        }

        if (values_len <= 1u) {
            return true;
        }

        id<MTLComputePipelineState> line_pipeline =
            stwo_metal_pipeline(runtime, @"ifft_line_stage_u32", error_message, error_message_len);
        if (line_pipeline == nil) {
            return false;
        }
        id<MTLComputePipelineState> rescale_pipeline =
            stwo_metal_pipeline(runtime, @"rescale_m31_values_u32", error_message, error_message_len);
        if (rescale_pipeline == nil) {
            return false;
        }

        uint32_t pair_count = values_len >> 1u;
        id<MTLCommandBuffer> command_buffer = [runtime.queue commandBuffer];
        if (command_buffer == nil) {
            stwo_metal_write_error(error_message, error_message_len, @"Failed to create Metal command buffer.");
            return false;
        }

        uint32_t twiddle_offset = 0u;
        for (uint32_t stage_domain_log_size = values_log_len; stage_domain_log_size > 0u; --stage_domain_log_size) {
            uint32_t stage_domain_size = ((uint32_t)1) << stage_domain_log_size;
            uint32_t half_stage_size = stage_domain_size >> 1u;

            id<MTLComputeCommandEncoder> line_encoder = [command_buffer computeCommandEncoder];
            if (line_encoder == nil) {
                stwo_metal_write_error(error_message, error_message_len, @"Failed to create Metal compute encoder.");
                return false;
            }

            [line_encoder setComputePipelineState:line_pipeline];
            [line_encoder setBuffer:values.buffer offset:0 atIndex:0];
            [line_encoder setBuffer:inverse_line_twiddles.buffer
                                  offset:(NSUInteger)(twiddle_offset * sizeof(uint32_t))
                                 atIndex:1];
            [line_encoder setBytes:&values_log_len length:sizeof(values_log_len) atIndex:2];
            [line_encoder setBytes:&stage_domain_log_size length:sizeof(stage_domain_log_size) atIndex:3];

            MTLSize grid_size = MTLSizeMake(pair_count, 1, 1);
            MTLSize threadgroup_size = MTLSizeMake(stwo_metal_threads_per_group(line_pipeline), 1, 1);
            [line_encoder dispatchThreads:grid_size threadsPerThreadgroup:threadgroup_size];
            [line_encoder endEncoding];

            twiddle_offset += half_stage_size;
        }

        id<MTLComputeCommandEncoder> rescale_encoder = [command_buffer computeCommandEncoder];
        if (rescale_encoder == nil) {
            stwo_metal_write_error(error_message, error_message_len, @"Failed to create Metal compute encoder.");
            return false;
        }

        [rescale_encoder setComputePipelineState:rescale_pipeline];
        [rescale_encoder setBuffer:values.buffer offset:0 atIndex:0];
        [rescale_encoder setBytes:&values_len length:sizeof(values_len) atIndex:1];
        [rescale_encoder setBytes:&scale_factor length:sizeof(scale_factor) atIndex:2];

        MTLSize rescale_grid = MTLSizeMake(values_len, 1, 1);
        MTLSize rescale_group = MTLSizeMake(stwo_metal_threads_per_group(rescale_pipeline), 1, 1);
        [rescale_encoder dispatchThreads:rescale_grid threadsPerThreadgroup:rescale_group];
        [rescale_encoder endEncoding];

        [command_buffer commit];
        [command_buffer waitUntilCompleted];

        if (command_buffer.status == MTLCommandBufferStatusError) {
            stwo_metal_write_error(error_message, error_message_len, command_buffer.error.localizedDescription ?: @"Metal kernel execution failed.");
            return false;
        }

        return true;
    }
}

bool stwo_metal_batch_eval_at_point_base_field_u32(
    void *runtime_ptr,
    void *flat_coeffs_ptr,
    void *factors_ptr,
    void *dst_ptr,
    uint32_t coeffs_log_len,
    uint32_t n_polys,
    char *error_message,
    size_t error_message_len
) {
    @autoreleasepool {
        StwoMetalRuntimeBox *runtime = stwo_metal_runtime_box(runtime_ptr);
        StwoMetalBufferBox *flat_coeffs = stwo_metal_buffer_box(flat_coeffs_ptr);
        StwoMetalBufferBox *factors = stwo_metal_buffer_box(factors_ptr);
        StwoMetalBufferBox *dst = stwo_metal_buffer_box(dst_ptr);

        uint32_t coeffs_size = 1u << coeffs_log_len;
        if (flat_coeffs.len != (NSUInteger)(coeffs_size * n_polys)) {
            stwo_metal_write_error(error_message, error_message_len, @"Point evaluation expects a flattened coefficient buffer with coeffs_size * n_polys base-field values.");
            return false;
        }
        if (factors.len != (NSUInteger)(coeffs_log_len * 4u)) {
            stwo_metal_write_error(error_message, error_message_len, @"Point evaluation expects one qm31 folding factor per coefficient level.");
            return false;
        }
        if (dst.len != (NSUInteger)(n_polys * 4u)) {
            stwo_metal_write_error(error_message, error_message_len, @"Point evaluation expects a destination buffer with one qm31 result per polynomial.");
            return false;
        }

        id<MTLComputePipelineState> first_pass_pipeline =
            stwo_metal_pipeline(runtime, @"batch_eval_at_point_first_pass_u32", error_message, error_message_len);
        if (first_pass_pipeline == nil) {
            return false;
        }
        id<MTLComputePipelineState> reduce_pipeline =
            stwo_metal_pipeline(runtime, @"batch_eval_at_point_reduce_u32", error_message, error_message_len);
        if (reduce_pipeline == nil) {
            return false;
        }
        id<MTLComputePipelineState> finalize_pipeline =
            stwo_metal_pipeline(runtime, @"batch_eval_at_point_finalize_u32", error_message, error_message_len);
        if (finalize_pipeline == nil) {
            return false;
        }

        uint32_t blocks_per_poly = coeffs_log_len > 9u ? (coeffs_size >> 9u) : 1u;
        NSUInteger temp_len = (NSUInteger)(blocks_per_poly * n_polys * 4u);
        id<MTLBuffer> temp_a = [runtime.device newBufferWithLength:(temp_len * sizeof(uint32_t))
                                                           options:MTLResourceStorageModeShared];
        id<MTLBuffer> temp_b = [runtime.device newBufferWithLength:(temp_len * sizeof(uint32_t))
                                                           options:MTLResourceStorageModeShared];
        if (temp_a == nil || temp_b == nil) {
            stwo_metal_write_error(error_message, error_message_len, @"Failed to allocate Metal point-evaluation temporary buffers.");
            return false;
        }

        id<MTLCommandBuffer> command_buffer = [runtime.queue commandBuffer];
        if (command_buffer == nil) {
            stwo_metal_write_error(error_message, error_message_len, @"Failed to create Metal command buffer.");
            return false;
        }

        id<MTLComputeCommandEncoder> first_encoder = [command_buffer computeCommandEncoder];
        if (first_encoder == nil) {
            stwo_metal_write_error(error_message, error_message_len, @"Failed to create Metal compute encoder.");
            return false;
        }

        [first_encoder setComputePipelineState:first_pass_pipeline];
        [first_encoder setBuffer:flat_coeffs.buffer offset:0 atIndex:0];
        [first_encoder setBuffer:factors.buffer offset:0 atIndex:1];
        [first_encoder setBuffer:temp_a offset:0 atIndex:2];
        [first_encoder setBytes:&coeffs_log_len length:sizeof(coeffs_log_len) atIndex:3];
        [first_encoder setBytes:&blocks_per_poly length:sizeof(blocks_per_poly) atIndex:4];
        [first_encoder dispatchThreadgroups:MTLSizeMake(blocks_per_poly, n_polys, 1)
                     threadsPerThreadgroup:MTLSizeMake(256, 1, 1)];
        [first_encoder endEncoding];

        uint32_t current_stride = blocks_per_poly;
        uint32_t remaining_log_len = coeffs_log_len > 9u ? coeffs_log_len - 9u : 0u;
        id<MTLBuffer> current = temp_a;
        id<MTLBuffer> next = temp_b;

        while (current_stride > 1u) {
            uint32_t output_stride = remaining_log_len > 9u ? (current_stride >> 9u) : 1u;
            uint32_t factor_offset = remaining_log_len - 1u;

            id<MTLComputeCommandEncoder> reduce_encoder = [command_buffer computeCommandEncoder];
            if (reduce_encoder == nil) {
                stwo_metal_write_error(error_message, error_message_len, @"Failed to create Metal compute encoder.");
                return false;
            }

            [reduce_encoder setComputePipelineState:reduce_pipeline];
            [reduce_encoder setBuffer:current offset:0 atIndex:0];
            [reduce_encoder setBuffer:factors.buffer offset:0 atIndex:1];
            [reduce_encoder setBuffer:next offset:0 atIndex:2];
            [reduce_encoder setBytes:&current_stride length:sizeof(current_stride) atIndex:3];
            [reduce_encoder setBytes:&output_stride length:sizeof(output_stride) atIndex:4];
            [reduce_encoder setBytes:&factor_offset length:sizeof(factor_offset) atIndex:5];
            [reduce_encoder dispatchThreadgroups:MTLSizeMake(output_stride, n_polys, 1)
                          threadsPerThreadgroup:MTLSizeMake(256, 1, 1)];
            [reduce_encoder endEncoding];

            current_stride = output_stride;
            remaining_log_len = remaining_log_len > 9u ? remaining_log_len - 9u : 0u;
            id<MTLBuffer> swap = current;
            current = next;
            next = swap;
        }

        id<MTLComputeCommandEncoder> finalize_encoder = [command_buffer computeCommandEncoder];
        if (finalize_encoder == nil) {
            stwo_metal_write_error(error_message, error_message_len, @"Failed to create Metal compute encoder.");
            return false;
        }

        [finalize_encoder setComputePipelineState:finalize_pipeline];
        [finalize_encoder setBuffer:current offset:0 atIndex:0];
        [finalize_encoder setBuffer:dst.buffer offset:0 atIndex:1];
        [finalize_encoder setBytes:&current_stride length:sizeof(current_stride) atIndex:2];
        [finalize_encoder setBytes:&n_polys length:sizeof(n_polys) atIndex:3];
        [finalize_encoder dispatchThreads:MTLSizeMake(n_polys, 1, 1)
                   threadsPerThreadgroup:MTLSizeMake(stwo_metal_threads_per_group(finalize_pipeline), 1, 1)];
        [finalize_encoder endEncoding];

        [command_buffer commit];
        [command_buffer waitUntilCompleted];

        if (command_buffer.status == MTLCommandBufferStatusError) {
            stwo_metal_write_error(error_message, error_message_len, command_buffer.error.localizedDescription ?: @"Metal kernel execution failed.");
            return false;
        }

        return true;
    }
}

bool stwo_metal_batch_eval_first_pass_base_field_u32(
    void *runtime_ptr,
    void *flat_coeffs_ptr,
    void *factors_ptr,
    void *dst_ptr,
    uint32_t coeffs_log_len,
    uint32_t n_polys,
    char *error_message,
    size_t error_message_len
) {
    @autoreleasepool {
        StwoMetalRuntimeBox *runtime = stwo_metal_runtime_box(runtime_ptr);
        StwoMetalBufferBox *flat_coeffs = stwo_metal_buffer_box(flat_coeffs_ptr);
        StwoMetalBufferBox *factors = stwo_metal_buffer_box(factors_ptr);
        StwoMetalBufferBox *dst = stwo_metal_buffer_box(dst_ptr);

        uint32_t coeffs_size = 1u << coeffs_log_len;
        uint32_t blocks_per_poly = coeffs_log_len > 9u ? (coeffs_size >> 9u) : 1u;
        if (flat_coeffs.len != (NSUInteger)(coeffs_size * n_polys)) {
            stwo_metal_write_error(error_message, error_message_len, @"Point-evaluation first pass expects a flattened coefficient buffer with coeffs_size * n_polys base-field values.");
            return false;
        }
        if (factors.len != (NSUInteger)(coeffs_log_len * 4u)) {
            stwo_metal_write_error(error_message, error_message_len, @"Point-evaluation first pass expects one qm31 folding factor per coefficient level.");
            return false;
        }
        if (dst.len != (NSUInteger)(blocks_per_poly * n_polys * 4u)) {
            stwo_metal_write_error(error_message, error_message_len, @"Point-evaluation first pass expects one qm31 partial result per 512-coefficient chunk.");
            return false;
        }

        id<MTLComputePipelineState> first_pass_pipeline =
            stwo_metal_pipeline(runtime, @"batch_eval_at_point_first_pass_u32", error_message, error_message_len);
        if (first_pass_pipeline == nil) {
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

        [encoder setComputePipelineState:first_pass_pipeline];
        [encoder setBuffer:flat_coeffs.buffer offset:0 atIndex:0];
        [encoder setBuffer:factors.buffer offset:0 atIndex:1];
        [encoder setBuffer:dst.buffer offset:0 atIndex:2];
        [encoder setBytes:&coeffs_log_len length:sizeof(coeffs_log_len) atIndex:3];
        [encoder setBytes:&blocks_per_poly length:sizeof(blocks_per_poly) atIndex:4];
        [encoder dispatchThreadgroups:MTLSizeMake(blocks_per_poly, n_polys, 1)
                     threadsPerThreadgroup:MTLSizeMake(256, 1, 1)];
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

bool stwo_metal_fix_first_variable_base_field_u32(
    void *runtime_ptr,
    void *src_ptr,
    void *dst_ptr,
    uint32_t src_log_len,
    const uint32_t *assignment_limbs,
    char *error_message,
    size_t error_message_len
) {
    @autoreleasepool {
        StwoMetalRuntimeBox *runtime = stwo_metal_runtime_box(runtime_ptr);
        StwoMetalBufferBox *src = stwo_metal_buffer_box(src_ptr);
        StwoMetalBufferBox *dst = stwo_metal_buffer_box(dst_ptr);
        if (src_log_len == 0u) {
            stwo_metal_write_error(error_message, error_message_len, @"MLE fix-first-variable expects at least one variable.");
            return false;
        }

        uint32_t src_len = ((uint32_t)1) << src_log_len;
        uint32_t midpoint = src_len >> 1u;
        if (src.len != (NSUInteger)src_len || dst.len != (NSUInteger)(midpoint * 4u)) {
            stwo_metal_write_error(error_message, error_message_len, @"Base-field MLE fix-first-variable expects a power-of-two source and a secure-field destination of half that logical length.");
            return false;
        }

        id<MTLComputePipelineState> pipeline =
            stwo_metal_pipeline(runtime, @"fix_first_variable_base_field_u32", error_message, error_message_len);
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
        [encoder setBytes:assignment_limbs length:sizeof(uint32_t) * 4u atIndex:2];
        [encoder setBytes:&midpoint length:sizeof(midpoint) atIndex:3];

        MTLSize grid_size = MTLSizeMake(midpoint, 1, 1);
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

bool stwo_metal_fix_first_variable_secure_field_u32x4(
    void *runtime_ptr,
    void *src_ptr,
    void *dst_ptr,
    uint32_t src_log_len,
    const uint32_t *assignment_limbs,
    char *error_message,
    size_t error_message_len
) {
    @autoreleasepool {
        StwoMetalRuntimeBox *runtime = stwo_metal_runtime_box(runtime_ptr);
        StwoMetalBufferBox *src = stwo_metal_buffer_box(src_ptr);
        StwoMetalBufferBox *dst = stwo_metal_buffer_box(dst_ptr);
        if (src_log_len == 0u) {
            stwo_metal_write_error(error_message, error_message_len, @"MLE fix-first-variable expects at least one variable.");
            return false;
        }

        uint32_t src_len = ((uint32_t)1) << src_log_len;
        uint32_t midpoint = src_len >> 1u;
        if (src.len != (NSUInteger)(src_len * 4u) || dst.len != (NSUInteger)(midpoint * 4u)) {
            stwo_metal_write_error(error_message, error_message_len, @"Secure-field MLE fix-first-variable expects four limbs per source and destination element.");
            return false;
        }

        id<MTLComputePipelineState> pipeline =
            stwo_metal_pipeline(runtime, @"fix_first_variable_secure_field_u32x4", error_message, error_message_len);
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
        [encoder setBytes:assignment_limbs length:sizeof(uint32_t) * 4u atIndex:2];
        [encoder setBytes:&midpoint length:sizeof(midpoint) atIndex:3];

        MTLSize grid_size = MTLSizeMake(midpoint, 1, 1);
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

bool stwo_metal_gkr_gen_eq_evals_u32x4(
    void *runtime_ptr,
    void *factors_ptr,
    void *dst_ptr,
    uint32_t y_size,
    const uint32_t *v_limbs,
    char *error_message,
    size_t error_message_len
) {
    @autoreleasepool {
        StwoMetalRuntimeBox *runtime = stwo_metal_runtime_box(runtime_ptr);
        StwoMetalBufferBox *factors = stwo_metal_buffer_box(factors_ptr);
        StwoMetalBufferBox *dst = stwo_metal_buffer_box(dst_ptr);
        uint32_t eval_count = 1u << y_size;
        if (factors.len != (NSUInteger)(y_size * 2u * 4u) || dst.len != (NSUInteger)(eval_count * 4u)) {
            stwo_metal_write_error(error_message, error_message_len, @"GKR eq-eval generation expects two qm31 factors per input coordinate and one qm31 output per hypercube point.");
            return false;
        }

        id<MTLComputePipelineState> pipeline =
            stwo_metal_pipeline(runtime, @"gkr_gen_eq_evals_u32x4", error_message, error_message_len);
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
        [encoder setBuffer:factors.buffer offset:0 atIndex:0];
        [encoder setBuffer:dst.buffer offset:0 atIndex:1];
        [encoder setBytes:v_limbs length:sizeof(uint32_t) * 4u atIndex:2];
        [encoder setBytes:&y_size length:sizeof(y_size) atIndex:3];

        MTLSize grid_size = MTLSizeMake(eval_count, 1, 1);
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

bool stwo_metal_gkr_next_grand_product_layer_u32x4(
    void *runtime_ptr,
    void *src_ptr,
    void *dst_ptr,
    uint32_t input_log_len,
    char *error_message,
    size_t error_message_len
) {
    @autoreleasepool {
        StwoMetalRuntimeBox *runtime = stwo_metal_runtime_box(runtime_ptr);
        StwoMetalBufferBox *src = stwo_metal_buffer_box(src_ptr);
        StwoMetalBufferBox *dst = stwo_metal_buffer_box(dst_ptr);
        if (input_log_len == 0u) {
            stwo_metal_write_error(error_message, error_message_len, @"GKR next layer expects at least one input variable.");
            return false;
        }

        uint32_t input_len = 1u << input_log_len;
        uint32_t next_layer_size = input_len >> 1u;
        if (src.len != (NSUInteger)(input_len * 4u) || dst.len != (NSUInteger)(next_layer_size * 4u)) {
            stwo_metal_write_error(error_message, error_message_len, @"GKR grand-product next layer expects secure-field source and destination buffers with half-size output.");
            return false;
        }

        id<MTLComputePipelineState> pipeline =
            stwo_metal_pipeline(runtime, @"gkr_next_grand_product_layer_u32x4", error_message, error_message_len);
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
        [encoder setBytes:&next_layer_size length:sizeof(next_layer_size) atIndex:2];

        MTLSize grid_size = MTLSizeMake(next_layer_size, 1, 1);
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

bool stwo_metal_gkr_next_logup_generic_layer_u32x4(
    void *runtime_ptr,
    void *numerators_ptr,
    void *denominators_ptr,
    void *next_numerators_ptr,
    void *next_denominators_ptr,
    uint32_t input_log_len,
    char *error_message,
    size_t error_message_len
) {
    @autoreleasepool {
        StwoMetalRuntimeBox *runtime = stwo_metal_runtime_box(runtime_ptr);
        StwoMetalBufferBox *numerators = stwo_metal_buffer_box(numerators_ptr);
        StwoMetalBufferBox *denominators = stwo_metal_buffer_box(denominators_ptr);
        StwoMetalBufferBox *next_numerators = stwo_metal_buffer_box(next_numerators_ptr);
        StwoMetalBufferBox *next_denominators = stwo_metal_buffer_box(next_denominators_ptr);
        if (input_log_len == 0u) {
            stwo_metal_write_error(error_message, error_message_len, @"GKR next layer expects at least one input variable.");
            return false;
        }

        uint32_t input_len = 1u << input_log_len;
        uint32_t next_layer_size = input_len >> 1u;
        if (numerators.len != (NSUInteger)(input_len * 4u) ||
            denominators.len != (NSUInteger)(input_len * 4u) ||
            next_numerators.len != (NSUInteger)(next_layer_size * 4u) ||
            next_denominators.len != (NSUInteger)(next_layer_size * 4u)) {
            stwo_metal_write_error(error_message, error_message_len, @"GKR generic next layer expects secure-field numerators and denominators with half-size secure-field outputs.");
            return false;
        }

        id<MTLComputePipelineState> pipeline =
            stwo_metal_pipeline(runtime, @"gkr_next_logup_generic_layer_u32x4", error_message, error_message_len);
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
        [encoder setBuffer:numerators.buffer offset:0 atIndex:0];
        [encoder setBuffer:denominators.buffer offset:0 atIndex:1];
        [encoder setBuffer:next_numerators.buffer offset:0 atIndex:2];
        [encoder setBuffer:next_denominators.buffer offset:0 atIndex:3];
        [encoder setBytes:&next_layer_size length:sizeof(next_layer_size) atIndex:4];

        MTLSize grid_size = MTLSizeMake(next_layer_size, 1, 1);
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

bool stwo_metal_gkr_next_logup_multiplicities_layer_u32(
    void *runtime_ptr,
    void *numerators_ptr,
    void *denominators_ptr,
    void *next_numerators_ptr,
    void *next_denominators_ptr,
    uint32_t input_log_len,
    char *error_message,
    size_t error_message_len
) {
    @autoreleasepool {
        StwoMetalRuntimeBox *runtime = stwo_metal_runtime_box(runtime_ptr);
        StwoMetalBufferBox *numerators = stwo_metal_buffer_box(numerators_ptr);
        StwoMetalBufferBox *denominators = stwo_metal_buffer_box(denominators_ptr);
        StwoMetalBufferBox *next_numerators = stwo_metal_buffer_box(next_numerators_ptr);
        StwoMetalBufferBox *next_denominators = stwo_metal_buffer_box(next_denominators_ptr);
        if (input_log_len == 0u) {
            stwo_metal_write_error(error_message, error_message_len, @"GKR next layer expects at least one input variable.");
            return false;
        }

        uint32_t input_len = 1u << input_log_len;
        uint32_t next_layer_size = input_len >> 1u;
        if (numerators.len != (NSUInteger)input_len ||
            denominators.len != (NSUInteger)(input_len * 4u) ||
            next_numerators.len != (NSUInteger)(next_layer_size * 4u) ||
            next_denominators.len != (NSUInteger)(next_layer_size * 4u)) {
            stwo_metal_write_error(error_message, error_message_len, @"GKR multiplicities next layer expects base-field numerators, secure-field denominators, and secure-field outputs.");
            return false;
        }

        id<MTLComputePipelineState> pipeline =
            stwo_metal_pipeline(runtime, @"gkr_next_logup_multiplicities_layer_u32", error_message, error_message_len);
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
        [encoder setBuffer:numerators.buffer offset:0 atIndex:0];
        [encoder setBuffer:denominators.buffer offset:0 atIndex:1];
        [encoder setBuffer:next_numerators.buffer offset:0 atIndex:2];
        [encoder setBuffer:next_denominators.buffer offset:0 atIndex:3];
        [encoder setBytes:&next_layer_size length:sizeof(next_layer_size) atIndex:4];

        MTLSize grid_size = MTLSizeMake(next_layer_size, 1, 1);
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

bool stwo_metal_gkr_next_logup_singles_layer_u32x4(
    void *runtime_ptr,
    void *denominators_ptr,
    void *next_numerators_ptr,
    void *next_denominators_ptr,
    uint32_t input_log_len,
    char *error_message,
    size_t error_message_len
) {
    @autoreleasepool {
        StwoMetalRuntimeBox *runtime = stwo_metal_runtime_box(runtime_ptr);
        StwoMetalBufferBox *denominators = stwo_metal_buffer_box(denominators_ptr);
        StwoMetalBufferBox *next_numerators = stwo_metal_buffer_box(next_numerators_ptr);
        StwoMetalBufferBox *next_denominators = stwo_metal_buffer_box(next_denominators_ptr);
        if (input_log_len == 0u) {
            stwo_metal_write_error(error_message, error_message_len, @"GKR next layer expects at least one input variable.");
            return false;
        }

        uint32_t input_len = 1u << input_log_len;
        uint32_t next_layer_size = input_len >> 1u;
        if (denominators.len != (NSUInteger)(input_len * 4u) ||
            next_numerators.len != (NSUInteger)(next_layer_size * 4u) ||
            next_denominators.len != (NSUInteger)(next_layer_size * 4u)) {
            stwo_metal_write_error(error_message, error_message_len, @"GKR singles next layer expects secure-field denominators and secure-field outputs.");
            return false;
        }

        id<MTLComputePipelineState> pipeline =
            stwo_metal_pipeline(runtime, @"gkr_next_logup_singles_layer_u32x4", error_message, error_message_len);
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
        [encoder setBuffer:denominators.buffer offset:0 atIndex:0];
        [encoder setBuffer:next_numerators.buffer offset:0 atIndex:1];
        [encoder setBuffer:next_denominators.buffer offset:0 atIndex:2];
        [encoder setBytes:&next_layer_size length:sizeof(next_layer_size) atIndex:3];

        MTLSize grid_size = MTLSizeMake(next_layer_size, 1, 1);
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

bool stwo_metal_gkr_sum_grand_product_u32x4(
    void *runtime_ptr,
    void *eq_evals_ptr,
    void *input_layer_ptr,
    uint32_t n_terms,
    uint32_t *eval_at_0_limbs,
    uint32_t *eval_at_2_limbs,
    char *error_message,
    size_t error_message_len
) {
    @autoreleasepool {
        StwoMetalRuntimeBox *runtime = stwo_metal_runtime_box(runtime_ptr);
        StwoMetalBufferBox *eq_evals = stwo_metal_buffer_box(eq_evals_ptr);
        StwoMetalBufferBox *input_layer = stwo_metal_buffer_box(input_layer_ptr);
        if (eq_evals.len != (NSUInteger)(n_terms * 4u) || input_layer.len != (NSUInteger)(n_terms * 16u)) {
            stwo_metal_write_error(error_message, error_message_len, @"GKR grand-product sum expects n_terms eq-evals and 4*n_terms secure-field input evaluations.");
            return false;
        }

        id<MTLComputePipelineState> terms_pipeline =
            stwo_metal_pipeline(runtime, @"gkr_sum_grand_product_terms_u32x4", error_message, error_message_len);
        id<MTLComputePipelineState> reduce_pipeline =
            stwo_metal_pipeline(runtime, @"gkr_sum_reduce_pairs_u32x4", error_message, error_message_len);
        if (terms_pipeline == nil || reduce_pipeline == nil) {
            return false;
        }

        NSUInteger term_bytes = (NSUInteger)(n_terms * 4u) * sizeof(uint32_t);
        id<MTLBuffer> eval0_terms = [runtime.device newBufferWithLength:term_bytes options:MTLResourceStorageModeShared];
        id<MTLBuffer> eval2_terms = [runtime.device newBufferWithLength:term_bytes options:MTLResourceStorageModeShared];
        id<MTLBuffer> temp0 = [runtime.device newBufferWithLength:term_bytes options:MTLResourceStorageModeShared];
        id<MTLBuffer> temp2 = [runtime.device newBufferWithLength:term_bytes options:MTLResourceStorageModeShared];
        if (eval0_terms == nil || eval2_terms == nil || temp0 == nil || temp2 == nil) {
            stwo_metal_write_error(error_message, error_message_len, @"Failed to allocate Metal GKR sum scratch buffers.");
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
        [encoder setComputePipelineState:terms_pipeline];
        [encoder setBuffer:eq_evals.buffer offset:0 atIndex:0];
        [encoder setBuffer:input_layer.buffer offset:0 atIndex:1];
        [encoder setBuffer:eval0_terms offset:0 atIndex:2];
        [encoder setBuffer:eval2_terms offset:0 atIndex:3];
        [encoder setBytes:&n_terms length:sizeof(n_terms) atIndex:4];
        MTLSize grid_size = MTLSizeMake(n_terms, 1, 1);
        MTLSize threadgroup_size = MTLSizeMake(stwo_metal_threads_per_group(terms_pipeline), 1, 1);
        [encoder dispatchThreads:grid_size threadsPerThreadgroup:threadgroup_size];
        [encoder endEncoding];

        id<MTLBuffer> final_eval0 = stwo_metal_encode_qm31_pair_reduction(
            command_buffer, reduce_pipeline, eval0_terms, temp0, n_terms, error_message, error_message_len);
        if (final_eval0 == nil) {
            return false;
        }
        id<MTLBuffer> final_eval2 = stwo_metal_encode_qm31_pair_reduction(
            command_buffer, reduce_pipeline, eval2_terms, temp2, n_terms, error_message, error_message_len);
        if (final_eval2 == nil) {
            return false;
        }

        [command_buffer commit];
        [command_buffer waitUntilCompleted];
        if (command_buffer.status == MTLCommandBufferStatusError) {
            stwo_metal_write_error(error_message, error_message_len, command_buffer.error.localizedDescription ?: @"Metal kernel execution failed.");
            return false;
        }

        memcpy(eval_at_0_limbs, final_eval0.contents, sizeof(uint32_t) * 4u);
        memcpy(eval_at_2_limbs, final_eval2.contents, sizeof(uint32_t) * 4u);
        return true;
    }
}

bool stwo_metal_gkr_sum_logup_generic_u32x4(
    void *runtime_ptr,
    void *eq_evals_ptr,
    void *numerators_ptr,
    void *denominators_ptr,
    uint32_t n_terms,
    const uint32_t *lambda_limbs,
    uint32_t *eval_at_0_limbs,
    uint32_t *eval_at_2_limbs,
    char *error_message,
    size_t error_message_len
) {
    @autoreleasepool {
        StwoMetalRuntimeBox *runtime = stwo_metal_runtime_box(runtime_ptr);
        StwoMetalBufferBox *eq_evals = stwo_metal_buffer_box(eq_evals_ptr);
        StwoMetalBufferBox *numerators = stwo_metal_buffer_box(numerators_ptr);
        StwoMetalBufferBox *denominators = stwo_metal_buffer_box(denominators_ptr);
        if (eq_evals.len != (NSUInteger)(n_terms * 4u) ||
            numerators.len != (NSUInteger)(n_terms * 16u) ||
            denominators.len != (NSUInteger)(n_terms * 16u)) {
            stwo_metal_write_error(error_message, error_message_len, @"GKR generic sum expects n_terms eq-evals and 4*n_terms secure-field numerator and denominator evaluations.");
            return false;
        }

        id<MTLComputePipelineState> terms_pipeline =
            stwo_metal_pipeline(runtime, @"gkr_sum_logup_generic_terms_u32x4", error_message, error_message_len);
        id<MTLComputePipelineState> reduce_pipeline =
            stwo_metal_pipeline(runtime, @"gkr_sum_reduce_pairs_u32x4", error_message, error_message_len);
        if (terms_pipeline == nil || reduce_pipeline == nil) {
            return false;
        }

        NSUInteger term_bytes = (NSUInteger)(n_terms * 4u) * sizeof(uint32_t);
        id<MTLBuffer> eval0_terms = [runtime.device newBufferWithLength:term_bytes options:MTLResourceStorageModeShared];
        id<MTLBuffer> eval2_terms = [runtime.device newBufferWithLength:term_bytes options:MTLResourceStorageModeShared];
        id<MTLBuffer> temp0 = [runtime.device newBufferWithLength:term_bytes options:MTLResourceStorageModeShared];
        id<MTLBuffer> temp2 = [runtime.device newBufferWithLength:term_bytes options:MTLResourceStorageModeShared];
        if (eval0_terms == nil || eval2_terms == nil || temp0 == nil || temp2 == nil) {
            stwo_metal_write_error(error_message, error_message_len, @"Failed to allocate Metal GKR sum scratch buffers.");
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
        [encoder setComputePipelineState:terms_pipeline];
        [encoder setBuffer:eq_evals.buffer offset:0 atIndex:0];
        [encoder setBuffer:numerators.buffer offset:0 atIndex:1];
        [encoder setBuffer:denominators.buffer offset:0 atIndex:2];
        [encoder setBuffer:eval0_terms offset:0 atIndex:3];
        [encoder setBuffer:eval2_terms offset:0 atIndex:4];
        [encoder setBytes:lambda_limbs length:sizeof(uint32_t) * 4u atIndex:5];
        [encoder setBytes:&n_terms length:sizeof(n_terms) atIndex:6];
        MTLSize grid_size = MTLSizeMake(n_terms, 1, 1);
        MTLSize threadgroup_size = MTLSizeMake(stwo_metal_threads_per_group(terms_pipeline), 1, 1);
        [encoder dispatchThreads:grid_size threadsPerThreadgroup:threadgroup_size];
        [encoder endEncoding];

        id<MTLBuffer> final_eval0 = stwo_metal_encode_qm31_pair_reduction(
            command_buffer, reduce_pipeline, eval0_terms, temp0, n_terms, error_message, error_message_len);
        if (final_eval0 == nil) {
            return false;
        }
        id<MTLBuffer> final_eval2 = stwo_metal_encode_qm31_pair_reduction(
            command_buffer, reduce_pipeline, eval2_terms, temp2, n_terms, error_message, error_message_len);
        if (final_eval2 == nil) {
            return false;
        }

        [command_buffer commit];
        [command_buffer waitUntilCompleted];
        if (command_buffer.status == MTLCommandBufferStatusError) {
            stwo_metal_write_error(error_message, error_message_len, command_buffer.error.localizedDescription ?: @"Metal kernel execution failed.");
            return false;
        }

        memcpy(eval_at_0_limbs, final_eval0.contents, sizeof(uint32_t) * 4u);
        memcpy(eval_at_2_limbs, final_eval2.contents, sizeof(uint32_t) * 4u);
        return true;
    }
}

bool stwo_metal_gkr_sum_logup_multiplicities_u32(
    void *runtime_ptr,
    void *eq_evals_ptr,
    void *numerators_ptr,
    void *denominators_ptr,
    uint32_t n_terms,
    const uint32_t *lambda_limbs,
    uint32_t *eval_at_0_limbs,
    uint32_t *eval_at_2_limbs,
    char *error_message,
    size_t error_message_len
) {
    @autoreleasepool {
        StwoMetalRuntimeBox *runtime = stwo_metal_runtime_box(runtime_ptr);
        StwoMetalBufferBox *eq_evals = stwo_metal_buffer_box(eq_evals_ptr);
        StwoMetalBufferBox *numerators = stwo_metal_buffer_box(numerators_ptr);
        StwoMetalBufferBox *denominators = stwo_metal_buffer_box(denominators_ptr);
        if (eq_evals.len != (NSUInteger)(n_terms * 4u) ||
            numerators.len != (NSUInteger)(n_terms * 4u) ||
            denominators.len != (NSUInteger)(n_terms * 16u)) {
            stwo_metal_write_error(error_message, error_message_len, @"GKR multiplicities sum expects n_terms eq-evals, 4*n_terms base-field numerators, and 4*n_terms secure-field denominators.");
            return false;
        }

        id<MTLComputePipelineState> terms_pipeline =
            stwo_metal_pipeline(runtime, @"gkr_sum_logup_multiplicities_terms_u32", error_message, error_message_len);
        id<MTLComputePipelineState> reduce_pipeline =
            stwo_metal_pipeline(runtime, @"gkr_sum_reduce_pairs_u32x4", error_message, error_message_len);
        if (terms_pipeline == nil || reduce_pipeline == nil) {
            return false;
        }

        NSUInteger term_bytes = (NSUInteger)(n_terms * 4u) * sizeof(uint32_t);
        id<MTLBuffer> eval0_terms = [runtime.device newBufferWithLength:term_bytes options:MTLResourceStorageModeShared];
        id<MTLBuffer> eval2_terms = [runtime.device newBufferWithLength:term_bytes options:MTLResourceStorageModeShared];
        id<MTLBuffer> temp0 = [runtime.device newBufferWithLength:term_bytes options:MTLResourceStorageModeShared];
        id<MTLBuffer> temp2 = [runtime.device newBufferWithLength:term_bytes options:MTLResourceStorageModeShared];
        if (eval0_terms == nil || eval2_terms == nil || temp0 == nil || temp2 == nil) {
            stwo_metal_write_error(error_message, error_message_len, @"Failed to allocate Metal GKR sum scratch buffers.");
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
        [encoder setComputePipelineState:terms_pipeline];
        [encoder setBuffer:eq_evals.buffer offset:0 atIndex:0];
        [encoder setBuffer:numerators.buffer offset:0 atIndex:1];
        [encoder setBuffer:denominators.buffer offset:0 atIndex:2];
        [encoder setBuffer:eval0_terms offset:0 atIndex:3];
        [encoder setBuffer:eval2_terms offset:0 atIndex:4];
        [encoder setBytes:lambda_limbs length:sizeof(uint32_t) * 4u atIndex:5];
        [encoder setBytes:&n_terms length:sizeof(n_terms) atIndex:6];
        MTLSize grid_size = MTLSizeMake(n_terms, 1, 1);
        MTLSize threadgroup_size = MTLSizeMake(stwo_metal_threads_per_group(terms_pipeline), 1, 1);
        [encoder dispatchThreads:grid_size threadsPerThreadgroup:threadgroup_size];
        [encoder endEncoding];

        id<MTLBuffer> final_eval0 = stwo_metal_encode_qm31_pair_reduction(
            command_buffer, reduce_pipeline, eval0_terms, temp0, n_terms, error_message, error_message_len);
        if (final_eval0 == nil) {
            return false;
        }
        id<MTLBuffer> final_eval2 = stwo_metal_encode_qm31_pair_reduction(
            command_buffer, reduce_pipeline, eval2_terms, temp2, n_terms, error_message, error_message_len);
        if (final_eval2 == nil) {
            return false;
        }

        [command_buffer commit];
        [command_buffer waitUntilCompleted];
        if (command_buffer.status == MTLCommandBufferStatusError) {
            stwo_metal_write_error(error_message, error_message_len, command_buffer.error.localizedDescription ?: @"Metal kernel execution failed.");
            return false;
        }

        memcpy(eval_at_0_limbs, final_eval0.contents, sizeof(uint32_t) * 4u);
        memcpy(eval_at_2_limbs, final_eval2.contents, sizeof(uint32_t) * 4u);
        return true;
    }
}

bool stwo_metal_gkr_sum_logup_singles_u32x4(
    void *runtime_ptr,
    void *eq_evals_ptr,
    void *denominators_ptr,
    uint32_t n_terms,
    const uint32_t *lambda_limbs,
    uint32_t *eval_at_0_limbs,
    uint32_t *eval_at_2_limbs,
    char *error_message,
    size_t error_message_len
) {
    @autoreleasepool {
        StwoMetalRuntimeBox *runtime = stwo_metal_runtime_box(runtime_ptr);
        StwoMetalBufferBox *eq_evals = stwo_metal_buffer_box(eq_evals_ptr);
        StwoMetalBufferBox *denominators = stwo_metal_buffer_box(denominators_ptr);
        if (eq_evals.len != (NSUInteger)(n_terms * 4u) || denominators.len != (NSUInteger)(n_terms * 16u)) {
            stwo_metal_write_error(error_message, error_message_len, @"GKR singles sum expects n_terms eq-evals and 4*n_terms secure-field denominators.");
            return false;
        }

        id<MTLComputePipelineState> terms_pipeline =
            stwo_metal_pipeline(runtime, @"gkr_sum_logup_singles_terms_u32x4", error_message, error_message_len);
        id<MTLComputePipelineState> reduce_pipeline =
            stwo_metal_pipeline(runtime, @"gkr_sum_reduce_pairs_u32x4", error_message, error_message_len);
        if (terms_pipeline == nil || reduce_pipeline == nil) {
            return false;
        }

        NSUInteger term_bytes = (NSUInteger)(n_terms * 4u) * sizeof(uint32_t);
        id<MTLBuffer> eval0_terms = [runtime.device newBufferWithLength:term_bytes options:MTLResourceStorageModeShared];
        id<MTLBuffer> eval2_terms = [runtime.device newBufferWithLength:term_bytes options:MTLResourceStorageModeShared];
        id<MTLBuffer> temp0 = [runtime.device newBufferWithLength:term_bytes options:MTLResourceStorageModeShared];
        id<MTLBuffer> temp2 = [runtime.device newBufferWithLength:term_bytes options:MTLResourceStorageModeShared];
        if (eval0_terms == nil || eval2_terms == nil || temp0 == nil || temp2 == nil) {
            stwo_metal_write_error(error_message, error_message_len, @"Failed to allocate Metal GKR sum scratch buffers.");
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
        [encoder setComputePipelineState:terms_pipeline];
        [encoder setBuffer:eq_evals.buffer offset:0 atIndex:0];
        [encoder setBuffer:denominators.buffer offset:0 atIndex:1];
        [encoder setBuffer:eval0_terms offset:0 atIndex:2];
        [encoder setBuffer:eval2_terms offset:0 atIndex:3];
        [encoder setBytes:lambda_limbs length:sizeof(uint32_t) * 4u atIndex:4];
        [encoder setBytes:&n_terms length:sizeof(n_terms) atIndex:5];
        MTLSize grid_size = MTLSizeMake(n_terms, 1, 1);
        MTLSize threadgroup_size = MTLSizeMake(stwo_metal_threads_per_group(terms_pipeline), 1, 1);
        [encoder dispatchThreads:grid_size threadsPerThreadgroup:threadgroup_size];
        [encoder endEncoding];

        id<MTLBuffer> final_eval0 = stwo_metal_encode_qm31_pair_reduction(
            command_buffer, reduce_pipeline, eval0_terms, temp0, n_terms, error_message, error_message_len);
        if (final_eval0 == nil) {
            return false;
        }
        id<MTLBuffer> final_eval2 = stwo_metal_encode_qm31_pair_reduction(
            command_buffer, reduce_pipeline, eval2_terms, temp2, n_terms, error_message, error_message_len);
        if (final_eval2 == nil) {
            return false;
        }

        [command_buffer commit];
        [command_buffer waitUntilCompleted];
        if (command_buffer.status == MTLCommandBufferStatusError) {
            stwo_metal_write_error(error_message, error_message_len, command_buffer.error.localizedDescription ?: @"Metal kernel execution failed.");
            return false;
        }

        memcpy(eval_at_0_limbs, final_eval0.contents, sizeof(uint32_t) * 4u);
        memcpy(eval_at_2_limbs, final_eval2.contents, sizeof(uint32_t) * 4u);
        return true;
    }
}

bool stwo_metal_inclusive_prefix_sum_bit_rev_circle_domain_u32(
    void *runtime_ptr,
    void *buffer_ptr,
    uint32_t log_len,
    char *error_message,
    size_t error_message_len
) {
    @autoreleasepool {
        StwoMetalRuntimeBox *runtime = stwo_metal_runtime_box(runtime_ptr);
        StwoMetalBufferBox *buffer = stwo_metal_buffer_box(buffer_ptr);
        uint32_t len = ((uint32_t)1) << log_len;
        if (buffer.len != (NSUInteger)len) {
            stwo_metal_write_error(error_message, error_message_len, @"Prefix sum expects a power-of-two base-field buffer matching log_len.");
            return false;
        }

        if (!stwo_metal_dispatch_unary_u32_kernel(
                runtime,
                @"bit_reverse_u32",
                buffer,
                log_len,
                error_message,
                error_message_len)) {
            return false;
        }

        id<MTLComputePipelineState> circle_to_coset =
            stwo_metal_pipeline(runtime, @"prefix_sum_circle_domain_order_to_coset_order_u32", error_message, error_message_len);
        if (circle_to_coset == nil) {
            return false;
        }
        id<MTLComputePipelineState> coset_to_circle =
            stwo_metal_pipeline(runtime, @"prefix_sum_coset_order_to_circle_domain_order_u32", error_message, error_message_len);
        if (coset_to_circle == nil) {
            return false;
        }
        id<MTLComputePipelineState> inclusive_step =
            stwo_metal_pipeline(runtime, @"prefix_sum_inclusive_step_u32", error_message, error_message_len);
        if (inclusive_step == nil) {
            return false;
        }

        NSUInteger bytes = (NSUInteger)len * sizeof(uint32_t);
        id<MTLBuffer> coset_buffer =
            [runtime.device newBufferWithLength:bytes options:MTLResourceStorageModeShared];
        id<MTLBuffer> scan_buffer =
            [runtime.device newBufferWithLength:bytes options:MTLResourceStorageModeShared];
        if (coset_buffer == nil || scan_buffer == nil) {
            stwo_metal_write_error(error_message, error_message_len, @"Failed to allocate Metal prefix-sum scratch buffers.");
            return false;
        }

        id<MTLCommandBuffer> command_buffer = [runtime.queue commandBuffer];
        if (command_buffer == nil) {
            stwo_metal_write_error(error_message, error_message_len, @"Failed to create Metal command buffer.");
            return false;
        }

        uint32_t half_len = len >> 1u;
        id<MTLComputeCommandEncoder> reorder_encoder = [command_buffer computeCommandEncoder];
        if (reorder_encoder == nil) {
            stwo_metal_write_error(error_message, error_message_len, @"Failed to create Metal compute encoder.");
            return false;
        }
        [reorder_encoder setComputePipelineState:circle_to_coset];
        [reorder_encoder setBuffer:buffer.buffer offset:0 atIndex:0];
        [reorder_encoder setBuffer:coset_buffer offset:0 atIndex:1];
        [reorder_encoder setBytes:&len length:sizeof(len) atIndex:2];
        MTLSize reorder_grid = MTLSizeMake(MAX((uint32_t)1u, half_len), 1, 1);
        MTLSize reorder_threads = MTLSizeMake(stwo_metal_threads_per_group(circle_to_coset), 1, 1);
        [reorder_encoder dispatchThreads:reorder_grid threadsPerThreadgroup:reorder_threads];
        [reorder_encoder endEncoding];

        id<MTLBuffer> current = coset_buffer;
        id<MTLBuffer> temp = scan_buffer;
        for (uint32_t stride = 1u; stride < len; stride <<= 1u) {
            id<MTLComputeCommandEncoder> scan_encoder = [command_buffer computeCommandEncoder];
            if (scan_encoder == nil) {
                stwo_metal_write_error(error_message, error_message_len, @"Failed to create Metal compute encoder.");
                return false;
            }
            [scan_encoder setComputePipelineState:inclusive_step];
            [scan_encoder setBuffer:current offset:0 atIndex:0];
            [scan_encoder setBuffer:temp offset:0 atIndex:1];
            [scan_encoder setBytes:&len length:sizeof(len) atIndex:2];
            [scan_encoder setBytes:&stride length:sizeof(stride) atIndex:3];
            MTLSize scan_grid = MTLSizeMake(len, 1, 1);
            MTLSize scan_threads = MTLSizeMake(stwo_metal_threads_per_group(inclusive_step), 1, 1);
            [scan_encoder dispatchThreads:scan_grid threadsPerThreadgroup:scan_threads];
            [scan_encoder endEncoding];

            id<MTLBuffer> swap = current;
            current = temp;
            temp = swap;
        }

        id<MTLComputeCommandEncoder> restore_encoder = [command_buffer computeCommandEncoder];
        if (restore_encoder == nil) {
            stwo_metal_write_error(error_message, error_message_len, @"Failed to create Metal compute encoder.");
            return false;
        }
        [restore_encoder setComputePipelineState:coset_to_circle];
        [restore_encoder setBuffer:current offset:0 atIndex:0];
        [restore_encoder setBuffer:buffer.buffer offset:0 atIndex:1];
        [restore_encoder setBytes:&len length:sizeof(len) atIndex:2];
        MTLSize restore_grid = MTLSizeMake(len, 1, 1);
        MTLSize restore_threads = MTLSizeMake(stwo_metal_threads_per_group(coset_to_circle), 1, 1);
        [restore_encoder dispatchThreads:restore_grid threadsPerThreadgroup:restore_threads];
        [restore_encoder endEncoding];

        [command_buffer commit];
        [command_buffer waitUntilCompleted];
        if (command_buffer.status == MTLCommandBufferStatusError) {
            stwo_metal_write_error(error_message, error_message_len, command_buffer.error.localizedDescription ?: @"Metal kernel execution failed.");
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

bool stwo_metal_pack_secure_column_coords_u32x4(
    void *runtime_ptr,
    void *coord_0_ptr,
    void *coord_1_ptr,
    void *coord_2_ptr,
    void *coord_3_ptr,
    void *dst_ptr,
    uint32_t element_len,
    char *error_message,
    size_t error_message_len
) {
    @autoreleasepool {
        StwoMetalRuntimeBox *runtime = stwo_metal_runtime_box(runtime_ptr);
        StwoMetalBufferBox *coord_0 = stwo_metal_buffer_box(coord_0_ptr);
        StwoMetalBufferBox *coord_1 = stwo_metal_buffer_box(coord_1_ptr);
        StwoMetalBufferBox *coord_2 = stwo_metal_buffer_box(coord_2_ptr);
        StwoMetalBufferBox *coord_3 = stwo_metal_buffer_box(coord_3_ptr);
        StwoMetalBufferBox *dst = stwo_metal_buffer_box(dst_ptr);
        NSUInteger expected_dst_len = (NSUInteger)element_len * 4u;
        if (
            coord_0.len != element_len || coord_1.len != element_len ||
            coord_2.len != element_len || coord_3.len != element_len ||
            dst.len != expected_dst_len
        ) {
            stwo_metal_write_error(error_message, error_message_len, @"Secure-column packing expects four equally sized coordinate buffers and one packed u32x4 destination.");
            return false;
        }

        id<MTLComputePipelineState> pipeline = stwo_metal_pipeline(runtime, @"pack_secure_column_coords_u32x4", error_message, error_message_len);
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
        [encoder setBuffer:coord_0.buffer offset:0 atIndex:0];
        [encoder setBuffer:coord_1.buffer offset:0 atIndex:1];
        [encoder setBuffer:coord_2.buffer offset:0 atIndex:2];
        [encoder setBuffer:coord_3.buffer offset:0 atIndex:3];
        [encoder setBuffer:dst.buffer offset:0 atIndex:4];

        MTLSize grid_size = MTLSizeMake(element_len, 1, 1);
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

bool stwo_metal_unpack_secure_column_coords_u32x4(
    void *runtime_ptr,
    void *src_ptr,
    void *coord_0_ptr,
    void *coord_1_ptr,
    void *coord_2_ptr,
    void *coord_3_ptr,
    uint32_t element_len,
    char *error_message,
    size_t error_message_len
) {
    @autoreleasepool {
        StwoMetalRuntimeBox *runtime = stwo_metal_runtime_box(runtime_ptr);
        StwoMetalBufferBox *src = stwo_metal_buffer_box(src_ptr);
        StwoMetalBufferBox *coord_0 = stwo_metal_buffer_box(coord_0_ptr);
        StwoMetalBufferBox *coord_1 = stwo_metal_buffer_box(coord_1_ptr);
        StwoMetalBufferBox *coord_2 = stwo_metal_buffer_box(coord_2_ptr);
        StwoMetalBufferBox *coord_3 = stwo_metal_buffer_box(coord_3_ptr);
        NSUInteger expected_src_len = (NSUInteger)element_len * 4u;
        if (
            src.len != expected_src_len ||
            coord_0.len != element_len || coord_1.len != element_len ||
            coord_2.len != element_len || coord_3.len != element_len
        ) {
            stwo_metal_write_error(error_message, error_message_len, @"Secure-column unpacking expects one packed u32x4 source and four equally sized coordinate buffers.");
            return false;
        }

        id<MTLComputePipelineState> pipeline = stwo_metal_pipeline(runtime, @"unpack_secure_column_coords_u32x4", error_message, error_message_len);
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
        [encoder setBuffer:coord_0.buffer offset:0 atIndex:1];
        [encoder setBuffer:coord_1.buffer offset:0 atIndex:2];
        [encoder setBuffer:coord_2.buffer offset:0 atIndex:3];
        [encoder setBuffer:coord_3.buffer offset:0 atIndex:4];

        MTLSize grid_size = MTLSizeMake(element_len, 1, 1);
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

bool stwo_metal_accumulate_secure_columns_coords_u32x4(
    void *runtime_ptr,
    void *lhs_0_ptr,
    void *lhs_1_ptr,
    void *lhs_2_ptr,
    void *lhs_3_ptr,
    void *rhs_0_ptr,
    void *rhs_1_ptr,
    void *rhs_2_ptr,
    void *rhs_3_ptr,
    void *dst_0_ptr,
    void *dst_1_ptr,
    void *dst_2_ptr,
    void *dst_3_ptr,
    uint32_t element_len,
    char *error_message,
    size_t error_message_len
) {
    @autoreleasepool {
        StwoMetalRuntimeBox *runtime = stwo_metal_runtime_box(runtime_ptr);
        StwoMetalBufferBox *lhs_0 = stwo_metal_buffer_box(lhs_0_ptr);
        StwoMetalBufferBox *lhs_1 = stwo_metal_buffer_box(lhs_1_ptr);
        StwoMetalBufferBox *lhs_2 = stwo_metal_buffer_box(lhs_2_ptr);
        StwoMetalBufferBox *lhs_3 = stwo_metal_buffer_box(lhs_3_ptr);
        StwoMetalBufferBox *rhs_0 = stwo_metal_buffer_box(rhs_0_ptr);
        StwoMetalBufferBox *rhs_1 = stwo_metal_buffer_box(rhs_1_ptr);
        StwoMetalBufferBox *rhs_2 = stwo_metal_buffer_box(rhs_2_ptr);
        StwoMetalBufferBox *rhs_3 = stwo_metal_buffer_box(rhs_3_ptr);
        StwoMetalBufferBox *dst_0 = stwo_metal_buffer_box(dst_0_ptr);
        StwoMetalBufferBox *dst_1 = stwo_metal_buffer_box(dst_1_ptr);
        StwoMetalBufferBox *dst_2 = stwo_metal_buffer_box(dst_2_ptr);
        StwoMetalBufferBox *dst_3 = stwo_metal_buffer_box(dst_3_ptr);
        if (
            lhs_0.len != element_len || lhs_1.len != element_len ||
            lhs_2.len != element_len || lhs_3.len != element_len ||
            rhs_0.len != element_len || rhs_1.len != element_len ||
            rhs_2.len != element_len || rhs_3.len != element_len ||
            dst_0.len != element_len || dst_1.len != element_len ||
            dst_2.len != element_len || dst_3.len != element_len
        ) {
            stwo_metal_write_error(error_message, error_message_len, @"Secure-column accumulation expects four equally sized lhs columns, four equally sized rhs columns, and four equally sized destinations.");
            return false;
        }

        id<MTLComputePipelineState> pipeline = stwo_metal_pipeline(runtime, @"accumulate_secure_columns_coords_u32x4", error_message, error_message_len);
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
        [encoder setBuffer:lhs_0.buffer offset:0 atIndex:0];
        [encoder setBuffer:lhs_1.buffer offset:0 atIndex:1];
        [encoder setBuffer:lhs_2.buffer offset:0 atIndex:2];
        [encoder setBuffer:lhs_3.buffer offset:0 atIndex:3];
        [encoder setBuffer:rhs_0.buffer offset:0 atIndex:4];
        [encoder setBuffer:rhs_1.buffer offset:0 atIndex:5];
        [encoder setBuffer:rhs_2.buffer offset:0 atIndex:6];
        [encoder setBuffer:rhs_3.buffer offset:0 atIndex:7];
        [encoder setBuffer:dst_0.buffer offset:0 atIndex:8];
        [encoder setBuffer:dst_1.buffer offset:0 atIndex:9];
        [encoder setBuffer:dst_2.buffer offset:0 atIndex:10];
        [encoder setBuffer:dst_3.buffer offset:0 atIndex:11];
        [encoder setBytes:&element_len length:sizeof(element_len) atIndex:12];

        MTLSize grid_size = MTLSizeMake(element_len, 1, 1);
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

bool stwo_metal_lift_accumulate_secure_columns_coords_u32x4(
    void *runtime_ptr,
    void *lifted_0_ptr,
    void *lifted_1_ptr,
    void *lifted_2_ptr,
    void *lifted_3_ptr,
    void *current_0_ptr,
    void *current_1_ptr,
    void *current_2_ptr,
    void *current_3_ptr,
    void *dst_0_ptr,
    void *dst_1_ptr,
    void *dst_2_ptr,
    void *dst_3_ptr,
    uint32_t current_log_size,
    uint32_t log_ratio,
    char *error_message,
    size_t error_message_len
) {
    @autoreleasepool {
        StwoMetalRuntimeBox *runtime = stwo_metal_runtime_box(runtime_ptr);
        StwoMetalBufferBox *lifted_0 = stwo_metal_buffer_box(lifted_0_ptr);
        StwoMetalBufferBox *lifted_1 = stwo_metal_buffer_box(lifted_1_ptr);
        StwoMetalBufferBox *lifted_2 = stwo_metal_buffer_box(lifted_2_ptr);
        StwoMetalBufferBox *lifted_3 = stwo_metal_buffer_box(lifted_3_ptr);
        StwoMetalBufferBox *current_0 = stwo_metal_buffer_box(current_0_ptr);
        StwoMetalBufferBox *current_1 = stwo_metal_buffer_box(current_1_ptr);
        StwoMetalBufferBox *current_2 = stwo_metal_buffer_box(current_2_ptr);
        StwoMetalBufferBox *current_3 = stwo_metal_buffer_box(current_3_ptr);
        StwoMetalBufferBox *dst_0 = stwo_metal_buffer_box(dst_0_ptr);
        StwoMetalBufferBox *dst_1 = stwo_metal_buffer_box(dst_1_ptr);
        StwoMetalBufferBox *dst_2 = stwo_metal_buffer_box(dst_2_ptr);
        StwoMetalBufferBox *dst_3 = stwo_metal_buffer_box(dst_3_ptr);
        uint32_t current_len = ((uint32_t)1) << current_log_size;
        uint32_t lifted_len = current_len >> log_ratio;
        if (
            current_0.len != current_len || current_1.len != current_len ||
            current_2.len != current_len || current_3.len != current_len ||
            dst_0.len != current_len || dst_1.len != current_len ||
            dst_2.len != current_len || dst_3.len != current_len ||
            lifted_0.len != lifted_len || lifted_1.len != lifted_len ||
            lifted_2.len != lifted_len || lifted_3.len != lifted_len
        ) {
            stwo_metal_write_error(error_message, error_message_len, @"Secure-column lift-and-accumulate expects a power-of-two current length, matching destinations, and lifted columns sized by the log-ratio.");
            return false;
        }

        id<MTLComputePipelineState> pipeline = stwo_metal_pipeline(runtime, @"lift_accumulate_secure_columns_coords_u32x4", error_message, error_message_len);
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
        [encoder setBuffer:lifted_0.buffer offset:0 atIndex:0];
        [encoder setBuffer:lifted_1.buffer offset:0 atIndex:1];
        [encoder setBuffer:lifted_2.buffer offset:0 atIndex:2];
        [encoder setBuffer:lifted_3.buffer offset:0 atIndex:3];
        [encoder setBuffer:current_0.buffer offset:0 atIndex:4];
        [encoder setBuffer:current_1.buffer offset:0 atIndex:5];
        [encoder setBuffer:current_2.buffer offset:0 atIndex:6];
        [encoder setBuffer:current_3.buffer offset:0 atIndex:7];
        [encoder setBuffer:dst_0.buffer offset:0 atIndex:8];
        [encoder setBuffer:dst_1.buffer offset:0 atIndex:9];
        [encoder setBuffer:dst_2.buffer offset:0 atIndex:10];
        [encoder setBuffer:dst_3.buffer offset:0 atIndex:11];
        [encoder setBytes:&current_log_size length:sizeof(current_log_size) atIndex:12];
        [encoder setBytes:&log_ratio length:sizeof(log_ratio) atIndex:13];

        MTLSize grid_size = MTLSizeMake(current_len, 1, 1);
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

bool stwo_metal_fri_fold_circle_into_line_accumulate_coords_u32x4(
    void *runtime_ptr,
    void *src_0_ptr,
    void *src_1_ptr,
    void *src_2_ptr,
    void *src_3_ptr,
    void *dst_0_ptr,
    void *dst_1_ptr,
    void *dst_2_ptr,
    void *dst_3_ptr,
    void *inverse_y_ptr,
    uint32_t src_log_len,
    const uint32_t *alpha_limbs,
    const uint32_t *alpha_sq_limbs,
    char *error_message,
    size_t error_message_len
) {
    @autoreleasepool {
        StwoMetalRuntimeBox *runtime = stwo_metal_runtime_box(runtime_ptr);
        StwoMetalBufferBox *src_0 = stwo_metal_buffer_box(src_0_ptr);
        StwoMetalBufferBox *src_1 = stwo_metal_buffer_box(src_1_ptr);
        StwoMetalBufferBox *src_2 = stwo_metal_buffer_box(src_2_ptr);
        StwoMetalBufferBox *src_3 = stwo_metal_buffer_box(src_3_ptr);
        StwoMetalBufferBox *dst_0 = stwo_metal_buffer_box(dst_0_ptr);
        StwoMetalBufferBox *dst_1 = stwo_metal_buffer_box(dst_1_ptr);
        StwoMetalBufferBox *dst_2 = stwo_metal_buffer_box(dst_2_ptr);
        StwoMetalBufferBox *dst_3 = stwo_metal_buffer_box(dst_3_ptr);
        StwoMetalBufferBox *inverse_y = stwo_metal_buffer_box(inverse_y_ptr);
        NSUInteger src_len = ((NSUInteger)1) << src_log_len;
        NSUInteger dst_len = src_len >> 1;
        if (
            src_0.len != src_len || src_1.len != src_len ||
            src_2.len != src_len || src_3.len != src_len ||
            inverse_y.len != dst_len ||
            dst_0.len != dst_len || dst_1.len != dst_len ||
            dst_2.len != dst_len || dst_3.len != dst_len
        ) {
            stwo_metal_write_error(error_message, error_message_len, @"FRI first-layer accumulation expects four source coordinate buffers, four destination coordinate buffers, and one inverse-y factor per output element.");
            return false;
        }

        id<MTLComputePipelineState> pipeline = stwo_metal_pipeline(runtime, @"fri_fold_circle_into_line_accumulate_coords_u32x4", error_message, error_message_len);
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
        [encoder setBuffer:src_0.buffer offset:0 atIndex:0];
        [encoder setBuffer:src_1.buffer offset:0 atIndex:1];
        [encoder setBuffer:src_2.buffer offset:0 atIndex:2];
        [encoder setBuffer:src_3.buffer offset:0 atIndex:3];
        [encoder setBuffer:dst_0.buffer offset:0 atIndex:4];
        [encoder setBuffer:dst_1.buffer offset:0 atIndex:5];
        [encoder setBuffer:dst_2.buffer offset:0 atIndex:6];
        [encoder setBuffer:dst_3.buffer offset:0 atIndex:7];
        [encoder setBuffer:inverse_y.buffer offset:0 atIndex:8];
        [encoder setBytes:alpha_limbs length:sizeof(uint32_t) * 4 atIndex:9];
        [encoder setBytes:alpha_sq_limbs length:sizeof(uint32_t) * 4 atIndex:10];

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

bool stwo_metal_fri_fold_circle_into_line_first_layer_coords_u32x4(
    void *runtime_ptr,
    void *src_0_ptr,
    void *src_1_ptr,
    void *src_2_ptr,
    void *src_3_ptr,
    void *dst_0_ptr,
    void *dst_1_ptr,
    void *dst_2_ptr,
    void *dst_3_ptr,
    void *inverse_y_ptr,
    uint32_t src_log_len,
    const uint32_t *alpha_limbs,
    char *error_message,
    size_t error_message_len
) {
    @autoreleasepool {
        StwoMetalRuntimeBox *runtime = stwo_metal_runtime_box(runtime_ptr);
        StwoMetalBufferBox *src_0 = stwo_metal_buffer_box(src_0_ptr);
        StwoMetalBufferBox *src_1 = stwo_metal_buffer_box(src_1_ptr);
        StwoMetalBufferBox *src_2 = stwo_metal_buffer_box(src_2_ptr);
        StwoMetalBufferBox *src_3 = stwo_metal_buffer_box(src_3_ptr);
        StwoMetalBufferBox *dst_0 = stwo_metal_buffer_box(dst_0_ptr);
        StwoMetalBufferBox *dst_1 = stwo_metal_buffer_box(dst_1_ptr);
        StwoMetalBufferBox *dst_2 = stwo_metal_buffer_box(dst_2_ptr);
        StwoMetalBufferBox *dst_3 = stwo_metal_buffer_box(dst_3_ptr);
        StwoMetalBufferBox *inverse_y = stwo_metal_buffer_box(inverse_y_ptr);
        NSUInteger src_len = ((NSUInteger)1) << src_log_len;
        NSUInteger dst_len = src_len >> 1;
        if (
            src_0.len != src_len || src_1.len != src_len ||
            src_2.len != src_len || src_3.len != src_len ||
            inverse_y.len != dst_len ||
            dst_0.len != dst_len || dst_1.len != dst_len ||
            dst_2.len != dst_len || dst_3.len != dst_len
        ) {
            stwo_metal_write_error(error_message, error_message_len, @"FRI first-layer coordinate fold expects four source coordinate buffers, four destination coordinate buffers, and one inverse-y factor per output element.");
            return false;
        }

        id<MTLComputePipelineState> pipeline = stwo_metal_pipeline(runtime, @"fri_fold_circle_into_line_first_layer_coords_u32x4", error_message, error_message_len);
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
        [encoder setBuffer:src_0.buffer offset:0 atIndex:0];
        [encoder setBuffer:src_1.buffer offset:0 atIndex:1];
        [encoder setBuffer:src_2.buffer offset:0 atIndex:2];
        [encoder setBuffer:src_3.buffer offset:0 atIndex:3];
        [encoder setBuffer:dst_0.buffer offset:0 atIndex:4];
        [encoder setBuffer:dst_1.buffer offset:0 atIndex:5];
        [encoder setBuffer:dst_2.buffer offset:0 atIndex:6];
        [encoder setBuffer:dst_3.buffer offset:0 atIndex:7];
        [encoder setBuffer:inverse_y.buffer offset:0 atIndex:8];
        [encoder setBytes:alpha_limbs length:sizeof(uint32_t) * 4 atIndex:9];

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

bool stwo_metal_fri_fold_line_step_coords_u32x4(
    void *runtime_ptr,
    void *src_0_ptr,
    void *src_1_ptr,
    void *src_2_ptr,
    void *src_3_ptr,
    void *dst_0_ptr,
    void *dst_1_ptr,
    void *dst_2_ptr,
    void *dst_3_ptr,
    void *inverse_x_ptr,
    uint32_t src_log_len,
    const uint32_t *alpha_limbs,
    char *error_message,
    size_t error_message_len
) {
    @autoreleasepool {
        StwoMetalRuntimeBox *runtime = stwo_metal_runtime_box(runtime_ptr);
        StwoMetalBufferBox *src_0 = stwo_metal_buffer_box(src_0_ptr);
        StwoMetalBufferBox *src_1 = stwo_metal_buffer_box(src_1_ptr);
        StwoMetalBufferBox *src_2 = stwo_metal_buffer_box(src_2_ptr);
        StwoMetalBufferBox *src_3 = stwo_metal_buffer_box(src_3_ptr);
        StwoMetalBufferBox *dst_0 = stwo_metal_buffer_box(dst_0_ptr);
        StwoMetalBufferBox *dst_1 = stwo_metal_buffer_box(dst_1_ptr);
        StwoMetalBufferBox *dst_2 = stwo_metal_buffer_box(dst_2_ptr);
        StwoMetalBufferBox *dst_3 = stwo_metal_buffer_box(dst_3_ptr);
        StwoMetalBufferBox *inverse_x = stwo_metal_buffer_box(inverse_x_ptr);
        NSUInteger src_len = ((NSUInteger)1) << src_log_len;
        NSUInteger dst_len = src_len >> 1;
        if (
            src_0.len != src_len || src_1.len != src_len ||
            src_2.len != src_len || src_3.len != src_len ||
            dst_0.len != dst_len || dst_1.len != dst_len ||
            dst_2.len != dst_len || dst_3.len != dst_len ||
            inverse_x.len != dst_len
        ) {
            stwo_metal_write_error(error_message, error_message_len, @"FRI line-fold step expects four source coordinate buffers, four destination coordinate buffers, and one inverse-x factor per output element.");
            return false;
        }

        id<MTLComputePipelineState> pipeline = stwo_metal_pipeline(runtime, @"fri_fold_line_step_coords_u32x4", error_message, error_message_len);
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
        [encoder setBuffer:src_0.buffer offset:0 atIndex:0];
        [encoder setBuffer:src_1.buffer offset:0 atIndex:1];
        [encoder setBuffer:src_2.buffer offset:0 atIndex:2];
        [encoder setBuffer:src_3.buffer offset:0 atIndex:3];
        [encoder setBuffer:dst_0.buffer offset:0 atIndex:4];
        [encoder setBuffer:dst_1.buffer offset:0 atIndex:5];
        [encoder setBuffer:dst_2.buffer offset:0 atIndex:6];
        [encoder setBuffer:dst_3.buffer offset:0 atIndex:7];
        [encoder setBuffer:inverse_x.buffer offset:0 atIndex:8];
        [encoder setBytes:alpha_limbs length:sizeof(uint32_t) * 4 atIndex:9];

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

bool stwo_metal_eval_program_v1_reference_u32x4(
    void *runtime_ptr,
    void *trace_values_ptr,
    void *interaction_offsets_ptr,
    void *preprocessed_values_ptr,
    void *base_params_ptr,
    void *ext_params_ptr,
    void *random_coeff_powers_ptr,
    void *base_insts_ptr,
    void *ext_insts_ptr,
    void *constraint_roots_ptr,
    void *dst_ptr,
    uint32_t row_count,
    uint32_t n_interactions,
    uint32_t n_preprocessed_columns,
    uint32_t n_base_params,
    uint32_t n_ext_params,
    uint32_t n_base_insts,
    uint32_t n_ext_insts,
    uint32_t n_constraints,
    char *error_message,
    size_t error_message_len
) {
    @autoreleasepool {
        StwoMetalRuntimeBox *runtime = stwo_metal_runtime_box(runtime_ptr);
        StwoMetalBufferBox *trace_values = stwo_metal_buffer_box(trace_values_ptr);
        StwoMetalBufferBox *interaction_offsets = stwo_metal_buffer_box(interaction_offsets_ptr);
        StwoMetalBufferBox *preprocessed_values = stwo_metal_buffer_box(preprocessed_values_ptr);
        StwoMetalBufferBox *base_params = stwo_metal_buffer_box(base_params_ptr);
        StwoMetalBufferBox *ext_params = stwo_metal_buffer_box(ext_params_ptr);
        StwoMetalBufferBox *random_coeff_powers = stwo_metal_buffer_box(random_coeff_powers_ptr);
        StwoMetalBufferBox *base_insts = stwo_metal_buffer_box(base_insts_ptr);
        StwoMetalBufferBox *ext_insts = stwo_metal_buffer_box(ext_insts_ptr);
        StwoMetalBufferBox *constraint_roots = stwo_metal_buffer_box(constraint_roots_ptr);
        StwoMetalBufferBox *dst = stwo_metal_buffer_box(dst_ptr);

        BOOL preprocessed_ok =
            (n_preprocessed_columns == 0u) ||
            (preprocessed_values.len == (NSUInteger)n_preprocessed_columns * row_count);
        BOOL base_params_ok =
            (n_base_params == 0u) ||
            (base_params.len == (NSUInteger)n_base_params);
        BOOL ext_params_ok =
            (n_ext_params == 0u) ||
            (ext_params.len == (NSUInteger)n_ext_params * 4u);

        if (interaction_offsets.len != (NSUInteger)n_interactions + 1u ||
            !preprocessed_ok ||
            !base_params_ok ||
            !ext_params_ok ||
            random_coeff_powers.len != (NSUInteger)n_constraints * 4u ||
            base_insts.len != (NSUInteger)n_base_insts * 4u ||
            ext_insts.len != (NSUInteger)n_ext_insts * 5u ||
            constraint_roots.len != (NSUInteger)n_constraints ||
            dst.len != (NSUInteger)row_count * 4u) {
            stwo_metal_write_error(error_message, error_message_len, @"MetalEvaluationProgramV1 reference lane expects canonical packed buffers and lengths.");
            return false;
        }

        id<MTLComputePipelineState> pipeline =
            stwo_metal_pipeline(runtime, @"eval_program_v1_reference_u32x4", error_message, error_message_len);
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
        [encoder setBuffer:trace_values.buffer offset:0 atIndex:0];
        [encoder setBuffer:interaction_offsets.buffer offset:0 atIndex:1];
        [encoder setBuffer:preprocessed_values.buffer offset:0 atIndex:2];
        [encoder setBuffer:base_params.buffer offset:0 atIndex:3];
        [encoder setBuffer:ext_params.buffer offset:0 atIndex:4];
        [encoder setBuffer:random_coeff_powers.buffer offset:0 atIndex:5];
        [encoder setBuffer:base_insts.buffer offset:0 atIndex:6];
        [encoder setBuffer:ext_insts.buffer offset:0 atIndex:7];
        [encoder setBuffer:constraint_roots.buffer offset:0 atIndex:8];
        [encoder setBuffer:dst.buffer offset:0 atIndex:9];
        [encoder setBytes:&row_count length:sizeof(row_count) atIndex:10];
        [encoder setBytes:&n_base_insts length:sizeof(n_base_insts) atIndex:11];
        [encoder setBytes:&n_ext_insts length:sizeof(n_ext_insts) atIndex:12];
        [encoder setBytes:&n_constraints length:sizeof(n_constraints) atIndex:13];

        MTLSize grid_size = MTLSizeMake(row_count, 1, 1);
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

bool stwo_metal_eval_program_v1_wide_fibonacci_u32x4(
    void *runtime_ptr,
    void *trace_values_ptr,
    void *interaction_offsets_ptr,
    void *random_coeff_powers_ptr,
    void *dst_ptr,
    uint32_t row_count,
    uint32_t n_interactions,
    uint32_t n_constraints,
    char *error_message,
    size_t error_message_len
) {
    @autoreleasepool {
        StwoMetalRuntimeBox *runtime = stwo_metal_runtime_box(runtime_ptr);
        StwoMetalBufferBox *trace_values = stwo_metal_buffer_box(trace_values_ptr);
        StwoMetalBufferBox *interaction_offsets = stwo_metal_buffer_box(interaction_offsets_ptr);
        StwoMetalBufferBox *random_coeff_powers = stwo_metal_buffer_box(random_coeff_powers_ptr);
        StwoMetalBufferBox *dst = stwo_metal_buffer_box(dst_ptr);

        if (n_interactions < 2u ||
            interaction_offsets.len != (NSUInteger)n_interactions + 1u ||
            random_coeff_powers.len != (NSUInteger)n_constraints * 4u ||
            dst.len != (NSUInteger)row_count * 4u) {
            stwo_metal_write_error(error_message, error_message_len, @"Wide-fibonacci overlay expects canonical packed trace interaction offsets, randomness, and destination buffers.");
            return false;
        }

        id<MTLComputePipelineState> pipeline =
            stwo_metal_pipeline(runtime, @"eval_program_v1_wide_fibonacci_u32x4", error_message, error_message_len);
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
        [encoder setBuffer:trace_values.buffer offset:0 atIndex:0];
        [encoder setBuffer:interaction_offsets.buffer offset:0 atIndex:1];
        [encoder setBuffer:random_coeff_powers.buffer offset:0 atIndex:2];
        [encoder setBuffer:dst.buffer offset:0 atIndex:3];
        [encoder setBytes:&row_count length:sizeof(row_count) atIndex:4];
        [encoder setBytes:&n_constraints length:sizeof(n_constraints) atIndex:5];

        MTLSize grid_size = MTLSizeMake(row_count, 1, 1);
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

bool stwo_metal_sampled_values_v1_wide_fibonacci_u32x4(
    void *runtime_ptr,
    void *tree_descs_ptr,
    void *column_descs_ptr,
    void *values_ptr,
    void *point_x_ptr,
    void *dst_ptr,
    uint32_t n_trees,
    char *error_message,
    size_t error_message_len
) {
    @autoreleasepool {
        StwoMetalRuntimeBox *runtime = stwo_metal_runtime_box(runtime_ptr);
        StwoMetalBufferBox *tree_descs = stwo_metal_buffer_box(tree_descs_ptr);
        StwoMetalBufferBox *column_descs = stwo_metal_buffer_box(column_descs_ptr);
        StwoMetalBufferBox *values = stwo_metal_buffer_box(values_ptr);
        StwoMetalBufferBox *point_x = stwo_metal_buffer_box(point_x_ptr);
        StwoMetalBufferBox *dst = stwo_metal_buffer_box(dst_ptr);

        if (n_trees == 0u ||
            tree_descs.len != (NSUInteger)n_trees * 2u ||
            point_x.len != 4u ||
            dst.len != 4u) {
            stwo_metal_write_error(error_message, error_message_len, @"Sampled-values V1 wide-fibonacci lane expects canonical tree descriptors, one secure-field point coordinate, and a single secure-field destination.");
            return false;
        }

        id<MTLComputePipelineState> pipeline =
            stwo_metal_pipeline(runtime, @"sampled_values_v1_wide_fibonacci_u32x4", error_message, error_message_len);
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
        [encoder setBuffer:tree_descs.buffer offset:0 atIndex:0];
        [encoder setBuffer:column_descs.buffer offset:0 atIndex:1];
        [encoder setBuffer:values.buffer offset:0 atIndex:2];
        [encoder setBuffer:point_x.buffer offset:0 atIndex:3];
        [encoder setBuffer:dst.buffer offset:0 atIndex:4];
        [encoder setBytes:&n_trees length:sizeof(n_trees) atIndex:5];

        MTLSize grid_size = MTLSizeMake(1, 1, 1);
        MTLSize threadgroup_size = MTLSizeMake(1, 1, 1);
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

bool stwo_metal_accumulate_partial_numerators_u32x4(
    void *runtime_ptr,
    void *columns_ptr,
    void *column_indices_ptr,
    void *b_coeffs_ptr,
    void *c_coeffs_ptr,
    void *dst_ptr,
    uint32_t row_count,
    uint32_t n_terms,
    char *error_message,
    size_t error_message_len
) {
    @autoreleasepool {
        StwoMetalRuntimeBox *runtime = stwo_metal_runtime_box(runtime_ptr);
        StwoMetalBufferBox *columns = stwo_metal_buffer_box(columns_ptr);
        StwoMetalBufferBox *column_indices = stwo_metal_buffer_box(column_indices_ptr);
        StwoMetalBufferBox *b_coeffs = stwo_metal_buffer_box(b_coeffs_ptr);
        StwoMetalBufferBox *c_coeffs = stwo_metal_buffer_box(c_coeffs_ptr);
        StwoMetalBufferBox *dst = stwo_metal_buffer_box(dst_ptr);
        if (
            row_count == 0u ||
            columns.len % row_count != 0u ||
            column_indices.len != n_terms ||
            b_coeffs.len != ((NSUInteger)n_terms) * 4u ||
            c_coeffs.len != ((NSUInteger)n_terms) * 4u ||
            dst.len != ((NSUInteger)row_count) * 4u
        ) {
            stwo_metal_write_error(error_message, error_message_len, @"Partial numerator accumulation expects flattened base columns, one column index per term, one qm31 b/c coefficient per term, and a packed u32x4 destination.");
            return false;
        }

        id<MTLComputePipelineState> pipeline = stwo_metal_pipeline(runtime, @"accumulate_partial_numerators_u32x4", error_message, error_message_len);
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
        [encoder setBuffer:columns.buffer offset:0 atIndex:0];
        [encoder setBuffer:column_indices.buffer offset:0 atIndex:1];
        [encoder setBuffer:b_coeffs.buffer offset:0 atIndex:2];
        [encoder setBuffer:c_coeffs.buffer offset:0 atIndex:3];
        [encoder setBuffer:dst.buffer offset:0 atIndex:4];
        [encoder setBytes:&row_count length:sizeof(row_count) atIndex:5];
        [encoder setBytes:&n_terms length:sizeof(n_terms) atIndex:6];

        MTLSize grid_size = MTLSizeMake(row_count, 1, 1);
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

bool stwo_metal_accumulate_partial_numerators_batched_u32x4(
    void *runtime_ptr,
    void *columns_ptr,
    void *column_indices_ptr,
    void *b_coeffs_ptr,
    void *c_coeffs_ptr,
    void *term_offsets_ptr,
    void *term_counts_ptr,
    void *dst_ptr,
    uint32_t row_count,
    uint32_t n_batches,
    char *error_message,
    size_t error_message_len
) {
    @autoreleasepool {
        StwoMetalRuntimeBox *runtime = stwo_metal_runtime_box(runtime_ptr);
        StwoMetalBufferBox *columns = stwo_metal_buffer_box(columns_ptr);
        StwoMetalBufferBox *column_indices = stwo_metal_buffer_box(column_indices_ptr);
        StwoMetalBufferBox *b_coeffs = stwo_metal_buffer_box(b_coeffs_ptr);
        StwoMetalBufferBox *c_coeffs = stwo_metal_buffer_box(c_coeffs_ptr);
        StwoMetalBufferBox *term_offsets = stwo_metal_buffer_box(term_offsets_ptr);
        StwoMetalBufferBox *term_counts = stwo_metal_buffer_box(term_counts_ptr);
        StwoMetalBufferBox *dst = stwo_metal_buffer_box(dst_ptr);

        if (row_count == 0u) {
            stwo_metal_write_error(error_message, error_message_len, @"Batched partial numerator accumulation requires a non-zero row count.");
            return false;
        }
        if (columns.len % row_count != 0u) {
            stwo_metal_write_error(error_message, error_message_len, @"Batched partial numerator accumulation expects flattened base columns with an integral number of rows.");
            return false;
        }
        if (term_offsets.len != n_batches) {
            stwo_metal_write_error(error_message, error_message_len, @"Batched partial numerator accumulation expects one term offset per batch.");
            return false;
        }
        if (term_counts.len != n_batches) {
            stwo_metal_write_error(error_message, error_message_len, @"Batched partial numerator accumulation expects one term count per batch.");
            return false;
        }
        if (column_indices.len * 4u != b_coeffs.len) {
            stwo_metal_write_error(error_message, error_message_len, @"Batched partial numerator accumulation expects one qm31 b coefficient per column index.");
            return false;
        }
        if (column_indices.len * 4u != c_coeffs.len) {
            stwo_metal_write_error(error_message, error_message_len, @"Batched partial numerator accumulation expects one qm31 c coefficient per column index.");
            return false;
        }
        if (dst.len != (NSUInteger)(row_count * n_batches * 4u)) {
            stwo_metal_write_error(error_message, error_message_len, @"Batched partial numerator accumulation expects one qm31 output per (batch, row).");
            return false;
        }

        id<MTLComputePipelineState> pipeline =
            stwo_metal_pipeline(runtime, @"accumulate_partial_numerators_batched_u32x4", error_message, error_message_len);
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
        [encoder setBuffer:columns.buffer offset:0 atIndex:0];
        [encoder setBuffer:column_indices.buffer offset:0 atIndex:1];
        [encoder setBuffer:b_coeffs.buffer offset:0 atIndex:2];
        [encoder setBuffer:c_coeffs.buffer offset:0 atIndex:3];
        [encoder setBuffer:term_offsets.buffer offset:0 atIndex:4];
        [encoder setBuffer:term_counts.buffer offset:0 atIndex:5];
        [encoder setBuffer:dst.buffer offset:0 atIndex:6];
        [encoder setBytes:&row_count length:sizeof(row_count) atIndex:7];
        [encoder setBytes:&n_batches length:sizeof(n_batches) atIndex:8];
        NSUInteger total_rows = (NSUInteger)row_count * (NSUInteger)n_batches;
        MTLSize grid_size = MTLSizeMake(total_rows, 1, 1);
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

bool stwo_metal_compute_quotients_and_combine_u32x4(
    void *runtime_ptr,
    void *partial_coord_0_ptr,
    void *partial_coord_1_ptr,
    void *partial_coord_2_ptr,
    void *partial_coord_3_ptr,
    void *sample_points_ptr,
    void *first_linear_terms_ptr,
    void *partial_log_sizes_ptr,
    void *partial_offsets_ptr,
    void *domain_x_ptr,
    void *domain_y_ptr,
    void *dst_ptr,
    uint32_t lifting_log_size,
    uint32_t n_accumulations,
    char *error_message,
    size_t error_message_len
) {
    @autoreleasepool {
        StwoMetalRuntimeBox *runtime = stwo_metal_runtime_box(runtime_ptr);
        StwoMetalBufferBox *partial_coord_0 = stwo_metal_buffer_box(partial_coord_0_ptr);
        StwoMetalBufferBox *partial_coord_1 = stwo_metal_buffer_box(partial_coord_1_ptr);
        StwoMetalBufferBox *partial_coord_2 = stwo_metal_buffer_box(partial_coord_2_ptr);
        StwoMetalBufferBox *partial_coord_3 = stwo_metal_buffer_box(partial_coord_3_ptr);
        StwoMetalBufferBox *sample_points = stwo_metal_buffer_box(sample_points_ptr);
        StwoMetalBufferBox *first_linear_terms = stwo_metal_buffer_box(first_linear_terms_ptr);
        StwoMetalBufferBox *partial_log_sizes = stwo_metal_buffer_box(partial_log_sizes_ptr);
        StwoMetalBufferBox *partial_offsets = stwo_metal_buffer_box(partial_offsets_ptr);
        StwoMetalBufferBox *domain_x = stwo_metal_buffer_box(domain_x_ptr);
        StwoMetalBufferBox *domain_y = stwo_metal_buffer_box(domain_y_ptr);
        StwoMetalBufferBox *dst = stwo_metal_buffer_box(dst_ptr);
        NSUInteger row_count = ((NSUInteger)1) << lifting_log_size;
        if (
            partial_coord_0.len != partial_coord_1.len ||
            partial_coord_0.len != partial_coord_2.len ||
            partial_coord_0.len != partial_coord_3.len ||
            sample_points.len != ((NSUInteger)n_accumulations) * 8u ||
            first_linear_terms.len != ((NSUInteger)n_accumulations) * 4u ||
            partial_log_sizes.len != n_accumulations ||
            partial_offsets.len != n_accumulations ||
            domain_x.len != row_count ||
            domain_y.len != row_count ||
            dst.len != row_count * 4u
        ) {
            stwo_metal_write_error(error_message, error_message_len, @"Quotient combination expects four flattened partial-numerator coordinate buffers, eight sample-point limbs per accumulation, one qm31 first-linear term per accumulation, one log-size and offset per accumulation, domain x/y buffers for the lifting domain, and a packed qm31 destination.");
            return false;
        }

        id<MTLComputePipelineState> pipeline = stwo_metal_pipeline(runtime, @"compute_quotients_and_combine_u32x4", error_message, error_message_len);
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
        [encoder setBuffer:partial_coord_0.buffer offset:0 atIndex:0];
        [encoder setBuffer:partial_coord_1.buffer offset:0 atIndex:1];
        [encoder setBuffer:partial_coord_2.buffer offset:0 atIndex:2];
        [encoder setBuffer:partial_coord_3.buffer offset:0 atIndex:3];
        [encoder setBuffer:sample_points.buffer offset:0 atIndex:4];
        [encoder setBuffer:first_linear_terms.buffer offset:0 atIndex:5];
        [encoder setBuffer:partial_log_sizes.buffer offset:0 atIndex:6];
        [encoder setBuffer:partial_offsets.buffer offset:0 atIndex:7];
        [encoder setBuffer:domain_x.buffer offset:0 atIndex:8];
        [encoder setBuffer:domain_y.buffer offset:0 atIndex:9];
        [encoder setBuffer:dst.buffer offset:0 atIndex:10];
        [encoder setBytes:&lifting_log_size length:sizeof(lifting_log_size) atIndex:11];
        [encoder setBytes:&n_accumulations length:sizeof(n_accumulations) atIndex:12];

        MTLSize grid_size = MTLSizeMake(row_count, 1, 1);
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

bool stwo_metal_compute_quotients_and_combine_packed_u32x4(
    void *runtime_ptr,
    void *partials_ptr,
    void *sample_points_ptr,
    void *first_linear_terms_ptr,
    void *partial_log_sizes_ptr,
    void *partial_offsets_ptr,
    void *domain_x_ptr,
    void *domain_y_ptr,
    void *dst_ptr,
    uint32_t lifting_log_size,
    uint32_t n_accumulations,
    char *error_message,
    size_t error_message_len
) {
    @autoreleasepool {
        StwoMetalRuntimeBox *runtime = stwo_metal_runtime_box(runtime_ptr);
        StwoMetalBufferBox *partials = stwo_metal_buffer_box(partials_ptr);
        StwoMetalBufferBox *sample_points = stwo_metal_buffer_box(sample_points_ptr);
        StwoMetalBufferBox *first_linear_terms = stwo_metal_buffer_box(first_linear_terms_ptr);
        StwoMetalBufferBox *partial_log_sizes = stwo_metal_buffer_box(partial_log_sizes_ptr);
        StwoMetalBufferBox *partial_offsets = stwo_metal_buffer_box(partial_offsets_ptr);
        StwoMetalBufferBox *domain_x = stwo_metal_buffer_box(domain_x_ptr);
        StwoMetalBufferBox *domain_y = stwo_metal_buffer_box(domain_y_ptr);
        StwoMetalBufferBox *dst = stwo_metal_buffer_box(dst_ptr);
        NSUInteger row_count = ((NSUInteger)1) << lifting_log_size;
        if (
            partials.len % 4u != 0u ||
            sample_points.len != ((NSUInteger)n_accumulations) * 8u ||
            first_linear_terms.len != ((NSUInteger)n_accumulations) * 4u ||
            partial_log_sizes.len != n_accumulations ||
            partial_offsets.len != n_accumulations ||
            domain_x.len != row_count ||
            domain_y.len != row_count ||
            dst.len != row_count * 4u
        ) {
            stwo_metal_write_error(error_message, error_message_len, @"Packed quotient combination expects one packed partial-numerator qm31 buffer, eight sample-point limbs per accumulation, one qm31 first-linear term per accumulation, one log-size and offset per accumulation, domain x/y buffers for the lifting domain, and a packed qm31 destination.");
            return false;
        }

        id<MTLComputePipelineState> pipeline = stwo_metal_pipeline(runtime, @"compute_quotients_and_combine_packed_u32x4", error_message, error_message_len);
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
        [encoder setBuffer:partials.buffer offset:0 atIndex:0];
        [encoder setBuffer:sample_points.buffer offset:0 atIndex:1];
        [encoder setBuffer:first_linear_terms.buffer offset:0 atIndex:2];
        [encoder setBuffer:partial_log_sizes.buffer offset:0 atIndex:3];
        [encoder setBuffer:partial_offsets.buffer offset:0 atIndex:4];
        [encoder setBuffer:domain_x.buffer offset:0 atIndex:5];
        [encoder setBuffer:domain_y.buffer offset:0 atIndex:6];
        [encoder setBuffer:dst.buffer offset:0 atIndex:7];
        [encoder setBytes:&lifting_log_size length:sizeof(lifting_log_size) atIndex:8];
        [encoder setBytes:&n_accumulations length:sizeof(n_accumulations) atIndex:9];

        MTLSize grid_size = MTLSizeMake(row_count, 1, 1);
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

bool stwo_metal_blake2s_build_leaves_lifted_u32(
    void *runtime_ptr,
    void *flat_columns_ptr,
    void *column_offsets_ptr,
    void *column_log_sizes_ptr,
    void *dst_ptr,
    uint32_t n_columns,
    uint32_t lifting_log_size,
    char *error_message,
    size_t error_message_len
) {
    @autoreleasepool {
        StwoMetalRuntimeBox *runtime = stwo_metal_runtime_box(runtime_ptr);
        StwoMetalBufferBox *flat_columns = stwo_metal_buffer_box(flat_columns_ptr);
        StwoMetalBufferBox *column_offsets = stwo_metal_buffer_box(column_offsets_ptr);
        StwoMetalBufferBox *column_log_sizes = stwo_metal_buffer_box(column_log_sizes_ptr);
        StwoMetalBufferBox *dst = stwo_metal_buffer_box(dst_ptr);
        NSUInteger row_count = ((NSUInteger)1) << lifting_log_size;
        if (
            column_offsets.len != n_columns ||
            column_log_sizes.len != n_columns ||
            dst.len != row_count * 8u
        ) {
            stwo_metal_write_error(error_message, error_message_len, @"Lifted Blake2s leaf construction expects flattened base columns, one offset and one log-size per column, and a packed eight-word destination per lifted row.");
            return false;
        }

        id<MTLComputePipelineState> pipeline = stwo_metal_pipeline(runtime, @"blake2s_build_leaves_lifted_u32", error_message, error_message_len);
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
        [encoder setBuffer:flat_columns.buffer offset:0 atIndex:0];
        [encoder setBuffer:column_offsets.buffer offset:0 atIndex:1];
        [encoder setBuffer:column_log_sizes.buffer offset:0 atIndex:2];
        [encoder setBuffer:dst.buffer offset:0 atIndex:3];
        [encoder setBytes:&n_columns length:sizeof(n_columns) atIndex:4];
        [encoder setBytes:&lifting_log_size length:sizeof(lifting_log_size) atIndex:5];

        MTLSize grid_size = MTLSizeMake(row_count, 1, 1);
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

bool stwo_metal_blake2s_build_leaves_lifted_wide_chunk_u32(
    void *runtime_ptr,
    void *const *column_buffers_ptr,
    void *state_ptr,
    void *dst_ptr,
    const uint32_t *column_log_sizes,
    uint32_t n_columns,
    uint32_t lifting_log_size,
    uint32_t processed_bytes_before,
    uint32_t is_first_chunk,
    uint32_t is_final_chunk,
    char *error_message,
    size_t error_message_len
) {
    @autoreleasepool {
        if (n_columns == 0 || n_columns > 16u) {
            stwo_metal_write_error(error_message, error_message_len, @"Wide lifted Blake2s chunking requires between one and sixteen source columns per dispatch.");
            return false;
        }
        if (column_buffers_ptr == NULL || column_log_sizes == NULL) {
            stwo_metal_write_error(error_message, error_message_len, @"Wide lifted Blake2s chunking requires source-column buffers and per-column log sizes.");
            return false;
        }

        StwoMetalRuntimeBox *runtime = stwo_metal_runtime_box(runtime_ptr);
        StwoMetalBufferBox *state = stwo_metal_buffer_box(state_ptr);
        StwoMetalBufferBox *dst = stwo_metal_buffer_box(dst_ptr);
        NSUInteger row_count = ((NSUInteger)1) << lifting_log_size;
        if (state.len != row_count * 8u || dst.len != row_count * 8u) {
            stwo_metal_write_error(error_message, error_message_len, @"Wide lifted Blake2s chunking expects state and destination buffers with eight words per lifted row.");
            return false;
        }

        StwoMetalBufferBox *column_boxes[16] = { nil };
        for (uint32_t i = 0; i < n_columns; ++i) {
            if (column_buffers_ptr[i] == NULL) {
                stwo_metal_write_error(error_message, error_message_len, @"Wide lifted Blake2s chunking received a null source column buffer.");
                return false;
            }
            column_boxes[i] = stwo_metal_buffer_box(column_buffers_ptr[i]);
            NSUInteger expected_len = ((NSUInteger)1) << column_log_sizes[i];
            if (column_boxes[i].len != expected_len || column_log_sizes[i] > lifting_log_size) {
                stwo_metal_write_error(error_message, error_message_len, @"Wide lifted Blake2s chunking requires each source column length to match its log size and not exceed the lifting size.");
                return false;
            }
        }

        id<MTLComputePipelineState> pipeline = stwo_metal_pipeline(runtime, @"blake2s_build_leaves_lifted_wide_chunk_u32", error_message, error_message_len);
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
        for (uint32_t i = 0; i < 16u; ++i) {
            id<MTLBuffer> buffer = i < n_columns ? column_boxes[i].buffer : nil;
            [encoder setBuffer:buffer offset:0 atIndex:i];
        }
        [encoder setBuffer:state.buffer offset:0 atIndex:16];
        [encoder setBuffer:dst.buffer offset:0 atIndex:17];
        [encoder setBytes:column_log_sizes length:n_columns * sizeof(uint32_t) atIndex:18];
        [encoder setBytes:&n_columns length:sizeof(n_columns) atIndex:19];
        [encoder setBytes:&lifting_log_size length:sizeof(lifting_log_size) atIndex:20];
        [encoder setBytes:&processed_bytes_before length:sizeof(processed_bytes_before) atIndex:21];
        [encoder setBytes:&is_first_chunk length:sizeof(is_first_chunk) atIndex:22];
        [encoder setBytes:&is_final_chunk length:sizeof(is_final_chunk) atIndex:23];

        MTLSize grid_size = MTLSizeMake(row_count, 1, 1);
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

bool stwo_metal_blake2s_build_next_layer_u32(
    void *runtime_ptr,
    void *prev_layer_ptr,
    void *dst_ptr,
    uint32_t next_len,
    char *error_message,
    size_t error_message_len
) {
    @autoreleasepool {
        StwoMetalRuntimeBox *runtime = stwo_metal_runtime_box(runtime_ptr);
        StwoMetalBufferBox *prev_layer = stwo_metal_buffer_box(prev_layer_ptr);
        StwoMetalBufferBox *dst = stwo_metal_buffer_box(dst_ptr);
        if (prev_layer.len != ((NSUInteger)next_len) * 16u || dst.len != ((NSUInteger)next_len) * 8u) {
            stwo_metal_write_error(error_message, error_message_len, @"Blake2s next-layer hashing expects sixteen packed words per parent input row and eight packed words per output row.");
            return false;
        }

        id<MTLComputePipelineState> pipeline = stwo_metal_pipeline(runtime, @"blake2s_build_next_layer_u32", error_message, error_message_len);
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
        [encoder setBuffer:prev_layer.buffer offset:0 atIndex:0];
        [encoder setBuffer:dst.buffer offset:0 atIndex:1];
        [encoder setBytes:&next_len length:sizeof(next_len) atIndex:2];

        MTLSize grid_size = MTLSizeMake(next_len, 1, 1);
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

bool stwo_metal_blake2s_build_merkle_layers_u32(
    void *runtime_ptr,
    void *leaf_layer_ptr,
    void *const *layer_ptrs,
    uint32_t leaf_log_size,
    char *error_message,
    size_t error_message_len
) {
    @autoreleasepool {
        StwoMetalRuntimeBox *runtime = stwo_metal_runtime_box(runtime_ptr);
        StwoMetalBufferBox *src_layer = stwo_metal_buffer_box(leaf_layer_ptr);
        if (src_layer.len != (((NSUInteger)1u) << leaf_log_size) * 8u) {
            stwo_metal_write_error(error_message, error_message_len, @"Packed Blake2s leaf layers must contain eight words per leaf hash.");
            return false;
        }
        if (leaf_log_size > 0u && layer_ptrs == NULL) {
            stwo_metal_write_error(error_message, error_message_len, @"Merkle-layer output pointers must be present when upper layers are requested.");
            return false;
        }

        id<MTLComputePipelineState> pipeline =
            stwo_metal_pipeline(runtime, @"blake2s_build_next_layer_u32", error_message, error_message_len);
        if (pipeline == nil) {
            return false;
        }

        id<MTLCommandBuffer> command_buffer = [runtime.queue commandBuffer];
        if (command_buffer == nil) {
            stwo_metal_write_error(error_message, error_message_len, @"Failed to create Metal command buffer.");
            return false;
        }

        uint32_t next_len = ((uint32_t)1u) << leaf_log_size;
        for (uint32_t layer_index = 0u; layer_index < leaf_log_size; ++layer_index) {
            next_len >>= 1u;
            StwoMetalBufferBox *dst_layer = stwo_metal_buffer_box(layer_ptrs[layer_index]);
            if (dst_layer.len != ((NSUInteger)next_len) * 8u) {
                stwo_metal_write_error(error_message, error_message_len, @"Packed Blake2s parent layers must contain eight words per hash.");
                return false;
            }

            id<MTLComputeCommandEncoder> encoder = [command_buffer computeCommandEncoder];
            if (encoder == nil) {
                stwo_metal_write_error(error_message, error_message_len, @"Failed to create Metal compute encoder.");
                return false;
            }

            [encoder setComputePipelineState:pipeline];
            [encoder setBuffer:src_layer.buffer offset:0 atIndex:0];
            [encoder setBuffer:dst_layer.buffer offset:0 atIndex:1];
            [encoder setBytes:&next_len length:sizeof(next_len) atIndex:2];

            MTLSize grid_size = MTLSizeMake(next_len, 1, 1);
            MTLSize threadgroup_size = MTLSizeMake(stwo_metal_threads_per_group(pipeline), 1, 1);
            [encoder dispatchThreads:grid_size threadsPerThreadgroup:threadgroup_size];
            [encoder endEncoding];

            src_layer = dst_layer;
        }

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
