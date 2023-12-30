use std::sync::Arc;

use vulkano::buffer::Subbuffer;
use vulkano::command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer};
use vulkano::descriptor_set::allocator::StandardDescriptorSetAllocator;
use vulkano::descriptor_set::layout::DescriptorBindingFlags;
use vulkano::descriptor_set::{PersistentDescriptorSet, WriteDescriptorSet};
use vulkano::device::Device;
use vulkano::image::sampler::Sampler;
use vulkano::image::view::ImageView;
use vulkano::pipeline::graphics::color_blend::{ColorBlendAttachmentState, ColorBlendState};
use vulkano::pipeline::graphics::depth_stencil::{DepthState, DepthStencilState};
use vulkano::pipeline::graphics::input_assembly::{InputAssemblyState, PrimitiveTopology};
use vulkano::pipeline::graphics::multisample::MultisampleState;
use vulkano::pipeline::graphics::rasterization::RasterizationState;
use vulkano::pipeline::graphics::vertex_input::{
    Vertex, VertexBufferDescription, VertexDefinition, VertexInputState,
};
use vulkano::pipeline::graphics::viewport::{Viewport, ViewportState};
use vulkano::pipeline::graphics::GraphicsPipelineCreateInfo;
use vulkano::pipeline::layout::{PipelineDescriptorSetLayoutCreateInfo, PipelineLayoutCreateInfo};
use vulkano::pipeline::{
    GraphicsPipeline, Pipeline, PipelineBindPoint, PipelineLayout, PipelineShaderStageCreateInfo,
};
use vulkano::render_pass::{RenderPass, Subpass};
use vulkano::shader::ShaderModule;

use lib::shader_types::{CameraUniform, LightInfo, MaterialInfo, MeshInfo, MyVertex};
use lib::SizedBuffer;

use crate::pipelines::PipelineProvider;

mod vs {
    vulkano_shaders::shader! {
        ty: "vertex",
        path: "../../assets/shaders/line.vert",
    }
}

mod fs {
    vulkano_shaders::shader! {
        ty: "fragment",
        path: "../../assets/shaders/line.frag",
    }
}

/**
Pipeline for drawing lines in 3D space, e.g. for axes
 */
pub struct LinePipelineProvider {
    vs: Arc<ShaderModule>,
    fs: Arc<ShaderModule>,
    vertex_buffers: Vec<SizedBuffer>,
    write_descriptor_sets: Vec<(u32, Vec<WriteDescriptorSet>)>, // tuples of WriteDescriptorSets and VARIABLE descriptor count, is cleared by init_descriptor_sets function
    descriptor_sets: Vec<Arc<PersistentDescriptorSet>>, // initially empty -> populated by init_descriptor_sets function
    viewport: Viewport,
    render_pass: Arc<RenderPass>,
    device: Arc<Device>,
    vertex_input_state: VertexInputState,
    pipeline: Option<Arc<GraphicsPipeline>>,
}

impl LinePipelineProvider {
    pub fn new(
        device: Arc<Device>,
        vertex_buffers: Vec<SizedBuffer>,
        camera_buffer: Subbuffer<CameraUniform>,
        line_info_buffers: impl IntoIterator<Item = Subbuffer<impl ?Sized>> + ExactSizeIterator,
        viewport: Viewport,
        render_pass: Arc<RenderPass>,
    ) -> Self {
        let vs = vs::load(device.clone()).expect("failed to create vertex shader module");
        let fs = fs::load(device.clone()).expect("failed to create fragment shader module");

        let write_descriptor_sets = vec![
            (
                0,
                vec![
                    // Level 0: Scene-global uniforms
                    WriteDescriptorSet::buffer(0, camera_buffer),
                ],
            ),
            (
                line_info_buffers.len() as u32,
                vec![
                    // Level 1: Model-specific uniforms
                    WriteDescriptorSet::buffer_array(0, 0, line_info_buffers),
                ],
            ),
        ];

        Self {
            vs: vs.clone(),
            fs,
            vertex_buffers,
            write_descriptor_sets,
            descriptor_sets: vec![],
            viewport,
            render_pass,
            device,
            vertex_input_state: [MyVertex::per_vertex()]
                .definition(&vs.entry_point("main").unwrap().info().input_interface)
                .unwrap(),
            pipeline: None,
        }
    }
}

