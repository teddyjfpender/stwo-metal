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
