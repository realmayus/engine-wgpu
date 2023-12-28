use std::sync::Arc;

use vulkano::buffer::Subbuffer;
use vulkano::command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer};
use vulkano::descriptor_set::allocator::StandardDescriptorSetAllocator;
use vulkano::descriptor_set::layout::DescriptorBindingFlags;
use vulkano::device::Device;
use vulkano::image::sampler::Sampler;
use vulkano::image::view::ImageView;
use vulkano::pipeline::graphics::depth_stencil::{DepthState, DepthStencilState};
use vulkano::pipeline::graphics::input_assembly::{InputAssemblyState, PrimitiveTopology};
use vulkano::pipeline::graphics::vertex_input::{Vertex, VertexBufferDescription, VertexDefinition, VertexInputState};
use vulkano::pipeline::graphics::viewport::{Viewport, ViewportState};
use vulkano::pipeline::{GraphicsPipeline, Pipeline, PipelineLayout, PipelineShaderStageCreateInfo};
use vulkano::pipeline::graphics::color_blend::{ColorBlendAttachmentState, ColorBlendState};
use vulkano::pipeline::graphics::GraphicsPipelineCreateInfo;
use vulkano::pipeline::graphics::multisample::MultisampleState;
use vulkano::pipeline::graphics::rasterization::RasterizationState;
use vulkano::pipeline::layout::PipelineDescriptorSetLayoutCreateInfo;
use vulkano::render_pass::{RenderPass, Subpass};
use vulkano::shader::ShaderModule;

use lib::scene::DrawableVertexInputs;
use lib::shader_types::{CameraUniform, LightInfo, MaterialInfo, MeshInfo, MyNormal, MyTangent, MyUV, MyVertex};

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
    vertex_input_state: VertexInputState,
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
            vs: vs.clone(),
            fs,
            cached_vertex_input_buffers: drawables,
            viewport,
            render_pass,
            device,
            vertex_input_state: [
                MyVertex::per_vertex(),
                MyNormal::per_vertex(),
                MyTangent::per_vertex(),
                MyUV::per_vertex(),
            ].definition(&vs.entry_point("main").unwrap().info().input_interface).unwrap(),
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
        let stages = [PipelineShaderStageCreateInfo::new(self.vs.entry_point("main").unwrap()), PipelineShaderStageCreateInfo::new(self.fs.entry_point("main").unwrap())];
        let layout = {
            let mut layout_create_info = PipelineDescriptorSetLayoutCreateInfo::from_stages(&stages);
            let binding = layout_create_info.set_layouts[1]
                .bindings
                .get_mut(&0)
                .unwrap();
            binding.binding_flags |= DescriptorBindingFlags::VARIABLE_DESCRIPTOR_COUNT;
            binding.descriptor_count = 128;

            let binding = layout_create_info.set_layouts[2]
                .bindings
                .get_mut(&0)
                .unwrap();
            binding.binding_flags |= DescriptorBindingFlags::VARIABLE_DESCRIPTOR_COUNT;
            binding.descriptor_count = 128;


            let binding = layout_create_info.set_layouts[3]
                .bindings
                .get_mut(&0)
                .unwrap();
            binding.binding_flags |= DescriptorBindingFlags::VARIABLE_DESCRIPTOR_COUNT;
            binding.descriptor_count = 128;


            let binding = layout_create_info.set_layouts[4]
                .bindings
                .get_mut(&0)
                .unwrap();
            binding.binding_flags |= DescriptorBindingFlags::VARIABLE_DESCRIPTOR_COUNT;
            binding.descriptor_count = 128;

            PipelineLayout::new(self.device.clone(), layout_create_info.into_pipeline_layout_create_info(self.device.clone()).unwrap()).unwrap()
        };
        let input_assembly_state = InputAssemblyState::default();
        let subpass = Subpass::from(self.render_pass.clone(), 0).unwrap();
        self.pipeline = Some(
            GraphicsPipeline::new(
                self.device.clone(),
                None,
                GraphicsPipelineCreateInfo {
                    stages: stages.into_iter().collect(),
                    vertex_input_state: Some(self.vertex_input_state.clone()),
                    input_assembly_state: Some(input_assembly_state),
                    viewport_state: Some(
                        ViewportState {
                            viewports: [self.viewport.clone()].into_iter().collect(),
                            ..Default::default()
                        }
                    ),
                    rasterization_state: Some(RasterizationState::default()),
                    depth_stencil_state: Some(DepthStencilState {
                        depth: Some(DepthState::simple()),
                        ..Default::default()
                    }),
                    multisample_state: Some(MultisampleState::default()),
                    color_blend_state: Some(ColorBlendState::with_attachment_states(subpass.num_color_attachments(), ColorBlendAttachmentState::default())),
                    subpass: Some(subpass.into()),
                    ..GraphicsPipelineCreateInfo::layout(layout)
                }
            ).unwrap()
        );
    }

    fn set_viewport(&mut self, viewport: Viewport) {
        self.viewport = viewport;
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
        self.descriptor_set_controller = Some(DescriptorSetController::init(
            camera,
            textures,
            material_info_buffers,
            mesh_info_buffers,
            light_info_buffers,
            descriptor_set_allocator,
            self.pipeline.clone().unwrap().layout().clone(),
        ));
    }

    fn render_pass(&self, builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>) {
        let _ = builder.bind_pipeline_graphics(self.pipeline.clone().unwrap().clone());

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
                        vertex_input.tangent_buffer.subbuffer.clone(),
                        vertex_input.uv_buffer.subbuffer.clone(),
                    ),
                ).unwrap()
                .bind_index_buffer(vertex_input.index_buffer.clone()).unwrap()
                .draw_indexed(vertex_input.index_buffer.len() as u32, 1, 0, 0, i as u32)
                .unwrap();
        }
    }

    fn must_recreate_render_passes(&mut self) -> bool {
        self.recreate_render_passes
    }
}
