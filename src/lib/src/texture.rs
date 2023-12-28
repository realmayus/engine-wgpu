use std::sync::Arc;

use log::debug;
use vulkano::buffer::{Buffer, BufferCreateInfo, BufferUsage, Subbuffer};
use vulkano::command_buffer::{AutoCommandBufferBuilder, CopyBufferToImageInfo, PrimaryAutoCommandBuffer};
use vulkano::{DeviceSize, format};
use vulkano::image::{Image, ImageCreateInfo, ImageType};
use vulkano::image::view::ImageView;
use vulkano::memory::allocator::{AllocationCreateInfo, MemoryTypeFilter, StandardMemoryAllocator};

pub fn create_texture(
    pixels: Vec<u8>,
    format: format::Format,
    width: u32,
    height: u32,
    allocator: Arc<StandardMemoryAllocator>,
    cmd_buf_builder:  &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
) -> Arc<ImageView> {
    assert_eq!(pixels.len(), (width * height * 4) as usize);
    debug!("Creating texture with format: {:?}", format);
    let extent = [width, height, 1];
    let image = Image::new(
        allocator.clone(),
        ImageCreateInfo {
            image_type: ImageType::Dim2d,
            format,
            extent,
            usage: vulkano::image::ImageUsage::TRANSFER_DST | vulkano::image::ImageUsage::SAMPLED,
            ..Default::default()
        },
        AllocationCreateInfo::default()
    )
    .expect("Couldn't create image");
    let upload_buf: Subbuffer<[u8]> = Buffer::from_iter(
        allocator,
        BufferCreateInfo {
            usage: BufferUsage::TRANSFER_SRC,
            ..Default::default()
        },
        AllocationCreateInfo {
            memory_type_filter: MemoryTypeFilter::PREFER_HOST | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
            ..Default::default()
        },
        pixels,
    ).unwrap();



    cmd_buf_builder.copy_buffer_to_image(CopyBufferToImageInfo::buffer_image(
        upload_buf,
        image.clone(),
    )).unwrap();
    ImageView::new_default(image).unwrap()
}
