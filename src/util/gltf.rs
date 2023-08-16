use crate::renderer::scene::{Material, Mesh, Model, Scene, Texture};
use crate::renderer::texture::create_texture;
use base64::{
    engine::{self, general_purpose},
    Engine as _,
};
use glam::{Vec2, Vec3};
use gltf::buffer::{Data, Source as BufferSource};
use gltf::image::{Source as ImageSource, Source};
use gltf::Node;
use image::DynamicImage;
use std::collections::HashMap;
use std::ops::Index;
use std::str::Split;
use vulkano::command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer};
use vulkano::memory::allocator::StandardMemoryAllocator;

fn parse_uri(uri: &str) -> Vec<u8> {
    if !(uri.starts_with("data:") && uri.contains("base64")) {
        panic!("Source URIs must be data URIs")
    }
    let parts: Vec<&str> = uri.split("base64,").collect();
    if parts.len() != 2 {
        panic!("Source uri must be data:<type>;base64,<data>, is: {}", uri)
    }
    general_purpose::STANDARD
        .decode(parts[1])
        .expect("Couldn't decode b64 data")
}

//TODO: Image loading optimization: cache buffers, don't decode/load them every time we load an image
fn load_image_source(source: ImageSource, buffers: &Vec<Data>) -> DynamicImage {
    match source {
        ImageSource::View { view, mime_type } => match view.buffer().source() {
            BufferSource::Bin => {
                let buffer: &Data = &buffers[view.buffer().index()];
                let data = &buffer[view.offset()..view.offset() + view.length()];
                image::load_from_memory(data).expect("Couldn't load image")
            }
            BufferSource::Uri(uri) => {
                let data = parse_uri(uri);
                image::load_from_memory(&data[view.offset()..view.offset() + view.length()])
                    .expect("Couldn't load image")
            }
        },
        ImageSource::Uri { uri, mime_type } => {
            let data = parse_uri(uri);
            if !(uri.contains("png") || uri.contains("jpg") || uri.contains("jpeg")) {
                panic!(
                    "Source uri must be data:image/{{png,jpeg}};base64,<data>, was: {}",
                    uri
                );
            }

            image::load_from_memory(&data).expect("Couldn't load image")
        }
    }
}

pub fn load_gltf(
    path: &str,
    allocator: &StandardMemoryAllocator,
    mut cmd_buf_builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
) -> Vec<Scene> {
    let (gltf, buffers, images) = gltf::import(path).unwrap();

    println!("GLTF has {:?} scenes", gltf.scenes().len());
    let mut scenes: Vec<Scene> = vec![];
    let mut textures: HashMap<usize, Texture> = HashMap::new();
    let mut materials: HashMap<usize, Material> = HashMap::new();
    for scene in gltf.scenes() {
        println!("Scene has {:?} nodes", scene.nodes().len());

        let children = scene.nodes().map(|n| load_node(&n, &buffers)).collect();
        scenes.push(Scene::from(children, scene.name().map(Box::from)));
    }
    for gltfTexture in gltf.textures() {
        let image = load_image_source(gltfTexture.source().source(), &buffers);
        let vk_texture = create_texture(image, allocator, cmd_buf_builder);
        let texture = Texture::from(vk_texture, gltfTexture.name().map(Box::from));
        textures.insert(gltfTexture.index(), texture);
    }
    for gltfMat in gltf.materials() {
        if let Some(index) = gltfMat.index() {
            let mat = Material {
                name: gltfMat.name().map(Box::from),
                base_texture: gltfMat.pbr_metallic_roughness().base_color_texture(),
                base_color: gltfMat.pbr_metallic_roughness().base_color_factor().into(),
                metallic_roughness_texture: gltfMat
                    .pbr_metallic_roughness()
                    .metallic_roughness_texture(),
                metallic_roughness_factors: Vec2::from((
                    gltfMat.pbr_metallic_roughness().metallic_factor(),
                    gltfMat.pbr_metallic_roughness().roughness_factor(),
                )),
                normal_texture: gltfMat.normal_texture(),
                occlusion_texture: gltfMat.occlusion_texture(),
                occlusion_strength: 1.0,
                emissive_texture: gltfMat.emissive_texture(),
                emissive_factors: gltfMat.emissive_factor().into(),
            };
            materials.insert(index, mat);
        }
    }
    scenes
}

fn load_node(node: &Node, buffers: &Vec<Data>) -> Model {
    let mut children: Vec<Model> = vec![];
    for child in node.children() {
        children.push(load_node(&child, buffers));
    }
    let mut meshes: Vec<Mesh> = vec![];
    match node.mesh() {
        Some(x) => {
            for gltf_primitive in x.primitives() {
                let mut positions: Vec<Vec3> = vec![];
                let mut indices: Vec<u32> = vec![];
                let mut normals: Vec<Vec3> = vec![];

                let reader = gltf_primitive.reader(|buffer| Some(&buffers[buffer.index()]));
                if let Some(iter) = reader.read_positions() {
                    positions = iter.map(|p| Vec3::from(p)).collect();
                }
                if let Some(iter) = reader.read_indices() {
                    indices = iter.into_u32().collect();
                }
                if let Some(iter) = reader.read_normals() {
                    normals = iter.map(|n| Vec3::from(n)).collect();
                }
                let mat = gltf_primitive.material();

                meshes.push(Mesh::from(positions, indices, normals));
            }
        }
        _ => {}
    }
    Model::from(meshes, node.name().map(Box::from), children)
}
