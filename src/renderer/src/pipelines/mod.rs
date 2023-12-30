use std::sync::Arc;

use vulkano::buffer::Subbuffer;
use vulkano::command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer};
use vulkano::descriptor_set::allocator::StandardDescriptorSetAllocator;
use vulkano::image::sampler::Sampler;
use vulkano::image::view::ImageView;
use vulkano::pipeline::graphics::viewport::Viewport;

use lib::shader_types::{CameraUniform, LightInfo, MaterialInfo, MeshInfo};

use crate::pipelines::line_pipeline::LinePipelineProvider;
use crate::pipelines::pbr_pipeline::PBRPipelineProvider;

mod descriptor_set_controller;
pub mod line_pipeline;
pub mod pbr_pipeline;
mod bind_group_controller;

pub trait PipelineProvider {
    fn create_pipeline(&mut self);
    fn set_viewport(&mut self, viewport: Viewport);

    fn init_descriptor_sets(
        &mut self,
        descriptor_set_allocator: &StandardDescriptorSetAllocator,
        camera: Subbuffer<CameraUniform>,
        textures: Vec<(Arc<ImageView>, Arc<Sampler>)>,
        material_info_buffers: Vec<Subbuffer<MaterialInfo>>,
        mesh_info_buffers: Vec<Subbuffer<MeshInfo>>,
        light_info_buffers: Vec<Subbuffer<LightInfo>>,
    );
    fn render_pass(&self, builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>);

    fn must_recreate_render_passes(&mut self) -> bool;
}

pub enum PipelineProviderKind {
    LINE(LinePipelineProvider),
    PBR(PBRPipelineProvider),
}

impl PipelineProvider for PipelineProviderKind {
    fn create_pipeline(&mut self) {
        match self {
            PipelineProviderKind::LINE(line_pipeline) => line_pipeline.create_pipeline(),
            PipelineProviderKind::PBR(pbr_pipeline) => pbr_pipeline.create_pipeline(),
        }
    }

    fn set_viewport(&mut self, viewport: Viewport) {
        match self {
            PipelineProviderKind::LINE(line_pipeline) => line_pipeline.set_viewport(viewport),
            PipelineProviderKind::PBR(pbr_pipeline) => pbr_pipeline.set_viewport(viewport),
        }
    }

    fn init_descriptor_sets(
        &mut self,
        descriptor_set_allocator: &StandardDescriptorSetAllocator,
        camera: Subbuffer<CameraUniform>,
        textures: Vec<(Arc<ImageView>, Arc<Sampler>)>,
        material_info_buffers: Vec<Subbuffer<MaterialInfo>>,
        mesh_info_buffers: Vec<Subbuffer<MeshInfo>>,
        light_info_buffers: Vec<Subbuffer<LightInfo>>,
    ) {
        match self {
            PipelineProviderKind::LINE(line_pipeline) => line_pipeline.init_descriptor_sets(
                descriptor_set_allocator,
                camera,
                textures,
                material_info_buffers,
                mesh_info_buffers,
                light_info_buffers,
            ),
            PipelineProviderKind::PBR(pbr_pipeline) => pbr_pipeline.init_descriptor_sets(
                descriptor_set_allocator,
                camera,
                textures,
                material_info_buffers,
                mesh_info_buffers,
                light_info_buffers,
            ),
        }
    }

    fn render_pass(&self, builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>) {
        match self {
            PipelineProviderKind::LINE(line_pipeline) => line_pipeline.render_pass(builder),
            PipelineProviderKind::PBR(pbr_pipeline) => pbr_pipeline.render_pass(builder),
        }
    }

    fn must_recreate_render_passes(&mut self) -> bool {
        let result;
        match self {
            PipelineProviderKind::LINE(line_pipeline) => {
                result = line_pipeline.must_recreate_render_passes(); // TODO set recreate_render_passes to false if line pipeline ever allows updating
            }
            PipelineProviderKind::PBR(pbr_pipeline) => {
                result = pbr_pipeline.must_recreate_render_passes();
                pbr_pipeline.recreate_render_passes = false;
            }
        };
        result
    }
}
