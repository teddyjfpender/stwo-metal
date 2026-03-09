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

        id<MTLComputePipelineState> pipeline = stwo_metal_pipeline(runtime, @"bit_reverse_u32", error_message, error_message_len);
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
        [encoder setBytes:&log_len length:sizeof(log_len) atIndex:1];

        NSUInteger threadgroup_width = pipeline.threadExecutionWidth > 0 ? pipeline.threadExecutionWidth : 1;
        NSUInteger threads_per_group = MIN((NSUInteger)256, MAX(threadgroup_width, pipeline.maxTotalThreadsPerThreadgroup));
        MTLSize grid_size = MTLSizeMake(len, 1, 1);
        MTLSize threadgroup_size = MTLSizeMake(threads_per_group, 1, 1);
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
