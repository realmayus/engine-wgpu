use dotenv::dotenv;
use log::info;
use systems::io::clear_run_dir;

mod gui;
mod renderer_impl;

fn main() {
    dotenv().ok(); // load environment variables
    env_logger::init();
    info!("Starting up engine...");

    renderer_impl::start(vec!["assets/models/cube.glb"]);
    // example_renderer::start(vec!["assets/models/sponza/Sponza.gltf"]);
    // renderer_impl::start(vec!["assets/models/sphere.glb"]);
}
