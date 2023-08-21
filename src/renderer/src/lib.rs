use std::fmt::Display;
use std::sync::Arc;

use egui_winit_vulkano::{Gui, GuiConfig};
use lib::Dirtyable;
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
    Device, DeviceCreateInfo, DeviceExtensions, Features, Queue, QueueCreateInfo, QueueFlags,
};
use vulkano::format::Format;
use vulkano::image::view::ImageView;
use vulkano::image::{AttachmentImage, ImageAccess, ImageUsage, SwapchainImage};
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

use crate::camera::Camera;

pub mod camera;

pub struct VertexBuffer {
    pub subbuffer: Subbuffer<[u8]>,
    pub vertex_count: u32,
}

pub trait StateCallable {
    fn setup_gui(&mut self, gui: &mut Gui, render_state: PartialRenderState);
    fn update(&mut self);
}

pub struct RenderInitState {
    pub device: Arc<Device>,
    surface: Arc<Surface>,
    caps: SurfaceCapabilities,
    image_format: Format,
    event_loop: EventLoop<()>,
    dimensions: PhysicalSize<u32>,
    composite_alpha: CompositeAlpha,
    pub window: Arc<Window>,
    pub memory_allocator: Arc<StandardMemoryAllocator>,
    pub queue: Arc<Queue>,
    pub swapchain: Arc<Swapchain>,
    pub images: Vec<Arc<SwapchainImage>>,
    pub cmd_buf_allocator: StandardCommandBufferAllocator,
    pub descriptor_set_allocator: StandardDescriptorSetAllocator,
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

pub fn init_renderer() -> RenderInitState {
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

    window.set_title("Engine Playground - Press ESC to release controls");

    let device_extensions = DeviceExtensions {
        khr_swapchain: true,
        ext_descriptor_indexing: true, // required for textures
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
            enabled_features: Features {
                descriptor_indexing: true,
                descriptor_binding_variable_descriptor_count: true,
                shader_sampled_image_array_non_uniform_indexing: true,
                shader_uniform_buffer_array_non_uniform_indexing: true,
                shader_storage_buffer_array_dynamic_indexing: true,
                shader_storage_buffer_array_non_uniform_indexing: true,
                runtime_descriptor_array: true,
                ..Features::empty()
            },
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
    let descriptor_set_allocator = StandardDescriptorSetAllocator::new(device.clone());

    RenderInitState {
        device,
        surface,
        caps,
        image_format,
        event_loop,
        dimensions,
        composite_alpha,
        window,
        memory_allocator: Arc::new(memory_allocator),
        queue,
        swapchain,
        images,
        cmd_buf_allocator,
        descriptor_set_allocator,
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
) -> (Vec<Arc<Framebuffer>>, Vec<Arc<ImageView<SwapchainImage>>>) {
    images
        .iter()
        .map(|image| {
            let view = ImageView::new_default(image.clone()).unwrap();
            (
                Framebuffer::new(
                    render_pass.clone(),
                    FramebufferCreateInfo {
                        attachments: vec![view.clone(), depth_buffer.clone()],
                        ..Default::default()
                    },
                )
                .unwrap(),
                view.clone(),
            )
        })
        .into_iter()
        .unzip()
}

fn get_command_buffers(
    // We now have one command buffer for each framebuffer
    queue: &Arc<Queue>,
    pipeline: &Arc<GraphicsPipeline>,
    framebuffers: &Vec<Arc<Framebuffer>>,
    vertex_buffers: &Vec<VertexBuffer>,
    normal_buffers: &Vec<VertexBuffer>,
    uv_buffers: &Vec<VertexBuffer>,
    index_buffers: &Vec<Subbuffer<[u32]>>,
    cmd_buf_allocator: &StandardCommandBufferAllocator,
    descriptor_sets_0: Arc<PersistentDescriptorSet>,
    descriptor_sets_1: Arc<PersistentDescriptorSet>,
    descriptor_sets_2: Arc<PersistentDescriptorSet>,
    descriptor_sets_3: Arc<PersistentDescriptorSet>,
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
                    descriptor_sets_0.clone(),
                )
                .bind_descriptor_sets(
                    PipelineBindPoint::Graphics,
                    pipeline.layout().clone(),
                    1,
                    descriptor_sets_1.clone(),
                )
                .bind_descriptor_sets(
                    PipelineBindPoint::Graphics,
                    pipeline.layout().clone(),
                    2,
                    descriptor_sets_2.clone(),
                )
                .bind_descriptor_sets(
                    PipelineBindPoint::Graphics,
                    pipeline.layout().clone(),
                    3,
                    descriptor_sets_3.clone(),
                );

            assert_eq!(vertex_buffers.len(), normal_buffers.len()); // TODO optimization: remove asserts
            assert_eq!(vertex_buffers.len(), index_buffers.len());
            for i in 0..vertex_buffers.len() {
                assert_eq!(
                    vertex_buffers[i].vertex_count,
                    normal_buffers[i].vertex_count
                );
                builder
                    .bind_vertex_buffers(
                        0,
                        (
                            vertex_buffers[i].subbuffer.clone(),
                            normal_buffers[i].subbuffer.clone(),
                            uv_buffers[i].subbuffer.clone(),
                        ),
                    )
                    .bind_index_buffer(index_buffers[i].clone())
                    .draw_indexed(index_buffers[i].len() as u32, 1, 0, 0, i as u32)
                    .unwrap();
            }

            builder.end_render_pass().unwrap();

            Arc::new(builder.build().unwrap())
        })
        .collect()
}

