use std::sync::Arc;

use crate::camera::Camera;
use vulkano::buffer::Subbuffer;
use vulkano::command_buffer::allocator::{
    StandardCommandBufferAllocator, StandardCommandBufferAllocatorCreateInfo,
};
use vulkano::command_buffer::{
    AutoCommandBufferBuilder, CommandBufferUsage, PrimaryAutoCommandBuffer,
    PrimaryCommandBufferAbstract, RenderPassBeginInfo, SubpassContents,
};
use vulkano::descriptor_set::allocator::StandardDescriptorSetAllocator;
use vulkano::descriptor_set::{
    DescriptorSetsCollection, PersistentDescriptorSet, WriteDescriptorSet,
};
use vulkano::device::physical::{PhysicalDevice, PhysicalDeviceType};
use vulkano::device::{
    Device, DeviceCreateInfo, DeviceExtensions, Queue, QueueCreateInfo, QueueFlags,
};
use vulkano::format::Format;
use vulkano::image::view::ImageView;
use vulkano::image::{AttachmentImage, ImageUsage, SwapchainImage};
use vulkano::instance::{Instance, InstanceCreateInfo};
use vulkano::memory::allocator::StandardMemoryAllocator;
use vulkano::pipeline::graphics::vertex_input::VertexBuffersCollection;
use vulkano::pipeline::graphics::viewport::Viewport;
use vulkano::pipeline::{GraphicsPipeline, Pipeline, PipelineBindPoint};
use vulkano::render_pass::{Framebuffer, FramebufferCreateInfo, RenderPass};
use vulkano::shader::ShaderModule;
use vulkano::swapchain::{
    AcquireError, CompositeAlpha, Surface, SurfaceCapabilities, Swapchain, SwapchainCreateInfo,
    SwapchainCreationError, SwapchainPresentInfo,
};
use vulkano::sync::{FlushError, GpuFuture};
use vulkano::{swapchain, sync, VulkanLibrary};
use vulkano_win::VkSurfaceBuild;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::Window;
use winit::window::WindowBuilder;

mod camera;
pub mod example_renderer;
pub mod scene;
pub(crate) mod texture;

pub struct RenderSetupInfo {
    device: Arc<Device>,
    surface: Arc<Surface>,
    caps: SurfaceCapabilities,
    image_format: Format,
    event_loop: EventLoop<()>,
    dimensions: PhysicalSize<u32>,
    composite_alpha: CompositeAlpha,
    window: Arc<Window>,
    memory_allocator: StandardMemoryAllocator,
    queue: Arc<Queue>,
    pub swapchain: Arc<Swapchain>,
    pub images: Vec<Arc<SwapchainImage>>,
    pub cmd_buf_allocator: StandardCommandBufferAllocator,
}

pub fn select_physical_device(
    instance: &Arc<Instance>,
    surface: &Arc<Surface>,
    device_extensions: &DeviceExtensions,
) -> (Arc<PhysicalDevice>, u32) {
    instance
        .enumerate_physical_devices()
        .expect("failed to enumerate physical devices")
        .filter(|p| p.supported_extensions().contains(device_extensions))
        .filter_map(|p| {
            p.queue_family_properties()
                .iter()
                .enumerate()
                .position(|(i, q)| {
                    q.queue_flags.contains(QueueFlags::GRAPHICS)
                        && p.surface_support(i as u32, surface).unwrap_or(false)
                })
                .map(|q| (p, q as u32))
        })
        .min_by_key(|(p, _)| match p.properties().device_type {
            PhysicalDeviceType::DiscreteGpu => 0,
            PhysicalDeviceType::IntegratedGpu => 1,
            PhysicalDeviceType::VirtualGpu => 2,
            PhysicalDeviceType::Cpu => 3,
            _ => 4,
        })
        .expect("no device available")
}

