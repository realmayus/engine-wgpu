use image::GenericImageView;
use std::fmt::format;
use std::sync::Arc;
use vulkano::buffer::{Buffer, BufferCreateInfo, BufferUsage};
use vulkano::command_buffer::{
    AutoCommandBufferBuilder, CopyBufferToImageInfo, PrimaryAutoCommandBuffer,
};
use vulkano::device::Device;
use vulkano::format::Format;
use vulkano::image::view::{ImageView, ImageViewCreateInfo};
use vulkano::image::{
    ImageDimensions, ImageSubresourceRange, ImageViewType, ImmutableImage, StorageImage,
};
use vulkano::memory::allocator::{AllocationCreateInfo, MemoryUsage, StandardMemoryAllocator};

pub fn create_texture_image(
    allocator: &StandardMemoryAllocator,
    queue_family_index: u32,
    cmd_buf_builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
) -> Arc<StorageImage> {
    let (buf, width, height) = {
        let img = image::open("assets/textures/statue.jpg").expect("Couldn't load image");
        let width = img.width();
        let height = img.height();
        (
            Buffer::from_iter(
                allocator,
                BufferCreateInfo {
                    usage: BufferUsage::TRANSFER_SRC,
                    ..Default::default()
                },
                AllocationCreateInfo {
                    usage: MemoryUsage::Upload,
                    ..Default::default()
                },
                img.to_rgba8().into_raw(),
            )
            .expect("Couldn't allocate buffer"),
            width,
            height,
        )
    };

    let image = StorageImage::new(
        allocator,
        ImageDimensions::Dim2d {
            width,
            height,
            array_layers: 1, // images can be arrays of layers
        },
        Format::R8G8B8A8_UNORM,
        Some(queue_family_index),
    )
    .expect("Couldn't create image");

    cmd_buf_builder
        .copy_buffer_to_image(CopyBufferToImageInfo::buffer_image(buf, image.clone()))
        .unwrap();
    return image;
}
