use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::vec::Vec;

use egui_winit_vulkano::egui::Ui;
use egui_winit_vulkano::{egui, Gui};
use glam::Mat4;
use image::DynamicImage;
use image::ImageFormat::Png;
use itertools::Itertools;
use log::{error, info};
use vulkano::buffer::{Buffer, BufferCreateInfo, BufferUsage, Subbuffer};
use vulkano::command_buffer::{
    AutoCommandBufferBuilder, CommandBufferUsage, PrimaryAutoCommandBuffer,
};
use vulkano::format;
use vulkano::image::ImageViewAbstract;
use vulkano::memory::allocator::{AllocationCreateInfo, MemoryUsage, StandardMemoryAllocator};
use vulkano::pipeline::graphics::viewport::Viewport;
use vulkano::sampler::{Sampler, SamplerCreateInfo};

use lib::scene::{Material, Mesh, Model, PointLight, Scene, Texture};
use lib::scene_serde::WorldSerde;
use lib::shader_types::{LineInfo, MaterialInfo};
use lib::texture::create_texture;
use lib::Dirtyable;
use renderer::camera::Camera;
use renderer::pipelines::line_pipeline::LinePipeline;
use renderer::pipelines::pbr_pipeline::PBRPipeline;
use renderer::{
    init_renderer, start_renderer, PartialRenderState, RenderState, StateCallable, VertexBuffer,
};
use systems::io;
use systems::io::gltf_loader::load_gltf;
use systems::io::{clear_run_dir, extract_image_to_file};

fn draw_model_collapsing(ui: &mut Ui, model: &mut Model, parent_transform: Mat4) {
    ui.collapsing(String::from(model.name.clone().unwrap_or_default()), |ui| {
        ui.label("Translation:");
        if ui
            .add(egui::Slider::new(&mut model.local_transform.w_axis.x, -10.0..=10.0).text("X"))
            .changed()
            || ui
                .add(egui::Slider::new(&mut model.local_transform.w_axis.y, -10.0..=10.0).text("Y"))
                .changed()
            || ui
                .add(egui::Slider::new(&mut model.local_transform.w_axis.z, -10.0..=10.0).text("Z"))
                .changed()
        {
            model.update_transforms(parent_transform);
        }

        ui.label("Meshes:");
        for mesh in model.meshes.as_slice() {
            ui.push_id(mesh.id, |ui| {
                ui.collapsing("Mesh", |ui| {
                    ui.label(format!(
                        "# of vert/norm/in: {}/{}/{}",
                        mesh.vertices.len(),
                        mesh.normals.len(),
                        mesh.indices.len()
                    ));
                    ui.label(
                        "Material: ".to_owned()
                            + &*String::from(
                                mesh.material.borrow().name.clone().unwrap_or_default(),
                            ),
                    );
                    if ui.button("Log material").clicked() {
                        info!("{:?}", mesh.material);
                    }
                })
            });
        }
        ui.separator();
        ui.label("Children:");
        for child in model.children.as_mut_slice() {
            draw_model_collapsing(ui, child, parent_transform * model.local_transform);
        }
    });
}

