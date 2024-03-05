use std::sync::mpsc;
use rfd::FileDialog;
use engine::lib::scene::World;
use engine::renderer::camera::{Camera, KeyState};
use engine::renderer::{commands, Hook};
use engine::renderer::commands::Commands;

struct Game {}

#[derive(PartialEq)]
enum CameraModes {
    Arcball,
    FPS,
}

impl Hook for Game {
    fn setup(&self, commands: Commands) {
        commands.send(commands::Command::LoadDefaultScene).unwrap();
    }

    fn update(&mut self, keys: &KeyState, delta_time: f32) {}

    fn update_ui(&mut self, ctx: &egui::Context, world: &mut World, camera: &mut Camera, sender: mpsc::Sender<commands::Command>) {
        egui::Window::new("World").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Load Scene").clicked() {
                    sender.send(commands::Command::LoadSceneFile("scene.json".into())).unwrap();
                }
                if ui.button("Import File").clicked() {
                    let picked_file = FileDialog::new().add_filter("GLTF files", &["glb", "gltf"]).pick_file();
                    if let Some(file) = picked_file {
                        sender.send(commands::Command::ImportFile(file)).unwrap();
                    }
                }
            });

            egui::CollapsingHeader::new("Camera").show(ui, |ui| {
                if ui.button("Reset").clicked() {
                    camera.reset();
                }
                ui.horizontal(|ui| {
                    let mut mode = if camera.fps { CameraModes::FPS } else { CameraModes::Arcball };
                    ui.selectable_value(&mut mode, CameraModes::Arcball, "Arcball");
                    ui.selectable_value(&mut mode, CameraModes::FPS, "FPS");
                    camera.fps = mode == CameraModes::FPS;
                });
            });

            for scene in world.scenes.as_slice().iter() {
                egui::CollapsingHeader::new(format!("Scene {}", scene.name.clone().unwrap_or(format!("{}", scene.id).into()))).show(ui, |ui| {
                    for model in scene.models.as_slice().iter() {
                        egui::CollapsingHeader::new(format!("Model {}", model.name.clone().unwrap_or(format!("{}", model.id).into()))).show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(format!("ID: {}", model.id));
                                if ui.button("Print").clicked() {
                                    println!("Model name={:?}, id={}", model.name.clone(), model.id);
                                    println!("| Local transform:");
                                    println!("| {:#?}", model.local_transform);
                                    println!("| Attached light:");
                                    println!("| {:?}", model.light);
                                }
                            });

                            if let Some(ref light) = model.light {
                                egui::CollapsingHeader::new("Attached light").show(ui, |ui| {
                                    ui.label(format!("Color: {:?}", light.color));
                                    ui.label(format!("Intensity: {}", light.intensity));
                                    ui.label(format!("Range: {:?}", light.range));
                                });
                            }
                            for mesh in model.meshes.as_slice().iter() {
                                egui::CollapsingHeader::new(format!("Mesh {}", mesh.id)).show(ui, |ui| {
                                    ui.label(format!("Material: {:?}", mesh.material));
                                    ui.label(format!("Vertices: {}", mesh.vertices.len()));
                                    ui.label(format!("Indices: {}", mesh.indices.len()));

                                });
                            }
                        });
                    }
                });
            }
        });
    }
}

fn main() {
    // enable logging
    env_logger::init();
    let game = Game {};
    pollster::block_on(engine::renderer::run(game));
}
