use std::sync::Arc;

use anyhow::*;
use wgpu::{Device, Queue, Surface, SurfaceConfiguration, SurfaceError};
use winit::event::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::{Window, WindowBuilder};

use lib::scene::{Texture, VertexInputs, World};

use crate::camera::{Camera, KeyState};
use crate::pipelines::{PipelineProvider, PipelineProviderKind};
use crate::pipelines::pbr_pipeline::PBRPipelineProvider;

pub mod camera;
pub mod pipelines;

pub trait Hook {
    fn setup() -> World;

    fn update(
        &mut self,
        keys: &KeyState,
        delta_time: f32,
    );
}

pub struct RenderInitState {
    pub device: Device,
    surface: Surface,
    surface_config: SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,
    pub window: Window,
    queue: Queue,
    depth_texture: Texture,
    pbr_pipeline: PBRPipelineProvider,
    camera: Camera,
    world: World,
}

impl RenderInitState {
    async fn new(window: Window, hook: impl Hook) -> Self {
        let size = window.inner_size();
        assert_ne!(size.width, 0);
        assert_ne!(size.height, 0);

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });
        // Safety: Surface needs to live as long as the window that created it. State owns window, so this is safe.
        let surface = unsafe { instance.create_surface(&window) }.unwrap();
        // adapter is handle to the graphics card (to get its name, backend etc.)
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .unwrap();

        let (device, queue) = adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: None,
                features: wgpu::Features::empty(),
                limits: wgpu::Limits::default(),
            },
            None,
        );

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .filter(|f| f.is_srgb())
            .next()
            .unwrap_or(surface_caps.formats[0]);
        let surface_config = SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
        };
        surface.configure(&device, &surface_config);
        let depth_texture = Texture::create_depth_texture(&device, &surface_config, "depth_texture");
        let world = hook.setup();

        //TODO create buffers for materials and textures (where do we store them? what if a new model with new textures is loaded?)
        let pipeline = PBRPipelineProvider::new(&device, vec![], vec![], vec![], vec![], vec![], (), 0, 0);
        Self {
            window,
            surface,
            device,
            queue,
            surface_config,
            size,
            depth_texture,
            pbr_pipeline: pipeline,
            camera: Camera::new_default(size.width as f32, size.height as f32, device.clone()),
            world,
        }
    }

    pub fn window(&self) -> &Window {
        &self.window
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.surface_config.width = new_size.width;
            self.surface_config.height = new_size.height;
            self.surface.configure(&self.device, &self.surface_config);
        }
    }

    fn input(&mut self, event: &winit::event::WindowEvent) -> bool {
        false
    }

    fn update(&mut self) {
        todo!();
    }

    fn render(&mut self) -> Result<(), SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });

        {
            self.pbr_pipeline.render_pass(
                &mut encoder,
                &self.world.pbr_meshes().map(|m| VertexInputs::from_mesh(m, &self.device)).collect::<Vec<VertexInputs>>(),
            )
        }
        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();
        Ok(())
    }

}

pub async fn run() {
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new().build(&event_loop).unwrap();

    let mut state = RenderInitState::new(window).await;
    event_loop.run(move |event, _, control_flow| {
        match event {
            Event::WindowEvent {
                ref event,
                window_id,
            } if window_id == state.window().id() => if !state.input(event) {
                match event {
                    WindowEvent::CloseRequested
                    | WindowEvent::KeyboardInput {
                        input:
                        KeyboardInput {
                            state: ElementState::Pressed,
                            virtual_keycode: Some(VirtualKeyCode::Escape),
                            ..
                        },
                        ..
                    } => *control_flow = ControlFlow::Exit,
                    WindowEvent::Resized(physical_size) => {
                        state.resize(*physical_size);
                    }
                    WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                        state.resize(**new_inner_size);
                    }

                    _ => {}
                }
            },
            Event::RedrawRequested(window_id) if window_id == state.window().id() => {
                state.update();
                match state.render() {
                    Ok(_) => {}
                    Err(wgpu::SurfaceError::Lost) => state.resize(state.size),
                    Err(wgpu::SurfaceError::OutOfMemory) => *control_flow = ControlFlow::Exit,
                    Err(e) => eprintln!("{:?}", e),
                }
            }
            Event::MainEventsCleared => {
                state.window().request_redraw();
            }
            _ => {}
        }
    });
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
                    },
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