pub struct RenderState {
    pub init_state: RenderInitState,
    pub viewport: Viewport,
    pub vertex_buffers: Vec<VertexBuffer>,
    pub normal_buffers: Vec<VertexBuffer>,
    pub uv_buffers: Vec<VertexBuffer>,
    pub index_buffers: Vec<Subbuffer<[u32]>>,
    pub vs: Arc<ShaderModule>,
    pub fs: Arc<ShaderModule>,
    pub get_pipeline: fn(
        vs: Arc<ShaderModule>,
        fs: Arc<ShaderModule>,
        device: Arc<Device>,
        viewport: Viewport,
        render_pass: Arc<RenderPass>,
    ) -> Arc<GraphicsPipeline>,
    pub write_descriptor_sets_0: Vec<WriteDescriptorSet>,
    pub write_descriptor_sets_1: Vec<WriteDescriptorSet>,
    pub descriptor_len_1: usize,
    pub write_descriptor_sets_2: Vec<WriteDescriptorSet>,
    pub descriptor_len_2: usize,
    pub write_descriptor_sets_3: Vec<WriteDescriptorSet>,
    pub descriptor_len_3: usize,
    pub cmd_buf_builder: AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
    pub camera: Camera,
}

pub struct PartialRenderState<'a> {
    pub camera: &'a mut Camera,
}

