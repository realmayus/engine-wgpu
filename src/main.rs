// mod renderer;
mod renderer;
mod util;

fn main() {
    // renderer::example_renderer::render();
    let scenes = util::gltf::load_gltf("assets/models/DamagedHelmet.gltf");

    println!("Scenes: {:?}", scenes);
}
