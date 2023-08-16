use std::collections::HashMap;
use std::ops::Index;
use std::rc::Rc;

use glam::{Vec2, Vec3};
use gltf::buffer::Data;
use gltf::Node;
use vulkano::command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer};
use vulkano::memory::allocator::StandardMemoryAllocator;

use crate::renderer::scene::{Material, Mesh, Model, Scene, Texture};
use crate::renderer::texture::create_texture;
use crate::util::map_gltf_format_to_vulkano;

pub fn load_gltf(
    path: &str,
    allocator: &StandardMemoryAllocator,
    mut cmd_buf_builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
) -> (
    Vec<Scene>,
    HashMap<usize, Rc<Texture>>,
    HashMap<usize, Rc<Material>>,
) {
    let (gltf, buffers, images) = gltf::import(path).unwrap();

    println!("GLTF has {:?} scenes", gltf.scenes().len());

    let mut scenes: Vec<Scene> = vec![];
    let mut textures: HashMap<usize, Rc<Texture>> = HashMap::new();
    let mut materials: HashMap<usize, Rc<Material>> = HashMap::new();

    for gltfTexture in gltf.textures() {
        let image = &images[gltfTexture.source().index()];
        let vk_texture = create_texture(
            image.pixels.clone(), //TODO: Texture load optimization: check if this clone is bad
            map_gltf_format_to_vulkano(image.format),
            image.width,
            image.height,
            allocator,
            cmd_buf_builder,
        );
        let texture = Texture::from(vk_texture, gltfTexture.name().map(Box::from));
        textures.insert(gltfTexture.index(), Rc::from(texture));
    }
    for gltfMat in gltf.materials() {
        if let Some(index) = gltfMat.index() {
            let mat = Material {
                name: gltfMat.name().map(Box::from),
                base_texture: gltfMat
                    .pbr_metallic_roughness()
                    .base_color_texture()
                    .map(|t| t.texture().index())
                    .map(|id| {
                        textures
                            .get(&id)
                            .expect("Couldn't find base texture")
                            .clone()
                    }),
                base_color: gltfMat.pbr_metallic_roughness().base_color_factor().into(),
                metallic_roughness_texture: gltfMat
                    .pbr_metallic_roughness()
                    .metallic_roughness_texture()
                    .map(|t| t.texture().index())
                    .map(|id| {
                        textures
                            .get(&id)
                            .expect("Couldn't find metallic roughness texture")
                            .clone()
                    }),
                metallic_roughness_factors: Vec2::from((
                    gltfMat.pbr_metallic_roughness().metallic_factor(),
                    gltfMat.pbr_metallic_roughness().roughness_factor(),
                )),
                normal_texture: gltfMat
                    .normal_texture()
                    .map(|t| t.texture().index())
                    .map(|id| *textures.get(&id).expect("Couldn't find normal texture")),
                occlusion_texture: gltfMat
                    .occlusion_texture()
                    .map(|t| t.texture().index())
                    .map(|id| *textures.get(&id).expect("Couldn't find occlusion texture")),
                occlusion_strength: 1.0, // TODO: Impl: try to read strength from glTF
                emissive_texture: gltfMat
                    .emissive_texture()
                    .map(|t| t.texture().index())
                    .map(|id| *textures.get(&id).expect("Couldn't find emissive texture")),
                emissive_factors: gltfMat.emissive_factor().into(),
            };
            materials.insert(index, Rc::from(mat));
        }
    }

    for scene in gltf.scenes() {
        println!("Scene has {:?} nodes", scene.nodes().len());

        let children = scene
            .nodes()
            .map(|n| load_node(&n, &buffers, &materials))
            .collect();
        scenes.push(Scene::from(children, scene.name().map(Box::from)));
    }

    (scenes, textures, materials)
}

fn load_node(node: &Node, buffers: &Vec<Data>, materials: &HashMap<usize, Rc<Material>>) -> Model {
    let mut children: Vec<Model> = vec![];
    for child in node.children() {
        children.push(load_node(&child, buffers, materials));
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
                let mat = gltf_primitive
                    .material()
                    .index()
                    .map(|i| materials.get(&i).expect("Couldn't find material").clone());

                meshes.push(Mesh::from(positions, indices, normals, mat));
            }
        }
        _ => {}
    }
    Model::from(meshes, node.name().map(Box::from), children)
}
