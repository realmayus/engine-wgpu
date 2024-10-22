use egui::Ui;
use glam::{Mat4, Vec3, Vec4};
use rfd::FileDialog;
use engine::lib::Dirtyable;

use engine::lib::scene::model::Model;
use engine::lib::scene::World;
use engine::renderer::camera::Camera;
use engine::renderer::{commands, Meta};
use engine::renderer::commands::Commands;

use crate::util::{CameraModes, Editable, SparseModel, SparseScene};
use crate::{mutate_indirect, observe};

pub(crate) fn update_ui(ctx: &egui::Context, world: &mut World, camera: &mut Camera, commands: Commands, meta: &mut Meta) {
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
        ui.label(format!("Frame time: {:.2} ms", meta.frame_time * 1000.0));
        ui.label(format!("FPS: {:.0}", 1.0 / meta.frame_time));
        observe!(
            meta.vsync,
            {
                ui.checkbox(&mut meta.vsync, "VSync");
            },
            |meta| {
                commands.send(commands::Command::SetVsync).unwrap();
            }
        );
        ui.checkbox(&mut meta.show_grid, "Show Grid");
        egui::CollapsingHeader::new("Camera").show(ui, |ui| {
            if ui.button("Reset").clicked() {
                camera.reset();
            }
            ui.horizontal(|ui| {
                let mut mode = if camera.fps {
                    CameraModes::FPS
                } else {
                    CameraModes::Arcball
                };
                ui.selectable_value(&mut mode, CameraModes::Arcball, "Arcball");
                ui.selectable_value(&mut mode, CameraModes::FPS, "FPS");
                camera.fps = mode == CameraModes::FPS;
            });
        });

        if let Some(scene) = world.scenes.get_mut(&world.active_scene) {
            egui::CollapsingHeader::new("Outline").show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Width");
                    ui.add(egui::DragValue::new(&mut scene.outline_width));
                });
                ui.horizontal(|ui| {
                    ui.label("Color");
                    ui.add(egui::DragValue::new(&mut scene.outline_color[0]));
                    ui.add(egui::DragValue::new(&mut scene.outline_color[1]));
                    ui.add(egui::DragValue::new(&mut scene.outline_color[2]));
                });
            });
        }

        let sparse_scenes: Vec<SparseScene> = world
            .scenes
            .iter()
            .map(|(id, scene)| SparseScene {
                id: *id as u32,
                name: scene.name.clone(),
            })
            .collect();

        let sparse_models: Vec<SparseModel> = world
            .scenes
            .iter()
            .flat_map(|(_, scene)| {
                scene.models.iter().map(move |model| SparseModel {
                    id: model.id,
                    name: model.name.clone(),
                })
            })
            .collect();

        for (_, scene) in world.scenes.iter_mut() {
            egui::CollapsingHeader::new(format!(
                "Scene {}",
                scene.name.clone().unwrap_or(format!("{}", scene.id).into())
            ))
            .show(ui, |ui| {
                add_model_menu(ui, &commands, None);
                for model in scene.models.as_mut_slice().iter_mut() {
                    draw_model_ui(model, scene.id, &sparse_scenes, &sparse_models, ui, &commands);
                }
            });
        }
    });

    egui::Window::new("Textures & Materials").default_open(false).show(ctx, |ui| {
        for (texid, texture) in world.textures.iter_with_ids() {
            egui::CollapsingHeader::new(format!(
                "Texture {:?} {} {}",
                texid,
                texture.id.unwrap_or(999),
                texture.name.clone().unwrap_or("untitled".into())
            ))
            .show(ui, |ui| {
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
                        ui.label(format!(
                            "Metallic Roughness Factors: {:?}",
                            pbr.metallic_roughness_factors
                        ));
                        ui.label(format!("Ambient Occlusion Factor: {:?}", pbr.occlusion_factor));
                        ui.label(format!("Emissive Factors: {:?}", pbr.emissive_factors));
                        ui.label(format!("Albedo Texture: {:?}", pbr.albedo_texture));
                        ui.label(format!("Normal Texture: {:?}", pbr.normal_texture));
                        ui.label(format!(
                            "Metallic Roughness Texture: {:?}",
                            pbr.metallic_roughness_texture
                        ));
                        ui.label(format!("Ambient Occlusion Texture: {:?}", pbr.occlusion_texture));
                        ui.label(format!("Emissive Texture: {:?}", pbr.emissive_texture));
                    }
                }
            });
        }
    });
}

