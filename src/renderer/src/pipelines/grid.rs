use wgpu::{
    BindGroup, CommandEncoder, DepthStencilState,
    Device, include_wgsl, PipelineLayout, RenderPassDepthStencilAttachment, RenderPipeline, ShaderModule,
    SurfaceConfiguration, TextureView,
};
use wgpu::util::DeviceExt;

use lib::shader_types::{BasicVertex, Vertex};
use lib::SizedBuffer;
use lib::texture::Texture;

use crate::camera::Camera;

pub struct GridPipeline {
    shader: ShaderModule,
    pipeline: Option<RenderPipeline>,
    pub pipeline_layout: PipelineLayout,
    pub depth_texture: Texture,
    vertices: SizedBuffer,
}

impl GridPipeline {
    pub fn new(device: &Device, config: &SurfaceConfiguration, camera: &Camera) -> Self {
        let shader = device.create_shader_module(include_wgsl!("../shaders/grid.wgsl"));
        let depth_texture = Texture::create_depth_texture(device, config.width, config.height, "depth_texture");

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Grid Pipeline Layout"),
            bind_group_layouts: &[&camera.bind_group_layout],
            push_constant_ranges: &[],
        });

        let vertices = [
            BasicVertex { position: [1.0, 1.0, 0.0] },
            BasicVertex { position: [-1.0, -1.0, 0.0] },
            BasicVertex { position: [-1.0, 1.0, 0.0] },
            BasicVertex { position: [-1.0, -1.0, 0.0] },
            BasicVertex { position: [1.0, 1.0, 0.0] },
            BasicVertex { position: [1.0, -1.0, 0.0] },
        ];
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Grid Vertex Buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let vertices = SizedBuffer {
            buffer,
            count: vertices.len() as u32,
        };

        Self {
            shader,
            pipeline: None,
            pipeline_layout,
            depth_texture,
            vertices,
        }
    }
    pub(crate) fn resize(&mut self, device: &Device, config: &SurfaceConfiguration) {
        self.depth_texture = Texture::create_depth_texture(device, config.width, config.height, "depth_texture");
    }

    // (re-)creates the pipeline
    pub(crate) fn create_pipeline(&mut self, device: &Device) {
        self.pipeline = Some(device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Grid Pipeline"),
            layout: Some(&self.pipeline_layout),
            vertex: wgpu::VertexState {
                module: &self.shader,
                entry_point: "vs_main",
                buffers: &[BasicVertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &self.shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Bgra8UnormSrgb,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(DepthStencilState {
                format: Texture::DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        }));
    }

    fn render_pass(
        &self,
        view: &TextureView,
        encoder: &mut CommandEncoder,
        camera_bind_group: &BindGroup,
    ) {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Grid Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                view: &self.depth_texture.view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Discard,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        render_pass.set_pipeline(self.pipeline.as_ref().unwrap());

        render_pass.set_bind_group(0, camera_bind_group, &[]);

        render_pass.set_vertex_buffer(0, self.vertices.buffer.slice(..));
        // render_pass.set_index_buffer(index_buffer.buffer.slice(..), wgpu::IndexFormat::Uint16);

        render_pass.draw(0..self.vertices.count, 0..1);

    }

    pub fn render(
        &self,
        encoder: &mut CommandEncoder,
        view: &TextureView,
        camera: &Camera,
    ) {
        self.render_pass(view, encoder, &camera.bind_group);
    }
}
