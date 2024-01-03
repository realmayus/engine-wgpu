use anyhow::Result;
use std::path::Path;
use std::time::Instant;
use glam::Vec2;
use wgpu::{Device, Limits, Queue, Surface, SurfaceConfiguration, SurfaceError};
use winit::event::{ElementState, Event, KeyboardInput, MouseButton, VirtualKeyCode, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::{Window, WindowBuilder};

use crate::camera::{Camera, KeyState};
use crate::pipelines::pbr_pipeline::PBRPipelineProvider;
use lib::scene::{MaterialManager, TextureManager, World};
use lib::texture::Texture;
use systems::io::gltf_loader::load_gltf;

pub mod camera;
pub mod pipelines;

pub trait Hook {
    fn setup<'a>(&self, world: &'a mut World, data: SetupData);

    fn update(&mut self, keys: &KeyState, delta_time: f32);
}

pub struct SetupData<'a> {
    pub tex_bind_group_layout: &'a wgpu::BindGroupLayout,
    pub device: &'a Device,
    pub queue: &'a Queue,
}

impl SetupData<'_> {
    pub fn load_default_scene(&self, world: &mut World) {
        let mut scenes = load_gltf(
            Path::new("assets/models/cube_light_tan.glb"),
            self.device,
            self.queue,
            self.tex_bind_group_layout,
            &mut world.textures,
            &mut world.materials,
        );
        let first = scenes.remove(0);
        world.scenes.push(first);
    }
}

pub struct RenderState {
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
    hook: Box<dyn Hook>,
}

impl RenderState {
    async fn new(window: Window, hook: impl Hook + 'static) -> Self {
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

        let limits = Limits {
            max_storage_buffers_per_shader_stage: 1_000,
            max_bind_groups: 5,
            ..Default::default()
        };

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    features: wgpu::Features::BUFFER_BINDING_ARRAY
                        | wgpu::Features::STORAGE_RESOURCE_BINDING_ARRAY
                        | wgpu::Features::PARTIALLY_BOUND_BINDING_ARRAY,
                    limits,
                },
                None,
            )
            .await
            .unwrap();

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
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
        let depth_texture =
            Texture::create_depth_texture(&device, &surface_config, "depth_texture");
        let world = World {
            scenes: vec![],
            active_scene: 0,
            materials: MaterialManager::new(&device),
            textures: TextureManager::new(&device, &queue),
        };

        let camera = Camera::new_default(size.width as f32, size.height as f32, &device);
        //TODO create buffers for materials and textures (where do we store them? what if a new model with new textures is loaded?)
        let mut pipeline = PBRPipelineProvider::new(&device, &[], &[], &[], &camera.buffer);
        pipeline.create_pipeline(&device);
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
            hook: Box::from(hook),
        }
    }

    fn setup(&mut self) {
        self.hook.setup(&mut self.world, SetupData {
            tex_bind_group_layout: &self.pbr_pipeline.tex_bind_group_layout,
            device: &self.device,
            queue: &self.queue,
        });
        self.pbr_pipeline.update_mesh_bind_group(&self.device, self.world.pbr_meshes());
        self.world.materials.update_dirty(&self.queue);
        self.pbr_pipeline.update_mat_bind_group(&self.device, &self.world.materials);
        self.camera.update_view(&self.queue);
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

    fn update(&mut self, keys: &KeyState, delta_time: f32, cursor_delta: Vec2) {
        self.hook.update(keys, delta_time);
        self.camera.recv_input(keys, cursor_delta, delta_time);
        self.camera.update_view(&self.queue);
    }

    fn render(&mut self) -> Result<(), SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        {
            self.pbr_pipeline.render_meshes(
                &mut encoder,
                &view,
                &self.world.pbr_meshes().collect::<Vec<_>>(),
                &self.world.materials,
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

    let mut state = RenderState::new(window, hook).await;
    let mut keys = KeyState::default();
    let mut cursor_pos = Vec2::default();
    let mut cursor_delta = Vec2::default();
    let mut delta_time = 0.0;
    state.setup();
    event_loop.run(move |event, _, control_flow| {
        match event {
            Event::WindowEvent {
                ref event,
                window_id,
            } if window_id == state.window().id() => {
                if !state.input(event) {
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
                        WindowEvent::KeyboardInput { input: KeyboardInput {
                            state,
                            virtual_keycode: Some(keycode),
                            ..
                        },  .. } => {
                            keys.update_keys(*keycode, *state);
                        }
                        WindowEvent::ModifiersChanged(state) => keys.set_modifiers(state),
                        WindowEvent::Resized(physical_size) => {
                            state.resize(*physical_size);
                        }
                        WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                            state.resize(**new_inner_size);
                        }
                        WindowEvent::CursorMoved { position, .. } => {
                            let new_pos = Vec2::new(position.x as f32 / state.surface_config.width as f32, position.y as f32 / state.surface_config.height as f32);
                            cursor_delta = cursor_pos - new_pos;
                            cursor_pos = new_pos;
                        }
                        WindowEvent::MouseInput { state, button, ..} => {
                            keys.update_mouse(state, button);
                        }

                        _ => {}
                    }
                }
            }
            Event::MainEventsCleared => {
                state.window().request_redraw();
                state.update(&keys, delta_time, cursor_delta);
                cursor_delta = Vec2::default();
            }
            Event::RedrawRequested(window_id) if window_id == state.window().id() => {
                let time = Instant::now();
                match state.render() {
                    Ok(_) => {}
                    Err(wgpu::SurfaceError::Lost) => state.resize(state.size),
                    Err(wgpu::SurfaceError::OutOfMemory) => *control_flow = ControlFlow::Exit,
                    Err(e) => eprintln!("{:?}", e),
                }
                let elapsed = time.elapsed().as_micros() as f32;
                delta_time = elapsed / 1_000_000.0;
            }
            _ => {}
        }
    });
}