impl PipelineProvider for LinePipelineProvider {
    fn create_pipeline(&mut self) {
        let stages = [
            PipelineShaderStageCreateInfo::new(self.vs.entry_point("main").unwrap()),
            PipelineShaderStageCreateInfo::new(self.fs.entry_point("main").unwrap()),
        ];
        let layout = {
            let mut layout_create_info =
                PipelineDescriptorSetLayoutCreateInfo::from_stages(&stages);
            let binding = layout_create_info.set_layouts[1]
                .bindings
                .get_mut(&0)
                .unwrap();
            binding.binding_flags |= DescriptorBindingFlags::VARIABLE_DESCRIPTOR_COUNT;
            binding.descriptor_count = 128;
            PipelineLayout::new(
                self.device.clone(),
                layout_create_info
                    .into_pipeline_layout_create_info(self.device.clone())
                    .unwrap(),
            )
            .unwrap()
        };
        let mut input_assembly_state = InputAssemblyState::default();
        input_assembly_state.topology = PrimitiveTopology::LineList;
        let subpass = Subpass::from(self.render_pass.clone(), 0).unwrap();
        self.pipeline = Some(
            GraphicsPipeline::new(
                self.device.clone(),
                None,
                GraphicsPipelineCreateInfo {
                    stages: stages.into_iter().collect(),
                    vertex_input_state: Some(self.vertex_input_state.clone()),
                    input_assembly_state: Some(input_assembly_state),
                    viewport_state: Some(ViewportState {
                        viewports: [self.viewport.clone()].into_iter().collect(),
                        ..Default::default()
                    }),
                    rasterization_state: Some(RasterizationState::default()),
                    depth_stencil_state: Some(DepthStencilState {
                        depth: Some(DepthState::simple()),
                        ..Default::default()
                    }),
                    multisample_state: Some(MultisampleState::default()),
                    color_blend_state: Some(ColorBlendState::with_attachment_states(
                        subpass.num_color_attachments(),
                        ColorBlendAttachmentState::default(),
                    )),
                    subpass: Some(subpass.into()),
                    ..GraphicsPipelineCreateInfo::layout(layout)
                },
            )
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
        textures: Vec<(Arc<ImageView>, Arc<Sampler>)>,
        material_info_buffers: Vec<Subbuffer<MaterialInfo>>,
        mesh_info_buffers: Vec<Subbuffer<MeshInfo>>,
        light_info_buffers: Vec<Subbuffer<LightInfo>>,
    ) {
        let mut temp = vec![];
        std::mem::swap(&mut self.write_descriptor_sets, &mut temp);

        for (i, (var_count, write_desc_set)) in temp.into_iter().enumerate() {
            let descriptor_set_layout = self
                .pipeline
                .clone()
                .unwrap()
                .layout()
                .set_layouts()
                .get(i)
                .unwrap()
                .clone();
            self.descriptor_sets.push(
                PersistentDescriptorSet::new_variable(
                    descriptor_set_allocator,
                    descriptor_set_layout,
                    var_count,
                    write_desc_set,
                    [],
                )
                .unwrap(),
            );
        }
    }

    fn render_pass(&self, builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>) {
        builder.bind_pipeline_graphics(self.pipeline.clone().unwrap().clone());

        for i in 0..self.descriptor_sets.len() {
            builder.bind_descriptor_sets(
                PipelineBindPoint::Graphics,
                self.pipeline.clone().unwrap().layout().clone(),
                i as u32,
                self.descriptor_sets[i].clone(),
            );
        }
        for i in 0..self.vertex_buffers.len() {
            builder
                .bind_vertex_buffers(0, self.vertex_buffers[i].subbuffer.clone())
                .unwrap()
                .draw(self.vertex_buffers[i].count, 1, 0, i as u32)
                .unwrap();
        }
    }

    fn must_recreate_render_passes(&mut self) -> bool {
        false
    }
}
