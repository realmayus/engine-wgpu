use bytemuck::{Pod, Zeroable};
use wgpu::{
    BindGroup, BindGroupLayoutDescriptor, Buffer, BufferAddress, Color, CommandEncoder, DepthStencilState,
    Device, include_wgsl, PipelineLayout, Queue, RenderPassDepthStencilAttachment, RenderPipeline, ShaderModule,
    SurfaceConfiguration, TextureView,
};

use lib::buffer_array::DynamicBufferMap;
use lib::scene::mesh::Mesh;
use lib::scene::VertexInputs;
use lib::shader_types::{MeshInfo, PbrVertex, Vertex};
use lib::texture::Texture;

use crate::camera::Camera;

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct PushConstants {
    mesh_index: u32,
    padding: [u32; 3],
    color: [f32; 4],
}

pub struct ObjectPickingPipeline {
    shader: ShaderModule,
    pipeline: Option<RenderPipeline>,
    pub pipeline_layout: PipelineLayout,
    pub depth_texture: Texture,
    pub render_target: wgpu::Texture,
    render_target_view: TextureView,
    staging_buffer: Buffer,
    target_size: (u32, u32),
    viewport_size: (u32, u32),
}

impl ObjectPickingPipeline {
    // Creates all necessary bind groups and layouts for the pipeline
    pub fn new(device: &Device, config: &SurfaceConfiguration, camera: &Camera) -> Self {
        let shader = device.create_shader_module(include_wgsl!("../shaders/object_picking.wgsl"));
        let target_size = (Self::round_to_next_multiple_of_256(config.width), config.height);
        let depth_texture = Texture::create_depth_texture(device, target_size.0, target_size.1, "depth_texture");

        let mesh_bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Object Picking Mesh Bindgroup Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Object Picking Pipeline Layout"),
            bind_group_layouts: &[&mesh_bind_group_layout, &camera.bind_group_layout],
            push_constant_ranges: &[wgpu::PushConstantRange {
                stages: wgpu::ShaderStages::VERTEX,
                range: 0..std::mem::size_of::<PushConstants>() as u32,
            }],
        });

        let render_target = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Object Picking Render Target"),
            size: wgpu::Extent3d {
                width: target_size.0,
                height: target_size.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[wgpu::TextureFormat::Rgba8Unorm],
        });

        let render_target_view = render_target.create_view(&wgpu::TextureViewDescriptor::default());

        let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Object Picking Staging Buffer"),
            size: (target_size.0 * target_size.1 * 4) as BufferAddress,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        Self {
            shader,
            pipeline: None,
            pipeline_layout,
            depth_texture,
            render_target,
            render_target_view,
            staging_buffer,
            target_size,
            viewport_size: (config.width, config.height),
        }
    }
    fn round_to_next_multiple_of_256(n: u32) -> u32 {
        (n + 255) & !255
    }
    pub(crate) fn resize(&mut self, device: &Device, config: &SurfaceConfiguration) {
        let target_size = (Self::round_to_next_multiple_of_256(config.width), config.height);
        self.depth_texture = Texture::create_depth_texture(device, target_size.0, target_size.1, "depth_texture");
        self.staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Object Picking Staging Buffer"),
            size: (target_size.0 * target_size.1 * 4) as BufferAddress,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        self.target_size = target_size;
        self.viewport_size = (config.width, config.height);
        self.render_target = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Object Picking Render Target"),
            size: wgpu::Extent3d {
                width: target_size.0,
                height: target_size.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[wgpu::TextureFormat::Rgba8Unorm],
        });
        self.render_target_view = self.render_target.create_view(&wgpu::TextureViewDescriptor::default());
    }

    // (re-)creates the pipeline
    pub(crate) fn create_pipeline(&mut self, device: &Device) {
        self.pipeline = Some(device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Object Picking Pipeline"),
            layout: Some(&self.pipeline_layout),
            vertex: wgpu::VertexState {
                module: &self.shader,
                entry_point: "vs_main",
                buffers: &[PbrVertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &self.shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                // Setting this to anything other than Fill requires Features::NON_FILL_POLYGON_MODE
                polygon_mode: wgpu::PolygonMode::Fill,
                // Requires Features::DEPTH_CLIP_CONTROL
                unclipped_depth: false,
                // Requires Features::CONSERVATIVE_RASTERIZATION
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

    fn render_pass<'a>(
        &self,
        encoder: &mut CommandEncoder,
        vertex_inputs: impl Iterator<Item = &'a VertexInputs>,
        mesh_info_map: &DynamicBufferMap<MeshInfo, u32>,
        camera_bind_group: &BindGroup,
    ) {
        let vertex_inputs = vertex_inputs.collect::<Vec<_>>();
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Object Picking Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.render_target_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(Color::TRANSPARENT),
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

        render_pass.set_bind_group(0, mesh_info_map.bind_group(), &[]);
        render_pass.set_bind_group(1, camera_bind_group, &[]);

        for VertexInputs {
            mesh_id,
            vertex_buffer,
            index_buffer,
        } in vertex_inputs.iter()
        {
            let mesh_index = mesh_info_map.get(mesh_id).expect("Mesh not found in mesh_info_map");
            let push_constants = PushConstants {
                mesh_index: *mesh_index as u32,
                padding: [0; 3],
                color: [
                    (mesh_id & 0xff) as f32 / 255.0,
                    ((mesh_id >> 8) & 0xff) as f32 / 255.0,
                    ((mesh_id >> 16) & 0xff) as f32 / 255.0,
                    ((mesh_id >> 24) & 0xff) as f32 / 255.0,
                ],
            };
            render_pass.set_push_constants(wgpu::ShaderStages::VERTEX, 0, bytemuck::bytes_of(&push_constants));
            render_pass.set_vertex_buffer(0, vertex_buffer.buffer.slice(..));
            render_pass.set_index_buffer(index_buffer.buffer.slice(..), wgpu::IndexFormat::Uint16);

            render_pass.draw_indexed(0..index_buffer.count, 0, 0..1);
        }
    }

    pub fn query_click(
        &self,
        device: &Device,
        queue: &Queue,
        x: u32,
        y: u32,
        meshes: &[&Mesh],
        mesh_buffer: &DynamicBufferMap<MeshInfo, u32>,
        camera: &Camera,
    ) -> u32 {
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Object Picking Query Encoder"),
        });
        let vertex_inputs = meshes.iter().map(|m| m.vertex_inputs.as_ref().unwrap());

        self.render_pass(&mut encoder, vertex_inputs, mesh_buffer, &camera.bind_group);

        encoder.copy_texture_to_buffer(
            wgpu::ImageCopyTexture {
                texture: &self.render_target,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::ImageCopyBuffer {
                buffer: &self.staging_buffer,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(self.target_size.0 * 4),
                    rows_per_image: Some(self.target_size.1),
                },
            },
            wgpu::Extent3d {
                width: self.target_size.0,
                height: self.target_size.1,
                depth_or_array_layers: 1,
            },
        );
        queue.submit(Some(encoder.finish()));
        let res = {
            let buffer_slice = self.staging_buffer.slice(..);
            let (sender, receiver) = flume::unbounded();
            buffer_slice.map_async(wgpu::MapMode::Read, move |r| sender.send(r).unwrap());
            device.poll(wgpu::Maintain::Wait);
            receiver.recv().unwrap().unwrap();
            let view = buffer_slice.get_mapped_range();
            // save buffer contents as image
            let ratio = self.viewport_size.0 as f32 / self.target_size.0 as f32;
            let x = (x as f32 / ratio) as u32;
            // get red pixel value at x, y from buffer; buffer is in RGBA format and image has width of target_size.0 and height of target_size.1
            let r = view[(x * 4 + y * self.target_size.0 * 4) as usize];
            let g = view[(x * 4 + y * self.target_size.0 * 4 + 1) as usize];
            let b = view[(x * 4 + y * self.target_size.0 * 4 + 2) as usize];
            let a = view[(x * 4 + y * self.target_size.0 * 4 + 3) as usize];
            u32::from_le_bytes([r, g, b, a])
        };
        self.staging_buffer.unmap();
        res
    }
}
