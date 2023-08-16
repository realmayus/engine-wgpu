use glam::{Mat4, Vec3};
use renderer::camera::Camera;
use renderer::{init_renderer, start_renderer, VertexBuffer};
use std::sync::Arc;
use systems::io::gltf_loader::load_gltf;
use vulkano::buffer::{Buffer, BufferContents, BufferCreateInfo, BufferUsage};
use vulkano::command_buffer::{AutoCommandBufferBuilder, CommandBufferUsage};
use vulkano::command_buffer::ResourceInCommand::VertexBuffer;
use vulkano::descriptor_set::{PersistentDescriptorSet, WriteDescriptorSet};
use vulkano::device::Device;
use vulkano::image::view::ImageView;
use vulkano::memory::allocator::{AllocationCreateInfo, MemoryUsage};
use vulkano::pipeline::graphics::depth_stencil::DepthStencilState;
use vulkano::pipeline::graphics::input_assembly::{InputAssemblyState, PrimitiveTopology};
use vulkano::pipeline::graphics::vertex_input::{Vertex, VertexDefinition};
use vulkano::pipeline::graphics::viewport;
use vulkano::pipeline::graphics::viewport::{Viewport, ViewportState};
use vulkano::pipeline::{GraphicsPipeline, Pipeline};
use vulkano::render_pass::{RenderPass, Subpass};
use vulkano::sampler::{
    BorderColor, Filter, Sampler, SamplerAddressMode, SamplerCreateInfo, SamplerMipmapMode,
    SamplerReductionMode,
};
use vulkano::shader::ShaderModule;
use vulkano::sync;
use vulkano::sync::GpuFuture;
use lib::scene::Scene;

#[derive(BufferContents, Vertex)]
#[repr(C)]
struct MyVertex {
    #[format(R32G32B32_SFLOAT)]
    position: [f32; 3],
    #[format(R32G32B32_SFLOAT)]
    color: [f32; 3],
}

#[derive(BufferContents, Debug, Default)]
#[repr(C)]
pub struct ModelUniform {
    model: [[f32; 4]; 4],
}

mod vs {
    vulkano_shaders::shader! {
        ty: "vertex",
        path: "assets/shaders/vertex.glsl",
    }
}

mod fs {
    vulkano_shaders::shader! {
        ty: "fragment",
        path: "assets/shaders/fragment.glsl",
    }
}

fn get_pipeline(
    vs: Arc<ShaderModule>,
    fs: Arc<ShaderModule>,
    device: Arc<Device>,
    viewport: Viewport,
    render_pass: Arc<RenderPass>,
) -> Arc<GraphicsPipeline> {
    GraphicsPipeline::start()
        .vertex_input_state(MyVertex::per_vertex()) // describes layout of vertex input
        .vertex_shader(vs.entry_point("main").unwrap(), ()) // specify entry point of vertex shader (vulkan shaders can technically have multiple)
        .input_assembly_state(InputAssemblyState::new().topology(PrimitiveTopology::TriangleList)) //Indicate type of primitives (default is list of triangles)
        .viewport_state(ViewportState::viewport_fixed_scissor_irrelevant([viewport])) // Set the *fixed* viewport -> makes it impossible to change viewport for each draw cmd, but increases performance. Need to create new pipeline object if size does change.
        .fragment_shader(fs.entry_point("main").unwrap(), ()) // Specify entry point of fragment shader
        .depth_stencil_state(DepthStencilState::simple_depth_test())
        .render_pass(Subpass::from(render_pass, 0).unwrap()) // This pipeline object concerns the first pass of the render pass
        .build(device)
        .unwrap()
}

pub fn render(gltf_paths: Vec<&str>) {
    let setup_info = init_renderer();

    let mut viewport = Viewport {
        origin: [0.0, 0.0],
        dimensions: setup_info.window.inner_size().into(),
        depth_range: 0.0..1.0,
    };

    let vs = vs::load(setup_info.device.clone()).expect("failed to create shader module");
    let fs = fs::load(setup_info.device.clone()).expect("failed to create shader module");

    let vertex_buffer = Buffer::from_iter(
        &setup_info.memory_allocator,
        BufferCreateInfo {
            usage: BufferUsage::VERTEX_BUFFER,
            ..Default::default()
        },
        AllocationCreateInfo {
            usage: MemoryUsage::Upload,
            ..Default::default()
        },
        vec![],
    )
    .expect("Couldn't create vertex buffer");
    let view = Mat4::from_cols_array_2d(&[[1.0f32; 4]; 4]);
    view.transform_vector3(Vec3::from((0.0f32, 0.0f32, 0.0f32)));
    let model_uniform = ModelUniform {
        model: view.to_cols_array_2d(),
    };
    let model_buffer = Buffer::from_data(
        &setup_info.memory_allocator,
        BufferCreateInfo {
            usage: BufferUsage::UNIFORM_BUFFER,
            ..Default::default()
        },
        AllocationCreateInfo {
            usage: MemoryUsage::Upload,
            ..Default::default()
        },
        model_uniform,
    )
    .unwrap();

    let mut cmd_buf_builder = AutoCommandBufferBuilder::primary(
        &setup_info.cmd_buf_allocator,
        setup_info.queue.queue_family_index(),
        CommandBufferUsage::OneTimeSubmit,
    )
    .unwrap();

    let scenes: Vec<Scene> = gltf_paths.iter().map(|gltf_path| {
        let (scenes, textures, materials) = load_gltf(
            gltf_path,
            &setup_info.memory_allocator,
            &mut cmd_buf_builder,
        );
        scenes
    }).flatten().collect();

    let vertex_buffers: Vec<VertexBuffer> = vec![];
    for scene in scenes {
        for model in scene.models {
            for mesh in model.meshes {
                mesh.vertices
            }
        }
    }

    let sampler = Sampler::new(
        setup_info.device.clone(),
        SamplerCreateInfo {
            mag_filter: Filter::Linear,
            min_filter: Filter::Linear,
            address_mode: [SamplerAddressMode::Repeat; 3],
            ..Default::default()
        },
    )
    .unwrap();

    let camera = Camera::new_default(
        viewport.dimensions[0],
        viewport.dimensions[1],
        &setup_info.memory_allocator,
    );

    start_renderer(
        setup_info,
        viewport,
        ,
        vs,
        fs,
        get_pipeline,
        vec![
            WriteDescriptorSet::image_view_sampler(0, texture, sampler),
            WriteDescriptorSet::buffer(1, camera.buffer.clone()),
            WriteDescriptorSet::buffer(2, model_buffer.clone()),
        ],
        cmd_buf_builder,
        camera,
    );
}
