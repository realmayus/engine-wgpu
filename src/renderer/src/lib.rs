use std::sync::Arc;

use egui_winit_vulkano::Gui;
use glam::Vec2;
use vulkano::buffer::Subbuffer;
use vulkano::command_buffer::allocator::StandardCommandBufferAllocator;
use vulkano::command_buffer::{AutoCommandBufferBuilder, CommandBufferUsage, PrimaryAutoCommandBuffer, RenderPassBeginInfo, SubpassBeginInfo, SubpassContents, SubpassEndInfo};
use vulkano::descriptor_set::allocator::StandardDescriptorSetAllocator;
use vulkano::device::physical::{PhysicalDevice, PhysicalDeviceType};
use vulkano::device::{Device, DeviceExtensions, Queue, QueueFlags};
use vulkano::format::Format;
use vulkano::image::Image;
use vulkano::image::sampler::Sampler;
use vulkano::image::view::ImageView;
use vulkano::instance::Instance;
use vulkano::memory::allocator::StandardMemoryAllocator;
use vulkano::pipeline::graphics::viewport::Viewport;
use vulkano::render_pass::{Framebuffer, FramebufferCreateInfo, RenderPass};
use vulkano::swapchain::{Surface, Swapchain};
use winit::event_loop::EventLoop;
use winit::window::Window;

use lib::shader_types::{CameraUniform, LightInfo, MaterialInfo, MeshInfo};

use crate::camera::{Camera, KeyState};
use crate::pipelines::{PipelineProvider, PipelineProviderKind};

pub mod camera;
pub mod initialization;
pub mod pipelines;
pub mod render_loop;

pub trait StateCallable {
    fn setup_gui(&mut self, gui: &mut Gui);
    fn update(
        &mut self,
        pipeline_providers: &mut [PipelineProviderKind],
        allocator: Arc<StandardMemoryAllocator>,
        descriptor_set_allocator: Arc<StandardDescriptorSetAllocator>,
        cmd_buf_allocator: Arc<StandardCommandBufferAllocator>,
        queue_family_index: u32,
        device: Arc<Device>,
        viewport: Viewport,
    ) -> Option<Arc<PrimaryAutoCommandBuffer>>;
    fn cleanup(&self);

    fn get_buffers(
        &self,
        device: Arc<Device>,
    ) -> (
        Subbuffer<CameraUniform>,
        Vec<(Arc<ImageView>, Arc<Sampler>)>,
        Vec<Subbuffer<MaterialInfo>>,
        Vec<Subbuffer<MeshInfo>>,
        Vec<Subbuffer<LightInfo>>,
    );

    fn recv_input(
        &mut self,
        keys: &KeyState,
        change: Vec2,
        delta_time: f32,
    );
}

pub struct RenderInitState {
    pub device: Arc<Device>,
    surface: Arc<Surface>,
    event_loop: EventLoop<()>,
    pub window: Arc<Window>,
    pub memory_allocator: Arc<StandardMemoryAllocator>,
    pub queue: Arc<Queue>,
    pub swapchain: Arc<Swapchain>,
    pub images: Vec<Arc<Image>>,
    pub cmd_buf_allocator: Arc<StandardCommandBufferAllocator>,
    pub descriptor_set_allocator: Arc<StandardDescriptorSetAllocator>,
    pub render_pass: Arc<RenderPass>,
    pub image_format: Format,
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
                format: swapchain.image_format(), // set the format the same as the swapchain
                samples: 1,
                load_op: Clear,
                store_op: Store,
            },
            depth: {
                format: Format::D16_UNORM,
                samples: 1,
                load_op: Clear,
                store_op: DontCare,
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
    images: &[Arc<Image>],
    render_pass: &Arc<RenderPass>,
    depth_buffer: Arc<ImageView>,
) -> (Vec<Arc<Framebuffer>>, Vec<Arc<ImageView>>) {
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
    cmd_buf_allocator: &StandardCommandBufferAllocator,
    queue_family_index: u32,
    pipeline_providers: &mut [PipelineProviderKind],
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
                    SubpassBeginInfo {
                        contents: SubpassContents::Inline,
                        ..Default::default()
                    }
                )
                .unwrap();

            for pipeline_provider in &mut *pipeline_providers {
                pipeline_provider.render_pass(&mut builder);
            }

            builder.end_render_pass(SubpassEndInfo::default()).unwrap();

            builder.build().unwrap()
        })
        .collect()
}

pub struct RenderState {
    pub init_state: RenderInitState,
    pub viewport: Viewport,
    pub cmd_buf_builder: AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
}

pub struct PartialRenderState<'a> {
    pub camera: &'a mut Camera,
}
