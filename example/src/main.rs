use engine::lib::scene::World;
use engine::renderer::camera::KeyState;
use engine::renderer::{Hook, SetupData};

struct Game {}

impl Hook for Game {
    fn setup(&self, world: &mut World, data: SetupData) {
        data.load_default_scene(world);
    }

    fn update(&mut self, keys: &KeyState, delta_time: f32) {}
}

fn main() {
    let game = Game {};
    pollster::block_on(engine::renderer::run(game));
}