fn render_gui(gui: &mut Gui, render_state: PartialRenderState, state: &mut GlobalState) {
    let ctx = gui.context();
    egui::Window::new("Scene").show(&ctx, |ui| {
        ui.with_layout(egui::Layout::left_to_right(egui::Align::default()), |ui| {
            if ui.button("Load world").clicked() {}
            if ui.button("Save world").clicked() {
                if let Some(path) = rfd::FileDialog::new().pick_folder() {
                    io::world_saver::save(
                        path.as_path(),
                        WorldSerde::from(
                            state.textures.clone(),
                            state.materials.clone(),
                            state.scenes.clone(),
                        ),
                    )
                    .expect("Couldn't save world");
                }
            }
        });
        if ui.button("Import glTF").clicked() {
            if let Some(paths) = rfd::FileDialog::new()
                .add_filter("glTF scenes", &["gltf", "glb"])
                .pick_files()
            {
                for path in paths {
                    println!("{}", path.display());
                }
            }
        }
        ui.label("Loaded models:");
        for scene in state.scenes.as_mut_slice() {
            ui.push_id(scene.id, |ui| {
                ui.collapsing(String::from(scene.name.clone().unwrap_or_default()), |ui| {
                    ui.label(format!("# of models: {}", scene.models.len()));
                    for model in scene.models.as_mut_slice() {
                        draw_model_collapsing(ui, model, Mat4::default());
                    }
                });
            });
        }
    });

    egui::Window::new("Camera").show(&ctx, |ui| {
        ui.label(format!("Eye: {}", &render_state.camera.eye));
        ui.label(format!("Target: {}", &render_state.camera.target));
        ui.label(format!("Up: {}", &render_state.camera.up));
        ui.add(egui::Slider::new(&mut render_state.camera.speed, 0.03..=0.3).text("Speed"));
        ui.add(
            egui::Slider::new(&mut render_state.camera.fovy, 0.0..=(std::f32::consts::PI))
                .text("Field of view"),
        );
        if ui.button("Reset").clicked() {
            render_state.camera.reset();
        }
    });

    egui::Window::new("Materials").show(&ctx, |ui| {
        for mat in state.materials.as_slice() {
            let (id, name) = { (mat.borrow().id, mat.borrow().name.clone()) };
            ui.push_id(id, |ui| {
                ui.collapsing(String::from(name.unwrap_or_default()), |ui| {
                    if ui.button("Update").clicked() {
                        mat.clone().borrow_mut().set_dirty(true);
                    }
                    ui.label(format!("Base color factors: {}", mat.borrow().albedo));
                    ui.label(format!(
                        "Metallic roughness factors: {}",
                        mat.borrow().metallic_roughness_factors
                    ));
                    ui.add(
                        egui::Slider::new(
                            &mut mat.borrow_mut().metallic_roughness_factors.x,
                            0.0..=1.0,
                        )
                        .text("Metallicness"),
                    );
                    ui.add(
                        egui::Slider::new(
                            &mut mat.borrow_mut().metallic_roughness_factors.y,
                            0.0..=1.0,
                        )
                        .text("Roughness"),
                    );
                    ui.label(format!(
                        "Emissive factors: {}",
                        mat.borrow().emissive_factors
                    ));
                    ui.label(format!(
                        "Occlusion strength: {}",
                        mat.borrow().occlusion_factor
                    ));
                    ui.separator();
                    ui.label(format!(
                        "Base color texture: {:?}",
                        mat.borrow().albedo_texture
                    ));
                    ui.label(format!("Normal texture: {:?}", mat.borrow().normal_texture));
                    ui.label(format!(
                        "Metallic roughness texture: {:?}",
                        mat.borrow().metallic_roughness_texture
                    ));
                    ui.label(format!(
                        "Emissive texture: {:?}",
                        mat.borrow().emissive_texture
                    ));
                    ui.label(format!(
                        "Occlusion texture: {:?}",
                        mat.borrow().occlusion_texture
                    ));
                });
            });
        }
    });

    egui::Window::new("Textures").show(&ctx, |ui| {
        for tex in state.textures.as_slice() {
            ui.label(format!("Id: {}", tex.id));
            ui.label(format!(
                "Name: {}",
                String::from(tex.name.clone().unwrap_or_default())
            ));
        }
    });
}

fn load_preview_meshes(
    preview_models: Vec<&str>,
    memory_allocator: &StandardMemoryAllocator,
    cmd_buf_builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
    default_material: Rc<RefCell<Material>>,
) -> (Vec<VertexBuffer>, Vec<VertexBuffer>, Vec<Subbuffer<[u32]>>) {
    let mut preview_vertices: Vec<VertexBuffer> = vec![];
    let mut normal_buffers: Vec<VertexBuffer> = vec![];
    let mut tangent_buffers = vec![];
    let mut index_buffers: Vec<Subbuffer<[u32]>> = vec![];

    for gltf_path in preview_models {
        let (scenes, ..) = load_gltf(
            gltf_path,
            memory_allocator,
            cmd_buf_builder,
            default_material.clone(),
            &mut 0,
            &mut 0,
        );
        for scene in scenes {
            for model in scene.models {
                for mesh in model.meshes {
                    let buffers = create_buffers(&mesh, memory_allocator);
                    preview_vertices.push(VertexBuffer {
                        subbuffer: buffers.vert_buf.into_bytes(),
                        vertex_count: mesh.vertices.len() as u32,
                    });
                    normal_buffers.push(VertexBuffer {
                        subbuffer: buffers.normal_buf.into_bytes(),
                        vertex_count: mesh.vertices.len() as u32,
                    });
                    tangent_buffers.push(VertexBuffer {
                        subbuffer: buffers.tangent_buf.into_bytes(),
                        vertex_count: mesh.vertices.len() as u32,
                    });
                    index_buffers.push(buffers.index_buf);
                }
            }
        }
    }
    (preview_vertices, normal_buffers, index_buffers)
}

