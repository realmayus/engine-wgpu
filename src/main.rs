use dotenv::dotenv;
use log::info;

mod renderer_impl;
fn main() {
    dotenv().ok(); // load environment variables
    env_logger::init();
    info!("Starting up engine...");

    //renderer_impl::start(vec!["assets/models/DamagedHelmet.gltf"]);
    // example_renderer::start(vec!["assets/models/sponza/Sponza.gltf"]);
    renderer_impl::start(vec!["assets/models/helmet_light.gltf"]);
    // example_renderer::render(vec!["assets/models/monke.gltf"]);
}