pub fn init_renderer() -> RenderSetupInfo {
    let library = VulkanLibrary::new().expect("No local Vulkan library");
    let required_extensions = vulkano_win::required_extensions(&library);
    let instance = Instance::new(
        library,
        InstanceCreateInfo {
            enabled_extensions: required_extensions,
            ..Default::default()
        },
    )
    .expect("Failed to create instance");

    let event_loop = EventLoop::new();
    let surface = WindowBuilder::new()
        .build_vk_surface(&event_loop, instance.clone())
        .unwrap();
    let window = surface
        .object()
        .unwrap()
        .clone()
        .downcast::<Window>()
        .unwrap();

    window.set_title("Engine Playground");

    let device_extensions = DeviceExtensions {
        khr_swapchain: true,
        ..DeviceExtensions::empty()
    };

    let (physical_device, queue_family_index) =
        select_physical_device(&instance, &surface, &device_extensions);

    println!(
        "Using device: {} (type: {:?})",
        physical_device.properties().device_name,
        physical_device.properties().device_type,
    );

    let (device, mut queues) = Device::new(
        physical_device.clone(),
        DeviceCreateInfo {
            queue_create_infos: vec![QueueCreateInfo {
                queue_family_index,
                ..Default::default()
            }],
            enabled_extensions: device_extensions, // new
            ..Default::default()
        },
    )
    .expect("failed to create device");

    let queue = queues.next().unwrap();

    let caps = physical_device
        .surface_capabilities(&surface, Default::default())
        .expect("failed to get surface capabilities");

    println!(
        "Max swapchain images: {:?}, min: {:?}",
        caps.max_image_count, caps.min_image_count
    );

    let dimensions = window.inner_size();
    let composite_alpha = caps.supported_composite_alpha.into_iter().next().unwrap();
    let image_format = physical_device
        .surface_formats(&surface, Default::default())
        .unwrap()[0]
        .0;

    // Create the swapchain
    let (mut swapchain, images) = {
        Swapchain::new(
            device.clone(),
            surface.clone(),
            SwapchainCreateInfo {
                min_image_count: caps.min_image_count,
                image_format: Some(image_format),
                image_extent: dimensions.into(),
                image_usage: ImageUsage::COLOR_ATTACHMENT,
                composite_alpha: composite_alpha,
                ..Default::default()
            },
        )
        .unwrap()
    };
    let cmd_buf_allocator = StandardCommandBufferAllocator::new(
        device.clone(),
        StandardCommandBufferAllocatorCreateInfo::default(),
    );

    let memory_allocator = StandardMemoryAllocator::new_default(device.clone());

    RenderSetupInfo {
        device,
        surface,
        caps,
        image_format,
        event_loop,
        dimensions,
        composite_alpha,
        window,
        memory_allocator,
        queue,
        swapchain,
        images,
        cmd_buf_allocator,
    }
}

fn get_render_pass(device: Arc<Device>, swapchain: &Arc<Swapchain>) -> Arc<RenderPass> {
    vulkano::single_pass_renderpass!(
        device,
        attachments: {
            color: {
                load: Clear,
                store: Store,
                format: swapchain.image_format(), // set the format the same as the swapchain
                samples: 1,
            },
            depth: {
                load: Clear,
                store: DontCare,
                format: Format::D16_UNORM,
                samples: 1,
            }
        },
        pass: {
            color: [color],
            depth_stencil: {depth},
        },
    )
    .unwrap()
}

fn get_framebuffers(
    images: &[Arc<SwapchainImage>],
    render_pass: &Arc<RenderPass>,
    depth_buffer: Arc<ImageView<AttachmentImage>>,
) -> Vec<Arc<Framebuffer>> {
    images
        .iter()
        .map(|image| {
            let view = ImageView::new_default(image.clone()).unwrap();
            Framebuffer::new(
                render_pass.clone(),
                FramebufferCreateInfo {
                    attachments: vec![view, depth_buffer.clone()],
                    ..Default::default()
                },
            )
            .unwrap()
        })
        .collect::<Vec<_>>()
}

