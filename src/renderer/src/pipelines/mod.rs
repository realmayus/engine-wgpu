use std::sync::Arc;
use vulkano::command_buffer::allocator::StandardCommandBufferAllocator;
use vulkano::command_buffer::PrimaryAutoCommandBuffer;
use vulkano::descriptor_set::allocator::StandardDescriptorSetAllocator;
use vulkano::descriptor_set::layout::DescriptorSetLayout;
use vulkano::pipeline::graphics::viewport::Viewport;
use vulkano::pipeline::GraphicsPipeline;
use vulkano::render_pass::Framebuffer;

pub mod pbr_pipeline;

pub trait PipelineProvider {
    fn get_pipeline(&self, viewport: Viewport) -> Arc<GraphicsPipeline>;

    fn init_descriptor_sets(&mut self, set_layouts: &[Arc<DescriptorSetLayout>], descriptor_set_allocator: &StandardDescriptorSetAllocator);
    fn begin_render_pass(
        &self,
        framebuffers: &Vec<Arc<Framebuffer>>,
        queue_family_index: u32,
        pipeline: Arc<GraphicsPipeline>,
        cmd_buf_allocator: &StandardCommandBufferAllocator,
    ) -> Vec<Arc<PrimaryAutoCommandBuffer>>;
}