struct Buffers {
    pub vert_buf: Subbuffer<[[f32; 3]]>,
    pub normal_buf: Subbuffer<[[f32; 3]]>,
    pub tangent_buf: Subbuffer<[[f32; 4]]>,
    pub uv_buf: Subbuffer<[[f32; 2]]>,
    pub index_buf: Subbuffer<[u32]>,
}

fn create_buffers(mesh: &Mesh, memory_allocator: &StandardMemoryAllocator) -> Buffers {
    Buffers {
        vert_buf: Buffer::from_iter(
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
        .expect("Couldn't allocate vertex buffer"),

        normal_buf: Buffer::from_iter(
            memory_allocator,
            BufferCreateInfo {
                usage: BufferUsage::VERTEX_BUFFER,
                ..Default::default()
            },
            AllocationCreateInfo {
                usage: MemoryUsage::Upload,
                ..Default::default()
            },
            mesh.normals.iter().map(|v| v.to_array()),
        )
        .expect("Couldn't allocate normal buffer"),

        tangent_buf: Buffer::from_iter(
            memory_allocator,
            BufferCreateInfo {
                usage: BufferUsage::VERTEX_BUFFER,
                ..Default::default()
            },
            AllocationCreateInfo {
                usage: MemoryUsage::Upload,
                ..Default::default()
            },
            if mesh.tangents.is_empty() {
                error!("Tangent buffer is emtpy!");
                panic!()
            } else {
                mesh.tangents.iter().map(|t| t.to_array())
            },
        )
        .expect("Couldn't allocate tangent buffer"),

        uv_buf: Buffer::from_iter(
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
        .expect("Couldn't allocate UV buffer"),

        index_buf: Buffer::from_iter(
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
        .expect("Couldn't allocate index buffer"),
    }
}

struct GlobalState {
    scenes: Vec<Scene>,
    materials: Vec<Rc<RefCell<Material>>>,
    textures: Vec<Rc<Texture>>,
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
                if let Some(ref mut light) = model.light {
                    if light.dirty {
                        light.update();
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
    // let preview_models = vec![
    //     "assets/models/cube.glb",
    //     "assets/models/sphere.glb",
    //     "assets/models/suzanne.glb",
    // ];

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

    let (default_material, default_texture, default_normal) = {
        // default texture
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
            &setup_info.memory_allocator,
            &mut cmd_buf_builder,
        );
        let texture = Rc::new(Texture::from(
            tex,
            Some(Box::from("Default texture")),
            0,
            path,
        ));

        // default normal texture
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
            &setup_info.memory_allocator,
            &mut cmd_buf_builder,
        );
        let texture_normal = Rc::new(Texture::from(
            tex,
            Some(Box::from("Default texture")),
            1,
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
                    MaterialInfo::default(),
                )
                .expect("Couldn't allocate MaterialInfo uniform"),
            ))),
            texture,
            texture_normal,
        )
    };

    // Load preview meshes (for material/texture preview)
    // load_preview_meshes(
    //     preview_models,
    //     &setup_info.memory_allocator,
    //     &mut cmd_buf_builder,
    //     default_material.clone(),
    // );

    // Load scene
    let mut scenes: Vec<Scene> = vec![];
    let mut textures: Vec<Rc<Texture>> = vec![default_texture, default_normal];
    let mut materials: Vec<Rc<RefCell<Material>>> = vec![default_material.clone()];
    let mut tex_i = 2; // 0 reserved for default texture, 1 reserved for default normal texture
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

        let mut texture_values: Vec<Rc<Texture>> = gltf_textures
            .into_iter()
            .sorted_by_key(|x| x.0)
            .map(|x| x.1)
            .collect_vec();

        textures.append(&mut texture_values); // TODO investigate if this is too performance-heavy?

        let mut material_values: Vec<Rc<RefCell<Material>>> = gltf_materials
            .into_iter()
            .sorted_by_key(|x| x.0)
            .map(|x| x.1)
            .collect_vec();

        materials.append(&mut material_values);
    }

    let mut vertex_buffers: Vec<VertexBuffer> = vec![];
    let mut normal_buffers: Vec<VertexBuffer> = vec![];
    let mut tangent_buffers: Vec<VertexBuffer> = vec![];
    let mut uv_buffers: Vec<VertexBuffer> = vec![];
    let mut index_buffers: Vec<Subbuffer<[u32]>> = vec![];
    let mut mesh_info_bufs = vec![];
    let mut lights_buffer: Vec<PointLight> = vec![];

    for scene in scenes.as_slice() {
        println!("{:?}", scene);
        for model in scene.models.as_slice() {
            println!("{:?}", model);

            if let Some(point_light) = model.light.clone() {
                lights_buffer.push(point_light);
            }

            for mesh in model.meshes.as_slice() {
                println!("{:?}", mesh);
                let buffers = create_buffers(mesh, &setup_info.memory_allocator);
                vertex_buffers.push(VertexBuffer {
                    subbuffer: buffers.vert_buf.into_bytes(),
                    vertex_count: mesh.vertices.len() as u32,
                });
                normal_buffers.push(VertexBuffer {
                    subbuffer: buffers.normal_buf.into_bytes(),
                    vertex_count: mesh.normals.len() as u32,
                });
                tangent_buffers.push(VertexBuffer {
                    subbuffer: buffers.tangent_buf.into_bytes(),
                    vertex_count: mesh.tangents.len() as u32,
                });
                uv_buffers.push(VertexBuffer {
                    subbuffer: buffers.uv_buf.into_bytes(),
                    vertex_count: mesh.uvs.len() as u32,
                });
                index_buffers.push(buffers.index_buf);
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
        .map(|mat| (mat.borrow().buffer.clone(), 0..mat.borrow().buffer.size())); //TODO so many clones!

    println!("# of materialUniforms: {}", material_info_bufs.len());

    let lights_buffer = lights_buffer.into_iter().map(|light| light.buffer.clone());

    let pbr_pipeline = PBRPipeline::new(
        device.clone(),
        vertex_buffers,
        normal_buffers,
        tangent_buffers,
        uv_buffers,
        index_buffers,
        camera.buffer.clone(),
        texs,
        material_info_bufs,
        mesh_info_bufs
            .into_iter()
            .map(|mesh_info| (mesh_info.clone(), 0..mesh_info.size())),
        lights_buffer,
        viewport.clone(),
        render_pass.clone(),
    );

    let line_vertex_buffers: Vec<VertexBuffer> = (0..3)
        .map(|axis| VertexBuffer {
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
                (0..2).map(|vert| {
                    let sign = if vert == 0 { 1. } else { -1. };
                    match axis {
                        0 => [sign * 1000.0, 0.0, 0.0],
                        1 => [0.0, sign * 1000.0, 0.0],
                        2 => [0.0, 0.0, sign * 1000.0],
                        _ => [0.0, 0.0, 0.0],
                    }
                }),
            )
            .expect("Couldn't allocate vertex buffer")
            .into_bytes(),
            vertex_count: 2,
        })
        .collect_vec();

    let line_info_buffers = (0..3).map(|i| {
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
                color: match i {
                    0 => [1.0, 0.0, 0.0, 1.0],
                    1 => [0.0, 1.0, 0.0, 1.0],
                    2 => [0.0, 0.0, 1.0, 1.0],
                    _ => [1.0, 1.0, 1.0, 1.0],
                },
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
