use image::{ImageBuffer, Rgba};
use vulkano::buffer::{Buffer, BufferCreateInfo, BufferUsage};
use vulkano::command_buffer::CopyImageToBufferInfo;
use vulkano::descriptor_set::allocator::StandardDescriptorSetAllocator;
use vulkano::descriptor_set::{PersistentDescriptorSet, WriteDescriptorSet};
use vulkano::format::Format;
use vulkano::image::view::ImageView;
use vulkano::image::{ImageDimensions, StorageImage};
use vulkano::memory::allocator::{AllocationCreateInfo, MemoryUsage};
use vulkano::pipeline::{ComputePipeline, Pipeline, PipelineBindPoint};
use vulkano::sync;
use vulkano::sync::GpuFuture;

use crate::playground::{init, init_cmd_buf};

pub fn image() {
    let (queue_family_index, device, queue, memory_allocator) = init(None);
    let (cmd_buf_alloc, mut cmd_buf_builder) = init_cmd_buf(device.clone(), queue.clone());

    mod cs {
        vulkano_shaders::shader! {
            ty: "compute",
            path: "src/playground/shaders/mandelbrot.glsl",
        }
    }

    let shader = cs::load(device.clone()).expect("failed to create shader module");

    let compute_pipeline = ComputePipeline::new(
        device.clone(),
        shader.entry_point("main").unwrap(),
        &(),
        None,
        |_| {},
    )
    .expect("Failed to create compute pipeline");

    let descriptor_set_allocator = StandardDescriptorSetAllocator::new(device.clone());

    // Images vs buffers: Memory layout of images is implementation detail and not known; not modifiable by CPU
    // the only way to read/write to an image is by asking the GPU to do it
    let image = StorageImage::new(
        &memory_allocator,
        ImageDimensions::Dim2d {
            width: 1024,
            height: 1024,
            array_layers: 1, // images can be arrays of layers
        },
        Format::R8G8B8A8_UNORM,
        Some(queue.queue_family_index()),
    )
    .unwrap();

    // To pass the image to the GPU shader, we need to create an ImageView of it
    let view = ImageView::new_default(image.clone()).unwrap();

    // Create descriptor set by adding te image view:
    let layout = compute_pipeline.layout().set_layouts().get(0).unwrap();
    let set = PersistentDescriptorSet::new(
        &descriptor_set_allocator,
        layout.clone(),
        [WriteDescriptorSet::image_view(0, view.clone())], // 0 is the binding
    )
    .unwrap();

    // As we have to ask the GPU to do *anything* with images, in order to read/export the image, we first have to create a buffer to which we copy the image.
    let buf = Buffer::from_iter(
        &memory_allocator,
        BufferCreateInfo {
            usage: BufferUsage::TRANSFER_DST,
            ..Default::default()
        },
        AllocationCreateInfo {
            usage: MemoryUsage::Download,
            ..Default::default()
        },
        (0..1024 * 1024 * 4).map(|_| 0u8),
    )
    .expect("Failed to create buffer");

    cmd_buf_builder
        .bind_pipeline_compute(compute_pipeline.clone())
        .bind_descriptor_sets(
            PipelineBindPoint::Compute,
            compute_pipeline.layout().clone(),
            0,
            set,
        )
        .dispatch([1024 / 8, 1024 / 8, 1])
        .unwrap()
        .copy_image_to_buffer(CopyImageToBufferInfo::image_buffer(
            image.clone(),
            buf.clone(),
        ))
        .unwrap();

    let cmd_buf = cmd_buf_builder.build().unwrap();

    let future = sync::now(device.clone())
        .then_execute(queue.clone(), cmd_buf)
        .unwrap()
        .then_signal_fence_and_flush()
        .unwrap();
    future.wait(None).unwrap();

    let buffer_content = buf.read().unwrap();
    let image = ImageBuffer::<Rgba<u8>, _>::from_raw(1024, 1024, &buffer_content[..]).unwrap();
    image.save("image.png").unwrap();
}
