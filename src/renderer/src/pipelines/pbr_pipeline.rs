use wgpu::{BindGroup, BindGroupLayoutDescriptor, Buffer, Color, CommandEncoder, Device, include_wgsl, PipelineLayout, RenderPipeline, Sampler, ShaderModule, TextureView};
use wgpu::VertexStepMode::Vertex;


use lib::scene::VertexInputs;


/**
Pipeline for physically-based rendering
*/
pub struct PBRPipelineProvider {
    shader: ShaderModule,
    cached_vertex_input_buffers: Vec<VertexInputs>,
    pipeline: Option<RenderPipeline>,
    pub pipeline_layout: PipelineLayout,
    pub tex_bind_group: BindGroup,
    pub mat_bind_group: BindGroup,
    pub mesh_bind_group: BindGroup,
    pub cam_bind_group: BindGroup,
}

impl PBRPipelineProvider {

    // Creates all necessary bind groups and layouts for the pipeline
    pub fn new(
        device: &Device,
        drawables: Vec<VertexInputs>,
        texture_views: Vec<TextureView>,
        samplers: Vec<Sampler>,
        mesh_info_buffers: Vec<Buffer>,
        material_info_buffers: Vec<Buffer>,
        camera_buffer: Buffer,
        num_textures: u32,
        num_materials: usize,
    ) -> Self {
        let shader = device.create_shader_module(include_wgsl!("assets/shaders/pbr.wgsl"));
        let pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("PBR Pipeline Layout"),
                bind_group_layouts: &[],
                push_constant_ranges: &[],
            });

        let tex_bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("PBR Texture Bindgroup Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: Some(num_textures.into()),  // TODO support 0 textures
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: Some(num_textures.into()),  // TODO support 0 textures
                }
            ],
        });

        let tex_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("PBR Texture Bindgroup"),
            layout: &tex_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureViewArray(&texture_views.into()),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::SamplerArray(&samplers.into()),
                },
            ],
        });


        let mat_bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("PBR Material Bindgroup Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: Some(num_materials.into()),
                }
            ],
        });

        let mat_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("PBR Material Bindgroup"),
            layout: &mat_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::BufferArray(&material_info_buffers.into()),
                },
            ],
        });

        let mesh_bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("PBR Mesh Bindgroup Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: Some(mesh_info_buffers.len().into()), // TODO support 0 meshes
                }
            ],
        });

        let mesh_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("PBR Mesh Bindgroup"),
            layout: &mesh_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::BufferArray(&mesh_info_buffers.into()),
                },
            ],
        });

        let cam_bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("PBR Camera Bindgroup Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }
            ],
        });

        let cam_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("PBR Camera Bindgroup"),
            layout: &cam_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_buffer.as_entire_binding(),
                },
            ],
        });

        Self {
            shader,
            cached_vertex_input_buffers: drawables,
            pipeline: None,
            pipeline_layout,
            tex_bind_group,
            mat_bind_group,
            mesh_bind_group,
            cam_bind_group,
        }
    }

    // (re-)creates the pipeline
    fn create_pipeline(&mut self, device: Device) {
        self.pipeline = Some(device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("PBR Pipeline"),
            layout: Some(&self.pipeline_layout),
            vertex: wgpu::VertexState {
                module: &self.shader,
                entry_point: "vs_main",
                buffers: &[
                    Vertex::desc(),
                ],
            },
            fragment: Some(wgpu::FragmentState {
                module: &self.shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
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
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        }));
    }


    pub fn render_pass(&self,
                   encoder: &mut CommandEncoder,
                   vertex_inputs: &Vec<VertexInputs>,
                   view: &TextureView,
                   texture_views: &Vec<TextureView>,
                   samplers: &Vec<Sampler>,
                   mesh_info_buffers: &Vec<Buffer>,
                   material_info_buffers: &Vec<Buffer>,
                   camera_buffer: &Buffer) {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("PBR Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(Color::BLACK),
                    store: wgpu::StoreOp::Store,
                }
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        render_pass.set_pipeline(self.pipeline.as_ref().unwrap());

        for (i, &VertexInputs {vertex_buffer, index_buffer}) in vertex_inputs.iter().enumerate() {
            render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            render_pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint16);

            render_pass.draw_indexed(0..index_buffer.count, i as i32, 0..1);
        }
    }
}