fn draw_model_ui(
    model: &mut Model,
    scene_id: u32,
    sparse_scenes: &Vec<SparseScene>,
    sparse_models: &Vec<SparseModel>,
    ui: &mut Ui,
    commands: &Commands,
) {
    egui::CollapsingHeader::new(format!(
        "Model {}",
        model.name.clone().unwrap_or(format!("{}", model.id).into())
    ))
    .show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(format!("ID: {}", model.id));
            model_actions(model, scene_id, sparse_scenes, sparse_models, &commands, ui);
        });

        observe!(
            model.local_transform,
            {
                model.local_transform.w_axis.editable(
                    Some("Position:".into()),
                    ui,
                    Vec4::from([-100.0, -100.0, -100.0, 1.0]),
                    Vec4::from([100.0, 100.0, 100.0, 1.0]),
                );
            },
            |model| {
                let mut mat = Mat4::IDENTITY;
                mat.y_axis *= -1.0;
                model.update_transforms(mat);
            }
        );

        observe!(
            model.scale,
            {
                model.scale.editable(
                    Some("Scale:".into()),
                    ui,
                    Vec3::from([0.0, 0.0, 0.0]),
                    Vec3::from([100.0, 100.0, 100.0]),
                );
            },
            |model| {
                let mut mat = Mat4::IDENTITY;
                mat.y_axis *= -1.0;
                model.update_transforms(mat);
            }
        );

        if let Some(light) = model.light.as_mut() {
            egui::CollapsingHeader::new("Attached light").show(ui, |ui| {
                observe!(
                    light.color,
                    {
                        light.color.editable(
                            Some("Color:".into()),
                            ui,
                            Vec3::from([0.0, 0.0, 0.0]),
                            Vec3::from([1.0, 1.0, 1.0]),
                        );
                    },
                    |light| {
                        light.set_dirty(true);
                    }
                );
                observe!(
                    light.intensity,
                    {
                        light.intensity.editable(Some("Intensity:".into()), ui, 0.0, 1000.0);
                    },
                    |light| {
                        light.set_dirty(true);
                    }
                );
                ui.label(format!("Range: {:?}", light.range));
            });
        }
        for mesh in model.meshes.as_mut_slice().iter_mut() {
            egui::CollapsingHeader::new(format!("Mesh {}", mesh.id)).show(ui, |ui| {
                mutate_indirect!(
                    mesh.is_outline(),
                    |outline| {
                        ui.checkbox(&mut outline, "Outline");
                    },
                    |mesh, outline| {
                        mesh.set_outline(outline);
                    }
                );
                ui.label(format!("Material: {:?}", mesh.material));
                ui.label(format!("Vertices: {}", mesh.vertices.len()));
                ui.label(format!("Indices: {}", mesh.indices.len()));
            });
        }
        ui.separator();
        for child in model.children.as_mut_slice().iter_mut() {
            draw_model_ui(child, scene_id, sparse_scenes, sparse_models, ui, commands);
        }
    });
}

fn model_actions(
    model: &mut Model,
    scene_id: u32,
    sparse_scenes: &[SparseScene],
    sparse_models: &[SparseModel],
    commands: &&Commands,
    ui: &mut Ui,
) {
    ui.menu_button("Actions", |ui| {
        add_model_menu(ui, commands, Some(model.id));
        add_mesh_menu(ui, commands, model.id);
        ui.menu_button("Rename", |ui| {
            let text = &*model.name.clone().unwrap_or("".into());
            let mut text = text.to_string();
            ui.add(egui::TextEdit::singleline(&mut text));
            model.name = if text.is_empty() {
                None
            } else {
                Some(text.into_boxed_str())
            };
        });
        ui.menu_button("Change parent", |ui| {
            for other_scene in sparse_scenes.iter() {
                if ui
                    .button(
                        format!(
                            "Root (Scene {}, {})",
                            other_scene.name.clone().map(|s| s.to_string()).unwrap_or("".into()),
                            other_scene.id
                        )
                        .as_str(),
                    )
                    .clicked()
                {
                    commands
                        .send(commands::Command::ChangeModelParent {
                            model_id: model.id,
                            new_parent_id: None,
                            new_scene_id: other_scene.id,
                        })
                        .unwrap();
                }
            }
            ui.separator();
            for other_model in sparse_models.iter() {
                if other_model.id == model.id {
                    continue;
                }
                if ui
                    .button(
                        format!(
                            "Model {}, {}",
                            other_model.name.clone().map(|s| s.to_string()).unwrap_or("".into()),
                            other_model.id
                        )
                        .as_str(),
                    )
                    .clicked()
                {
                    commands
                        .send(commands::Command::ChangeModelParent {
                            model_id: model.id,
                            new_parent_id: Some(other_model.id),
                            new_scene_id: scene_id,
                        })
                        .unwrap();
                }
            }
        });
        if ui.button("Delete").on_hover_text("Delete this model").clicked() {
            commands.send(commands::Command::DeleteModel(model.id)).unwrap();
        }
        if ui.button("Duplicate").on_hover_text("Duplicate this model").clicked() {
            commands.send(commands::Command::DuplicateModel(model.id)).unwrap();
        }
        if ui.button("Print debug info").clicked() {
            println!("Model name={:?}, id={}", model.name.clone(), model.id);
            println!("| Local transform:");
            println!("| {:#?}", model.local_transform);
            println!("| Attached light:");
            println!("| {:?}", model.light);
        }
    });
}

fn add_model_menu(ui: &mut Ui, commands: &Commands, parent_id: Option<u32>) {
    ui.menu_button(
        if parent_id.is_some() {
            "Add child model"
        } else {
            "Add model"
        },
        |ui| {
            if ui.button("Cube model").on_hover_text("Add a cube model").clicked() {
                println!("Adding cube TODO");
                ui.close_menu();
            }
            if ui
                .button("Light model")
                .on_hover_text("Add a point light model")
                .clicked()
            {
                println!("Adding light");
                ui.close_menu();
                commands
                    .send(commands::Command::CreateModel(
                        commands::CreateModel::Light {
                            position: glam::Vec3::ZERO,
                            color: glam::Vec3::ONE,
                            intensity: 1.0,
                        },
                        parent_id,
                    ))
                    .unwrap();
            }
        },
    );
}

fn add_mesh_menu(ui: &mut Ui, commands: &Commands, model_id: u32) {
    ui.menu_button("Add mesh", |ui| {
        if ui.button("Cube mesh").on_hover_text("Add a cube mesh").clicked() {
            println!("Adding cube mesh TODO");
            ui.close_menu();
        }
    });
}
