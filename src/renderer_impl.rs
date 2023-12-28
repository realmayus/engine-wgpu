use std::path::PathBuf;
use std::sync::Arc;
use std::vec::Vec;

use egui_winit_vulkano::Gui;
use glam::{Mat4, Vec2};
use image::DynamicImage;
use image::ImageFormat::Png;
use itertools::Itertools;
use log::info;
use rand::Rng;
use vulkano::buffer::{Buffer, BufferCreateInfo, BufferUsage, Subbuffer};
use vulkano::command_buffer::allocator::StandardCommandBufferAllocator;
use vulkano::command_buffer::{
    AutoCommandBufferBuilder, CommandBufferUsage, PrimaryAutoCommandBuffer,
};
use vulkano::descriptor_set::allocator::StandardDescriptorSetAllocator;
use vulkano::device::Device;
use vulkano::format;
use vulkano::image::sampler::Sampler;
use vulkano::image::view::ImageView;
use vulkano::memory::allocator::{AllocationCreateInfo, MemoryTypeFilter, StandardMemoryAllocator};
use vulkano::pipeline::graphics::viewport::Viewport;

use lib::scene::{DrawableVertexInputs, Material, MaterialManager, Texture, TextureManager, World};
use lib::shader_types::{CameraUniform, LightInfo, LineInfo, MaterialInfo, MeshInfo};
use lib::texture::create_texture;
use lib::util::extract_image_to_file;
use lib::{Dirtyable, VertexInputBuffer};
use renderer::camera::{Camera, KeyState};
use renderer::initialization::init_renderer;
use renderer::pipelines::line_pipeline::LinePipelineProvider;
use renderer::pipelines::pbr_pipeline::PBRPipelineProvider;
use renderer::pipelines::PipelineProviderKind;
use renderer::render_loop::start_renderer;
use renderer::{RenderState, StateCallable};
use systems::io::clear_run_dir;
use systems::io::gltf_loader::load_gltf;

use crate::commands::Command;
use crate::gui::render_gui;

pub(crate) struct InnerState {
    pub(crate) world: World,
    pub(crate) opened_file: Option<PathBuf>,
    pub(crate) camera: Camera,
}
pub(crate) struct GlobalState {
    pub(crate) inner_state: InnerState,
    pub(crate) commands: Vec<Box<dyn Command>>,
}

impl StateCallable for GlobalState {
    fn setup_gui(&mut self, gui: &mut Gui) {
        render_gui(gui, self);
    }

    fn update(
        &mut self,
        pipeline_providers: &mut [PipelineProviderKind],
        allocator: Arc<StandardMemoryAllocator>,
        descriptor_set_allocator: Arc<StandardDescriptorSetAllocator>,
        cmd_buf_allocator: Arc<StandardCommandBufferAllocator>,
        queue_family_index: u32,
        device: Arc<Device>,
        viewport: Viewport,
    ) -> Option<Arc<PrimaryAutoCommandBuffer>> {
        let mut cmd_buf_builder = AutoCommandBufferBuilder::primary(
            //TODO would it make sense to only use builder if a command requests it?
            cmd_buf_allocator.clone().as_ref(),
            queue_family_index,
            CommandBufferUsage::OneTimeSubmit,
        )
        .unwrap();
        for command in self.commands.as_slice() {
            command.execute(
                &mut self.inner_state,
                pipeline_providers,
                allocator.clone(),
                descriptor_set_allocator.as_ref(),
                &mut cmd_buf_builder,
                device.clone(),
            );
        }

        self.commands.clear();

        for scene in self.inner_state.world.scenes.as_mut_slice() {
            for model in scene.models.as_mut_slice() {
                for mesh in model.meshes.as_mut_slice() {
                    if mesh.dirty() {
                        mesh.update();
                    }
                }
            }
        }
        for material in self.inner_state.world.materials.iter() {
            let dirty = { material.borrow().dirty() };
            if dirty {
                material.borrow_mut().update();
            }
        }

        self.inner_state
            .camera
            .update_aspect(viewport.extent[0], viewport.extent[1]);
        self.inner_state.camera.update_view();

        Some(cmd_buf_builder.build().unwrap())
    }

