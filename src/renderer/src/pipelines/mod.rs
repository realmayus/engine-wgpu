use std::sync::Arc;
use vulkano::buffer::Subbuffer;

use lib::VertexBuffer;
use vulkano::command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer};
use vulkano::descriptor_set::allocator::StandardDescriptorSetAllocator;
use vulkano::descriptor_set::layout::DescriptorSetLayout;
use vulkano::pipeline::graphics::viewport::Viewport;
use vulkano::pipeline::GraphicsPipeline;

pub mod line_pipeline;
pub mod pbr_pipeline;

pub trait PipelineProvider {
    fn kind(&self) -> PipelineKind;
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
        vertex_buffers: Vec<VertexBuffer>,
        normal_buffers: Vec<VertexBuffer>,
        uv_buffers: Vec<VertexBuffer>,
        index_buffers: Vec<Subbuffer<[u32]>>,
    );
}

pub enum PipelineKind {
    LINE,
    PBR,
}

impl PipelineKind {
    pub fn name(&self) -> String {
        match self {
            PipelineKind::LINE => "Line Pipeline".to_string(),
            PipelineKind::PBR => "PBR Pipeline".to_string(),
        }
    }
}
