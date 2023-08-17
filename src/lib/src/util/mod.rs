use gltf;

mod shader_types;
pub mod texture;

pub fn map_gltf_format_to_vulkano(format: gltf::image::Format) -> vulkano::format::Format {
    match format {
        gltf::image::Format::R8 => vulkano::format::Format::R8_UNORM,
        gltf::image::Format::R8G8 => vulkano::format::Format::R8G8_UNORM,
        gltf::image::Format::R8G8B8 => vulkano::format::Format::R8G8B8_UNORM,
        gltf::image::Format::R8G8B8A8 => vulkano::format::Format::R8G8B8A8_UNORM,
        gltf::image::Format::R16 => vulkano::format::Format::R16_UNORM,
        gltf::image::Format::R16G16 => vulkano::format::Format::R16G16_UNORM,
        gltf::image::Format::R16G16B16 => vulkano::format::Format::R16G16B16_UNORM,
        gltf::image::Format::R16G16B16A16 => vulkano::format::Format::R16G16B16A16_UNORM,
        gltf::image::Format::R32G32B32FLOAT => vulkano::format::Format::R32G32B32_SFLOAT,
        gltf::image::Format::R32G32B32A32FLOAT => vulkano::format::Format::R32G32B32A32_SFLOAT,
    }
}
