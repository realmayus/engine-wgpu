use std::path::PathBuf;
use std::sync::Arc;

use glam::Mat4;
use itertools::Itertools;
use log::info;
use vulkano::command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer};
use vulkano::descriptor_set::allocator::StandardDescriptorSetAllocator;
use vulkano::device::Device;
use vulkano::memory::allocator::StandardMemoryAllocator;

use lib::scene::DrawableVertexInputs;
use renderer::pipelines::PipelineProviderKind;
use systems::io::gltf_loader::load_gltf;
use systems::io::world_loader::load_world;

use crate::renderer_impl::InnerState;

pub(crate) trait Command {
    fn execute(
        &self,
        state: &mut InnerState,
        pipeline_providers: &mut [PipelineProviderKind],
        allocator: &StandardMemoryAllocator,
        descriptor_set_allocator: &StandardDescriptorSetAllocator,
        cmd_buf_builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
        device: Arc<Device>,
    );
}

pub(crate) struct DeleteModelCommand {
    pub(crate) to_delete: u32,
}

impl Command for DeleteModelCommand {
    fn execute(
        &self,
        state: &mut InnerState,
        pipeline_providers: &mut [PipelineProviderKind],
        allocator: &StandardMemoryAllocator,
        _descriptor_set_allocator: &StandardDescriptorSetAllocator,
        _cmd_buf_builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
        _device: Arc<Device>,
    ) {
        info!("Deleting model with ID {}", self.to_delete);
        for scene in state.world.scenes.as_mut_slice() {
            // we don't know which scene the model is in
            scene.models.retain(|m| m.id != self.to_delete);
        }
        for pipeline_provider in pipeline_providers {
            //TODO don't assume there's only one instance of a provider
            match pipeline_provider {
                PipelineProviderKind::LINE(_) => {}
                PipelineProviderKind::PBR(pbr) => {
                    pbr.update_drawables(
                        state
                            .world
                            .get_active_scene()
                            .iter_meshes()
                            .map(|mesh| DrawableVertexInputs::from_mesh(mesh, allocator.clone()))
                            .collect_vec(),
                    );
                    pbr.recreate_render_passes = true;
                }
            }
        }
    }
}

pub(crate) struct UpdateModelCommand {
    pub(crate) to_update: u32,
    pub(crate) parent_transform: Mat4,
    pub(crate) local_transform: Mat4,
}

impl Command for UpdateModelCommand {
    fn execute(
        &self,
        state: &mut InnerState,
        _pipeline_providers: &mut [PipelineProviderKind],
        _allocator: &StandardMemoryAllocator,
        _descriptor_set_allocator: &StandardDescriptorSetAllocator,
        _cmd_buf_builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
        _device: Arc<Device>,
    ) {
        for scene in state.world.scenes.as_mut_slice() {
            for m in scene.models.as_mut_slice() {
                if m.id == self.to_update {
                    m.local_transform = self.local_transform;
                    m.update_transforms(self.parent_transform);
                }
            }
        }
    }
}

pub(crate) struct ImportGltfCommand {
    pub(crate) path: PathBuf,
}

impl Command for ImportGltfCommand {
    fn execute(
        &self,
        state: &mut InnerState,
        pipeline_providers: &mut [PipelineProviderKind],
        allocator: &StandardMemoryAllocator,
        _descriptor_set_allocator: &StandardDescriptorSetAllocator,
        cmd_buf_builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
        _device: Arc<Device>,
    ) {
        let gltf_scenes = load_gltf(
            self.path.as_path(),
            allocator,
            cmd_buf_builder,
            &mut state.world.textures,
            &mut state.world.materials,
        );
        state
            .world
            .get_active_scene_mut()
            .models
            .extend(gltf_scenes.iter().flat_map(|s| s.models.clone()));

        for pipeline_provider in pipeline_providers {
            //TODO don't assume there's only one instance of a provider
            match pipeline_provider {
                PipelineProviderKind::LINE(_) => {}
                PipelineProviderKind::PBR(pbr) => {
                    pbr.update_drawables(
                        state
                            .world
                            .get_active_scene()
                            .iter_meshes()
                            .map(|mesh| DrawableVertexInputs::from_mesh(mesh, allocator))
                            .collect_vec(),
                    );
                    pbr.recreate_render_passes = true;
                }
            }
        }
    }
}

pub(crate) struct LoadWorldCommand {
    pub(crate) path: PathBuf, // path to world.json
}

impl Command for LoadWorldCommand {
    fn execute(
        &self,
        state: &mut InnerState,
        pipeline_providers: &mut [PipelineProviderKind],
        allocator: &StandardMemoryAllocator,
        _descriptor_set_allocator: &StandardDescriptorSetAllocator,
        cmd_buf_builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
        _device: Arc<Device>,
    ) {
        let world = load_world(self.path.as_path(), allocator, cmd_buf_builder);
        state.world = world;

        for pipeline_provider in pipeline_providers {
            //TODO don't assume there's only one instance of a provider
            match pipeline_provider {
                PipelineProviderKind::LINE(_) => {}
                PipelineProviderKind::PBR(pbr) => {
                    pbr.update_drawables(
                        state
                            .world
                            .get_active_scene()
                            .iter_meshes()
                            .map(|mesh| DrawableVertexInputs::from_mesh(mesh, allocator))
                            .collect_vec(),
                    );
                    pbr.recreate_render_passes = true;
                }
            }
        }
    }
}