fn get_command_buffers(
    // We now have one command buffer for each framebuffer
    device: &Arc<Device>,
    queue: &Arc<Queue>,
    pipeline: &Arc<GraphicsPipeline>,
    framebuffers: &Vec<Arc<Framebuffer>>,
    vertex_buffer: &Subbuffer<[u8]>,
    vertex_count: u32,
    cmd_buf_allocator: &StandardCommandBufferAllocator,
    descriptor_sets: Arc<PersistentDescriptorSet>,
) -> Vec<Arc<PrimaryAutoCommandBuffer>> {
    framebuffers
        .iter()
        .map(|framebuffer| {
            let mut builder = AutoCommandBufferBuilder::primary(
                cmd_buf_allocator,
                queue.queue_family_index(),
                CommandBufferUsage::MultipleSubmit, // don't forget to write the correct buffer usage
            )
            .unwrap();

            builder
                .begin_render_pass(
                    RenderPassBeginInfo {
                        clear_values: vec![Some([0.1, 0.1, 0.1, 1.0].into()), Some(1f32.into())],
                        ..RenderPassBeginInfo::framebuffer(framebuffer.clone())
                    },
                    SubpassContents::Inline,
                )
                .unwrap()
                .bind_pipeline_graphics(pipeline.clone())
                .bind_descriptor_sets(
                    PipelineBindPoint::Graphics,
                    pipeline.layout().clone(),
                    0,
                    descriptor_sets.clone(),
                )
                .bind_vertex_buffers(0, vertex_buffer.clone())
                .draw(vertex_count, 1, 0, 0)
                .unwrap()
                .end_render_pass()
                .unwrap();

            Arc::new(builder.build().unwrap())
        })
        .collect()
}

