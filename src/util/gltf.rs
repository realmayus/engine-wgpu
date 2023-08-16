use crate::util::scene::{Mesh, Model, Scene};
use glam::Vec3;
use gltf::buffer::Data;
use gltf::Node;

pub fn load_gltf(path: &str) -> Vec<Scene> {
    let (gltf, buffers, _) = gltf::import(path).unwrap();
    // let (gltf, buffers, _) = gltf::import("assets/models/DamagedHelmet.gltf").unwrap();
    println!("GLTF has {:?} scenes", gltf.scenes().len());
    let mut scenes: Vec<Scene> = vec![];
    for scene in gltf.scenes() {
        println!("Scene has {:?} nodes", scene.nodes().len());

        let children = scene.nodes().map(|n| load_node(&n, &buffers)).collect();
        scenes.push(Scene::from(
            vec![],
            children,
            scene.name().map(str::to_string),
        ));
    }
    scenes
}

fn load_node(node: &Node, buffers: &Vec<Data>) -> Scene {
    let mut children: Vec<Scene> = vec![];
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
                meshes.push(Mesh::from(positions, indices, normals));
            }
        }
        _ => {}
    }
    let model = Model {
        meshes,
        name: node.name().map(Box::from(str::to_string)),
    };
    Scene::from(meshes, children, node.name().map(Box::from(str::to_string)))
}
