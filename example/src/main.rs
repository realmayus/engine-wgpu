mod gui;
mod util;

use engine::lib::scene::World;
use engine::renderer::camera::{Camera, KeyState};
use engine::renderer::commands::Commands;
use engine::renderer::Hook;

struct Game {}

impl Hook for Game {
    fn setup(&self, commands: Commands) {
        //commands.send(commands::Command::LoadDefaultScene).unwrap();
    }

    fn update(&mut self, keys: &KeyState, delta_time: f32) {}

    fn update_ui(
        &mut self,
        ctx: &egui::Context,
        world: &mut World,
        camera: &mut Camera,
        commands: Commands,
    ) {
        gui::update_ui(ctx, world, camera, commands);
    }
}

fn main() {
    // enable logging
    env_logger::init();
    let game = Game {};
    pollster::block_on(engine::renderer::run(game));
}
