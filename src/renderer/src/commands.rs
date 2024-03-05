use std::path::{Path, PathBuf};
use std::sync::mpsc;

use glam::Mat4;
use log::{debug, error, info};

use lib::managers::{MaterialManager, TextureManager};
use lib::scene::{Model, PointLight, World};
use systems::io::gltf_loader::load_gltf;

use crate::{commands, RenderState};

#[derive(Debug)]
pub enum CreateModel {
    Light {
        position: glam::Vec3,
        color: glam::Vec3,
        intensity: f32,
    },
}

pub type Commands = mpsc::Sender<commands::Command>;

#[derive(Debug)]
pub enum Command {
    LoadDefaultScene,
    LoadSceneFile(PathBuf),
    ImportFile(PathBuf),
    CreateModel(CreateModel),
}

impl Command {
    pub(crate) fn process(&self, state: &mut RenderState) {
        debug!("Processing command: {:?}", self);
        match self {
            Command::LoadDefaultScene => {
                let mut scenes = load_gltf(
                    // Path::new("assets/models/cube_light_tan.glb"),
                    Path::new("assets/models/cube_brick.glb"),
                    // Path::new("assets/models/DamagedHelmetTangents.glb"),
                    // Path::new("assets/models/monkeyabuse.glb"),
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
                state.world.scenes.push(first);
            }
            Command::LoadSceneFile(path) => {
                let textures = TextureManager::new(&state.device, &state.queue, &state.pbr_pipeline.tex_bind_group_layout);
                let materials = MaterialManager::new(&state.device, &state.queue, &state.pbr_pipeline.mat_bind_group_layout, &state.pbr_pipeline.tex_bind_group_layout, &textures);
                state.world = World {
                    scenes: vec![],
                    active_scene: 0,
                    materials,
                    textures,
                };

                let mut scenes = load_gltf(
                    path,
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
                state.world.scenes.push(first);
                state.world.materials.update_dirty(&state.queue);
                state.world.update_active_scene(&state.queue);  // updates lights and mesh info buffers
                state.camera.update_light_count(state.world.get_active_scene().light_buffer.len());
                state.camera.update_view(&state.queue);
            }
            Command::ImportFile(path) => {
                info!("Importing file: {:?}", path);
                if path.extension().unwrap() == "glb" || path.extension().unwrap() == "gltf" {
                    let mut scenes = load_gltf(
                        path,
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
                    state.world.scenes[state.world.active_scene].join(first, &state.device, &state.queue, &state.world.materials, &state.pbr_pipeline.mesh_bind_group_layout, &state.pbr_pipeline.light_bind_group_layout);
                    state.world.materials.update_dirty(&state.queue);
                    state.camera.update_light_count(state.world.get_active_scene().light_buffer.len());
                    state.camera.update_view(&state.queue);
                } else {
                    error!("Unsupported file type: {:?}", path);
                }
            }
            Command::CreateModel(info) => {
                match info {
                    CreateModel::Light { position, color, intensity } => {
                        let transform = Mat4::from_translation(*position);
                        let mut model = Model::from(vec![], None, vec![], transform, Some(PointLight::new(transform, state.camera.light_count() as usize, *color, *intensity, None, &state.device)));
                        model.update_transforms(Mat4::IDENTITY);
                        state.world.scenes[state.world.active_scene].add_model(model, &state.device, &state.queue, &state.world.materials, &state.pbr_pipeline.mesh_bind_group_layout, &state.pbr_pipeline.light_bind_group_layout);
                        state.camera.update_light_count(state.world.get_active_scene().light_buffer.len());
                        state.camera.update_view(&state.queue);
                    }
                }
            }
        }
        debug!("Finished processing command.");
    }
}