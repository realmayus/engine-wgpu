use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::vec::Vec;

use egui_winit_vulkano::egui::Ui;
use egui_winit_vulkano::{egui, Gui};
use glam::Mat4;
use image::DynamicImage;
use itertools::Itertools;
use vulkano::buffer::{Buffer, BufferContents, BufferCreateInfo, BufferUsage, Subbuffer};
use vulkano::command_buffer::{
    AutoCommandBufferBuilder, CommandBufferUsage, PrimaryAutoCommandBuffer,
};
use vulkano::descriptor_set::WriteDescriptorSet;
use vulkano::device::Device;
use vulkano::format;
use vulkano::image::ImageViewAbstract;
use vulkano::memory::allocator::{AllocationCreateInfo, MemoryUsage, StandardMemoryAllocator};
use vulkano::pipeline::graphics::depth_stencil::DepthStencilState;
use vulkano::pipeline::graphics::input_assembly::InputAssemblyState;
use vulkano::pipeline::graphics::vertex_input::Vertex;
use vulkano::pipeline::graphics::viewport::{Viewport, ViewportState};
use vulkano::pipeline::GraphicsPipeline;
use vulkano::render_pass::{RenderPass, Subpass};
use vulkano::sampler::{Sampler, SamplerCreateInfo};
use vulkano::shader::ShaderModule;

use lib::scene::{Material, Mesh, Model, Scene, Texture};
use lib::util::shader_types::MaterialInfo;
use lib::util::texture::create_texture;
use lib::Dirtyable;
use renderer::camera::Camera;
use renderer::{
    init_renderer, start_renderer, PartialRenderState, RenderState, StateCallable, VertexBuffer,
};
use systems::io::gltf_loader::load_gltf;

#[derive(BufferContents, Vertex)]
#[repr(C)]
struct MyVertex {
    #[format(R32G32B32_SFLOAT)]
    position: [f32; 3],
}

#[derive(BufferContents, Vertex)]
#[repr(C)]
struct MyNormal {
    #[format(R32G32B32_SFLOAT)]
    normal: [f32; 3],
}

#[derive(BufferContents, Vertex)]
#[repr(C)]
struct MyUV {
    #[format(R32G32_SFLOAT)]
    uv: [f32; 2],
}

#[derive(BufferContents, Debug, Default)]
#[repr(C)]
pub struct ModelUniform {
    model: [[f32; 4]; 4],
}

mod vs {
    vulkano_shaders::shader! {
        ty: "vertex",
        path: "assets/shaders/vertex.glsl",
    }
}

mod fs {
    vulkano_shaders::shader! {
        ty: "fragment",
        path: "assets/shaders/fragment.glsl",
    }
}

fn get_pipeline(
    vs: Arc<ShaderModule>,
    fs: Arc<ShaderModule>,
    device: Arc<Device>,
    viewport: Viewport,
    render_pass: Arc<RenderPass>,
) -> Arc<GraphicsPipeline> {
    GraphicsPipeline::start()
        .vertex_input_state([
            MyVertex::per_vertex(),
            MyNormal::per_vertex(),
            MyUV::per_vertex(),
        ]) // describes layout of vertex input
        .vertex_shader(vs.entry_point("main").unwrap(), ()) // specify entry point of vertex shader (vulkan shaders can technically have multiple)
        .input_assembly_state(InputAssemblyState::new()) //Indicate type of primitives (default is list of triangles)
        .viewport_state(ViewportState::viewport_fixed_scissor_irrelevant([viewport])) // Set the *fixed* viewport -> makes it impossible to change viewport for each draw cmd, but increases performance. Need to create new pipeline object if size does change.
        .fragment_shader(fs.entry_point("main").unwrap(), ()) // Specify entry point of fragment shader
        .depth_stencil_state(DepthStencilState::simple_depth_test())
        .render_pass(Subpass::from(render_pass, 0).unwrap()) // This pipeline object concerns the first pass of the render pass
        .with_auto_layout(device.clone(), |x| {
            let binding = x[1].bindings.get_mut(&0).unwrap();
            binding.variable_descriptor_count = true;
            binding.descriptor_count = 128; //TODO this is an upper bound to the number of textures, perhaps make it dynamic

            let binding = x[2].bindings.get_mut(&0).unwrap();
            binding.variable_descriptor_count = true;
            binding.descriptor_count = 128; //TODO this is an upper bound to the number of textures, perhaps make it dynamic

            let binding = x[3].bindings.get_mut(&0).unwrap(); // drawCallInfo
            binding.variable_descriptor_count = true;
            binding.descriptor_count = 128; //TODO this is an upper bound to the number of textures, perhaps make it dynamic
        })
        .unwrap()
}

