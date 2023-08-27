extern crate core;

use image::{DynamicImage, ImageFormat};
use lib::scene::Material;
use log::debug;
use rand::distributions::{Alphanumeric, DistString};
use std::cell::RefCell;
use std::error::Error;
use std::fs;
use std::ops::Add;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use vulkano::command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer};
use vulkano::memory::allocator::StandardMemoryAllocator;

pub mod gltf_loader;
pub mod world_loader;
pub mod world_saver;

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

/**
Copies a subdirectory of run/ to the specified location. Does not recursively copy directories.
 */
pub fn copy_run_subdir_to_path(copy_from: &Path, copy_to: &Path) -> Result<(), Box<dyn Error>> {
    let mut path = if let Ok(cwd) = std::env::var("WORKING_DIR") {
        PathBuf::from(cwd).join("run")
    } else {
        PathBuf::from("run")
    };

    path.push(copy_from);
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        fs::copy(entry.path(), copy_to.join(entry.file_name()))?;
    }
    Ok(())
}

pub fn clear_run_dir() {
    let path = if let Ok(cwd) = std::env::var("WORKING_DIR") {
        cwd + "run"
    } else {
        String::from("run")
    };
    fs::remove_dir_all(path).expect("Couldn't clear run dir");
}
