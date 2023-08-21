use std::sync::Arc;

use vulkano::command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer};
use vulkano::format;
use vulkano::image::view::ImageView;
use vulkano::image::{ImageDimensions, ImmutableImage, MipmapsCount};
use vulkano::memory::allocator::StandardMemoryAllocator;

use image::GenericImageView;

pub fn create_texture(
    pixels: Vec<u8>,
    format: format::Format,
    width: u32,
    height: u32,
    allocator: &StandardMemoryAllocator,
    mut cmd_buf_builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
) -> Arc<ImageView<ImmutableImage>> {
    println!("Format: {:?}", format);
    let image = ImmutableImage::from_iter(
        allocator,
        pixels,
        ImageDimensions::Dim2d {
            width,
            height,
            array_layers: 1, // images can be arrays of layers
        },
        MipmapsCount::One,
        format::Format::R8G8B8A8_UNORM,
        &mut cmd_buf_builder,
    )
    .expect("Couldn't create image");

    ImageView::new_default(image).unwrap()
}