    fn cleanup(&self) {
        info!("Cleaning up...");
        clear_run_dir();
    }

    fn get_buffers(
        &self,
        device: Arc<Device>,
    ) -> (
        Subbuffer<CameraUniform>,
        Vec<(Arc<ImageView>, Arc<Sampler>)>,
        Vec<Subbuffer<MaterialInfo>>,
        Vec<Subbuffer<MeshInfo>>,
        Vec<Subbuffer<LightInfo>>,
    ) {
        let texs = self
            .inner_state
            .world
            .textures
            .get_view_sampler_array(device.clone());

        let material_info_bufs = self.inner_state.world.materials.get_buffer_array();

        let mesh_info_bufs = self
            .inner_state
            .world
            .get_active_scene()
            .iter_meshes()
            .map(|mesh| mesh.buffer.clone())
            .collect_vec()
            .into_iter();

        let lights = self
            .inner_state
            .world
            .get_active_scene()
            .models
            .iter()
            .as_slice()
            .iter()
            .filter_map(|model| {
                if let Some(ref light) = model.light {
                    Some(light.buffer.clone())
                } else {
                    None
                }
            });

        (
            self.inner_state.camera.buffer.clone(),
            texs,
            material_info_bufs,
            mesh_info_bufs.collect_vec(),
            lights.collect_vec(),
        )
    }

    fn recv_input(
        &mut self,
        keys: &KeyState,
        change: Vec2,
        delta_time: f32,
    ) {
        self.inner_state.camera.recv_input(
            keys,
            change,
            delta_time,
        );
    }
}

fn load_default_world(
    mut texture_manager: TextureManager,
    mut material_manager: MaterialManager,
    memory_allocator: Arc<StandardMemoryAllocator>,
    cmd_buf_builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
) -> World {
    let cube = load_gltf(
        PathBuf::from("assets")
            .join("models")
            .join("DamagedHelmetTangents.glb")
            .as_path(),
        memory_allocator,
        cmd_buf_builder,
        &mut texture_manager,
        &mut material_manager,
    );
    World {
        textures: texture_manager,
        materials: material_manager,
        scenes: cube,
        active_scene: 0,
    }
}

