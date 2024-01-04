use lib::shader_types::{LightInfo, MaterialInfo, MeshInfo, PbrVertex, Vertex};
use lib::Material;
use log::debug;
use std::cell::Ref;
use std::iter;
use std::num::NonZeroU32;
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::SamplerBindingType::Filtering;
use wgpu::{include_wgsl, BindGroup, BindGroupLayoutDescriptor, Buffer, BufferBinding, Color, CommandEncoder, Device, PipelineLayout, RenderPipeline, Sampler, ShaderModule, TextureView, BindGroupLayout, BindGroupDescriptor, BindGroupEntry, BindingResource};

use lib::scene::{MaterialManager, Mesh, VertexInputs, World};

/**
Pipeline for physically-based rendering
*/
pub struct PBRPipelineProvider {
    shader: ShaderModule,
    pipeline: Option<RenderPipeline>,
    pub pipeline_layout: PipelineLayout,
    pub mat_bind_group: BindGroup,
    pub mesh_bind_group: BindGroup,
    pub cam_bind_group: BindGroup,
    pub tex_bind_group_layout: wgpu::BindGroupLayout,
    mat_bind_group_layout: wgpu::BindGroupLayout,
    mesh_bind_group_layout: wgpu::BindGroupLayout,
    pub light_bind_group: BindGroup,
    pub light_bind_group_layout: wgpu::BindGroupLayout,
}

