use std::fs;
use std::path::PathBuf;

use gltf;
use image::{DynamicImage, ImageFormat};
use log::debug;
use rand::distributions::{Alphanumeric, DistString};

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

pub fn extract_image_to_file(name: &str, img: &DynamicImage, file_format: ImageFormat) -> PathBuf {
    debug!("Extracting image '{:?}' into file", name);
    let mut path = if let Ok(cwd) = std::env::var("WORKING_DIR") {
        PathBuf::from(cwd).join("run").join("images")
    } else {
        PathBuf::from("run").join("images")
    };

    fs::create_dir_all(path.clone())
        .unwrap_or_else(|_| panic!("Couldn't create directories {}", path.to_str().unwrap()));

    path.push(name);
    path.set_extension(file_format.extensions_str()[0]);

    let path = {
        while path.is_file() {
            let file_stem = path.file_stem().unwrap();

            path.set_file_name(format!(
                "{}_{}",
                file_stem.to_str().unwrap(),
                Alphanumeric
                    .sample_string(&mut rand::thread_rng(), 4)
                    .as_str()
            ));
            path.set_extension(file_format.extensions_str()[0]);
        }
        path
    };
    img.save_with_format(path.as_path(), file_format)
        .expect("Couldn't save image at ");
    path.strip_prefix("run").unwrap().to_path_buf()
}
