use dotenv::dotenv;
use log::info;

mod gui;
mod renderer_impl;

fn main() {
    dotenv().ok(); // load environment variables
    env_logger::init();
    info!("Starting up engine...");

    // renderer_impl::start(vec!["assets/models/sponza/Sponza.gltf"]);
    renderer_impl::start(vec!["assets/models/test_scene.gltf"]);
}
