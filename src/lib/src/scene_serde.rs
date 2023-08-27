use crate::scene::{Material, Mesh, Model, Scene, Texture, World};
use crate::shader_types::{MaterialInfo, MeshInfo};
use crate::texture::create_texture;
use crate::VertexBuffer;
use glam::{Mat4, Vec2, Vec3, Vec4};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use vulkano::buffer::{Buffer, BufferCreateInfo, BufferUsage, Subbuffer};
use vulkano::command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer};
use vulkano::format;
use vulkano::memory::allocator::{AllocationCreateInfo, MemoryUsage, StandardMemoryAllocator};

#[derive(Serialize, Deserialize)]
pub struct TextureSerde {
    pub id: u32,
    pub name: Option<Box<str>>,
    pub img_path: PathBuf, // relative to run directory
}

impl From<Rc<Texture>> for TextureSerde {
    fn from(value: Rc<Texture>) -> Self {
        Self {
            id: value.id,
            name: value.name.clone(),
            img_path: value.img_path.clone(),
        }
    }
}

impl Texture {
    fn from_serde(
        value: &TextureSerde,
        allocator: &StandardMemoryAllocator,
        cmd_buf_builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
    ) -> Texture {
        let img = image::open(value.img_path.to_str().unwrap()).unwrap_or_else(|_| {
            panic!(
                "Couldn't load texture at {}",
                value.img_path.to_str().unwrap()
            )
        });
        let (width, height) = (img.width(), img.height());
        let texture = create_texture(
            img.into_bytes(),
            format::Format::R8G8B8A8_UNORM,
            width,
            height,
            allocator,
            cmd_buf_builder,
        );
        Texture::from(
            texture,
            value.name.to_owned(),
            value.id,
            value.img_path.to_owned(),
        )
    }
}

#[derive(Serialize, Deserialize)]
pub struct MaterialSerde {
    pub id: u32,
    pub name: Option<Box<str>>,
    pub base_texture: u32,
    pub base_color: Vec4, // this scales the RGBA components of the base_texture if defined; otherwise defines the color
    pub metallic_roughness_texture: u32,
    pub metallic_roughness_factors: Vec2, // this scales the metallic & roughness components of the metallic_roughness_texture if defined; otherwise defines the reflection characteristics
    pub normal_texture: u32,
    pub occlusion_texture: u32,
    pub occlusion_strength: f32,
    pub emissive_texture: u32,
    pub emissive_factors: Vec3,
}

impl From<Rc<RefCell<Material>>> for MaterialSerde {
    fn from(value: Rc<RefCell<Material>>) -> Self {
        MaterialSerde {
            id: value.borrow().id,
            name: value.borrow().name.clone(),
            base_texture: value
                .borrow()
                .base_texture
                .as_ref()
                .map(|t| t.id)
                .unwrap_or(0),
            base_color: value.borrow().base_color,
            metallic_roughness_texture: value
                .borrow()
                .metallic_roughness_texture
                .as_ref()
                .map(|t| t.id)
                .unwrap_or(0),
            metallic_roughness_factors: value.borrow().metallic_roughness_factors,
            normal_texture: value
                .borrow()
                .normal_texture
                .as_ref()
                .map(|t| t.id)
                .unwrap_or(0),
            occlusion_texture: value
                .borrow()
                .occlusion_texture
                .as_ref()
                .map(|t| t.id)
                .unwrap_or(0),
            occlusion_strength: value.borrow().occlusion_strength,
            emissive_texture: value
                .borrow()
                .emissive_texture
                .as_ref()
                .map(|t| t.id)
                .unwrap_or(0),
            emissive_factors: value.borrow().emissive_factors,
        }
    }
}