impl PBRPipelineProvider {
    // Creates all necessary bind groups and layouts for the pipeline
    pub fn new(
        device: &Device,
        mesh_info_buffers: &[Buffer],
        material_info_buffers: &[Buffer],
        light_info_buffers: &[Buffer],
        camera_buffer: &Buffer,
    ) -> Self {
        let shader =
            device.create_shader_module(include_wgsl!("../../../../assets/shaders/pbr.wgsl"));

        let tex_bind_group_layout = {
            let mut tex_bind_group_layout_entries = Vec::new();
            for i in (0..9).step_by(2) {
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
                    binding: i + 1,
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
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: Some(NonZeroU32::new(1_000).unwrap()),
            }],
        });

        let mat_bind_group = {
            let dummy_material = device.create_buffer_init(&BufferInitDescriptor {
                label: Some("Dummy Material Buffer"),
                contents: bytemuck::cast_slice(&[MaterialInfo::default()]),
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            });

            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("PBR Material Bindgroup"),
                layout: &mat_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::BufferArray(
                        &iter::once(dummy_material.as_entire_buffer_binding())
                            .chain(
                                material_info_buffers
                                    .iter()
                                    .map(|m| m.as_entire_buffer_binding()),
                            )
                            .collect::<Vec<_>>(),
                    ),
                }],
            })
        };

        let mesh_bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("PBR Mesh Bindgroup Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT | wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: Some(NonZeroU32::new(1_000).unwrap()),
            }],
        });

        let mesh_bind_group = {
            let dummy_mesh = device.create_buffer_init(&BufferInitDescriptor {
                label: Some("Dummy Mesh Buffer"),
                contents: bytemuck::cast_slice(&[MeshInfo::default()]),
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            });

            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("PBR Mesh Bindgroup"),
                layout: &mesh_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::BufferArray(
                        &iter::once(dummy_mesh.as_entire_buffer_binding())
                            .chain(
                                mesh_info_buffers
                                    .iter()
                                    .map(|m| m.as_entire_buffer_binding()),
                            )
                            .collect::<Vec<_>>(),
                    ),
                }],
            })
        };

        let cam_bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("PBR Camera Bindgroup Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let cam_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("PBR Camera Bindgroup"),
            layout: &cam_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });

        let light_bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("PBR Lights Bindgroup Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: Some(NonZeroU32::new(1_000).unwrap()),
            }],
        });

        let light_bind_group = {
            let dummy_light = device.create_buffer_init(&BufferInitDescriptor {
                label: Some("Dummy Light Buffer"),
                contents: bytemuck::cast_slice(&[LightInfo::default()]),
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            });

            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("PBR Light Bindgroup"),
                layout: &light_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::BufferArray(
                        &iter::once(dummy_light.as_entire_buffer_binding())
                            .chain(
                                light_info_buffers
                                    .iter()
                                    .map(|m| m.as_entire_buffer_binding()),
                            )
                            .collect::<Vec<_>>(),
                    ),
                }],
            })
        };

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("PBR Pipeline Layout"),
            bind_group_layouts: &[
                &tex_bind_group_layout,
                &mat_bind_group_layout,
                &mesh_bind_group_layout,
                &cam_bind_group_layout,
                &light_bind_group_layout,
            ],
            push_constant_ranges: &[],
        });

        Self {
            shader,
            pipeline: None,
            pipeline_layout,
            mat_bind_group,
            mesh_bind_group,
            light_bind_group,
            cam_bind_group,
            tex_bind_group_layout,
            mat_bind_group_layout,
            mesh_bind_group_layout,
            light_bind_group_layout,
        }
    }
    pub fn update_mat_bind_group(&mut self, device: &Device, material_manager: &MaterialManager) {
        self.mat_bind_group =
            material_manager.create_bind_group(device, &self.mat_bind_group_layout);
    }

    pub fn update_lights_bind_group(&mut self, device: &Device, world: &World) -> u32 {
        let mut entries = vec![];
        let dummy_light = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("Dummy Light Buffer"),
            contents: bytemuck::cast_slice(&[LightInfo::default()]),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });
        entries.push(dummy_light.as_entire_buffer_binding());
        let mut num_lights = 0;
        for model in world.get_active_scene().models.iter().filter(|model| model.light.is_some()) {
            let light = model.light.as_ref().unwrap();
            entries.push(light.buffer.as_entire_buffer_binding());
            num_lights += 1;
        }
        self.light_bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Point Lights Bind Group"),
            layout: &self.light_bind_group_layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: BindingResource::BufferArray(&entries),
            }],
        });
        println!("Light bind group got updated, containing {} lights", entries.len());
        num_lights
    }


    pub fn update_mesh_bind_group<'a>(&mut self, device: &Device, meshes: impl Iterator<Item = &'a Mesh>) {
        let mesh_info_buffers = meshes
            .map(|m| m.buffer.as_entire_buffer_binding())
            .collect::<Vec<BufferBinding>>();
        self.mesh_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("PBR Mesh Bindgroup"),
            layout: &self.mesh_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::BufferArray(&mesh_info_buffers),
            }],
        });
    }

    // (re-)creates the pipeline
    pub(crate) fn create_pipeline(&mut self, device: &Device) {
        self.pipeline = Some(
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("PBR Pipeline"),
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
            }),
        );
    }

    fn render_pass<'a>(
        &self,
        encoder: &mut CommandEncoder,
        vertex_inputs: impl Iterator<Item = &'a VertexInputs>,
        view: &TextureView,
        textures_bind_groups: &[&BindGroup],
        material_info_bind_group: &BindGroup,
        mesh_info_bind_group: &BindGroup,
        camera_bind_group: &BindGroup,
        light_bind_group: &BindGroup,
    ) {
        let vertex_inputs = vertex_inputs.collect::<Vec<_>>();
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("PBR Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        render_pass.set_pipeline(self.pipeline.as_ref().unwrap());

        render_pass.set_bind_group(1, material_info_bind_group, &[]);
        render_pass.set_bind_group(2, mesh_info_bind_group, &[]);
        render_pass.set_bind_group(3, camera_bind_group, &[]);
        render_pass.set_bind_group(4, light_bind_group, &[]);

        for (
            i,
            VertexInputs {
                vertex_buffer,
                index_buffer,
            },
        ) in vertex_inputs.iter().enumerate()
        {
            render_pass.set_bind_group(0, textures_bind_groups[i], &[]);

            render_pass.set_vertex_buffer(0, vertex_buffer.buffer.slice(..));
            render_pass.set_index_buffer(index_buffer.buffer.slice(..), wgpu::IndexFormat::Uint16);

            render_pass.draw_indexed(0..index_buffer.count, i as i32, 0..1);
        }
    }

    pub fn render_meshes(
        &self,
        encoder: &mut CommandEncoder,
        view: &TextureView,
        meshes: &[&Mesh],
        material_manager: &MaterialManager,
    ) {
        let vertex_inputs = meshes.iter().map(|m| m.vertex_inputs.as_ref().unwrap());
        let textures_bind_groups = meshes
            .iter()
            .map(|m| match material_manager.get_material(m.material) {
                Material::Pbr(ref mat) => mat
                    .texture_bind_group
                    .as_ref()
                    .expect("PBR material must have a texture bind group"),
                _ => panic!("Unsupported material type for PBR pipeline"),
            })
            .collect::<Vec<_>>();

        self.render_pass(
            encoder,
            vertex_inputs,
            view,
            &textures_bind_groups,
            &self.mat_bind_group,
            &self.mesh_bind_group,
            &self.cam_bind_group,
            &self.light_bind_group,
        )
    }
}