pub fn start_renderer<'a>(mut state: RenderState, mut callable: impl StateCallable + 'static) {
    let render_pass = get_render_pass(state.init_state.device.clone(), &state.init_state.swapchain);
    println!(
        "Viewport dimensions: x={} y={}",
        state.viewport.dimensions[0] as u32, state.viewport.dimensions[1] as u32
    );
    let depth_buffer = ImageView::new_default(
        AttachmentImage::transient(
            &state.init_state.memory_allocator,
            [
                state.viewport.dimensions[0] as u32,
                state.viewport.dimensions[1] as u32,
            ],
            Format::D16_UNORM,
        )
        .unwrap(),
    )
    .unwrap();

    let (framebuffers, mut image_views) =
        get_framebuffers(&state.init_state.images, &render_pass, depth_buffer);

    let pipeline = (state.get_pipeline)(
        state.vs.clone(),
        state.fs.clone(),
        state.init_state.device.clone(),
        state.viewport.clone(),
        render_pass.clone(),
    );

    let layout = pipeline.layout().set_layouts().get(0).unwrap();

    let set_0 = PersistentDescriptorSet::new(
        &state.init_state.descriptor_set_allocator,
        layout.clone(),
        state.write_descriptor_sets_0,
    )
    .unwrap();

    let layout = pipeline.layout().set_layouts().get(1).unwrap();

    let set_1 = PersistentDescriptorSet::new_variable(
        &state.init_state.descriptor_set_allocator,
        layout.clone(),
        state.descriptor_len_1 as u32, //TODO this somehow is an upper bound as well?
        state.write_descriptor_sets_1,
    )
    .unwrap();

    let layout = pipeline.layout().set_layouts().get(2).unwrap();

    let set_2 = PersistentDescriptorSet::new_variable(
        &state.init_state.descriptor_set_allocator,
        layout.clone(),
        state.descriptor_len_2 as u32,
        state.write_descriptor_sets_2,
    )
    .unwrap();

    let layout = pipeline.layout().set_layouts().get(3).unwrap();

    let set_3 = PersistentDescriptorSet::new_variable(
        &state.init_state.descriptor_set_allocator,
        layout.clone(),
        state.descriptor_len_3 as u32,
        state.write_descriptor_sets_3,
    )
    .unwrap();

    let mut command_buffers = get_command_buffers(
        &state.init_state.queue,
        &pipeline,
        &framebuffers,
        &state.vertex_buffers,
        &state.normal_buffers,
        &state.uv_buffers,
        &state.index_buffers,
        &state.init_state.cmd_buf_allocator,
        set_0.clone(),
        set_1.clone(),
        set_2.clone(),
        set_3.clone(),
    );

    let mut window_resized = false;
    let mut recreate_swapchain = false;

    let cmd_buf = state.cmd_buf_builder.build().unwrap();

    let future = sync::now(state.init_state.device.clone())
        .then_execute(state.init_state.queue.clone(), cmd_buf)
        .unwrap()
        .then_signal_fence_and_flush()
        .unwrap();
    future.wait(None).unwrap();

    let mut gui = Gui::new(
        &state.init_state.event_loop,
        state.init_state.surface,
        state.init_state.queue.clone(),
        GuiConfig {
            is_overlay: true,
            ..Default::default()
        },
    );

    let mut is_left_pressed = false;
    let mut is_right_pressed = false;
    let mut is_up_pressed = false;
    let mut is_down_pressed = false;
    let mut gui_catch = false;
    let event_loop = state.init_state.event_loop;
    // blocks main thread forever and calls closure whenever the event loop receives an event

    event_loop.run(move |event, _, control_flow| match event {
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
        } if !gui_catch && keycode != VirtualKeyCode::Escape => {
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
        Event::WindowEvent {
            event:
                WindowEvent::KeyboardInput {
                    input:
                        KeyboardInput {
                            state: key_state,
                            virtual_keycode: Some(keycode),
                            ..
                        },
                    ..
                },
            ..
        } if keycode == VirtualKeyCode::Escape => {
            if key_state == ElementState::Released {
                gui_catch = !gui_catch;
                if gui_catch {
                    state.init_state.window.set_title("Engine Playground");
                } else {
                    state
                        .init_state
                        .window
                        .set_title("Engine Playground - Press ESC to release controls");
                }
                println!(
                    "Gui catch is now: {}",
                    if gui_catch { "enabled" } else { "disabled" }
                );
            }
        }
        Event::WindowEvent { event, .. } => {
            gui.update(&event);
        }
        Event::MainEventsCleared => {
            state.camera.recv_input(
                is_up_pressed,
                is_down_pressed,
                is_left_pressed,
                is_right_pressed,
            );
        }
        Event::RedrawEventsCleared => {
            // TODO: Optimization: Implement Frames in Flight
            if window_resized || recreate_swapchain {
                recreate_swapchain = false;

                let new_dimensions = state.init_state.window.inner_size();

                let (new_swapchain, new_images) =
                    match state.init_state.swapchain.recreate(SwapchainCreateInfo {
                        image_extent: new_dimensions.into(),
                        ..state.init_state.swapchain.create_info()
                    }) {
                        Ok(r) => r,
                        Err(SwapchainCreationError::ImageExtentNotSupported { .. }) => return,
                        Err(e) => panic!("failed to recreate swapchain: {e}"),
                    };
                state.init_state.swapchain = new_swapchain;
                let depth_buffer = ImageView::new_default(
                    AttachmentImage::transient(
                        &state.init_state.memory_allocator,
                        new_dimensions.into(),
                        Format::D16_UNORM,
                    )
                    .unwrap(),
                )
                .unwrap();
                let (new_framebuffers, new_image_views) =
                    get_framebuffers(&new_images, &render_pass, depth_buffer.clone());
                image_views = new_image_views;
                if window_resized {
                    window_resized = false;

                    state.viewport.dimensions = new_dimensions.into();
                    let new_pipeline = (state.get_pipeline)(
                        state.vs.clone(),
                        state.fs.clone(),
                        state.init_state.device.clone(),
                        state.viewport.clone(),
                        render_pass.clone(),
                    );
                    command_buffers = get_command_buffers(
                        &state.init_state.queue,
                        &new_pipeline,
                        &new_framebuffers,
                        &state.vertex_buffers,
                        &state.normal_buffers,
                        &state.uv_buffers,
                        &state.index_buffers,
                        &state.init_state.cmd_buf_allocator,
                        set_0.clone(),
                        set_1.clone(),
                        set_2.clone(),
                        set_3.clone(),
                    );
                    state
                        .camera
                        .update_aspect(state.viewport.dimensions[0], state.viewport.dimensions[1]);
                }
            }

            gui.immediate_ui(|gui| {
                callable.setup_gui(
                    gui,
                    PartialRenderState {
                        camera: &mut state.camera,
                    },
                )
            });

            // acquire_next_image gives us the image index on which we are allowed to draw and a future indicating when the GPU will gain access to that image
            // suboptimal: the acquired image is still usable, but the swapchain should be recreated as the surface's properties no longer match the swapchain.
            let (image_i, suboptimal, acquire_future) =
                match swapchain::acquire_next_image(state.init_state.swapchain.clone(), None) {
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
            state.camera.update_view(); // TODO optimization: only update camera uniform if dirty
            callable.update();
            let main_drawings = sync::now(state.init_state.device.clone())
                .join(acquire_future) // cmd buf can't be executed immediately, as it needs to wait for the image to actually become available
                .then_execute(
                    state.init_state.queue.clone(),
                    command_buffers[image_i as usize].clone(),
                ) // execute cmd buf which is selected based on image index
                .unwrap();

            let after_egui =
                gui.draw_on_image(main_drawings, image_views[image_i as usize].clone());

            let present = after_egui
                .then_swapchain_present(
                    // tell the swapchain that we finished drawing and the image is ready for display
                    state.init_state.queue.clone(),
                    SwapchainPresentInfo::swapchain_image_index(
                        state.init_state.swapchain.clone(),
                        image_i,
                    ),
                )
                .then_signal_fence_and_flush();

            match present {
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
