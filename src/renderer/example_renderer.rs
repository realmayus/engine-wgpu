use crate::renderer::texture::create_texture_image;
use crate::renderer::{init_renderer, start_renderer};
use std::sync::Arc;
use vulkano::buffer::{Buffer, BufferContents, BufferCreateInfo, BufferUsage};
use vulkano::command_buffer::{AutoCommandBufferBuilder, CommandBufferUsage};
use vulkano::device::Device;
use vulkano::image::view::ImageView;
use vulkano::memory::allocator::{AllocationCreateInfo, MemoryUsage};
use vulkano::pipeline::graphics::input_assembly::InputAssemblyState;
use vulkano::pipeline::graphics::vertex_input::{Vertex, VertexDefinition};
use vulkano::pipeline::graphics::viewport;
use vulkano::pipeline::graphics::viewport::{Viewport, ViewportState};
use vulkano::pipeline::GraphicsPipeline;
use vulkano::render_pass::{RenderPass, Subpass};
use vulkano::sampler::{BorderColor, Sampler, SamplerCreateInfo, SamplerMipmapMode, SamplerReductionMode};
use vulkano::shader::ShaderModule;
use vulkano::sync;
use vulkano::sync::GpuFuture;

#[derive(BufferContents, Vertex)]
#[repr(C)]
struct MyVertex {
    #[format(R32G32_SFLOAT)]
    position: [f32; 2],
    #[format(R32G32B32_SFLOAT)]
    color: [f32; 3],
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
        .input_assembly_state(InputAssemblyState::new()) //Indicate type of primitives (default is list of triangles)
        .viewport_state(ViewportState::viewport_fixed_scissor_irrelevant([viewport])) // Set the *fixed* viewport -> makes it impossible to change viewport for each draw cmd, but increases performance. Need to create new pipeline object if size does change.
        .fragment_shader(fs.entry_point("main").unwrap(), ()) // Specify entry point of fragment shader
        .render_pass(Subpass::from(render_pass, 0).unwrap()) // This pipeline object concerns the first pass of the render pass
        .build(device)
        .unwrap()
}

pub(crate) fn render() {
    let setup_info = init_renderer();

    let mut viewport = Viewport {
        origin: [0.0, 0.0],
        dimensions: setup_info.window.inner_size().into(),
        depth_range: 0.0..1.0,
    };

    let vs = vs::load(setup_info.device.clone()).expect("failed to create shader module");
    let fs = fs::load(setup_info.device.clone()).expect("failed to create shader module");

    let vertex1 = MyVertex {
        position: [-0.5, -0.5],
        color: [1.0, 0.0, 0.0],
    };
    let vertex2 = MyVertex {
        position: [0.0, 0.5],
        color: [0.0, 1.0, 0.0],
    };
    let vertex3 = MyVertex {
        position: [0.5, -0.25],
        color: [0.0, 0.0, 1.0],
    };

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
        vec![vertex1, vertex2, vertex3],
    )
    .expect("Couldn't create vertex buffer");

    let mut cmd_buf_builder = AutoCommandBufferBuilder::primary(
        &setup_info.cmd_buf_allocator,
        setup_info.queue.queue_family_index(),
        CommandBufferUsage::OneTimeSubmit,
    )
    .unwrap();

    let image = create_texture_image(
        &setup_info.memory_allocator,
        setup_info.queue.queue_family_index(),
        &mut cmd_buf_builder,
    );

    let view = ImageView::new_default(image.clone()).unwrap();
    
    Sampler::new(
        setup_info.device.clone(),
        SamplerCreateInfo {
            mag_filter: Filter::Nearest,
            min_filter: Filter::Nearest,
            mipmap_mode: SamplerMipmapMode::Nearest,
            address_mode: [],
            mip_lod_bias: 0.0,
            anisotropy: None,
            compare: None,
            lod: (),
            border_color: BorderColor::FloatTransparentBlack,
            unnormalized_coordinates: false,
            reduction_mode: SamplerReductionMode::WeightedAverage,
            sampler_ycbcr_conversion: None,
            _ne: (),
        }
    )

    let cmd_buf = cmd_buf_builder.build().unwrap();

    let future = sync::now(setup_info.device.clone())
        .then_execute(setup_info.queue.clone(), cmd_buf)
        .unwrap()
        .then_signal_fence_and_flush()
        .unwrap();
    future.wait(None).unwrap();

    start_renderer(
        setup_info,
        viewport,
        vertex_buffer.into_bytes(),
        3,
        vs,
        fs,
        get_pipeline,
    );
}
