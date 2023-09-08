use vulkano::command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer};
use vulkano::descriptor_set::allocator::StandardDescriptorSetAllocator;
use vulkano::pipeline::graphics::viewport::Viewport;

use crate::pipelines::line_pipeline::LinePipelineProvider;
use crate::pipelines::pbr_pipeline::PBRPipelineProvider;

pub mod line_pipeline;
pub mod pbr_pipeline;

pub trait PipelineProvider {
    fn create_pipeline(&mut self);
    fn set_viewport(&mut self, viewport: Viewport);
    fn init_descriptor_sets(&mut self, descriptor_set_allocator: &StandardDescriptorSetAllocator);
    fn render_pass(&self, builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>);
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

    fn init_descriptor_sets(&mut self, descriptor_set_allocator: &StandardDescriptorSetAllocator) {
        match self {
            PipelineProviderKind::LINE(line_pipeline) => {
                line_pipeline.init_descriptor_sets(descriptor_set_allocator)
            }
            PipelineProviderKind::PBR(pbr_pipeline) => {
                pbr_pipeline.init_descriptor_sets(descriptor_set_allocator)
            }
        }
    }

    fn render_pass(&self, builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>) {
        match self {
            PipelineProviderKind::LINE(line_pipeline) => line_pipeline.render_pass(builder),
            PipelineProviderKind::PBR(pbr_pipeline) => pbr_pipeline.render_pass(builder),
        }
    }
}
