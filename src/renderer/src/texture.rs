use image::{DynamicImage, GenericImageView};
use std::fmt::format;
use std::sync::Arc;
use vulkano::buffer::{Buffer, BufferCreateInfo, BufferUsage};
use vulkano::command_buffer::{
    AutoCommandBufferBuilder, CopyBufferToImageInfo, PrimaryAutoCommandBuffer,
};
use vulkano::device::Device;
use vulkano::format;
use vulkano::image::sys::Image;
use vulkano::image::view::{ImageView, ImageViewCreateInfo};
use vulkano::image::{
    ImageCreateFlags, ImageDimensions, ImageSubresourceRange, ImageUsage, ImageViewType,
    ImmutableImage, MipmapsCount, StorageImage,
};
use vulkano::memory::allocator::{AllocationCreateInfo, MemoryUsage, StandardMemoryAllocator};

pub fn create_texture(
    pixels: Vec<u8>,
    format: format::Format,
    width: u32,
    height: u32,
    allocator: &StandardMemoryAllocator,
    mut cmd_buf_builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
) -> Arc<ImageView<ImmutableImage>> {
    let image = ImmutableImage::from_iter(
        allocator,
        pixels,
        ImageDimensions::Dim2d {
            width,
            height,
            array_layers: 1, // images can be arrays of layers
        },
        MipmapsCount::One,
        format::Format::R8G8B8A8_SRGB,
        &mut cmd_buf_builder,
    )
    .expect("Couldn't create image");

    ImageView::new_default(image).unwrap()
}
