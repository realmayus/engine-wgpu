extern crate core;

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use rand::distributions::DistString;

pub mod gltf_loader;
pub mod world_loader;
pub mod world_saver;

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
