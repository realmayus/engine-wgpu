use std::cell::RefCell;
use std::fs;
use std::rc::Rc;

use vulkano::command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer};
use vulkano::memory::allocator::StandardMemoryAllocator;

use lib::scene::Material;
use lib::scene_serde::SceneSerde;

// Loads scenes from a scenes.json file
fn load_scenes(
    path: &str,
    _allocator: &StandardMemoryAllocator,
    _cmd_buf_builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
    _default_material: Rc<RefCell<Material>>,
    _tex_i: &mut u32,
    _mat_i: &mut u32,
) {
    let data = fs::read(path).expect("Couldn't read world.json");
    let _scenes: Vec<SceneSerde> =
        serde_json::from_slice(data.as_slice()).expect("Couldn't parse json file");
}
