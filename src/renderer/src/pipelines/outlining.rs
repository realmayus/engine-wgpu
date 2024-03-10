use bytemuck::{Pod, Zeroable};
use wgpu::{
    BindGroup, BindGroupLayoutDescriptor, CommandEncoder, DepthStencilState, Device, include_wgsl, PipelineLayout,
    RenderPass, RenderPassDepthStencilAttachment, RenderPipeline, ShaderModule, SurfaceConfiguration, TextureView,
};

use lib::buffer_array::DynamicBufferMap;
use lib::scene::mesh::Mesh;
use lib::scene::VertexInputs;
use lib::shader_types::{MeshInfo, PbrVertex, Vertex};

use crate::camera::Camera;

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct PushConstants {
    mesh_index: u32,
    outline_config: u32,
}

pub struct OutliningPipeline {
    shader: ShaderModule,
    mask_pipeline: Option<RenderPipeline>,
    outline_pipeline: Option<RenderPipeline>,
    pipeline_layout: PipelineLayout,
    stencil_view: wgpu::TextureView,
}

impl OutliningPipeline {
    // Creates all necessary bind groups and layouts for the pipeline
    pub fn new(device: &Device, config: &SurfaceConfiguration, camera: &Camera) -> Self {
        let shader = device.create_shader_module(include_wgsl!("../shaders/outlining.wgsl"));

        let mesh_bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Outlining Mesh Bindgroup Layout"),
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
            label: Some("Outlining Mask Pipeline Layout"),
            bind_group_layouts: &[&mesh_bind_group_layout, &camera.bind_group_layout],
            push_constant_ranges: &[wgpu::PushConstantRange {
                stages: wgpu::ShaderStages::VERTEX,
                range: 0..std::mem::size_of::<PushConstants>() as u32,
            }],
        });

        let stencil_buffer = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Outlining Stencil Texture"),
            size: wgpu::Extent3d {
                width: config.width,
                height: config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Stencil8,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });

        let stencil_view = stencil_buffer.create_view(&wgpu::TextureViewDescriptor::default());

        Self {
            shader,
            mask_pipeline: None,
            outline_pipeline: None,
            pipeline_layout,
            stencil_view,
        }
    }

    pub(crate) fn resize(&mut self, device: &Device, config: &SurfaceConfiguration) {
        let stencil_buffer = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Outlining Stencil Texture"),
            size: wgpu::Extent3d {
                width: config.width,
                height: config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Stencil8,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        self.stencil_view = stencil_buffer.create_view(&wgpu::TextureViewDescriptor::default());
    }

    // (re-)creates the pipeline
    pub(crate) fn create_pipelines(&mut self, device: &Device) {
        self.mask_pipeline = Some(device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Outlining Mask Pipeline"),
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
                    format: wgpu::TextureFormat::Bgra8UnormSrgb,
                    blend: None,
                    write_mask: wgpu::ColorWrites::empty(),
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Cw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(DepthStencilState {
                format: wgpu::TextureFormat::Stencil8,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Always,
                stencil: wgpu::StencilState {
                    front: wgpu::StencilFaceState {
                        compare: wgpu::CompareFunction::Always,
                        pass_op: wgpu::StencilOperation::Replace,
                        ..Default::default()
                    },
                    back: wgpu::StencilFaceState::IGNORE,
                    read_mask: !0,
                    write_mask: !0,
                },
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        }));

        self.outline_pipeline = Some(device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Outlining Outline Pipeline"),
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
                    format: wgpu::TextureFormat::Bgra8UnormSrgb,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Cw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(DepthStencilState {
                format: wgpu::TextureFormat::Stencil8,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Always,
                stencil: wgpu::StencilState {
                    front: wgpu::StencilFaceState {
                        compare: wgpu::CompareFunction::Greater,
                        ..Default::default()
                    },
                    back: wgpu::StencilFaceState::IGNORE,
                    read_mask: !0,
                    write_mask: !0,
                },
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        }));
    }

    fn render_pass<'a>(
        &self,
        encoder: &mut CommandEncoder,
        view: &TextureView,
        vertex_inputs: impl Iterator<Item = &'a VertexInputs>,
        mesh_info_map: &DynamicBufferMap<MeshInfo, u32>,
        camera_bind_group: &BindGroup,
        outline_value: u32,
    ) {
        let vertex_inputs = vertex_inputs.collect::<Vec<_>>();
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Outlining Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                view: &self.stencil_view,
                depth_ops: None,
                stencil_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(0),
                    store: wgpu::StoreOp::Store,
                }),
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        render_pass.set_stencil_reference(1);
        render_pass.set_pipeline(self.mask_pipeline.as_ref().unwrap());

        Self::draw(mesh_info_map, camera_bind_group, &vertex_inputs, &mut render_pass, 0);

        render_pass.set_pipeline(self.outline_pipeline.as_ref().unwrap());

        Self::draw(
            mesh_info_map,
            camera_bind_group,
            &vertex_inputs,
            &mut render_pass,
            outline_value,
        );
    }

    fn draw<'a, 'b: 'a>(
        mesh_info_map: &'b DynamicBufferMap<MeshInfo, u32>,
        camera_bind_group: &'b BindGroup,
        vertex_inputs: &[&'a VertexInputs],
        render_pass: &mut RenderPass<'a>,
        outline_value: u32,
    ) {
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
                outline_config: outline_value,
            };
            render_pass.set_push_constants(wgpu::ShaderStages::VERTEX, 0, bytemuck::bytes_of(&push_constants));
            render_pass.set_vertex_buffer(0, vertex_buffer.buffer.slice(..));
            render_pass.set_index_buffer(index_buffer.buffer.slice(..), wgpu::IndexFormat::Uint16);

            render_pass.draw_indexed(0..index_buffer.count, 0, 0..1);
        }
    }

    pub fn render_outline(
        &self,
        encoder: &mut CommandEncoder,
        view: &TextureView,
        meshes: &[&Mesh],
        mesh_buffer: &DynamicBufferMap<MeshInfo, u32>,
        camera: &Camera,
        outline_width: u8,
        outline_color: [u8; 3],
    ) {
        let vertex_inputs = meshes.iter().map(|m| m.vertex_inputs.as_ref().unwrap());
        let outline_value = (outline_color[0] as u32) << 24
            | (outline_color[1] as u32) << 16
            | (outline_color[2] as u32) << 8
            | outline_width as u32;

        self.render_pass(
            encoder,
            view,
            vertex_inputs,
            mesh_buffer,
            &camera.bind_group,
            outline_value,
        );
    }
}
