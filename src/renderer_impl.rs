use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;
use std::vec::Vec;

use egui_winit_vulkano::egui::Ui;
use egui_winit_vulkano::{egui, Gui};
use glam::{Mat4, Vec3};
use image::DynamicImage;
use image::ImageFormat::Png;
use itertools::Itertools;
use log::info;
use rand::Rng;
use vulkano::buffer::{Buffer, BufferCreateInfo, BufferUsage, Subbuffer};
use vulkano::command_buffer::{
    AutoCommandBufferBuilder, CommandBufferUsage, PrimaryAutoCommandBuffer,
};
use vulkano::format;
use vulkano::image::ImageViewAbstract;
use vulkano::memory::allocator::{AllocationCreateInfo, MemoryUsage, StandardMemoryAllocator};
use vulkano::pipeline::graphics::viewport::Viewport;
use vulkano::sampler::{Sampler, SamplerCreateInfo};

use crate::gui::render_gui;
use lib::scene::{Material, Mesh, Model, Scene, Texture};
use lib::scene_serde::WorldSerde;
use lib::shader_types::{LineInfo, MaterialInfo};
use lib::texture::create_texture;
use lib::Dirtyable;
use renderer::camera::Camera;
use renderer::pipelines::line_pipeline::LinePipeline;
use renderer::pipelines::pbr_pipeline::{DrawableVertexInputs, PBRPipeline};
use renderer::{
    init_renderer, start_renderer, PartialRenderState, RenderState, StateCallable,
    VertexInputBuffer,
};
use systems::io;
use systems::io::gltf_loader::load_gltf;
use systems::io::{clear_run_dir, extract_image_to_file};

fn create_buffers(
    mesh: &Mesh,
    memory_allocator: &StandardMemoryAllocator,
) -> (
    Subbuffer<[[f32; 3]]>,
    Subbuffer<[[f32; 3]]>,
    Subbuffer<[[f32; 2]]>,
    Subbuffer<[u32]>,
) {
    let vert_buf: Subbuffer<[[f32; 3]]> = Buffer::from_iter(
        memory_allocator,
        BufferCreateInfo {
            usage: BufferUsage::VERTEX_BUFFER,
            ..Default::default()
        },
        AllocationCreateInfo {
            usage: MemoryUsage::Upload,
            ..Default::default()
        },
        mesh.vertices.iter().map(|v| v.to_array()),
    )
    .expect("Couldn't allocate vertex buffer");

    let normal_buf: Subbuffer<[[f32; 3]]> = Buffer::from_iter(
        memory_allocator,
        BufferCreateInfo {
            usage: BufferUsage::VERTEX_BUFFER,
            ..Default::default()
        },
        AllocationCreateInfo {
            usage: MemoryUsage::Upload,
            ..Default::default()
        },
        mesh.vertices.iter().map(|v| v.to_array()),
    )
    .expect("Couldn't allocate normal buffer");

    let uv_buf: Subbuffer<[[f32; 2]]> = Buffer::from_iter(
        memory_allocator,
        BufferCreateInfo {
            usage: BufferUsage::VERTEX_BUFFER,
            ..Default::default()
        },
        AllocationCreateInfo {
            usage: MemoryUsage::Upload,
            ..Default::default()
        },
        mesh.uvs.iter().map(|v| v.to_array()),
    )
    .expect("Couldn't allocate UV buffer");

    let index_buf = Buffer::from_iter(
        memory_allocator,
        BufferCreateInfo {
            usage: BufferUsage::INDEX_BUFFER,
            ..Default::default()
        },
        AllocationCreateInfo {
            usage: MemoryUsage::Upload,
            ..Default::default()
        },
        mesh.indices.clone(),
    )
    .expect("Couldn't allocate index buffer");

    (vert_buf, normal_buf, uv_buf, index_buf)
}

pub(crate) struct GlobalState {
    pub(crate) scenes: Vec<Scene>,
    pub(crate) materials: Vec<Rc<RefCell<Material>>>,
    pub(crate) textures: Vec<Rc<Texture>>,
}

impl StateCallable for GlobalState {
    fn setup_gui(&mut self, gui: &mut Gui, render_state: PartialRenderState) {
        render_gui(gui, render_state, self);
    }

