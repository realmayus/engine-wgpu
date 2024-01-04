use std::num::NonZeroU32;
use egui::{Context};
use egui_wgpu::{winit::Painter, WgpuConfiguration, renderer};
use egui_winit::{State};
use wgpu::{Color, CommandBuffer, CommandEncoder, Device, Queue, TextureView};
use winit::event::WindowEvent;
use winit::window::Window;

pub(crate) struct EguiContext {
    context: Context,
    painter: Painter,
    state: State,
}

impl EguiContext {
    pub(crate) async fn new(window: &Window) -> EguiContext {
        let mut painter = Painter::new(WgpuConfiguration::default(), 1, None, true);
        let context = Context::default();
        painter.set_window(context.viewport_id(), Some(window)).await.unwrap();
        let state = State::new(context.viewport_id(), window, Some(window.scale_factor() as f32), None);
        Self {
            context,
            painter,
            state,
        }
    }
    pub fn context_mut(&mut self) -> &mut Context {
        &mut self.context
    }

    pub fn handle_window_event(&mut self, event: &WindowEvent) -> bool {
        self.state.on_window_event(&self.context, event).repaint
    }

    pub fn on_resized(&mut self, width: u32, height: u32) {
        self.painter.on_window_resized(self.context.viewport_id(), NonZeroU32::new(width).unwrap(), NonZeroU32::new(height).unwrap());
    }

    pub fn render(&mut self, view: &TextureView, encoder: &mut CommandEncoder, window: &Window, ui: impl FnOnce(&Context)) -> Vec<CommandBuffer> {
        // let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        //     label: Some("egui Render Pass"),
        //     color_attachments: &[Some(wgpu::RenderPassColorAttachment {
        //         view,
        //         resolve_target: None,
        //         ops: wgpu::Operations {
        //             load: wgpu::LoadOp::Clear(Color::BLACK),
        //             store: wgpu::StoreOp::Store,
        //         },
        //     })],
        //     depth_stencil_attachment: None,
        //     timestamp_writes: None,
        //     occlusion_query_set: None,
        // });
        //
        // let raw_input = self.state.take_egui_input(window);
        // let full_output = self.context.run(raw_input, |ctx| ui(ctx));
        // self.state.handle_platform_output(window, &self.context, full_output.platform_output);
        // let clipped_primitives = self.context.tessellate(full_output.shapes, window.scale_factor() as f32);
        //
        // let screen_descriptor = renderer::ScreenDescriptor {
        //     size_in_pixels: window.inner_size().into(),
        //     pixels_per_point: window.scale_factor() as f32,
        // };
        //
        // let render_state = self.painter.render_state().unwrap();
        // let user_cmd_bufs = {
        //     let mut renderer = render_state.renderer.write();
        //     for (id, image_delta) in &full_output.textures_delta.set {
        //         renderer.update_texture(
        //             &render_state.device,
        //             &render_state.queue,
        //             *id,
        //             image_delta,
        //         );
        //     }
        //
        //     renderer.update_buffers(
        //         &render_state.device,
        //         &render_state.queue,
        //         encoder,
        //         &clipped_primitives,
        //         &screen_descriptor,
        //     )
        // };
        // let renderer= render_state.renderer.read();
        // renderer.render(&mut render_pass, &clipped_primitives, &screen_descriptor);
        // user_cmd_bufs
        vec![]
    }
}