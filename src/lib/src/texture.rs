use std::sync::Arc;

use log::debug;
use vulkano::command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer};
use vulkano::format;
use vulkano::image::view::ImageView;
use vulkano::image::{ImageDimensions, ImmutableImage, MipmapsCount};
use vulkano::memory::allocator::StandardMemoryAllocator;

pub fn create_texture(
    pixels: Vec<u8>,
    format: format::Format,
    width: u32,
    height: u32,
    allocator: &StandardMemoryAllocator,
    cmd_buf_builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
) -> Arc<ImageView<ImmutableImage>> {
    debug!("Creating texture with format: {:?}", format);
    let image = ImmutableImage::from_iter(
        allocator,
        pixels,
        ImageDimensions::Dim2d {
            width,
            height,
            array_layers: 1, // images can be arrays of layers
        },
        MipmapsCount::One,
        format,
        cmd_buf_builder,
    )
    .expect("Couldn't create image");

    ImageView::new_default(image).unwrap()
}
