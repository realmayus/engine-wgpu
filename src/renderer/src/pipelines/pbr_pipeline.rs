use std::ops::Range;
use std::sync::Arc;

use lib::shader_types::{CameraUniform, MyNormal, MyTangent, MyUV, MyVertex};
use vulkano::buffer::Subbuffer;
use vulkano::command_buffer::allocator::StandardCommandBufferAllocator;
use vulkano::command_buffer::{
    AutoCommandBufferBuilder, CommandBufferUsage, PrimaryAutoCommandBuffer, RenderPassBeginInfo,
    SubpassContents,
};
use vulkano::descriptor_set::allocator::StandardDescriptorSetAllocator;
use vulkano::descriptor_set::layout::DescriptorSetLayout;
use vulkano::descriptor_set::{PersistentDescriptorSet, WriteDescriptorSet};
use vulkano::device::Device;
use vulkano::image::ImageViewAbstract;
use vulkano::pipeline::graphics::depth_stencil::DepthStencilState;
use vulkano::pipeline::graphics::input_assembly::InputAssemblyState;
use vulkano::pipeline::graphics::vertex_input::{Vertex, VertexBufferDescription};
use vulkano::pipeline::graphics::viewport::{Viewport, ViewportState};
use vulkano::pipeline::{GraphicsPipeline, Pipeline, PipelineBindPoint};
use vulkano::render_pass::{RenderPass, Subpass};
use vulkano::sampler::Sampler;
use vulkano::shader::ShaderModule;
use vulkano::DeviceSize;

use crate::pipelines::PipelineProvider;
use crate::VertexBuffer;

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
pub struct PBRPipeline {
    vs: Arc<ShaderModule>,
    fs: Arc<ShaderModule>,
    vertex_buffers: Vec<VertexBuffer>,
    normal_buffers: Vec<VertexBuffer>,
    tangent_buffers: Vec<VertexBuffer>,
    uv_buffers: Vec<VertexBuffer>,
    index_buffers: Vec<Subbuffer<[u32]>>,
    write_descriptor_sets: Vec<(u32, Vec<WriteDescriptorSet>)>, // tuples of WriteDescriptorSets and VARIABLE descriptor count, is cleared by init_descriptor_sets function
    descriptor_sets: Vec<Arc<PersistentDescriptorSet>>, // initially empty -> populated by init_descriptor_sets function
    viewport: Viewport,
    render_pass: Arc<RenderPass>,
    device: Arc<Device>,
    vertex_input_state: Vec<VertexBufferDescription>,
}
impl PBRPipeline {
    pub fn new(
        device: Arc<Device>,
        vertex_buffers: Vec<VertexBuffer>,
        normal_buffers: Vec<VertexBuffer>,
        tangent_buffers: Vec<VertexBuffer>,
        uv_buffers: Vec<VertexBuffer>,
        index_buffers: Vec<Subbuffer<[u32]>>,
        camera_buffer: Subbuffer<CameraUniform>,
        textures: impl IntoIterator<Item = (Arc<dyn ImageViewAbstract>, Arc<Sampler>)>
            + ExactSizeIterator,
        material_info_buffers: impl IntoIterator<Item = (Subbuffer<impl ?Sized>, Range<DeviceSize>)>
            + ExactSizeIterator,
        mesh_info_buffers: impl IntoIterator<Item = (Subbuffer<impl ?Sized>, Range<DeviceSize>)>
            + ExactSizeIterator,
        light_buffer: impl IntoIterator<Item = Subbuffer<impl ?Sized>> + ExactSizeIterator,
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
                textures.len() as u32,
                vec![
                    // Level 1: Pipeline-specific uniforms
                    WriteDescriptorSet::image_view_sampler_array(0, 0, textures),
                ],
            ),
            (
                material_info_buffers.len() as u32,
                vec![
                    // Level 2: Pipeline-specific uniforms
                    WriteDescriptorSet::buffer_with_range_array(0, 0, material_info_buffers),
                ],
            ),
            ({
                let m_len = mesh_info_buffers.len() as u32;
                (
                    m_len,
                    vec![
                        // Level 3: Model-specific uniforms
                        WriteDescriptorSet::buffer_with_range_array(0, 0, mesh_info_buffers),
                        // gives error: InvalidBinding { binding: 1 }
                        // WriteDescriptorSet::buffer_array(1,  m_len, light_buffer),
                    ],
                )
            }),
            (
                light_buffer.len() as u32,
                vec![
                    // Level 3: Model-specific uniforms
                    WriteDescriptorSet::buffer_array(0, 0, light_buffer),
                ],
            ),
            // (0, vec![WriteDescriptorSet::buffer(0, light_buffer.0)]),
        ];

        Self {
            vs,
            fs,
            vertex_buffers,
            normal_buffers,
            tangent_buffers,
            uv_buffers,
            index_buffers,
            write_descriptor_sets,
            descriptor_sets: vec![],
            viewport,
            render_pass,
            device,
            vertex_input_state: vec![
                MyVertex::per_vertex(),
                MyNormal::per_vertex(),
                MyTangent::per_vertex(),
                MyUV::per_vertex(),
            ],
        }
    }
}
impl PipelineProvider for PBRPipeline {
    fn get_pipeline(&self) -> Arc<GraphicsPipeline> {
        GraphicsPipeline::start()
            // describes layout of vertex input
            .vertex_input_state(self.vertex_input_state.clone())
            // specify entry point of vertex shader (vulkan shaders can technically have multiple)
            .vertex_shader(self.vs.entry_point("main").unwrap(), ())
            //Indicate type of primitives (default is list of triangles)
            .input_assembly_state(InputAssemblyState::new())
            // Set the *fixed* viewport -> makes it impossible to change viewport for each draw cmd, but increases performance. Need to create new pipeline object if size does change.
            .viewport_state(ViewportState::viewport_fixed_scissor_irrelevant([self
                .viewport
                .clone()]))
            // Specify entry point of fragment shader
            .fragment_shader(self.fs.entry_point("main").unwrap(), ())
            .depth_stencil_state(DepthStencilState::simple_depth_test())
            // This pipeline object concerns the first pass of the render pass
            .render_pass(Subpass::from((self.render_pass).clone(), 0).unwrap())
            .with_auto_layout(self.device.clone(), |x| {
                // textures
                let binding = x[1].bindings.get_mut(&0).unwrap();
                binding.variable_descriptor_count = true;
                binding.descriptor_count = 128; // TODO this is an upper bound to the number of textures, perhaps make it dynamic

                // material info
                let binding = x[2].bindings.get_mut(&0).unwrap();
                binding.variable_descriptor_count = true;
                binding.descriptor_count = 128;

                // MeshInfo
                let binding = x[3].bindings.get_mut(&0).unwrap();
                binding.variable_descriptor_count = true;
                binding.descriptor_count = 128;

                // LightInfo
                let binding = x[4].bindings.get_mut(&0).unwrap();
                binding.variable_descriptor_count = true;
                binding.descriptor_count = 128;
            })
            .unwrap()
    }

