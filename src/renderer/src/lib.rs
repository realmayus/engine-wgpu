use std::sync::mpsc;
use std::time::Instant;

use anyhow::Result;
use egui_wgpu::renderer::ScreenDescriptor;
use glam::Vec2;
use hashbrown::HashMap;
use wgpu::{Device, Features, Limits, PresentMode, Queue, Surface, SurfaceConfiguration, SurfaceError};
use wgpu::PresentMode::AutoVsync;
use winit::event::{DeviceEvent, ElementState, KeyboardInput, VirtualKeyCode, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::{Window, WindowBuilder};

use lib::managers::{MaterialManager, TextureManager};
use lib::scene::World;

use crate::camera::{Camera, KeyState};
use crate::events::{Event, MouseButton};
use crate::pipelines::grid::GridPipeline;
use crate::pipelines::object_picking::ObjectPickingPipeline;
use crate::pipelines::outlining::OutliningPipeline;
use crate::pipelines::pbr::PBRPipeline;

pub mod camera;
pub mod commands;
pub mod events;
mod gui;
pub mod pipelines;

pub trait Hook {
    fn setup<'a>(&mut self, commands: mpsc::Sender<commands::Command>, event_receiver: mpsc::Receiver<Event>);

    fn update(&mut self, keys: &KeyState, delta_time: f32, world: &mut World);

    fn update_ui(
        &mut self,
        ctx: &egui::Context,
        x: &mut World,
        x0: &mut Camera,
        sender: mpsc::Sender<commands::Command>,
        meta: &mut Meta,
    );
}

pub struct RenderState {
    pub device: Device,
    surface: Surface,
    surface_config: SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,
    pub window: Window,
    queue: Queue,
    pbr_pipeline: PBRPipeline,
    object_picking_pipeline: ObjectPickingPipeline,
    outlining_pipeline: OutliningPipeline,
    grid_pipeline: GridPipeline,
    camera: Camera,
    world: World,
    hook: Box<dyn Hook>,
    show_gui: bool,
    egui: gui::EguiRenderer,
    command_channel: (mpsc::Sender<commands::Command>, mpsc::Receiver<commands::Command>), // Commands: impl -> renderer
    event_channel: (mpsc::Sender<Event>, Option<mpsc::Receiver<Event>>),                   // Events: renderer -> impl
    meta: Meta,
}
const FRAME_TIME_WINDOW: usize = 1000;
pub struct Meta {
    pub frame_time: f32,
    frame_times : [f32; FRAME_TIME_WINDOW],
    index: usize,
    pub show_grid: bool,
    pub vsync: bool,
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
            max_bind_groups: 5,
            max_push_constant_size: 32,
            ..Default::default()
        };

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    limits,
                    features: Features::PUSH_CONSTANTS,
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
            present_mode: AutoVsync,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
        };
        surface.configure(&device, &surface_config);

        let camera = Camera::new_default(size.width as f32, size.height as f32, &device);
        let mut pbr_pipeline = PBRPipeline::new(&device, &surface_config, &camera);
        pbr_pipeline.create_pipeline(&device);

        let textures = TextureManager::new(&device, &queue);
        let materials = MaterialManager::new(
            &device,
            &queue,
            &pbr_pipeline.mat_bind_group_layout,
            &pbr_pipeline.tex_bind_group_layout,
            &textures,
        );

        let world = World {
            scenes: HashMap::new(),
            active_scene: 0,
            materials,
            textures,
        };

        let mut object_picking_pipeline = ObjectPickingPipeline::new(&device, &surface_config, &camera);
        object_picking_pipeline.create_pipeline(&device);

        let mut outlining_pipeline = OutliningPipeline::new(&device, &surface_config, &camera);
        outlining_pipeline.create_pipelines(&device);

        let mut grid_pipeline = GridPipeline::new(&device, &surface_config, &camera);
        grid_pipeline.create_pipeline(&device);

        let egui = gui::EguiRenderer::new(&device, surface_config.format, None, 1, &window);
        let event_channel = mpsc::channel();
        let event_channel = (event_channel.0, Some(event_channel.1));

        Self {
            window,
            surface,
            device,
            queue,
            surface_config,
            size,
            pbr_pipeline,
            object_picking_pipeline,
            outlining_pipeline,
            grid_pipeline,
            camera,
            world,
            show_gui: true,
            hook: Box::from(hook),
            command_channel: mpsc::channel(),
            event_channel,
            egui,
            meta: Meta {
                frame_time: 0.0,
                frame_times: [0.0; FRAME_TIME_WINDOW],
                index: 0,
                show_grid: false,
                vsync: true,
            },
        }
    }

    fn setup(&mut self) {
        self.hook
            .setup(self.command_channel.0.clone(), self.event_channel.1.take().unwrap());
        while let Ok(command) = self.command_channel.1.try_recv() {
            command.process(self, self.event_channel.0.clone());
        }
    }
    pub fn window(&self) -> &Window {
        &self.window
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        self.size = new_size;
        self.surface_config.width = new_size.width.max(1);
        self.surface_config.height = new_size.height.max(1);
        self.surface.configure(&self.device, &self.surface_config);

        self.pbr_pipeline.resize(&self.device, &self.surface_config);
        self.object_picking_pipeline.resize(&self.device, &self.surface_config);
        self.outlining_pipeline.resize(&self.device, &self.surface_config);
        self.grid_pipeline.resize(&self.device, &self.surface_config);

        self.camera.update_aspect(new_size.width as f32, new_size.height as f32);
        self.window.request_redraw();
    }

    fn input(&mut self, event: &WindowEvent) -> bool {
        if !self.show_gui {
            false
        } else {
            self.egui.handle_input(&self.window, event)
        }
    }

    fn update(&mut self, keys: &KeyState, delta_time: f32, cursor_delta: Vec2) {
        self.hook.update(keys, delta_time, &mut self.world);
        self.camera.recv_input(keys, cursor_delta, delta_time);
        self.camera.update_view(&self.queue);
        self.world.update_active_scene(&self.queue); // updates lights and mesh info buffers
        while let Ok(command) = self.command_channel.1.try_recv() {
            command.process(self, self.event_channel.0.clone());
        }
    }

    fn render(&mut self) -> Result<(), SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });

        {
            if let Some(scene) = self.world.get_active_scene() {
                if let Some(meshes) = self.world.pbr_meshes() {
                    let meshes = meshes.collect::<Vec<_>>();
                    self.pbr_pipeline.render_meshes(
                        &mut encoder,
                        &view,
                        &meshes,
                        &self.world.materials,
                        &self.world.materials.buffer,
                        &scene.mesh_buffer,
                        &scene.light_buffer,
                        &self.camera,
                    );

                    let outlined_meshes = meshes.iter().filter(|m| m.is_outline()).copied().collect::<Vec<_>>();
                    if !outlined_meshes.is_empty() {
                        self.outlining_pipeline.render_outline(
                            &mut encoder,
                            &view,
                            &outlined_meshes[..],
                            &scene.mesh_buffer,
                            &self.camera,
                            scene.outline_width,
                            scene.outline_color,
                        );
                    }
                }
            }
            if self.meta.show_grid {
                self.grid_pipeline.render(&mut encoder, &view, &self.camera);
            }
        }
        let screen_descriptor = ScreenDescriptor {
            size_in_pixels: [self.surface_config.width, self.surface_config.height],
            pixels_per_point: self.window.scale_factor() as f32,
        };

        if self.show_gui {
            self.egui.draw(
                &self.device,
                &self.queue,
                &mut encoder,
                &self.window,
                &view,
                screen_descriptor,
                |ui| {
                    self.hook
                        .update_ui(ui, &mut self.world, &mut self.camera, self.command_channel.0.clone(), &mut self.meta);
                },
            );
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();
        Ok(())
    }

    fn update_frame_time(&mut self, frame_time: f32) {
        self.meta.frame_times[self.meta.index] = frame_time;
        self.meta.index = (self.meta.index + 1) % FRAME_TIME_WINDOW;
        self.meta.frame_time = self.meta.frame_times.iter().sum::<f32>() / FRAME_TIME_WINDOW as f32;
    }
}