    fn update(&mut self) {
        for scene in self.scenes.as_mut_slice() {
            for model in scene.models.as_mut_slice() {
                for mesh in model.meshes.as_mut_slice() {
                    if mesh.dirty() {
                        mesh.update();
                    }
                }
            }
        }
        for material in self.materials.as_slice() {
            let dirty = { material.borrow().dirty() };
            if dirty {
                material.borrow_mut().update();
            }
        }
    }

    fn cleanup(&self) {
        info!("Cleaning up...");
        clear_run_dir();
    }
}

pub fn start(gltf_paths: Vec<&str>) {
    let setup_info = init_renderer();

    let viewport = Viewport {
        origin: [0.0, 0.0],
        dimensions: setup_info.window.inner_size().into(),
        depth_range: 0.0..1.0,
    };

    let mut cmd_buf_builder = AutoCommandBufferBuilder::primary(
        &setup_info.cmd_buf_allocator,
        setup_info.queue.queue_family_index(),
        CommandBufferUsage::OneTimeSubmit,
    )
    .unwrap();

    let (default_material, default_texture) = {
        let img = image::open("assets/textures/no_texture.png")
            .expect("Couldn't load default texture")
            .to_rgba8();
        let width = img.width();
        let height = img.height();
        let dyn_img = DynamicImage::from(img);

        let path = extract_image_to_file("no_texture", &dyn_img, Png);

        let tex = create_texture(
            dyn_img.into_bytes(),
            format::Format::R8G8B8A8_UNORM,
            width,
            height,
            &setup_info.memory_allocator,
            &mut cmd_buf_builder,
        );

        let texture = Rc::new(Texture::from(
            tex,
            Some(Box::from("Default texture")),
            0,
            path,
        ));

        (
            Rc::new(RefCell::new(Material::from_default(
                Some(texture.clone()),
                Buffer::from_data(
                    &setup_info.memory_allocator,
                    BufferCreateInfo {
                        usage: BufferUsage::STORAGE_BUFFER,
                        ..Default::default()
                    },
                    AllocationCreateInfo {
                        usage: MemoryUsage::Upload,
                        ..Default::default()
                    },
                    MaterialInfo {
                        base_color: [1.0, 1.0, 1.0, 1.0],
                        base_texture: 0,
                    },
                )
                .expect("Couldn't allocate MaterialInfo uniform"),
            ))),
            texture.clone(),
        )
    };

    // Load scene
    let mut scenes: Vec<Scene> = vec![];
    let mut textures: Vec<Rc<Texture>> = vec![default_texture];
    let mut materials: Vec<Rc<RefCell<Material>>> = vec![default_material.clone()];
    let mut tex_i = 1; // 0 reserved for default tex, mat
    let mut mat_i = 1;
    for gltf_path in gltf_paths {
        let (mut gltf_scenes, gltf_textures, gltf_materials) = load_gltf(
            gltf_path,
            &setup_info.memory_allocator,
            &mut cmd_buf_builder,
            default_material.clone(),
            &mut tex_i,
            &mut mat_i,
        );
        scenes.append(&mut gltf_scenes);
        let texture_values: Vec<Rc<Texture>> = gltf_textures
            .into_iter()
            .sorted_by_key(|x| x.0)
            .map(|x| x.1)
            .collect_vec();

        textures.append(&mut texture_values.clone()); //TODO investigate if this is too performance-heavy?
        let material_values: Vec<Rc<RefCell<Material>>> = gltf_materials
            .into_iter()
            .sorted_by_key(|x| x.0)
            .map(|x| x.1)
            .collect_vec();

        materials.append(&mut material_values.clone());
    }

    let mut vertex_buffers: Vec<VertexInputBuffer> = vec![];
    let mut normal_buffers: Vec<VertexInputBuffer> = vec![];
    let mut uv_buffers: Vec<VertexInputBuffer> = vec![];
    let mut index_buffers: Vec<Subbuffer<[u32]>> = vec![];
    let mut mesh_info_bufs = vec![];
    for scene in scenes.as_slice() {
        for model in scene.models.as_slice() {
            for mesh in model.meshes.as_slice() {
                let (vert_buf, normal_buf, uv_buf, index_buf) =
                    create_buffers(mesh, &setup_info.memory_allocator);
                vertex_buffers.push(VertexInputBuffer {
                    subbuffer: vert_buf.into_bytes(),
                    vertex_count: mesh.vertices.len() as u32,
                });
                normal_buffers.push(VertexInputBuffer {
                    subbuffer: normal_buf.into_bytes(),
                    vertex_count: mesh.normals.len() as u32,
                });
                uv_buffers.push(VertexInputBuffer {
                    subbuffer: uv_buf.into_bytes(),
                    vertex_count: mesh.uvs.len() as u32,
                });
                index_buffers.push(index_buf);
                mesh_info_bufs.push(mesh.buffer.clone());
            }
        }
    }

    let camera = Camera::new_default(
        viewport.dimensions[0],
        viewport.dimensions[1],
        &setup_info.memory_allocator,
    );

    let global_state = GlobalState {
        scenes,
        materials,
        textures,
    };

    let device = setup_info.device.clone();
    let render_pass = setup_info.render_pass.clone();
    let texs = global_state.textures.iter().map(|t| {
        (
            t.view.clone() as Arc<dyn ImageViewAbstract>,
            Sampler::new(device.clone(), SamplerCreateInfo::simple_repeat_linear()).unwrap(),
        )
    });

    let material_info_bufs = global_state
        .materials
        .as_slice()
        .iter()
        .map(|mat| mat.borrow().buffer.clone()); //TODO so many clones!

    let pbr_pipeline = PBRPipeline::new(
        device.clone(),
        vertex_buffers
            .into_iter()
            .zip(normal_buffers.into_iter())
            .zip(uv_buffers.into_iter())
            .zip(index_buffers.into_iter())
            .map(
                |(((vertex_buffer, normal_buffer), uv_buffer), index_buffer)| {
                    DrawableVertexInputs {
                        vertex_buffer,
                        normal_buffer,
                        uv_buffer,
                        index_buffer,
                    }
                },
            )
            .collect(),
        camera.buffer.clone(),
        texs,
        material_info_bufs,
        mesh_info_bufs.into_iter(),
        viewport.clone(),
        render_pass.clone(),
    );

    let line_vertex_buffers: Vec<VertexInputBuffer> = (0..10)
        .map(|_| VertexInputBuffer {
            subbuffer: Buffer::from_iter(
                &setup_info.memory_allocator,
                BufferCreateInfo {
                    usage: BufferUsage::VERTEX_BUFFER,
                    ..Default::default()
                },
                AllocationCreateInfo {
                    usage: MemoryUsage::Upload,
                    ..Default::default()
                },
                (0..2).map(|_| {
                    [
                        rand::thread_rng().gen_range(-10f32..10f32),
                        rand::thread_rng().gen_range(-10f32..10f32),
                        rand::thread_rng().gen_range(-10f32..10f32),
                    ]
                }),
            )
            .expect("Couldn't allocate vertex buffer")
            .into_bytes(),
            vertex_count: 2,
        })
        .collect_vec();

    let line_info_buffers = (0..10).map(|i| {
        Buffer::from_data(
            &setup_info.memory_allocator,
            BufferCreateInfo {
                usage: BufferUsage::STORAGE_BUFFER,
                ..Default::default()
            },
            AllocationCreateInfo {
                usage: MemoryUsage::Upload,
                ..Default::default()
            },
            LineInfo {
                model_transform: Mat4::default().to_cols_array_2d(),
                color: [
                    1.0 / (i as f32 + 1.0),
                    1.0 / (i as f32 + 1.0),
                    1.0 / (i as f32 + 1.0),
                    1.0,
                ],
            },
        )
        .expect("Couldn't allocate vertex buffer")
        .into_bytes()
    });

    let line_pipeline = LinePipeline::new(
        device.clone(),
        line_vertex_buffers,
        camera.buffer.clone(),
        line_info_buffers,
        viewport.clone(),
        render_pass,
    );

    start_renderer(
        RenderState {
            init_state: setup_info,
            viewport,
            cmd_buf_builder,
            camera,
        },
        vec![Box::from(pbr_pipeline), Box::from(line_pipeline)],
        global_state,
    );
}