    fn set_viewport(&mut self, viewport: Viewport) {
        self.viewport = viewport;
    }

    fn init_descriptor_sets(
        &mut self,
        set_layouts: &[Arc<DescriptorSetLayout>],
        descriptor_set_allocator: &StandardDescriptorSetAllocator,
    ) {
        let mut temp = vec![];
        std::mem::swap(&mut self.write_descriptor_sets, &mut temp);

        for (i, (var_count, write_desc_set)) in temp.into_iter().enumerate() {
            let descriptor_set_layout = set_layouts.get(i).unwrap().clone();
            self.descriptor_sets.push(
                PersistentDescriptorSet::new_variable(
                    descriptor_set_allocator,
                    descriptor_set_layout,
                    var_count,
                    write_desc_set,
                )
                .unwrap(),
            );
        }
    }

    fn render_pass(
        &self,
        builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
        pipeline: Arc<GraphicsPipeline>,
    ) {
        builder.bind_pipeline_graphics(pipeline.clone());

        for i in 0..self.descriptor_sets.len() {
            builder.bind_descriptor_sets(
                PipelineBindPoint::Graphics,
                pipeline.layout().clone(),
                i as u32,
                self.descriptor_sets[i].clone(),
            );
        }
        for i in 0..self.vertex_buffers.len() {
            builder
                .bind_vertex_buffers(
                    0,
                    (
                        self.vertex_buffers[i].subbuffer.clone(),
                        self.normal_buffers[i].subbuffer.clone(),
                        self.tangent_buffers[i].subbuffer.clone(),
                        self.uv_buffers[i].subbuffer.clone(),
                    ),
                )
                .bind_index_buffer(self.index_buffers[i].clone())
                .draw_indexed(self.index_buffers[i].len() as u32, 1, 0, 0, i as u32)
                .unwrap();
        }
    }
}
