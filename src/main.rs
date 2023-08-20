use renderer;
mod example_renderer;
fn main() {
    example_renderer::render(vec!["assets/models/helmet_light.gltf"]);
    // example_renderer::render(vec!["assets/models/monke.gltf"]);
}
