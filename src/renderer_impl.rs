use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::vec::Vec;

use egui_winit_vulkano::Gui;
use glam::Mat4;
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

use lib::scene::{Material, Texture, World};
use lib::shader_types::{LineInfo, MaterialInfo};
use lib::texture::create_texture;
use lib::{Dirtyable, VertexBuffer};
use renderer::camera::Camera;
use renderer::initialization::init_renderer;
use renderer::pipelines::line_pipeline::LinePipeline;
use renderer::pipelines::pbr_pipeline::PBRPipeline;
use renderer::renderer::start_renderer;
use renderer::{PartialRenderState, RenderState, StateCallable};
use systems::io::gltf_loader::load_gltf;
use systems::io::{clear_run_dir, extract_image_to_file};

use crate::gui::render_gui;

pub(crate) struct GlobalState {
    pub(crate) world: World,
    pub(crate) opened_file: Option<PathBuf>,
    line_vertex_buffers: Vec<VertexBuffer>,
}

impl StateCallable for GlobalState {
    fn setup_gui(&mut self, gui: &mut Gui, render_state: PartialRenderState) {
        render_gui(gui, render_state, self);
    }

    fn update(&mut self) {
        for scene in self.world.scenes.as_mut_slice() {
            for model in scene.models.as_mut_slice() {
                for mesh in model.meshes.as_mut_slice() {
                    if mesh.dirty() {
                        mesh.update();
                    }
                }
            }
        }
        for material in self.world.materials.values() {
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

    fn get_subbuffers(
        &mut self,
        memory_allocator: &StandardMemoryAllocator,
    ) -> VecDeque<(
        Vec<VertexBuffer>,
        Vec<VertexBuffer>,
        Vec<VertexBuffer>,
        Vec<Subbuffer<[u32]>>,
    )> {
        VecDeque::from([
            (
                self.world
                    .cached_vertex_buffers
                    .clone()
                    .expect("Vertex buffers uninitialized!"),
                self.world
                    .cached_normal_buffers
                    .clone()
                    .expect("Normal buffers uninitialized!"),
                self.world
                    .cached_uv_buffers
                    .clone()
                    .expect("UV buffers uninitialized!"),
                self.world
                    .cached_index_buffers
                    .clone()
                    .expect("Index buffers uninitialized!"),
            ),
            (self.line_vertex_buffers.clone(), vec![], vec![], vec![]),
        ])
    }
}

fn load_default_world(
    default_texture: Rc<Texture>,
    default_material: Rc<RefCell<Material>>,
    memory_allocator: &StandardMemoryAllocator,
    cmd_buf_builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
) -> World {
    let (cube, texs, mats) = load_gltf(
        PathBuf::from("assets")
            .join("models")
            .join("cube.glb")
            .as_path(),
        memory_allocator,
        cmd_buf_builder,
        default_material.clone(),
        &mut 1,
        &mut 1,
    );
    World {
        textures: HashMap::from([(0u32, default_texture)])
            .into_iter()
            .chain(texs)
            .collect(),
        materials: HashMap::from([(0u32, default_material)])
            .into_iter()
            .chain(mats)
            .collect(),
        scenes: cube,
        cached_vertex_buffers: None,
        cached_normal_buffers: None,
        cached_uv_buffers: None,
        cached_index_buffers: None,
        highest_material_index: 1,
        highest_texture_index: 1,
    }
}

fn load_default_mat_tex(
    memory_allocator: &StandardMemoryAllocator,
    cmd_buf_builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
) -> (Rc<RefCell<Material>>, Rc<Texture>) {
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
        memory_allocator,
        cmd_buf_builder,
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
                memory_allocator,
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
}

pub fn start() {
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

    let camera = Camera::new_default(
        viewport.dimensions[0],
        viewport.dimensions[1],
        &setup_info.memory_allocator,
    );

    let (default_material, default_texture) =
        load_default_mat_tex(&setup_info.memory_allocator, &mut cmd_buf_builder);

    let default_world = load_default_world(
        default_texture,
        default_material,
        &setup_info.memory_allocator,
        &mut cmd_buf_builder,
    );

    let mut global_state = GlobalState {
        world: default_world,
        opened_file: None,
        line_vertex_buffers: (0..10)
            .map(|_| VertexBuffer {
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
            .collect_vec(),
    };

    let device = setup_info.device.clone();
    let render_pass = setup_info.render_pass.clone();
    let texs = global_state.world.textures.values().map(|t| {
        (
            t.view.clone() as Arc<dyn ImageViewAbstract>,
            Sampler::new(device.clone(), SamplerCreateInfo::simple_repeat_linear()).unwrap(),
        )
    });

    let pbr_pipeline = PBRPipeline::new(
        device.clone(),
        camera.buffer.clone(),
        texs,
        (vec![global_state
            .world
            .materials
            .get(&0)
            .unwrap()
            .borrow()
            .buffer
            .clone()])
        .into_iter(),
        global_state
            .world
            .scenes
            .iter()
            .flat_map(|s| s.iter_meshes())
            .map(|m| m.buffer.clone())
            .collect_vec()
            .into_iter(),
        viewport.clone(),
        render_pass.clone(),
    );

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
        camera.buffer.clone(),
        line_info_buffers,
        viewport.clone(),
        render_pass,
    );

    global_state
        .world
        .create_vertex_buffers(&setup_info.memory_allocator);
    global_state
        .world
        .create_normal_buffers(&setup_info.memory_allocator);
    global_state
        .world
        .create_uv_buffers(&setup_info.memory_allocator);
    global_state
        .world
        .create_index_buffers(&setup_info.memory_allocator);

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
