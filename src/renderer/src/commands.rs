use std::path::{Path, PathBuf};
use std::sync::mpsc;

use glam::Mat4;
use hashbrown::HashMap;
use log::{debug, error, info};

use lib::managers::{MaterialManager, TextureManager};
use lib::scene::light::PointLight;
use lib::scene::model::Model;
use lib::scene::World;
use systems::io::gltf_loader::load_gltf;

use crate::events::Event;
use crate::RenderState;

#[derive(Debug)]
pub enum CommandResult {
    ClickQuery(u32),
}

#[derive(Debug)]
pub enum CreateModel {
    Light {
        position: glam::Vec3,
        color: glam::Vec3,
        intensity: f32,
    },
}

pub type Commands = mpsc::Sender<Command>;

#[derive(Debug)]
pub enum Command {
    LoadSceneFile(PathBuf),
    ImportFile(PathBuf),
    CreateModel(CreateModel, Option<u32>),
    ChangeModelParent {
        model_id: u32,
        new_parent_id: Option<u32>,
        new_scene_id: u32,
    },
    DeleteModel(u32),
    DuplicateModel(u32),
    QueryClick((u32, u32)),
}

impl Command {
    pub(crate) fn process(self, state: &mut RenderState, event_sender: mpsc::Sender<Event>) {
        debug!("Processing command: {:?}", self);
        match self {
            Command::LoadSceneFile(path) => {
                let textures = TextureManager::new(&state.device, &state.queue);
                let materials = MaterialManager::new(
                    &state.device,
                    &state.queue,
                    &state.pbr_pipeline.mat_bind_group_layout,
                    &state.pbr_pipeline.tex_bind_group_layout,
                    &textures,
                );
                state.world = World {
                    scenes: HashMap::new(),
                    active_scene: 0,
                    materials,
                    textures,
                };

                let mut scenes = load_gltf(
                    &path,
                    &state.device,
                    &state.queue,
                    &state.pbr_pipeline.tex_bind_group_layout,
                    &state.pbr_pipeline.mat_bind_group_layout,
                    &state.pbr_pipeline.mesh_bind_group_layout,
                    &state.pbr_pipeline.light_bind_group_layout,
                    &mut state.world.textures,
                    &mut state.world.materials,
                );
                let mut first = scenes.remove(0);
                let id = state.world.scenes.keys().max().unwrap_or(&0) + 1;
                state.world.active_scene = id;
                first.id = id as u32;
                state.world.scenes.insert(first.id as usize, first);

                state.camera.update_light_count(
                    state
                        .world
                        .get_active_scene()
                        .expect("No active scene")
                        .light_buffer
                        .len() as u32,
                );
                state.camera.update_view(&state.queue);
                state.world.materials.update_dirty(&state.queue);
                state.world.update_active_scene(&state.queue); // updates lights and mesh info buffers
            }
            Command::ImportFile(path) => {
                info!("Importing file: {:?}", path);
                if path.extension().unwrap() == "glb" || path.extension().unwrap() == "gltf" {
                    let mut scenes = load_gltf(
                        &path,
                        &state.device,
                        &state.queue,
                        &state.pbr_pipeline.tex_bind_group_layout,
                        &state.pbr_pipeline.mat_bind_group_layout,
                        &state.pbr_pipeline.mesh_bind_group_layout,
                        &state.pbr_pipeline.light_bind_group_layout,
                        &mut state.world.textures,
                        &mut state.world.materials,
                    );

                    let first = scenes.remove(0);
                    state
                        .world
                        .scenes
                        .get_mut(&state.world.active_scene)
                        .expect("Scene does not exist")
                        .join(
                            first,
                            &state.device,
                            &state.queue,
                            &state.world.materials,
                            &state.pbr_pipeline.mesh_bind_group_layout,
                            &state.pbr_pipeline.light_bind_group_layout,
                        );
                    state.world.materials.update_dirty(&state.queue);
                    state.camera.update_light_count(
                        state
                            .world
                            .get_active_scene()
                            .expect("No active scene")
                            .light_buffer
                            .len() as u32,
                    );
                    state.camera.update_view(&state.queue);
                    state.world.update_active_scene(&state.queue); // updates lights and mesh info buffers
                } else {
                    error!("Unsupported file type: {:?}", path);
                }
            }
            Command::CreateModel(info, parent_id) => match info {
                CreateModel::Light {
                    position,
                    color,
                    intensity,
                } => {
                    let transform = Mat4::from_translation(position);
                    let mut model = Model::from(
                        vec![],
                        None,
                        vec![],
                        transform,
                        Some(PointLight::new(
                            transform,
                            state.camera.light_count() as usize,
                            color,
                            intensity,
                            Some(200.0),
                            &state.device,
                        )),
                    );
                    model.update_transforms(Mat4::IDENTITY);
                    state
                        .world
                        .scenes
                        .get_mut(&state.world.active_scene)
                        .expect("Scene does not exist")
                        .add_model(
                            model,
                            parent_id,
                            &state.device,
                            &state.queue,
                            &state.world.materials,
                            &state.pbr_pipeline.mesh_bind_group_layout,
                            &state.pbr_pipeline.light_bind_group_layout,
                        );
                    state.camera.update_light_count(
                        state
                            .world
                            .get_active_scene()
                            .expect("No active scene")
                            .light_buffer
                            .len() as u32,
                    );
                    state.camera.update_view(&state.queue);
                    state.world.update_active_scene(&state.queue); // updates lights and mesh info buffers
                }
            },
            Command::ChangeModelParent {
                model_id,
                new_parent_id,
                new_scene_id,
            } => {
                let mut model = None;
                for (_, scene) in state.world.scenes.iter_mut() {
                    if let Some(found_model) = scene.remove_model(model_id, &state.queue, &state.world.materials) {
                        model = Some(found_model);
                        break;
                    }
                }
                if let Some(model) = model {
                    state
                        .world
                        .scenes
                        .get_mut(&(new_scene_id as usize))
                        .expect("Scene does not exist")
                        .add_model(
                            model,
                            new_parent_id,
                            &state.device,
                            &state.queue,
                            &state.world.materials,
                            &state.pbr_pipeline.mesh_bind_group_layout,
                            &state.pbr_pipeline.light_bind_group_layout,
                        );
                } else {
                    error!("Model not found: {}", model_id);
                }
            }
            Command::DeleteModel(model_id) => {
                for (_, scene) in state.world.scenes.iter_mut() {
                    if scene
                        .remove_model(model_id, &state.queue, &state.world.materials)
                        .is_some()
                    {
                        state.camera.update_light_count(
                            state
                                .world
                                .get_active_scene()
                                .expect("No active scene")
                                .light_buffer
                                .len() as u32,
                        );
                        state.camera.update_view(&state.queue);
                        break;
                    }
                }
            }
            Command::DuplicateModel(model_id) => {
                for (_, scene) in state.world.scenes.iter_mut() {
                    let mut new_model = None;
                    for model in scene.iter_models_deep() {
                        if model.id == model_id {
                            new_model = Some(Model::from(
                                model.meshes.iter().map(|mesh| mesh.clone(&state.device)).collect(),
                                Some(format!("{} duplicate", model.name.clone().unwrap_or("".into())).into_boxed_str()),
                                vec![],
                                model.local_transform,
                                None,
                            )); // todo clone lights and child models
                            break;
                        }
                    }
                    if let Some(new_model) = new_model {
                        scene.add_model(
                            new_model,
                            None,
                            &state.device,
                            &state.queue,
                            &state.world.materials,
                            &state.pbr_pipeline.mesh_bind_group_layout,
                            &state.pbr_pipeline.light_bind_group_layout,
                        );
                    }
                }
                state.camera.update_light_count(
                    state
                        .world
                        .get_active_scene()
                        .expect("No active scene")
                        .light_buffer
                        .len() as u32,
                );
                state.camera.update_view(&state.queue);
            }
            Command::QueryClick((x, y)) => {
                let Some(scene) = state.world.get_active_scene() else {
                    event_sender
                        .send(Event::CommandResult(CommandResult::ClickQuery(0)))
                        .unwrap();
                    return;
                };

                let query_result = state.object_picking_pipeline.query_click(
                    &state.device,
                    &state.queue,
                    x,
                    y,
                    &scene.iter_meshes().collect::<Vec<_>>(),
                    &scene.mesh_buffer,
                    &state.camera,
                );
                debug!("Query result: {}", query_result);
                event_sender
                    .send(Event::CommandResult(CommandResult::ClickQuery(query_result)))
                    .unwrap();
            }
        }
        debug!("Finished processing command.");
    }
}
