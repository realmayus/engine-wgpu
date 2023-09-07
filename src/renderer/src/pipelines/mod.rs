use std::sync::Arc;

use lib::scene::Texture;
use vulkano::command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer};
use vulkano::descriptor_set::allocator::StandardDescriptorSetAllocator;
use vulkano::descriptor_set::layout::DescriptorSetLayout;
use vulkano::descriptor_set::PersistentDescriptorSet;
use vulkano::pipeline::graphics::viewport::Viewport;
use vulkano::pipeline::GraphicsPipeline;

pub mod line_pipeline;
pub mod pbr_pipeline;

pub trait PipelineProvider {
    fn get_pipeline(&self) -> Arc<GraphicsPipeline>;
    fn set_viewport(&mut self, viewport: Viewport);
    fn init_descriptor_sets(
        &mut self,
        set_layouts: &[Arc<DescriptorSetLayout>],
        descriptor_set_allocator: &StandardDescriptorSetAllocator,
    );
    fn render_pass(
        &self,
        builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
        pipeline: Arc<GraphicsPipeline>,
    );
}
