use std::collections::VecDeque;
use std::sync::Arc;

use egui_winit_vulkano::Gui;
use log::debug;
use vulkano::buffer::Subbuffer;
use vulkano::command_buffer::allocator::StandardCommandBufferAllocator;
use vulkano::command_buffer::{
    AutoCommandBufferBuilder, CommandBufferUsage, PrimaryAutoCommandBuffer, RenderPassBeginInfo,
    SubpassContents,
};
use vulkano::descriptor_set::allocator::StandardDescriptorSetAllocator;
use vulkano::device::physical::{PhysicalDevice, PhysicalDeviceType};
use vulkano::device::{Device, DeviceExtensions, Queue, QueueFlags};
use vulkano::format::Format;
use vulkano::image::view::ImageView;
use vulkano::image::{AttachmentImage, SwapchainImage};
use vulkano::instance::Instance;
use vulkano::memory::allocator::StandardMemoryAllocator;
use vulkano::pipeline::graphics::viewport::Viewport;
use vulkano::pipeline::GraphicsPipeline;
use vulkano::render_pass::{Framebuffer, FramebufferCreateInfo, RenderPass};
use vulkano::swapchain::{CompositeAlpha, Surface, SurfaceCapabilities, Swapchain};
use winit::dpi::PhysicalSize;
use winit::event_loop::EventLoop;
use winit::window::Window;

use lib::VertexBuffer;

use crate::camera::Camera;
use crate::pipelines::PipelineProvider;

pub mod camera;
pub mod initialization;
pub mod pipelines;
pub mod renderer;

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
    pub render_pass: Arc<RenderPass>,
}

pub struct RenderState {
    pub init_state: RenderInitState,
    pub viewport: Viewport,
    pub cmd_buf_builder: AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
    pub camera: Camera,
}

pub struct PartialRenderState<'a> {
    pub camera: &'a mut Camera,
    pub allocator: &'a StandardMemoryAllocator,
    pub cmd_buf_allocator: &'a StandardCommandBufferAllocator,
    pub queue_family_index: u32,
}

// unified communication interface between renderer lib and implementation, to avoid revealing the scene complexity to the lib
pub trait StateCallable {
    fn setup_gui(&mut self, gui: &mut Gui, render_state: PartialRenderState);
    fn update(&mut self);
    fn cleanup(&self);

    /**
    Should return a copy of vertex buffers, normal buffers, UV buffers, and index buffers for use in render passes
     */
    fn get_subbuffers(
        &mut self,
        memory_allocator: &StandardMemoryAllocator,
    ) -> VecDeque<(
        Vec<VertexBuffer>,
        Vec<VertexBuffer>,
        Vec<VertexBuffer>,
        Vec<Subbuffer<[u32]>>,
    )>;
}

fn select_physical_device(
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
        .unzip()
}

fn get_finalized_render_passes(
    framebuffers: Vec<Arc<Framebuffer>>,
    allocator: &StandardMemoryAllocator,
    cmd_buf_allocator: &StandardCommandBufferAllocator,
    queue_family_index: u32,
    pipeline_providers: &mut [Box<dyn PipelineProvider>],
    pipelines: Vec<Arc<GraphicsPipeline>>,
    callable: &mut dyn StateCallable,
) -> Vec<Arc<PrimaryAutoCommandBuffer>> {
    framebuffers
        .iter()
        .map(|framebuffer| {
            let mut builder = AutoCommandBufferBuilder::primary(
                cmd_buf_allocator,
                queue_family_index,
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
                .unwrap();

            let mut pipeline_subbuffers = callable.get_subbuffers(allocator);

            for i in 0..pipeline_providers.len() {
                debug!("Render pass for pipeline {}", pipeline_providers[i].name());
                let (vertex_buffers, normal_buffers, uv_buffers, index_buffers) =
                    pipeline_subbuffers.pop_front().unwrap();
                pipeline_providers[i].render_pass(
                    &mut builder,
                    pipelines[i].clone(),
                    vertex_buffers,
                    normal_buffers,
                    uv_buffers,
                    index_buffers,
                );
            }

            builder.end_render_pass().unwrap();

            Arc::new(builder.build().unwrap())
        })
        .collect()
}
