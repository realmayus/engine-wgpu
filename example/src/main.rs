use glam::{Mat4, Vec3, Vec4, Vec4Swizzles};
use rfd::FileDialog;

use engine::lib::scene::World;
use engine::renderer::{commands, Hook};
use engine::renderer::camera::{Camera, KeyState};
use engine::renderer::commands::Commands;

struct Game {}

#[derive(PartialEq)]
enum CameraModes {
    Arcball,
    FPS,
}

trait Editable<T> {
    fn editable(&mut self, label: Option<String>, ui: &mut egui::Ui, min: T, max: T);
}

impl Editable<f32> for f32 {
    fn editable(&mut self, label: Option<String>, ui: &mut egui::Ui, min: f32, max: f32) {
        ui.horizontal(|ui| {
            if let Some(label) = label {
                ui.label(label);
            }
            ui.add(egui::DragValue::new(self).clamp_range(min..=max));
        });
    }
}

impl Editable<glam::Vec3> for glam::Vec3 {
    fn editable(&mut self, label: Option<String>, ui: &mut egui::Ui, min: Vec3, max: Vec3) {
        ui.horizontal(|ui| {
            if let Some(label) = label {
                ui.label(label);
            }
            ui.add(egui::DragValue::new(&mut self.x).clamp_range(min.x..=max.x));
            ui.add(egui::DragValue::new(&mut self.y).clamp_range(min.y..=max.y));
            ui.add(egui::DragValue::new(&mut self.z).clamp_range(min.z..=max.z));
        });
    }
}

impl Editable<glam::Vec4> for glam::Vec4 {
    fn editable(&mut self, label: Option<String>, ui: &mut egui::Ui, min: Vec4, max: Vec4) {
        ui.horizontal(|ui| {
            if let Some(label) = label {
                ui.label(label);
            }
            ui.add(egui::DragValue::new(&mut self.x).clamp_range(min.x..=max.x));
            ui.add(egui::DragValue::new(&mut self.y).clamp_range(min.y..=max.y));
            ui.add(egui::DragValue::new(&mut self.z).clamp_range(min.z..=max.z));
            ui.add(egui::DragValue::new(&mut self.w).clamp_range(min.w..=max.w));
        });
    }
}


/*
Given observe!(model.position, {let some = code;}, |model| {model.update_transforms()}), this should generate:
let before = model.position.clone();
{let some = code;}
if before != model.position {
    model.update_transforms();
}
 */
macro_rules! observe {
    ($field:expr, $code:block, |$model:ident| $update:block) => {
        let before = $field.clone();
        $code
        if before != $field {
            $update
        }
    };
}

impl Hook for Game {
    fn setup(&self, commands: Commands) {
        commands.send(commands::Command::LoadDefaultScene).unwrap();
    }

    fn update(&mut self, keys: &KeyState, delta_time: f32) {}

    fn update_ui(&mut self, ctx: &egui::Context, world: &mut World, camera: &mut Camera, commands: Commands) {
        egui::Window::new("World").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Load Scene").clicked() {
                    let picked_file = FileDialog::new().add_filter("GLTF files", &["glb", "gltf"]).pick_file();
                    if let Some(file) = picked_file {
                        commands.send(commands::Command::LoadSceneFile(file)).unwrap();
                    }
                }
                if ui.button("Import File").clicked() {
                    let picked_file = FileDialog::new().add_filter("GLTF files", &["glb", "gltf"]).pick_file();
                    if let Some(file) = picked_file {
                        commands.send(commands::Command::ImportFile(file)).unwrap();
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

            for scene in world.scenes.as_mut_slice().iter_mut() {
                egui::CollapsingHeader::new(format!("Scene {}", scene.name.clone().unwrap_or(format!("{}", scene.id).into()))).show(ui, |ui| {
                    ui.menu_button("Add Model", |ui| {
                        if ui.button("Cube").on_hover_text("Add a cube").clicked() {
                            println!("Adding cube");
                        }
                        if ui.button("Light").on_hover_text("Add a point light").clicked() {
                            println!("Adding light");
                            commands.send(commands::Command::CreateModel(commands::CreateModel::Light {
                                position: glam::Vec3::ZERO,
                                color: glam::Vec3::ONE,
                                intensity: 1.0,
                            })).unwrap();
                        }
                    });
                    for model in scene.models.as_mut_slice().iter_mut() {
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

                            observe!(model.local_transform, {
                                model.local_transform.w_axis.editable(Some("Position:".into()), ui, Vec4::from([-100.0, -100.0, -100.0, 1.0]), Vec4::from([100.0, 100.0, 100.0, 1.0]));
                            }, |model| {
                                model.update_transforms(Mat4::IDENTITY);
                            });

                            if let Some(light) = model.light.as_mut() {
                                egui::CollapsingHeader::new("Attached light").show(ui, |ui| {
                                    light.color.editable(Some("Color:".into()), ui, Vec3::from([0.0, 0.0, 0.0]), Vec3::from([1.0, 1.0, 1.0]));
                                    light.intensity.editable(Some("Intensity:".into()), ui, 0.0, 1000.0);
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

        egui::Window::new("Textures & Materials").show(ctx, |ui| {
            for (texid, texture) in world.textures.iter_with_ids() {
                egui::CollapsingHeader::new(format!("Texture {:?} {} {}", texid, texture.id.unwrap_or(999), texture.name.clone().unwrap_or("untitled".into()))).show(ui, |ui| {
                    ui.label(format!("Kind: {:?}", texture.kind));
                });
            }
            ui.separator();
            for (matid, material) in world.materials.iter_with_ids() {
                egui::CollapsingHeader::new(format!("Material {:?} {:?}", matid, material.name())).show(ui, |ui| {
                    match material {
                        engine::lib::Material::Pbr(pbr) => {
                            ui.label(format!("Name: {:?}", pbr.name));
                            ui.label(format!("Albedo: {:?}", pbr.albedo));
                            ui.label(format!("Metallic Roughness Factors: {:?}", pbr.metallic_roughness_factors));
                            ui.label(format!("Ambient Occlusion Factor: {:?}", pbr.occlusion_factor));
                            ui.label(format!("Emissive Factors: {:?}", pbr.emissive_factors));
                            ui.label(format!("Albedo Texture: {:?}", pbr.albedo_texture));
                            ui.label(format!("Normal Texture: {:?}", pbr.normal_texture));
                            ui.label(format!("Metallic Roughness Texture: {:?}", pbr.metallic_roughness_texture));
                            ui.label(format!("Ambient Occlusion Texture: {:?}", pbr.occlusion_texture));
                            ui.label(format!("Emissive Texture: {:?}", pbr.emissive_texture));
                        }
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
