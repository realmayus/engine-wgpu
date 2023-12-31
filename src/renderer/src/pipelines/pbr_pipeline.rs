use std::cell::Ref;
use log::debug;
use wgpu::{BindGroup, BindGroupLayoutDescriptor, Buffer, Color, CommandEncoder, Device, include_wgsl, PipelineLayout, RenderPipeline, Sampler, ShaderModule, TextureView};
use wgpu::SamplerBindingType::Filtering;
use wgpu::VertexStepMode::Vertex;
use lib::Material;


use lib::scene::{MaterialManager, Mesh, VertexInputs};


/**
Pipeline for physically-based rendering
*/
pub struct PBRPipelineProvider {
    shader: ShaderModule,
    cached_vertex_input_buffers: Vec<VertexInputs>,
    pipeline: Option<RenderPipeline>,
    pub pipeline_layout: PipelineLayout,
    pub mat_bind_group: BindGroup,
    pub mesh_bind_group: BindGroup,
    pub cam_bind_group: BindGroup,
    tex_bind_group_layout: wgpu::BindGroupLayout,
    mat_bind_group_layout: wgpu::BindGroupLayout,
    mesh_bind_group_layout: wgpu::BindGroupLayout,
}

impl PBRPipelineProvider {

    // Creates all necessary bind groups and layouts for the pipeline
    pub fn new(
        device: &Device,
        drawables: Vec<VertexInputs>,
        mesh_info_buffers: Vec<Buffer>,
        material_info_buffers: Vec<Buffer>,
        camera_buffer: Buffer,
        num_textures: u32,
        num_materials: usize,
    ) -> Self {
        let shader = device.create_shader_module(include_wgsl!("assets/shaders/pbr.wgsl"));


        let tex_bind_group_layout = {
            let mut tex_bind_group_layout_entries = Vec::new();
            for i in 0..5 {
                tex_bind_group_layout_entries.push(wgpu::BindGroupLayoutEntry {
                    binding: i,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                });
                tex_bind_group_layout_entries.push(wgpu::BindGroupLayoutEntry {
                    binding: i + num_textures,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(Filtering),
                    count: None,
                });
            }

            device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("PBR Texture Bindgroup Layout"),
                entries: &tex_bind_group_layout_entries,
            })
        };

        
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

        let pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("PBR Pipeline Layout"),
                bind_group_layouts: &[&tex_bind_group_layout, &mat_bind_group_layout, &mesh_bind_group_layout, &cam_bind_group_layout],
                push_constant_ranges: &[],
            });


        Self {
            shader,
            cached_vertex_input_buffers: drawables,
            pipeline: None,
            pipeline_layout,
            mat_bind_group,
            mesh_bind_group,
            cam_bind_group,
            tex_bind_group_layout,
            mat_bind_group_layout,
            mesh_bind_group_layout,
        }
    }
    fn update_mat_bind_group(&mut self, device: &Device, material_manager: &MaterialManager) {
        self.mat_bind_group = material_manager.create_bind_group(device, &self.mat_bind_group_layout);
    }

    fn update_mesh_bind_group(&mut self, device: &Device, meshes: &[Mesh]) {
        let mesh_info_buffers = meshes.iter().map(|m| m.buffer.as_ref().unwrap()).collect::<Vec<&Buffer>>();
        self.mesh_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("PBR Mesh Bindgroup"),
            layout: &self.mesh_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::BufferArray(&mesh_info_buffers.into()),
                },
            ],
        });
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
                    format: wgpu::TextureFormat::Bgra8UnormSrgb,  // todo right?
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
                       vertex_inputs: impl Iterator<Item=VertexInputs>,
                       view: &TextureView,
                       textures_bind_groups: &[BindGroup],
                       material_info_bind_group: &BindGroup,
                       mesh_info_bind_group: &BindGroup,
                       camera_bind_group: &BindGroup) {
        assert_eq!(vertex_inputs.len(), mesh_info_bind_group.len());
        
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

        render_pass.set_bind_group(1, material_info_bind_group, &[]);
        render_pass.set_bind_group(2, mesh_info_bind_group, &[]);
        render_pass.set_bind_group(3, camera_bind_group, &[]);

        for (i, VertexInputs {vertex_buffer, index_buffer}) in vertex_inputs.enumerate() {
            render_pass.set_bind_group(0, &textures_bind_groups[i], &[]);

            render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            render_pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint16);

            render_pass.draw_indexed(0..index_buffer.count, i as i32, 0..1);
        }
    }
    
    pub fn render_meshes(&self, encoder: &mut CommandEncoder, view: &TextureView, meshes: &[Mesh]) {
        let vertex_inputs = meshes.iter().map(|m| m.vertex_inputs.unwrap()).collect();
        let textures_bind_groups = meshes.iter().map(|m| match *m.material.borrow() {
            Material::Pbr(ref mat) => {
                mat.texture_bind_group.expect("PBR material must have a texture bind group")
            },
            _ => panic!("Unsupported material type for PBR pipeline")
        }).collect();

        self.render_pass(
            encoder,
            vertex_inputs,
            view,
            &textures_bind_groups,
            &self.mat_bind_group,
            &self.mesh_bind_group,
            &self.cam_bind_group,
        )
    }
}
