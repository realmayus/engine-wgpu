use egui_winit_vulkano::egui::{vec2, PaintCallback, Rgba, Sense, Ui};
use egui_winit_vulkano::{egui, CallbackFn, Gui};
use glam::{Mat4, Vec3};
use image::DynamicImage;
use lib::scene::{Material, Model, Scene, Texture};
use lib::util::texture::create_texture;
use renderer::camera::Camera;
use renderer::{init_renderer, start_renderer, PartialRenderState, RenderState, VertexBuffer};
use std::rc::Rc;
use std::sync::Arc;
use systems::io::gltf_loader::load_gltf;
use vulkano::buffer::{Buffer, BufferContents, BufferCreateInfo, BufferUsage, Subbuffer};
use vulkano::command_buffer::{AutoCommandBufferBuilder, CommandBufferUsage, CopyImageInfo};
use vulkano::descriptor_set::{PersistentDescriptorSet, WriteDescriptorSet};
use vulkano::device::Device;
use vulkano::format;
use vulkano::memory::allocator::{AllocationCreateInfo, MemoryUsage};
use vulkano::pipeline::graphics::depth_stencil::DepthStencilState;
use vulkano::pipeline::graphics::input_assembly::{InputAssemblyState, PrimitiveTopology};
use vulkano::pipeline::graphics::vertex_input::{Vertex, VertexDefinition};
use vulkano::pipeline::graphics::viewport;
use vulkano::pipeline::graphics::viewport::{Viewport, ViewportState};
use vulkano::pipeline::{GraphicsPipeline, Pipeline};
use vulkano::render_pass::{RenderPass, Subpass};
use vulkano::sampler::{
    BorderColor, Filter, Sampler, SamplerAddressMode, SamplerCreateInfo, SamplerMipmapMode,
    SamplerReductionMode,
};
use vulkano::shader::ShaderModule;
use vulkano::sync::GpuFuture;

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
        .vertex_input_state([MyVertex::per_vertex(), MyNormal::per_vertex()]) // describes layout of vertex input
        .vertex_shader(vs.entry_point("main").unwrap(), ()) // specify entry point of vertex shader (vulkan shaders can technically have multiple)
        .input_assembly_state(InputAssemblyState::new()) //Indicate type of primitives (default is list of triangles)
        .viewport_state(ViewportState::viewport_fixed_scissor_irrelevant([viewport])) // Set the *fixed* viewport -> makes it impossible to change viewport for each draw cmd, but increases performance. Need to create new pipeline object if size does change.
        .fragment_shader(fs.entry_point("main").unwrap(), ()) // Specify entry point of fragment shader
        .depth_stencil_state(DepthStencilState::simple_depth_test())
        .render_pass(Subpass::from(render_pass, 0).unwrap()) // This pipeline object concerns the first pass of the render pass
        .build(device)
        .unwrap()
}

fn draw_model_collapsing(ui: &mut Ui, model: &Model) {
    ui.collapsing(String::from(model.name.clone().unwrap_or_default()), |ui| {
        ui.label("Meshes:");
        for mesh in model.meshes.as_slice() {
            ui.collapsing("Mesh", |ui| {
                ui.label(format!(
                    "# of vert/norm/in: {}/{}/{}",
                    mesh.vertices.len(),
                    mesh.normals.len(),
                    mesh.indices.len()
                ));
                ui.label(
                    "Material: ".to_owned()
                        + &*String::from(mesh.material.name.clone().unwrap_or_default()),
                );
                if ui.button("Go to material").clicked() {
                    println!("TBA, {:?}", mesh.material);
                }
            });
        }
        ui.separator();
        ui.label("Children:");
        for child in model.children.as_slice() {
            draw_model_collapsing(ui, child);
        }
    });
}

