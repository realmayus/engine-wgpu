use std::sync::Arc;

use vulkano::buffer::Subbuffer;
use vulkano::command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer};
use vulkano::descriptor_set::allocator::StandardDescriptorSetAllocator;
use vulkano::device::Device;
use vulkano::image::ImageViewAbstract;
use vulkano::pipeline::graphics::depth_stencil::DepthStencilState;
use vulkano::pipeline::graphics::input_assembly::InputAssemblyState;
use vulkano::pipeline::graphics::vertex_input::{Vertex, VertexBufferDescription};
use vulkano::pipeline::graphics::viewport::{Viewport, ViewportState};
use vulkano::pipeline::{GraphicsPipeline, Pipeline};
use vulkano::render_pass::{RenderPass, Subpass};
use vulkano::sampler::Sampler;
use vulkano::shader::ShaderModule;

use lib::scene::DrawableVertexInputs;
use lib::shader_types::{CameraUniform, MaterialInfo, MeshInfo, MyNormal, MyUV, MyVertex};

use crate::pipelines::descriptor_set_controller::DescriptorSetController;
use crate::pipelines::PipelineProvider;

mod vs {
    vulkano_shaders::shader! {
        ty: "vertex",
        path: "../../assets/shaders/pbr.vert",
    }
}

mod fs {
    vulkano_shaders::shader! {
        ty: "fragment",
        path: "../../assets/shaders/pbr.frag",
    }
}

/**
Pipeline for physically-based rendering
*/
pub struct PBRPipelineProvider {
    vs: Arc<ShaderModule>,
    fs: Arc<ShaderModule>,
    cached_vertex_input_buffers: Vec<DrawableVertexInputs>,
    viewport: Viewport,
    render_pass: Arc<RenderPass>,
    device: Arc<Device>,
    vertex_input_state: Vec<VertexBufferDescription>,
    pipeline: Option<Arc<GraphicsPipeline>>,
    pub recreate_render_passes: bool,
    descriptor_set_controller: Option<DescriptorSetController>,
    // will get initialized later
}

impl PBRPipelineProvider {
    pub fn new(
        device: Arc<Device>,
        drawables: Vec<DrawableVertexInputs>,
        viewport: Viewport,
        render_pass: Arc<RenderPass>,
    ) -> Self {
        let vs = vs::load(device.clone()).expect("failed to create vertex shader module");
        let fs = fs::load(device.clone()).expect("failed to create fragment shader module");

        Self {
            vs,
            fs,
            cached_vertex_input_buffers: drawables,
            viewport,
            render_pass,
            device,
            vertex_input_state: vec![
                MyVertex::per_vertex(),
                MyNormal::per_vertex(),
                MyUV::per_vertex(),
            ],
            pipeline: None,
            recreate_render_passes: false,
            descriptor_set_controller: None,
        }
    }

    pub fn update_drawables(&mut self, new_inputs: Vec<DrawableVertexInputs>) {
        self.cached_vertex_input_buffers = new_inputs;
    }

    pub fn update_descriptor_sets<F>(&mut self, f: F)
    where
        F: Fn(&mut DescriptorSetController),
    {
        f(self.descriptor_set_controller.as_mut().unwrap());
    }
}

impl PipelineProvider for PBRPipelineProvider {
    fn create_pipeline(&mut self) {
        self.pipeline = Some(
            GraphicsPipeline::start()
                .vertex_input_state(self.vertex_input_state.clone()) // describes layout of vertex input
                .vertex_shader(self.vs.entry_point("main").unwrap(), ()) // specify entry point of vertex shader (vulkan shaders can technically have multiple)
                .input_assembly_state(InputAssemblyState::new()) //Indicate type of primitives (default is list of triangles)
                .viewport_state(ViewportState::viewport_fixed_scissor_irrelevant([self
                    .viewport
                    .clone()])) // Set the *fixed* viewport -> makes it impossible to change viewport for each draw cmd, but increases performance. Need to create new pipeline object if size does change.
                .fragment_shader(self.fs.entry_point("main").unwrap(), ()) // Specify entry point of fragment shader
                .depth_stencil_state(DepthStencilState::simple_depth_test())
                .render_pass(Subpass::from(self.render_pass.clone(), 0).unwrap()) // This pipeline object concerns the first pass of the render pass
                .with_auto_layout(self.device.clone(), |x| {
                    let binding = x[1].bindings.get_mut(&0).unwrap();
                    binding.variable_descriptor_count = true;
                    binding.descriptor_count = 128; //TODO this is an upper bound to the number of textures, perhaps make it dynamic

                    let binding = x[2].bindings.get_mut(&0).unwrap();
                    binding.variable_descriptor_count = true;
                    binding.descriptor_count = 128;

                    let binding = x[3].bindings.get_mut(&0).unwrap(); // MeshInfo
                    binding.variable_descriptor_count = true;
                    binding.descriptor_count = 128
                })
                .unwrap(),
        );
    }

    fn set_viewport(&mut self, viewport: Viewport) {
        self.viewport = viewport;
    }

    fn init_descriptor_sets(
        &mut self,
        descriptor_set_allocator: &StandardDescriptorSetAllocator,
        camera: Subbuffer<CameraUniform>,
        textures: Vec<(Arc<dyn ImageViewAbstract>, Arc<Sampler>)>,
        material_info_buffers: Vec<Subbuffer<MaterialInfo>>,
        mesh_info_buffers: Vec<Subbuffer<MeshInfo>>,
    ) {
        self.descriptor_set_controller = Some(DescriptorSetController::init(
            camera,
            textures,
            material_info_buffers,
            mesh_info_buffers,
            descriptor_set_allocator,
            self.pipeline.clone().unwrap().layout().clone(),
        ));
    }

    fn render_pass(&self, builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>) {
        builder.bind_pipeline_graphics(self.pipeline.clone().unwrap().clone());

        self.descriptor_set_controller
            .as_ref()
            .unwrap()
            .bind(builder);

        for (i, vertex_input) in self.cached_vertex_input_buffers.iter().enumerate() {
            builder
                .bind_vertex_buffers(
                    0,
                    (
                        vertex_input.vertex_buffer.subbuffer.clone(),
                        vertex_input.normal_buffer.subbuffer.clone(),
                        vertex_input.uv_buffer.subbuffer.clone(),
                    ),
                )
                .bind_index_buffer(vertex_input.index_buffer.clone())
                .draw_indexed(vertex_input.index_buffer.len() as u32, 1, 0, 0, i as u32)
                .unwrap();
        }
    }

    fn must_recreate_render_passes(&mut self) -> bool {
        self.recreate_render_passes
    }
}
