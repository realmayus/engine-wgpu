use std::sync::mpsc;

use log::debug;

use engine::lib::scene::World;
use engine::renderer::{commands, Hook};
use engine::renderer::camera::{Camera, KeyState};
use engine::renderer::commands::{Command, CommandResult, Commands};
use engine::renderer::events::{Event, MouseButton};

use crate::util::RainbowAnimation;

mod gui;
mod util;

struct Game {
    event_receiver: Option<mpsc::Receiver<Event>>,
    command_sender: Option<Commands>,
    animation: RainbowAnimation,
}

impl Hook for Game {
    fn setup(&mut self, commands: Commands, event_receiver: mpsc::Receiver<Event>) {
        self.event_receiver = Some(event_receiver);
        self.command_sender = Some(commands);
    }

    fn update(&mut self, keys: &KeyState, delta_time: f32, world: &mut World) {
        self.animation.update(delta_time as u32);
        if let Some(scene) = world.scenes.get_mut(&world.active_scene) {
            scene.outline_color = self.animation.get_current_color();
        }
        while let Ok(event) = self.event_receiver.as_ref().unwrap().try_recv() {
            match event {
                Event::Click { x, y, mouse_button } => {
                    if mouse_button == MouseButton::Left {
                        self.command_sender
                            .clone()
                            .unwrap()
                            .send(Command::QueryClick((x, y)))
                            .unwrap();
                    }
                }
                Event::CommandResult(command_result) => {
                    debug!("Command result: {:?}", command_result);
                    match command_result {
                        CommandResult::ClickQuery(res) => {
                            let Some(scene) = world.scenes.get_mut(&world.active_scene) else {
                                return;
                            };
                            for model in scene.models.as_mut_slice() {
                                for mesh in model.meshes.as_mut_slice() {
                                    mesh.set_outline(false);
                                }
                            }
                            if res == 0 {
                                return;
                            }
                            self.animation.reset();
                            scene.get_mesh_mut(res).unwrap().set_outline(true);
                            debug!("Clicked on mesh: {}", res);
                        }
                    }
                }
            }
        }
    }

    fn update_ui(&mut self, ctx: &egui::Context, world: &mut World, camera: &mut Camera, commands: Commands) {
        gui::update_ui(ctx, world, camera, commands);
    }
}

fn main() {
    // enable logging
    env_logger::init();
    let game = Game {
        event_receiver: None,
        command_sender: None,
        animation: RainbowAnimation::new(),
    };
    pollster::block_on(engine::renderer::run(game));
}
