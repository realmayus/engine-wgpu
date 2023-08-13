use image::GenericImageView;
use std::fmt::format;
use std::sync::Arc;
use vulkano::buffer::{Buffer, BufferCreateInfo, BufferUsage};
use vulkano::command_buffer::{
    AutoCommandBufferBuilder, CopyBufferToImageInfo, PrimaryAutoCommandBuffer,
};
use vulkano::device::Device;
use vulkano::format::Format;
use vulkano::image::sys::Image;
use vulkano::image::view::{ImageView, ImageViewCreateInfo};
use vulkano::image::{
    ImageCreateFlags, ImageDimensions, ImageSubresourceRange, ImageUsage, ImageViewType,
    ImmutableImage, MipmapsCount, StorageImage,
};
use vulkano::memory::allocator::{AllocationCreateInfo, MemoryUsage, StandardMemoryAllocator};

pub fn create_texture(
    allocator: &StandardMemoryAllocator,
    mut cmd_buf_builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
) -> Arc<ImageView<ImmutableImage>> {
    let img = image::open("assets/textures/statue.jpg").expect("Couldn't load image");
    let width = img.width();
    let height = img.height();

    let image = ImmutableImage::from_iter(
        allocator,
        img.to_rgba8().into_raw(),
        ImageDimensions::Dim2d {
            width,
            height,
            array_layers: 1, // images can be arrays of layers
        },
        MipmapsCount::One,
        Format::R8G8B8A8_SRGB,
        &mut cmd_buf_builder,
    )
    .expect("Couldn't create image");

    ImageView::new_default(image).unwrap()
}
