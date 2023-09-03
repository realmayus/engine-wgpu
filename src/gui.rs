use crate::renderer_impl::GlobalState;
use egui_winit_vulkano::egui::Ui;
use egui_winit_vulkano::{egui, Gui};
use glam::Mat4;
use lib::scene::Model;
use lib::scene_serde::WorldSerde;
use lib::Dirtyable;
use log::info;
use renderer::PartialRenderState;
use systems::io;

pub(crate) fn render_gui(gui: &mut Gui, render_state: PartialRenderState, state: &mut GlobalState) {
    let ctx = gui.context();
    egui::Window::new("Scene").show(&ctx, |ui| {
        ui.with_layout(egui::Layout::left_to_right(egui::Align::default()), |ui| {
            if ui.button("Load world").clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("World files", &["json"])
                    .pick_file()
                {
                    state.opened_file = Some(path.clone());
                    // let mut cmd_buf_builder = AutoCommandBufferBuilder::primary(
                    //     render_state.cmd_buf_allocator,
                    //     render_state.queue_family_index,
                    //     CommandBufferUsage::OneTimeSubmit,
                    // )
                    // .unwrap();
                    //
                    // io::world_loader::load_world(
                    //     path.as_path(),
                    //     render_state.allocator,
                    //     &mut cmd_buf_builder,
                    // );
                    // let cmd_buf = cmd_buf_builder.build().unwrap();
                }
            }
            if ui.button("Save world as").clicked() {
                if let Some(path) = rfd::FileDialog::new().pick_folder() {
                    state.opened_file = Some(path);
                    io::world_saver::save(
                        state.opened_file.as_ref().unwrap().as_path(),
                        WorldSerde::from(&state.world),
                    )
                    .expect("Couldn't save world");
                }
            }
            ui.add_enabled_ui(state.opened_file.is_some(), |ui| {
                if ui.button("Save world").clicked() {
                    io::world_saver::save(
                        state.opened_file.as_ref().unwrap().as_path(),
                        WorldSerde::from(&state.world),
                    )
                    .expect("Couldn't save world");
                }
            });
        });
        if ui.button("Import glTF").clicked() {
            if let Some(paths) = rfd::FileDialog::new()
                .add_filter("glTF scenes", &["gltf", "glb"])
                .pick_files()
            {
                // let mut cmd_buf_builder = AutoCommandBufferBuilder::primary(
                //     render_state.cmd_buf_allocator,
                //     render_state.queue_family_index,
                //     CommandBufferUsage::OneTimeSubmit,
                // )
                // .unwrap();

                for path in paths {
                    // load_gltf(
                    //     path.as_path(),
                    //     render_state.allocator,
                    //     &mut cmd_buf_builder,
                    //     state.world.materials.get(&0).unwrap().clone(),
                    //     &mut state.world.highest_texture_index,
                    //     &mut state.world.highest_material_index,
                    // );
                }
            }
        }
        ui.label("Loaded models:");
        for scene in state.world.scenes.as_mut_slice() {
            ui.push_id(scene.id, |ui| {
                ui.collapsing(String::from(scene.name.clone().unwrap_or_default()), |ui| {
                    ui.label(format!("# of models: {}", scene.models.len()));
                    for model in scene.models.as_mut_slice() {
                        draw_model_collapsing(ui, model, Mat4::default());
                    }
                });
            });
        }
    });

    egui::Window::new("Camera").show(&ctx, |ui| {
        ui.label(format!("Eye: {}", &render_state.camera.eye));
        ui.label(format!("Target: {}", &render_state.camera.target));
        ui.label(format!("Up: {}", &render_state.camera.up));
        ui.add(egui::Slider::new(&mut render_state.camera.speed, 0.03..=0.3).text("Speed"));
        ui.add(egui::Slider::new(&mut render_state.camera.fovy, 0.0..=180.0).text("Field of view"));
        if ui.button("Reset").clicked() {
            render_state.camera.reset();
        }
    });

    egui::Window::new("Materials").show(&ctx, |ui| {
        for mat in state.world.materials.values() {
            let (id, name) = { (mat.borrow().id, mat.borrow().name.clone()) };
            ui.push_id(id, |ui| {
                ui.collapsing(String::from(name.unwrap_or_default()), |ui| {
                    if ui.button("Update").clicked() {
                        mat.clone().borrow_mut().set_dirty(true);
                    }
                    ui.label(format!("Base color factors: {}", mat.borrow().base_color));
                    ui.label(format!(
                        "Metallic roughness factors: {}",
                        mat.borrow().metallic_roughness_factors
                    ));
                    ui.label(format!(
                        "Emissive factors: {}",
                        mat.borrow().emissive_factors
                    ));
                    ui.label(format!(
                        "Occlusion strength: {}",
                        mat.borrow().occlusion_strength
                    ));
                    ui.separator();
                    ui.label(format!(
                        "Base color texture: {:?}",
                        mat.borrow().base_texture
                    ));
                    ui.label(format!("Normal texture: {:?}", mat.borrow().normal_texture));
                    ui.label(format!(
                        "Metallic roughness texture: {:?}",
                        mat.borrow().metallic_roughness_texture
                    ));
                    ui.label(format!(
                        "Emissive texture: {:?}",
                        mat.borrow().emissive_texture
                    ));
                    ui.label(format!(
                        "Occlusion texture: {:?}",
                        mat.borrow().occlusion_texture
                    ));
                });
            });
        }
    });

    egui::Window::new("Textures").show(&ctx, |ui| {
        for tex in state.world.textures.values() {
            ui.label(format!("Id: {}", tex.id));
            ui.label(format!(
                "Name: {}",
                String::from(tex.name.clone().unwrap_or_default())
            ));
        }
    });
}

fn draw_model_collapsing(ui: &mut Ui, model: &mut Model, parent_transform: Mat4) {
    ui.collapsing(String::from(model.name.clone().unwrap_or_default()), |ui| {
        ui.label("Translation:");
        if ui
            .add(egui::Slider::new(&mut model.local_transform.w_axis.x, -10.0..=10.0).text("X"))
            .changed()
        {
            model.update_transforms(parent_transform);
        }

        if ui
            .add(egui::Slider::new(&mut model.local_transform.w_axis.y, -10.0..=10.0).text("Y"))
            .changed()
        {
            model.update_transforms(parent_transform);
        }

        if ui
            .add(egui::Slider::new(&mut model.local_transform.w_axis.z, -10.0..=10.0).text("Z"))
            .changed()
        {
            model.update_transforms(parent_transform);
        }

        ui.label("Meshes:");
        for mesh in model.meshes.as_slice() {
            ui.push_id(mesh.id, |ui| {
                ui.collapsing("Mesh", |ui| {
                    ui.label(format!(
                        "# of vert/norm/in: {}/{}/{}",
                        mesh.vertices.len(),
                        mesh.normals.len(),
                        mesh.indices.len()
                    ));
                    ui.label(
                        format!(
                            "Material: {}",
                            mesh.material.borrow().name.clone().unwrap_or_default()
                        )
                        .as_str(),
                    );
                    if ui.button("Log material").clicked() {
                        info!("{:?}", mesh.material);
                    }
                })
            });
        }
        ui.separator();
        ui.label("Children:");
        for child in model.children.as_mut_slice() {
            draw_model_collapsing(ui, child, parent_transform * model.local_transform);
        }
    });
}
