use std::path::Path;
use anyhow::Result;
use wgpu::{Device, Queue, Surface, SurfaceConfiguration, SurfaceError};
use winit::event::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::{Window, WindowBuilder};

use lib::scene::{World};
use lib::texture::{Texture};
use systems::io::gltf_loader::load_gltf;
use crate::camera::{Camera, KeyState};
use crate::pipelines::pbr_pipeline::PBRPipelineProvider;

pub mod camera;
pub mod pipelines;

pub trait Hook {
    fn setup(&self, state: &mut RenderState);

    fn update(
        &mut self,
        keys: &KeyState,
        delta_time: f32,
    );
}

pub struct RenderState<'a> {
    pub device: Device,
    surface: Surface,
    surface_config: SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,
    pub window: Window,
    queue: Queue,
    depth_texture: Texture,
    pbr_pipeline: PBRPipelineProvider,
    camera: Camera,
    world: World<'a>,
}

impl<'a> RenderState<'a> {
    async fn new(window: Window) -> Self {
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
        ).await.unwrap();

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
        let world = World::default();

        let camera = Camera::new_default(size.width as f32, size.height as f32, &device);
        //TODO create buffers for materials and textures (where do we store them? what if a new model with new textures is loaded?)
        let pipeline = PBRPipelineProvider::new(&device, &[], &[], &camera.buffer);
        Self {
            window,
            surface,
            device,
            queue,
            surface_config,
            size,
            depth_texture,
            pbr_pipeline: pipeline,
            camera,
            world,
        }
    }
    pub fn load_default_scene(&'a mut self, device: &Device, queue: &Queue) {
        let mut scenes = load_gltf(Path::new("../../../assets/models/cube.glb"), device, queue, &self.pbr_pipeline.tex_bind_group_layout, &mut self.world.textures, &mut self.world.materials);
        let first = scenes.remove(0);
        self.world.scenes.push(first);
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
            self.pbr_pipeline.render_meshes(
                &mut encoder,
                &view,
                &self.world.pbr_meshes().collect::<Vec<_>>(),
            )
        }
        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();
        Ok(())
    }

}

pub async fn run(hook: impl Hook + 'static) {
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new().build(&event_loop).unwrap();

    let mut state = RenderState::new(window).await;
    hook.setup(&mut state);
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