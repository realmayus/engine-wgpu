use std::path::{Path, PathBuf};
use std::sync::mpsc;
use log::{debug, error, info};
use systems::io::gltf_loader::load_gltf;
use crate::{commands, RenderState};

pub type Commands = mpsc::Sender<commands::Command>;
#[derive(Debug)]
pub enum Command {
    LoadDefaultScene,
    LoadSceneFile(PathBuf),
    ImportFile(PathBuf),
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
                info!("Loading scene file: {:?}", path);
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
                    state.world.scenes.push(first);
                } else {
                    error!("Unsupported file type: {:?}", path);
                }
            }
        }
        debug!("Finished processing command.");
    }
}