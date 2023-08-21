use renderer;
mod example_renderer;
fn main() {
    example_renderer::start(vec!["assets/models/DamagedHelmet.gltf"]);
    // example_renderer::start(vec!["assets/models/sponza/Sponza.gltf"]);
    // example_renderer::render(vec!["assets/models/monke.gltf"]);
}
