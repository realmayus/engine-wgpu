use lib::scene::Material;
use std::cell::RefCell;
use std::fs;
use std::rc::Rc;
use serde_json::from_value;
use vulkano::command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer};
use vulkano::memory::allocator::StandardMemoryAllocator;
use lib::scene_serde::SceneSerde;

// Loads scenes from a scenes.json file
fn load_scenes(
    path: &str,
    allocator: &StandardMemoryAllocator,
    cmd_buf_builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
    default_material: Rc<RefCell<Material>>,
    tex_i: &mut u32,
    mat_i: &mut u32,
) {
    let data = fs::read(path).expect("Couldn't read world.json");
    let scenes: Vec<SceneSerde> = serde_json::from_slice(data.as_slice()).expect("Couldn't parse json file");
    
}