pub fn start() {
    let setup_info = init_renderer();

    let viewport = Viewport {
        offset: [0.0, 0.0],
        extent: setup_info.window.inner_size().into(),
        depth_range: 0.0..=1.0,
    };

    let mut cmd_buf_builder = AutoCommandBufferBuilder::primary(
        setup_info.cmd_buf_allocator.clone().as_ref(),
        setup_info.queue.queue_family_index(),
        CommandBufferUsage::OneTimeSubmit,
    )
    .unwrap();

    let mut texture_manager = TextureManager::new();
    let mut material_manager = MaterialManager::new();
    {
        let img = image::open("assets/textures/default.png")
            .expect("Couldn't load default texture")
            .to_rgba8();
        let width = img.width();
        let height = img.height();
        let dyn_img = DynamicImage::from(img);

        let path = extract_image_to_file("default", &dyn_img, Png);

        let tex = create_texture(
            dyn_img.into_bytes(),
            format::Format::R8G8B8A8_UNORM,
            width,
            height,
            setup_info.memory_allocator.clone(),
            &mut cmd_buf_builder,
        );

        let texture = Texture::from(tex, Some(Box::from("Default texture")), 0, path);
        texture_manager.add_texture(texture);

        let img = image::open("assets/textures/default_normal.png")
            .expect("Couldn't load default normal texture")
            .to_rgba8();
        let width = img.width();
        let height = img.height();
        let dyn_img = DynamicImage::from(img);

        let path = extract_image_to_file("default_normal", &dyn_img, Png);

        let tex = create_texture(
            dyn_img.into_bytes(),
            format::Format::R8G8B8A8_UNORM,
            width,
            height,
            setup_info.memory_allocator.clone(),
            &mut cmd_buf_builder,
        );
        let texture_normal = Texture::from(tex, Some(Box::from("Default normal texture")), 1, path);
        texture_manager.add_texture(texture_normal);

        let material = Material::from_default(
            Some(texture_manager.get_texture(0)),
            Buffer::from_data(
                setup_info.memory_allocator.clone(),
                BufferCreateInfo {
                    usage: BufferUsage::STORAGE_BUFFER,
                    ..Default::default()
                },
                AllocationCreateInfo {
                    memory_type_filter: MemoryTypeFilter::PREFER_DEVICE | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                    ..Default::default()
                },
                MaterialInfo::default(),
            )
            .expect("Couldn't allocate MaterialInfo uniform"),
        );

        material_manager.add_material(material);
    };

    {
        let img = image::open("assets/textures/white.png")
            .expect("Couldn't load white texture")
            .to_rgba8();
        let width = img.width();
        let height = img.height();
        let dyn_img = DynamicImage::from(img);

        let path = extract_image_to_file("white", &dyn_img, Png);

        let tex = create_texture(
            dyn_img.into_bytes(),
            format::Format::R8G8B8A8_UNORM,
            width,
            height,
            setup_info.memory_allocator.clone(),
            &mut cmd_buf_builder,
        );

        let texture = Texture::from(tex, Some(Box::from("White texture")), 1, path);
        texture_manager.add_texture(texture);
    }

    let camera = Camera::new_default(
        viewport.extent[0],
        viewport.extent[1],
        setup_info.memory_allocator.clone(),
    );

    let world = load_default_world(
        texture_manager,
        material_manager,
        setup_info.memory_allocator.clone(),
        &mut cmd_buf_builder,
    );

    let global_state = GlobalState {
        inner_state: InnerState {
            world,
            opened_file: None,
            camera,
        },
        commands: vec![],
    };

    let device = setup_info.device.clone();
    let render_pass = setup_info.render_pass.clone();
    let pbr_pipeline = PBRPipelineProvider::new(
        device.clone(),
        global_state
            .inner_state
            .world
            .get_active_scene()
            .iter_meshes()
            .map(|mesh| DrawableVertexInputs::from_mesh(mesh, setup_info.memory_allocator.clone()))
            .collect_vec(),
        viewport.clone(),
        render_pass.clone(),
    );

    let line_vertex_buffers: Vec<VertexInputBuffer> = (0..10)
        .map(|_| VertexInputBuffer {
            subbuffer: Buffer::from_iter(
                setup_info.memory_allocator.clone(),
                BufferCreateInfo {
                    usage: BufferUsage::VERTEX_BUFFER,
                    ..Default::default()
                },
                AllocationCreateInfo {
                    memory_type_filter: MemoryTypeFilter::PREFER_DEVICE | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
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
            setup_info.memory_allocator.clone(),
            BufferCreateInfo {
                usage: BufferUsage::STORAGE_BUFFER,
                ..Default::default()
            },
            AllocationCreateInfo {
                memory_type_filter: MemoryTypeFilter::PREFER_DEVICE | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
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

    let line_pipeline = LinePipelineProvider::new(
        device.clone(),
        line_vertex_buffers,
        global_state.inner_state.camera.buffer.clone(),
        line_info_buffers,
        viewport.clone(),
        render_pass,
    );

    start_renderer(
        RenderState {
            init_state: setup_info,
            viewport,
            cmd_buf_builder,
        },
        vec![
            PipelineProviderKind::PBR(pbr_pipeline),
            PipelineProviderKind::LINE(line_pipeline),
        ],
        global_state,
    );
}