pub async fn run(hook: impl Hook + 'static) {
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new().build(&event_loop).unwrap();

    let mut state = RenderState::new(window, hook).await;
    let mut keys = KeyState::default();
    let mut cursor_delta = Vec2::default();
    let mut cursor_position = (0, 0);
    let mut delta_time = 0.0;
    let sender = state.command_channel.0.clone();
    let mut time = Instant::now();
    state.setup();
    event_loop.run(move |event, _, control_flow| {
        match event {
            winit::event::Event::WindowEvent { ref event, window_id } if window_id == state.window().id() => {
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
                        WindowEvent::KeyboardInput {
                            input:
                            KeyboardInput {
                                state,
                                virtual_keycode: Some(keycode),
                                ..
                            },
                            ..
                        } => {
                            keys.update_keys(*keycode, *state);
                        }
                        WindowEvent::ModifiersChanged(state) => keys.set_modifiers(state),
                        WindowEvent::Resized(physical_size) => {
                            state.resize(*physical_size);
                        }
                        WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                            state.resize(**new_inner_size);
                        }
                        WindowEvent::MouseInput {
                            state: element_state,
                            button,
                            ..
                        } => {
                            if !keys.update_mouse(element_state, button) && element_state == &ElementState::Pressed {
                                let button = match button {
                                    winit::event::MouseButton::Left => MouseButton::Left,
                                    winit::event::MouseButton::Right => MouseButton::Right,
                                    winit::event::MouseButton::Middle => MouseButton::Middle,
                                    _ => return,
                                };
                                let (x, y): (u32, u32) = cursor_position;
                                state
                                    .event_channel
                                    .0
                                    .clone()
                                    .send(Event::Click {
                                        x,
                                        y,
                                        mouse_button: button,
                                    })
                                    .unwrap();
                            }
                        }
                        WindowEvent::CursorMoved { position, .. } => {
                            cursor_position = (*position).into();
                        }

                        _ => {}
                    }
                }
            }
            winit::event::Event::MainEventsCleared => {
                state.window().request_redraw();
                state.update(&keys, delta_time, cursor_delta);
                cursor_delta = Vec2::default();
            }
            winit::event::Event::RedrawRequested(window_id) if window_id == state.window().id() => {
                match state.render() {
                    Ok(_) => {}
                    Err(SurfaceError::Lost) => state.resize(state.size),
                    Err(SurfaceError::OutOfMemory) => *control_flow = ControlFlow::Exit,
                    Err(e) => eprintln!("{:?}", e),
                }
                state.update_frame_time(time.elapsed().as_secs_f32());
                delta_time = time.elapsed().as_secs_f32();
            }
            winit::event::Event::DeviceEvent {
                event: DeviceEvent::MouseMotion { delta },
                ..
            } => {
                cursor_delta = Vec2::new(
                    delta.0 as f32 / state.surface_config.width as f32,
                    delta.1 as f32 / state.surface_config.height as f32,
                );
            }
            _ => {}
        }
        time = Instant::now();
    });
}