pub fn start_renderer(
    mut setup_info: RenderSetupInfo,
    mut viewport: Viewport,
    vertex_buffer: Subbuffer<[u8]>,
    vertex_count: u32,
    vs: Arc<ShaderModule>,
    fs: Arc<ShaderModule>,
    get_pipeline: fn(
        vs: Arc<ShaderModule>,
        fs: Arc<ShaderModule>,
        device: Arc<Device>,
        viewport: Viewport,
        render_pass: Arc<RenderPass>,
    ) -> Arc<GraphicsPipeline>,
    write_descriptor_sets: Vec<WriteDescriptorSet>,
    cmd_buf_builder: AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
    mut camera: Camera,
) {
    let render_pass = get_render_pass(setup_info.device.clone(), &setup_info.swapchain);
    let descriptor_set_allocator = StandardDescriptorSetAllocator::new(setup_info.device.clone());

    let depth_buffer = ImageView::new_default(
        AttachmentImage::transient(
            &setup_info.memory_allocator,
            [viewport.dimensions[0] as u32, viewport.dimensions[1] as u32],
            Format::D16_UNORM,
        )
        .unwrap(),
    )
    .unwrap();

    let framebuffers = get_framebuffers(&setup_info.images, &render_pass, depth_buffer);

    let pipeline = get_pipeline(
        vs.clone(),
        fs.clone(),
        setup_info.device.clone(),
        viewport.clone(),
        render_pass.clone(),
    );

    let layout = pipeline.layout().set_layouts().get(0).unwrap();

    let set = PersistentDescriptorSet::new(
        &descriptor_set_allocator,
        layout.clone(),
        write_descriptor_sets,
    )
    .unwrap();

    let mut command_buffers = get_command_buffers(
        &setup_info.device.clone(),
        &setup_info.queue,
        &pipeline,
        &framebuffers,
        &vertex_buffer,
        vertex_count,
        &setup_info.cmd_buf_allocator,
        set.clone(),
    );

    let mut window_resized = false;
    let mut recreate_swapchain = false;

    let cmd_buf = cmd_buf_builder.build().unwrap();

    let future = sync::now(setup_info.device.clone())
        .then_execute(setup_info.queue.clone(), cmd_buf)
        .unwrap()
        .then_signal_fence_and_flush()
        .unwrap();
    future.wait(None).unwrap();

    let mut is_left_pressed = false;
    let mut is_right_pressed = false;
    let mut is_up_pressed = false;
    let mut is_down_pressed = false;

    // blocks main thread forever and calls closure whenever the event loop receives an event
    setup_info
        .event_loop
        .run(move |event, _, control_flow| match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                *control_flow = ControlFlow::Exit;
            }
            Event::WindowEvent {
                event: WindowEvent::Resized(_),
                ..
            } => {
                window_resized = true;
            }
            Event::WindowEvent {
                event:
                    WindowEvent::KeyboardInput {
                        input:
                            KeyboardInput {
                                state,
                                virtual_keycode: Some(keycode),
                                ..
                            },
                        ..
                    },
                ..
            } => {
                let is_pressed = state == ElementState::Pressed;
                match keycode {
                    VirtualKeyCode::W | VirtualKeyCode::Up => {
                        is_up_pressed = is_pressed;
                    }
                    VirtualKeyCode::A | VirtualKeyCode::Left => {
                        is_left_pressed = is_pressed;
                    }
                    VirtualKeyCode::S | VirtualKeyCode::Down => {
                        is_down_pressed = is_pressed;
                    }
                    VirtualKeyCode::D | VirtualKeyCode::Right => {
                        is_right_pressed = is_pressed;
                    }
                    _ => {}
                }
            }
            Event::MainEventsCleared => {
                let forward = camera.target - camera.eye;
                let forward_norm = forward.normalize();
                let forward_mag = forward.length();

                if is_up_pressed && forward_mag > 5.0 {
                    camera.eye += forward_norm * 5.0;
                }
                if is_down_pressed {
                    camera.eye -= forward_norm * 5.0;
                }

                let right = forward_norm.cross(camera.up);

                let forward = camera.target - camera.eye;
                let forward_mag = forward.length();

                if is_right_pressed {
                    camera.eye = camera.target - (forward - right * 5.0).normalize() * forward_mag;
                }
                if is_left_pressed {
                    camera.eye = camera.target - (forward + right * 5.0).normalize() * forward_mag;
                }
            }
            Event::RedrawEventsCleared => {
                // TODO: Optimization: Implement Frames in Flight
                if window_resized || recreate_swapchain {
                    recreate_swapchain = false;

                    let new_dimensions = setup_info.window.inner_size();

                    let (new_swapchain, new_images) =
                        match setup_info.swapchain.recreate(SwapchainCreateInfo {
                            image_extent: new_dimensions.into(),
                            ..setup_info.swapchain.create_info()
                        }) {
                            Ok(r) => r,
                            Err(SwapchainCreationError::ImageExtentNotSupported { .. }) => return,
                            Err(e) => panic!("failed to recreate swapchain: {e}"),
                        };
                    setup_info.swapchain = new_swapchain;
                    let depth_buffer = ImageView::new_default(
                        AttachmentImage::transient(
                            &setup_info.memory_allocator,
                            new_dimensions.into(),
                            Format::D16_UNORM,
                        )
                        .unwrap(),
                    )
                    .unwrap();
                    let new_framebuffers =
                        get_framebuffers(&new_images, &render_pass, depth_buffer.clone());

                    if window_resized {
                        window_resized = false;

                        viewport.dimensions = new_dimensions.into();
                        let new_pipeline = get_pipeline(
                            vs.clone(),
                            fs.clone(),
                            setup_info.device.clone(),
                            viewport.clone(),
                            render_pass.clone(),
                        );
                        command_buffers = get_command_buffers(
                            &setup_info.device.clone(),
                            &setup_info.queue,
                            &new_pipeline,
                            &new_framebuffers,
                            &vertex_buffer,
                            vertex_count,
                            &setup_info.cmd_buf_allocator,
                            set.clone(),
                        );
                    }
                }

                // acquire_next_image gives us the image index on which we are allowed to draw and a future indicating when the GPU will gain access to that image
                // suboptimal: the acquired image is still usable, but the swapchain should be recreated as the surface's properties no longer match the swapchain.
                let (image_i, suboptimal, acquire_future) =
                    match swapchain::acquire_next_image(setup_info.swapchain.clone(), None) {
                        Ok(r) => r,
                        Err(AcquireError::OutOfDate) => {
                            recreate_swapchain = true;
                            return;
                        }
                        Err(e) => panic!("Failed to acquire next image: {e}"),
                    };
                if suboptimal {
                    recreate_swapchain = true;
                }
                acquire_future.wait(None).unwrap();
                camera.update_view();
                // Create future that is to be submitted to the GPU:
                let execution = sync::now(setup_info.device.clone())
                    .join(acquire_future) // cmd buf can't be executed immediately, as it needs to wait for the image to actually become available
                    .then_execute(
                        setup_info.queue.clone(),
                        command_buffers[image_i as usize].clone(),
                    ) // execute cmd buf which is selected based on image index
                    .unwrap()
                    .then_swapchain_present(
                        // tell the swapchain that we finished drawing and the image is ready for display
                        setup_info.queue.clone(),
                        SwapchainPresentInfo::swapchain_image_index(
                            setup_info.swapchain.clone(),
                            image_i,
                        ),
                    )
                    .then_signal_fence_and_flush();

                match execution {
                    Ok(future) => future.wait(None).unwrap(),
                    Err(FlushError::OutOfDate) => {
                        recreate_swapchain = true;
                    }
                    Err(e) => {
                        println!("Failed to flush future: {e}");
                    }
                }
            }
            _ => {}
        });
}
