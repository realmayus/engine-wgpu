use std::path::PathBuf;
use std::sync::Arc;
use std::vec::Vec;

use egui_winit_vulkano::Gui;
use glam::Mat4;
use image::DynamicImage;
use image::ImageFormat::Png;
use itertools::Itertools;
use log::info;
use rand::Rng;
use vulkano::buffer::{Buffer, BufferCreateInfo, BufferUsage};
use vulkano::command_buffer::{
    AutoCommandBufferBuilder, CommandBufferUsage, PrimaryAutoCommandBuffer,
};
use vulkano::format;
use vulkano::image::ImageViewAbstract;
use vulkano::memory::allocator::{AllocationCreateInfo, MemoryUsage, StandardMemoryAllocator};
use vulkano::pipeline::graphics::viewport::Viewport;
use vulkano::sampler::{Sampler, SamplerCreateInfo};

use lib::scene::{DrawableVertexInputs, Material, MaterialManager, Texture, TextureManager, World};
use lib::shader_types::{LineInfo, MaterialInfo};
use lib::texture::create_texture;
use lib::{Dirtyable, VertexInputBuffer};
use renderer::camera::Camera;
use renderer::pipelines::line_pipeline::LinePipelineProvider;
use renderer::pipelines::pbr_pipeline::PBRPipelineProvider;
use renderer::pipelines::PipelineProviderKind;
use renderer::{init_renderer, start_renderer, PartialRenderState, RenderState, StateCallable};
use systems::io::gltf_loader::load_gltf;
use systems::io::{clear_run_dir, extract_image_to_file};

use crate::gui::render_gui;

pub(crate) struct InnerState {
    pub(crate) world: World,
    pub(crate) opened_file: Option<PathBuf>,
}
pub(crate) struct GlobalState {
    pub(crate) inner_state: InnerState,
    pub(crate) commands: Vec<Box<dyn Command>>,
}

pub(crate) trait Command {
    fn execute(&self, state: &mut InnerState, pipeline_providers: &mut [PipelineProviderKind]);
}

pub(crate) struct DeleteModelCommand {
    pub(crate) to_delete: u32,
}

impl Command for DeleteModelCommand {
    fn execute(&self, state: &mut InnerState, pipeline_providers: &mut [PipelineProviderKind]) {
        for scene in state.world.scenes.as_mut_slice() {
            let mut models = vec![];
            for m in scene.models.clone() {
                //TODO get rid of this clone
                if m.id != self.to_delete {
                    models.push(m);
                    break;
                }
            }
            scene.models = models;
        }
        for pipeline_provider in pipeline_providers {}
    }
}

pub(crate) struct UpdateModelCommand {
    pub(crate) to_update: u32,
    pub(crate) parent_transform: Mat4,
    pub(crate) local_transform: Mat4,
}

impl Command for UpdateModelCommand {
    fn execute(&self, state: &mut InnerState, pipeline_providers: &mut [PipelineProviderKind]) {
        for scene in state.world.scenes.as_mut_slice() {
            for m in scene.models.as_mut_slice() {
                if m.id == self.to_update {
                    m.local_transform = self.local_transform;
                    m.update_transforms(self.parent_transform);
                }
            }
        }
    }
}

impl StateCallable for GlobalState {
    fn setup_gui(&mut self, gui: &mut Gui, render_state: PartialRenderState) {
        render_gui(gui, render_state, self);
    }

    fn update(&mut self, pipeline_providers: &mut [PipelineProviderKind]) {
        for command in self.commands.as_slice() {
            command.execute(&mut self.inner_state, pipeline_providers);
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
    }

    fn cleanup(&self) {
        info!("Cleaning up...");
        clear_run_dir();
    }
}

fn load_default_world(
    mut texture_manager: TextureManager,
    mut material_manager: MaterialManager,
    memory_allocator: &StandardMemoryAllocator,
    cmd_buf_builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
) -> World {
    let cube = load_gltf(
        PathBuf::from("assets")
            .join("models")
            .join("cube.glb")
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

    let mut texture_manager = TextureManager::new();
    let mut material_manager = MaterialManager::new();
    {
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

        let texture = Texture::from(tex, Some(Box::from("Default texture")), 0, path);
        texture_manager.add_texture(texture);

        let material = Material::from_default(
            Some(texture_manager.get_texture(0)),
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
        );

        material_manager.add_material(material);
    };

    let camera = Camera::new_default(
        viewport.dimensions[0],
        viewport.dimensions[1],
        &setup_info.memory_allocator,
    );

    let world = load_default_world(
        texture_manager,
        material_manager,
        &setup_info.memory_allocator,
        &mut cmd_buf_builder,
    );

    let global_state = GlobalState {
        inner_state: InnerState {
            world,
            opened_file: None,
        },
        commands: vec![],
    };

    let device = setup_info.device.clone();
    let render_pass = setup_info.render_pass.clone();
    let texs = global_state.inner_state.world.textures.iter().map(|t| {
        (
            t.view.clone() as Arc<dyn ImageViewAbstract>,
            Sampler::new(device.clone(), SamplerCreateInfo::simple_repeat_linear()).unwrap(),
        )
    });

    let material_info_bufs = global_state
        .inner_state
        .world
        .materials
        .iter()
        .map(|mat| mat.borrow().buffer.clone()); //TODO so many clones!

    let mesh_info_bufs = global_state
        .inner_state
        .world
        .get_active_scene()
        .iter_meshes()
        .map(|mesh| mesh.buffer.clone())
        .collect_vec()
        .into_iter();

    let pbr_pipeline = PBRPipelineProvider::new(
        device.clone(),
        global_state
            .inner_state
            .world
            .get_active_scene()
            .iter_meshes()
            .map(|mesh| DrawableVertexInputs::from_mesh(mesh, &setup_info.memory_allocator))
            .collect_vec(),
        camera.buffer.clone(),
        texs,
        material_info_bufs,
        mesh_info_bufs,
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

    let line_pipeline = LinePipelineProvider::new(
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
        vec![
            PipelineProviderKind::PBR(pbr_pipeline),
            PipelineProviderKind::LINE(line_pipeline),
        ],
        global_state,
    );
}
