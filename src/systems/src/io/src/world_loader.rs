use lib::scene::{Material, World};
use lib::scene_serde::{SceneSerde, WorldSerde};
use serde_json::from_value;
use std::cell::RefCell;
use std::fs;
use std::path::Path;
use std::rc::Rc;
use vulkano::command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer};
use vulkano::memory::allocator::StandardMemoryAllocator;

// Loads scenes from a scenes.json file
pub fn load_world(
    path: &Path,
    allocator: &StandardMemoryAllocator,
    cmd_buf_builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
) -> World {
    let data = fs::read(path).expect("Couldn't read world");
    let serde_world: WorldSerde =
        serde_json::from_slice(data.as_slice()).expect("Couldn't parse json file");
    serde_world.parse(allocator, cmd_buf_builder)
}