fn render_gui(gui: &mut Gui, render_state: PartialRenderState, state: Rc<GlobalState>) {
    let ctx = gui.context();
    egui::Window::new("Scene").show(&ctx, |ui| {
        ui.label("Loaded models:");
        for scene in &state.scenes {
            ui.collapsing(String::from(scene.name.clone().unwrap_or_default()), |ui| {
                ui.label(format!("# of models: {}", scene.models.len()));
                for model in &scene.models {
                    draw_model_collapsing(ui, model);
                }
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
            ui.collapsing(String::from(mat.name.clone().unwrap_or_default()), |ui| {
                ui.label(format!("Base color factors: {}", mat.base_color));
                ui.label(format!(
                    "Metallic roughness factors: {}",
                    mat.metallic_roughness_factors
                ));
                ui.label(format!("Emissive factors: {}", mat.emissive_factors));
                ui.label(format!("Occlusion strength: {}", mat.occlusion_strength));
                ui.separator();
                ui.label(format!("Base color texture: {:?}", mat.base_texture));
                ui.label(format!("Normal texture: {:?}", mat.normal_texture));
                ui.label(format!(
                    "Metallic roughness texture: {:?}",
                    mat.metallic_roughness_texture
                ));
                ui.label(format!("Emissive texture: {:?}", mat.emissive_texture));
                ui.label(format!("Occlusion texture: {:?}", mat.occlusion_texture));
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

struct GlobalState {
    scenes: Vec<Scene>,
    materials: Vec<Rc<Material>>,
    textures: Vec<Rc<Texture>>,
}

pub fn render(gltf_paths: Vec<&str>) {
    let setup_info = init_renderer();

    let mut viewport = Viewport {
        origin: [0.0, 0.0],
        dimensions: setup_info.window.inner_size().into(),
        depth_range: 0.0..1.0,
    };

    let vs = vs::load(setup_info.device.clone()).expect("failed to create shader module");
    let fs = fs::load(setup_info.device.clone()).expect("failed to create shader module");

    // let vertex_buffer = Buffer::from_iter(
    //     &setup_info.memory_allocator,
    //     BufferCreateInfo {
    //         usage: BufferUsage::VERTEX_BUFFER,
    //         ..Default::default()
    //     },
    //     AllocationCreateInfo {
    //         usage: MemoryUsage::Upload,
    //         ..Default::default()
    //     },
    //     vec![],
    // )
    // .expect("Couldn't create vertex buffer");
    let view = Mat4::from_cols_array_2d(&[[1.0f32; 4]; 4]);
    view.transform_vector3(Vec3::from((0.0f32, 0.0f32, 0.0f32)));
    let model_uniform = ModelUniform {
        model: view.to_cols_array_2d(),
    };
    let model_buffer = Buffer::from_data(
        &setup_info.memory_allocator,
        BufferCreateInfo {
            usage: BufferUsage::UNIFORM_BUFFER,
            ..Default::default()
        },
        AllocationCreateInfo {
            usage: MemoryUsage::Upload,
            ..Default::default()
        },
        model_uniform,
    )
    .unwrap();

    let mut cmd_buf_builder = AutoCommandBufferBuilder::primary(
        &setup_info.cmd_buf_allocator,
        setup_info.queue.queue_family_index(),
        CommandBufferUsage::OneTimeSubmit,
    )
    .unwrap();

    let mut scenes: Vec<Scene> = vec![];
    let mut textures: Vec<Rc<Texture>> = vec![];
    let mut materials: Vec<Rc<Material>> = vec![];

    for gltf_path in gltf_paths {
        let (mut gltf_scenes, gltf_textures, gltf_materials) = load_gltf(
            gltf_path,
            &setup_info.memory_allocator,
            &mut cmd_buf_builder,
        );
        scenes.append(&mut gltf_scenes);
        let texture_values: Vec<Rc<Texture>> = gltf_textures.into_values().collect();
        textures.append(&mut texture_values.clone()); //TODO investigate if this is too performance-heavy?
        let material_values: Vec<Rc<Material>> = gltf_materials.into_values().collect();
        materials.append(&mut material_values.clone());
    }

    let mut vertex_buffers: Vec<VertexBuffer> = vec![];
    let mut normal_buffers: Vec<VertexBuffer> = vec![];
    let mut index_buffers: Vec<Subbuffer<[u32]>> = vec![];

    for scene in scenes.as_slice() {
        println!("{:?}", scene);
        for model in scene.models.as_slice() {
            println!("{:?}", model);
            for mesh in model.meshes.as_slice() {
                println!("{:?}", mesh);
                let vert_buf: Subbuffer<[[f32; 3]]> = Buffer::from_iter(
                    &setup_info.memory_allocator,
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
                vertex_buffers.push(VertexBuffer {
                    subbuffer: vert_buf.into_bytes(),
                    vertex_count: mesh.vertices.len() as u32,
                });

                let normal_buf: Subbuffer<[[f32; 3]]> = Buffer::from_iter(
                    &setup_info.memory_allocator,
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
                normal_buffers.push(VertexBuffer {
                    subbuffer: normal_buf.into_bytes(),
                    vertex_count: mesh.vertices.len() as u32,
                });

                let index_buf = Buffer::from_iter(
                    &setup_info.memory_allocator,
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
                index_buffers.push(index_buf);
            }
        }
    }

    let sampler = Sampler::new(
        setup_info.device.clone(),
        SamplerCreateInfo {
            mag_filter: Filter::Nearest,
            min_filter: Filter::Nearest,
            address_mode: [SamplerAddressMode::Repeat; 3],
            ..Default::default()
        },
    )
    .unwrap();

    let camera = Camera::new_default(
        viewport.dimensions[0],
        viewport.dimensions[1],
        &setup_info.memory_allocator,
    );

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
    let mut global_state = Rc::new(GlobalState {
        scenes,
        materials,
        textures,
    });
    start_renderer(
        RenderState {
            init_state: setup_info,
            viewport,
            vertex_buffers,
            normal_buffers,
            index_buffers,
            vs,
            fs,
            get_pipeline,
            write_descriptor_sets: vec![
                // WriteDescriptorSet::image_view_sampler(3, tex, sampler),
                WriteDescriptorSet::buffer(1, camera.buffer.clone()),
                WriteDescriptorSet::buffer(2, model_buffer.clone()),
            ],
            cmd_buf_builder,
            camera,
        },
        move |gui, partial_render_state| {
            render_gui(gui, partial_render_state, global_state.clone())
        },
    );
}