fn draw_model_collapsing(ui: &mut Ui, model: &mut Model, parent_transform: Mat4) {
    ui.collapsing(String::from(model.name.clone().unwrap_or_default()), |ui| {
        ui.label("Translation:");
        if ui
            .add(egui::Slider::new(&mut model.local_transform.w_axis.x, -10.0..=10.0).text("X"))
            .changed()
        {
            model.update_transforms(parent_transform);
        }

        if ui
            .add(egui::Slider::new(&mut model.local_transform.w_axis.y, -10.0..=10.0).text("Y"))
            .changed()
        {
            model.update_transforms(parent_transform);
        }

        if ui
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
                    if ui.button("Go to material").clicked() {
                        println!("TBA, {:?}", mesh.material);
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
        ui.add(egui::Slider::new(&mut render_state.camera.fovy, 0.0..=180.0).text("Field of view"));
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
                    ui.label(format!("Base color factors: {}", mat.borrow().base_color));
                    ui.label(format!(
                        "Metallic roughness factors: {}",
                        mat.borrow().metallic_roughness_factors
                    ));
                    ui.label(format!(
                        "Emissive factors: {}",
                        mat.borrow().emissive_factors
                    ));
                    ui.label(format!(
                        "Occlusion strength: {}",
                        mat.borrow().occlusion_strength
                    ));
                    ui.separator();
                    ui.label(format!(
                        "Base color texture: {:?}",
                        mat.borrow().base_texture
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
                    let (vert_buf, normal_buf, _uvs, index_buf) =
                        create_buffers(&mesh, memory_allocator);
                    preview_vertices.push(VertexBuffer {
                        subbuffer: vert_buf.into_bytes(),
                        vertex_count: mesh.vertices.len() as u32,
                    });
                    normal_buffers.push(VertexBuffer {
                        subbuffer: normal_buf.into_bytes(),
                        vertex_count: mesh.vertices.len() as u32,
                    });
                    index_buffers.push(index_buf);
                }
            }
        }
    }
    (preview_vertices, normal_buffers, index_buffers)
}

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
            }
        }
        for material in self.materials.as_slice() {
            let dirty = { material.borrow().dirty() };
            if dirty {
                material.borrow_mut().update();
            }
        }
    }
}

pub fn start(gltf_paths: Vec<&str>) {
    let preview_models = vec![
        "assets/models/cube.glb",
        "assets/models/sphere.glb",
        "assets/models/suzanne.glb",
    ];

    let setup_info = init_renderer();

    let viewport = Viewport {
        origin: [0.0, 0.0],
        dimensions: setup_info.window.inner_size().into(),
        depth_range: 0.0..1.0,
    };

    let vs = vs::load(setup_info.device.clone()).expect("failed to create shader module");
    let fs = fs::load(setup_info.device.clone()).expect("failed to create shader module");

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

        let tex = create_texture(
            DynamicImage::from(img).into_bytes(),
            format::Format::R8G8B8A8_UNORM,
            width,
            height,
            &setup_info.memory_allocator,
            &mut cmd_buf_builder,
        );
        let texture = Rc::new(Texture::from(tex, Some(Box::from("Default texture")), 0));

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

    // Load preview meshes (for material/texture preview)
    load_preview_meshes(
        preview_models,
        &setup_info.memory_allocator,
        &mut cmd_buf_builder,
        default_material.clone(),
    );

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

    let mut vertex_buffers: Vec<VertexBuffer> = vec![];
    let mut normal_buffers: Vec<VertexBuffer> = vec![];
    let mut uv_buffers: Vec<VertexBuffer> = vec![];
    let mut index_buffers: Vec<Subbuffer<[u32]>> = vec![];
    let mut mesh_info_bufs = vec![];
    for scene in scenes.as_slice() {
        println!("{:?}", scene);
        for model in scene.models.as_slice() {
            println!("{:?}", model);

            for mesh in model.meshes.as_slice() {
                println!("{:?}", mesh);
                let (vert_buf, normal_buf, uv_buf, index_buf) =
                    create_buffers(mesh, &setup_info.memory_allocator);
                vertex_buffers.push(VertexBuffer {
                    subbuffer: vert_buf.into_bytes(),
                    vertex_count: mesh.vertices.len() as u32,
                });
                normal_buffers.push(VertexBuffer {
                    subbuffer: normal_buf.into_bytes(),
                    vertex_count: mesh.normals.len() as u32,
                });
                uv_buffers.push(VertexBuffer {
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
    let texs = global_state.textures.iter().map(|t| {
        (
            t.view.clone() as Arc<dyn ImageViewAbstract>,
            Sampler::new(device.clone(), SamplerCreateInfo::simple_repeat_linear()).unwrap(),
        )
    });

    let material_info_bufs = global_state
        .materials
        .as_slice()
        .into_iter()
        .map(|mat| mat.borrow().buffer.clone()); //TODO so many clones!

    println!("# of materialUniforms: {}", material_info_bufs.len());

    start_renderer(
        RenderState {
            init_state: setup_info,
            viewport,
            vertex_buffers,
            normal_buffers,
            uv_buffers,
            index_buffers,
            vs,
            fs,
            get_pipeline,
            write_descriptor_sets_0: vec![
                // Level 0: Scene-global uniforms
                WriteDescriptorSet::buffer(0, camera.buffer.clone()),
            ],
            descriptor_len_1: texs.len(),
            write_descriptor_sets_1: vec![
                // Level 1: Pipeline-specific uniforms
                WriteDescriptorSet::image_view_sampler_array(0, 0, texs),
            ],
            descriptor_len_2: material_info_bufs.len(),
            write_descriptor_sets_2: vec![
                // Level 2: Pipeline-specific uniforms
                WriteDescriptorSet::buffer_array(0, 0, material_info_bufs),
            ],
            descriptor_len_3: mesh_info_bufs.len(),
            write_descriptor_sets_3: vec![
                // Level 3: Model-specific uniforms
                WriteDescriptorSet::buffer_array(0, 0, mesh_info_bufs),
            ],
            cmd_buf_builder,
            camera,
        },
        global_state,
    );
}
