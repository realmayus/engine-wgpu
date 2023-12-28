use std::sync::Arc;
use std::fs;
use std::path::Path;

use vulkano::command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer};
use vulkano::memory::allocator::StandardMemoryAllocator;

use lib::scene::World;
use lib::scene_serde::WorldSerde;

// Loads scenes from a scenes.json file
pub fn load_world(
    path: &Path,
    allocator: Arc<StandardMemoryAllocator>,
    cmd_buf_builder:  &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
) -> World {
    let data = fs::read(path).expect("Couldn't read world");
    let mut serde_world: WorldSerde =
        serde_json::from_slice(data.as_slice()).expect("Couldn't parse json file");
    serde_world.parse(allocator, cmd_buf_builder, path.parent().unwrap())
}
