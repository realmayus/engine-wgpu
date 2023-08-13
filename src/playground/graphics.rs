use crate::playground::image::image;
use crate::playground::{init, init_cmd_buf};
use image::{ImageBuffer, Rgba};
use vulkano::buffer::{Buffer, BufferContents, BufferCreateInfo, BufferUsage};
use vulkano::command_buffer::{CopyImageToBufferInfo, RenderPassBeginInfo, SubpassContents};
use vulkano::format::Format;
use vulkano::image::view::ImageView;
use vulkano::image::{ImageDimensions, StorageImage};
use vulkano::memory::allocator::{AllocationCreateInfo, MemoryUsage};
use vulkano::pipeline::graphics::input_assembly::InputAssemblyState;
use vulkano::pipeline::graphics::vertex_input::Vertex;
use vulkano::pipeline::graphics::viewport::{Viewport, ViewportState};
use vulkano::pipeline::GraphicsPipeline;
use vulkano::render_pass::{Framebuffer, FramebufferCreateInfo, Subpass};
use vulkano::sync;
use vulkano::sync::GpuFuture;

#[derive(BufferContents, Vertex)]
#[repr(C)]
struct MyVertex {
    #[format(R32G32_SFLOAT)]
    position: [f32; 2],
}

pub fn graphics() {
    let (queue_family_index, device, queue, memory_allocator) = init(None);
    let (cmd_buf_alloc, mut cmd_buf_builder) = init_cmd_buf(device.clone(), queue.clone());

    let v1 = MyVertex {
        position: [-0.5, -0.5],
    };
    let v2 = MyVertex {
        position: [0.0, 0.5],
    };
    let v3 = MyVertex {
        position: [0.5, -0.25],
    };

    let vertex_buf = Buffer::from_iter(
        &memory_allocator,
        BufferCreateInfo {
            usage: BufferUsage::VERTEX_BUFFER,
            ..Default::default()
        },
        AllocationCreateInfo {
            usage: MemoryUsage::Upload,
            ..Default::default()
        },
        vec![v1, v2, v3],
    )
    .expect("Couldn't allocate vertex buffer");

    // Creating a render pass - which is just the format
    let render_pass = vulkano::single_pass_renderpass!(
        device.clone(),
        attachments: {  // RenderPass consists of multiple attachments and passes; here we declare one with the name 'color'
            color: {
                load: Clear, // we want the GPU to clear the image when entering the render pass
                store: Store, // actually store output of our draw commands to image (sometimes content is only relevant inside of a render pass; can use store: DontCare instead)
                format: Format::R8G8B8A8_UNORM,
                samples: 1,
            },
        },
        pass: {
            color: [color],
            depth_stencil: {},
        },
    )
    .unwrap();

    let image = StorageImage::new(
        &memory_allocator,
        ImageDimensions::Dim2d {
            width: 1024,
            height: 1024,
            array_layers: 1, // images can be arrays of layers
        },
        Format::R8G8B8A8_UNORM,
        Some(queue.queue_family_index()),
    )
    .unwrap();

    // now need to specify actual list of attachments -> create framebuffer
    let view = ImageView::new_default(image.clone()).unwrap();
    let framebuffer = Framebuffer::new(
        render_pass.clone(),
        FramebufferCreateInfo {
            attachments: vec![view],
            ..Default::default()
        },
    )
    .unwrap();

    // enter drawing mode by calling `begin_render_pass`
    cmd_buf_builder
        .begin_render_pass(
            RenderPassBeginInfo {
                clear_values: vec![Some([0.0, 0.0, 1.0, 1.0].into())],
                ..RenderPassBeginInfo::framebuffer(framebuffer.clone())
            },
            SubpassContents::Inline, // directly invoke draw commands, don't use secondary cmd buffers
        )
        .unwrap();

    mod vs {
        vulkano_shaders::shader! {
            ty: "vertex",
            path: "src/playground/shaders/vertex.glsl",
        }
    }
    mod fs {
        vulkano_shaders::shader! {
            ty: "fragment",
            path: "src/playground/shaders/fragment.glsl",
        }
    }

    let vs = vs::load(device.clone()).expect("failed to create shader module");
    let fs = fs::load(device.clone()).expect("failed to create shader module");

    let viewport = Viewport {
        // only draw to this specific rectangle, anything outside the viewport will be discarded
        origin: [0.0, 0.0],
        dimensions: [1024.0, 1024.0],
        depth_range: 0.0..1.0,
    };

    let pipeline = GraphicsPipeline::start()
        .vertex_input_state(MyVertex::per_vertex()) // describes layout of vertex input
        .vertex_shader(vs.entry_point("main").unwrap(), ()) // specify entry point of vertex shader (vulkan shaders can technically have multiple)
        .input_assembly_state(InputAssemblyState::new()) //Indicate type of primitives (default is list of triangles)
        .viewport_state(ViewportState::viewport_fixed_scissor_irrelevant([viewport])) // Set the *fixed* viewport -> makes it impossible to change viewport for each draw cmd, but increases performance. Need to create new pipeline object if size does change.
        .fragment_shader(fs.entry_point("main").unwrap(), ()) // Specify entry point of fragment shader
        .render_pass(Subpass::from(render_pass.clone(), 0).unwrap()) // This pipeline object concerns the first pass of the render pass
        .build(device.clone())
        .unwrap();

    cmd_buf_builder
        .bind_pipeline_graphics(pipeline.clone())
        .bind_vertex_buffers(0, vertex_buf.clone())
        .draw(3, 1, 0, 0)
        .unwrap()
        .end_render_pass()
        .unwrap();

    // Create buffer to copy the output to
    let out_buf = Buffer::from_iter(
        &memory_allocator,
        BufferCreateInfo {
            usage: BufferUsage::TRANSFER_DST,
            ..Default::default()
        },
        AllocationCreateInfo {
            usage: MemoryUsage::Download,
            ..Default::default()
        },
        (0..1024 * 1024 * 4).map(|_| 0u8),
    )
    .expect("Failed to create output buffer");

    cmd_buf_builder
        .copy_image_to_buffer(CopyImageToBufferInfo::image_buffer(image, out_buf.clone()))
        .unwrap();

    let command_buffer = cmd_buf_builder.build().unwrap();

    let future = sync::now(device.clone())
        .then_execute(queue.clone(), command_buffer)
        .unwrap()
        .then_signal_fence_and_flush()
        .unwrap();
    future.wait(None).unwrap();

    let buffer_content = out_buf.read().unwrap();
    let image = ImageBuffer::<Rgba<u8>, _>::from_raw(1024, 1024, &buffer_content[..]).unwrap();
    image.save("image.png").unwrap();
}
