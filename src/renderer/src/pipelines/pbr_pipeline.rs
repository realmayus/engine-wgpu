use wgpu::{BindGroup, BindGroupLayoutDescriptor, Buffer, Color, CommandEncoder, DepthStencilState, Device, include_wgsl, PipelineLayout, RenderPassDepthStencilAttachment, RenderPipeline, ShaderModule, SurfaceConfiguration, TextureView};
use wgpu::SamplerBindingType::Filtering;

use lib::buffer_array::DynamicBufferArray;
use lib::managers::MaterialManager;
use lib::Material;
use lib::scene::{Mesh, VertexInputs};
use lib::shader_types::{LightInfo, MaterialInfo, MeshInfo, PbrVertex, Vertex};
use lib::texture::Texture;

/**
Pipeline for physically-based rendering
 */
pub struct PBRPipelineProvider {
    shader: ShaderModule,
    pipeline: Option<RenderPipeline>,
    pub pipeline_layout: PipelineLayout,
    pub cam_bind_group: BindGroup,
    pub tex_bind_group_layout: wgpu::BindGroupLayout,
    pub(crate) mat_bind_group_layout: wgpu::BindGroupLayout,
    pub(crate) mesh_bind_group_layout: wgpu::BindGroupLayout,
    pub light_bind_group_layout: wgpu::BindGroupLayout,
    pub depth_texture: Texture,
}

impl PBRPipelineProvider {
    // Creates all necessary bind groups and layouts for the pipeline
    pub fn new(
        device: &Device,
        config: &SurfaceConfiguration,
        camera_buffer: &Buffer,
    ) -> Self {
        let shader =
            device.create_shader_module(include_wgsl!("../../../../assets/shaders/pbr.wgsl"));
        let depth_texture =
            Texture::create_depth_texture(device, config, "depth_texture");

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
                count: None,
            }],
        });

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
                count: None,
            }],
        });

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
                count: None,
            }],
        });


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
            cam_bind_group,
            tex_bind_group_layout,
            mat_bind_group_layout,
            mesh_bind_group_layout,
            light_bind_group_layout,
            depth_texture,
        }
    }

    pub(crate) fn resize(&mut self, device: &Device, config: &SurfaceConfiguration) {
        self.depth_texture = Texture::create_depth_texture(device, config, "depth_texture");
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
                    front_face: wgpu::FrontFace::Cw,
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
            }),
        );
    }

    fn render_pass<'a>(
        &self,
        encoder: &mut CommandEncoder,
        vertex_inputs: impl Iterator<Item=&'a VertexInputs>,
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
        mat_buffer: &DynamicBufferArray<MaterialInfo>,
        mesh_buffer: &DynamicBufferArray<MeshInfo>,
        light_buffer: &DynamicBufferArray<LightInfo>,
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
            &mat_buffer.bind_group,
            &mesh_buffer.bind_group,
            &self.cam_bind_group,
            &light_buffer.bind_group,
        )
    }
}