impl Material {
    fn from_serde(
        value: &MaterialSerde,
        textures: &HashMap<u32, Rc<Texture>>,
        allocator: &StandardMemoryAllocator,
    ) -> Material {
        Material {
            dirty: true,
            id: value.id,
            name: value.name.to_owned(),
            base_texture: textures.get(&value.base_texture).cloned(),
            base_color: value.base_color,
            metallic_roughness_texture: textures.get(&value.metallic_roughness_texture).cloned(),
            metallic_roughness_factors: value.metallic_roughness_factors,
            normal_texture: textures.get(&value.normal_texture).cloned(),
            occlusion_texture: textures.get(&value.occlusion_texture).cloned(),
            occlusion_strength: value.occlusion_strength,
            emissive_texture: textures.get(&value.emissive_texture).cloned(),
            emissive_factors: value.emissive_factors,
            buffer: Buffer::from_data(
                allocator,
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
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct MeshSerde {
    pub id: u32, // for key purposes in GUIs and stuff
    pub vertices: Vec<Vec3>,
    pub indices: Vec<u32>,
    pub normals: Vec<Vec3>,
    pub material: u32,
    pub uvs: Vec<Vec2>,
    pub global_transform: Mat4, // computed as product of the parent models' local transforms
}

impl From<Mesh> for MeshSerde {
    fn from(value: Mesh) -> Self {
        Self {
            id: value.id,
            vertices: value.vertices,
            indices: value.indices,
            normals: value.normals,
            material: value.material.borrow().id,
            uvs: value.uvs,
            global_transform: value.global_transform,
        }
    }
}

impl Mesh {
    fn from_serde(
        value: &MeshSerde,
        materials: &HashMap<u32, Rc<RefCell<Material>>>,
        allocator: &StandardMemoryAllocator,
    ) -> Self {
        Mesh::from(
            value.vertices.clone(),
            value.indices.clone(),
            value.normals.clone(),
            materials.get(&value.material).cloned().unwrap(),
            value.uvs.clone(),
            value.global_transform,
            Buffer::from_data(
                allocator,
                BufferCreateInfo {
                    usage: BufferUsage::STORAGE_BUFFER,
                    ..Default::default()
                },
                AllocationCreateInfo {
                    usage: MemoryUsage::Upload,
                    ..Default::default()
                },
                MeshInfo::default(),
            )
            .expect("Couldn't allocate MeshInfo uniform"),
        )
    }
}

#[derive(Serialize, Deserialize)]
pub struct ModelSerde {
    pub id: u32,
    pub meshes: Vec<MeshSerde>,
    pub children: Vec<ModelSerde>,
    pub name: Option<Box<str>>,
    pub local_transform: Mat4,
}

impl From<Model> for ModelSerde {
    fn from(value: Model) -> Self {
        Self {
            id: value.id,
            meshes: value.meshes.into_iter().map(MeshSerde::from).collect(),
            children: value.children.into_iter().map(ModelSerde::from).collect(),
            name: value.name,
            local_transform: value.local_transform,
        }
    }
}

impl Model {
    fn from_serde(
        value: &ModelSerde,
        materials: &HashMap<u32, Rc<RefCell<Material>>>,
        allocator: &StandardMemoryAllocator,
    ) -> Self {
        Model {
            id: value.id,
            meshes: value
                .meshes
                .iter()
                .map(|m| Mesh::from_serde(m, materials, allocator))
                .collect(),
            children: value
                .children
                .iter()
                .map(|m| Model::from_serde(m, materials, allocator))
                .collect(),
            name: value.name.clone(),
            local_transform: value.local_transform,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct SceneSerde {
    pub id: u32,
    pub models: Vec<ModelSerde>,
    pub name: Option<Box<str>>,
}

impl From<Scene> for SceneSerde {
    fn from(value: Scene) -> Self {
        Self {
            id: value.id,
            models: value.models.into_iter().map(ModelSerde::from).collect(),
            name: value.name,
        }
    }
}

impl Scene {
    fn from_serde(
        value: &SceneSerde,
        materials: &HashMap<u32, Rc<RefCell<Material>>>,
        allocator: &StandardMemoryAllocator,
    ) -> Self {
        Self {
            id: value.id,
            models: value
                .models
                .iter()
                .map(|m| Model::from_serde(m, materials, allocator))
                .collect(),
            name: value.name.clone(),
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct WorldSerde {
    pub textures: Vec<TextureSerde>,
    pub materials: Vec<MaterialSerde>,
    pub scenes: Vec<SceneSerde>,
}

impl WorldSerde {
    pub fn from(world: &World) -> Self {
        Self {
            textures: world
                .textures
                .values()
                .map(|t| TextureSerde::from(t.clone()))
                .collect(),
            materials: world
                .materials
                .values()
                .map(|m| MaterialSerde::from(m.clone()))
                .collect(),
            scenes: world
                .scenes
                .clone()
                .into_iter()
                .map(SceneSerde::from)
                .collect(),
        }
    }

    pub fn parse(
        &self,
        allocator: &StandardMemoryAllocator,
        cmd_buf_builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
    ) -> World {
        let mut textures = HashMap::new();
        for serde_texture in self.textures.as_slice() {
            textures.insert(
                serde_texture.id,
                Rc::new(Texture::from_serde(
                    serde_texture,
                    allocator,
                    cmd_buf_builder,
                )),
            );
        }

        let mut materials = HashMap::new();
        for serde_material in self.materials.as_slice() {
            materials.insert(
                serde_material.id,
                Rc::new(RefCell::new(Material::from_serde(
                    serde_material,
                    &textures,
                    allocator,
                ))),
            );
        }

        World {
            scenes: self
                .scenes
                .iter()
                .map(|s| Scene::from_serde(s, &materials, allocator))
                .collect(),
            textures,
            materials,
            cached_vertex_buffers: None,
            cached_normal_buffers: None,
            cached_uv_buffers: None,
            cached_index_buffers: None,
        }
    }
}
