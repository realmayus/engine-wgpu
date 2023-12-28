use std::cell::RefCell;
use std::mem;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use glam::{Mat4, Vec2, Vec3, Vec4};
use image::{DynamicImage, ImageFormat};
use serde::{Deserialize, Serialize};
use vulkano::buffer::{Buffer, BufferCreateInfo, BufferUsage};
use vulkano::command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer};
use vulkano::format;
use vulkano::memory::allocator::{AllocationCreateInfo, MemoryUsage, StandardMemoryAllocator};

use crate::scene::{Material, MaterialManager, Mesh, Model, Scene, Texture, TextureManager, World};
use crate::shader_types::{MaterialInfo, MeshInfo};
use crate::texture::create_texture;
use crate::util::extract_image_to_file;

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
        root_dir: &Path,
    ) -> Texture {
        let img = DynamicImage::from(
            image::open(root_dir.join(value.img_path.as_path()))
                .unwrap_or_else(|_| {
                    panic!(
                        "Couldn't load texture at {}",
                        value.img_path.to_str().unwrap()
                    )
                })
                .to_rgba8(),
        );
        let path = extract_image_to_file(
            value.img_path.file_stem().unwrap().to_str().unwrap(),
            &img,
            ImageFormat::from_path(value.img_path.as_path()).unwrap(),
        );

        let (width, height) = (img.width(), img.height());
        let texture = create_texture(
            img.into_bytes(),
            format::Format::R8G8B8A8_UNORM,
            width,
            height,
            allocator,
            cmd_buf_builder,
        );
        Texture::from(texture, value.name.clone(), value.id, path)
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
        textures: &TextureManager,
        allocator: &StandardMemoryAllocator,
    ) -> Material {
        Material {
            dirty: true,
            id: value.id,
            name: value.name.clone(),
            base_texture: Some(textures.get_texture(value.base_texture)),
            base_color: value.base_color,
            metallic_roughness_texture: Some(
                textures.get_texture(value.metallic_roughness_texture),
            ),
            metallic_roughness_factors: value.metallic_roughness_factors,
            normal_texture: Some(textures.get_texture(value.normal_texture)),
            occlusion_texture: Some(textures.get_texture(value.occlusion_texture)),
            occlusion_strength: value.occlusion_strength,
            emissive_texture: Some(textures.get_texture(value.emissive_texture)),
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
        value: MeshSerde,
        materials: &MaterialManager,
        allocator: &StandardMemoryAllocator,
    ) -> Self {
        Mesh::from(
            value.vertices,
            value.indices,
            value.normals,
            materials.get_material(value.material),
            value.uvs,
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
        value: ModelSerde,
        materials: &MaterialManager,
        allocator: &StandardMemoryAllocator,
    ) -> Self {
        Model {
            id: value.id,
            meshes: value
                .meshes
                .into_iter()
                .map(|m| Mesh::from_serde(m, materials, allocator))
                .collect(),
            children: value
                .children
                .into_iter()
                .map(|m| Model::from_serde(m, materials, allocator))
                .collect(),
            name: value.name,
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
        value: SceneSerde,
        materials: &MaterialManager,
        allocator: &StandardMemoryAllocator,
    ) -> Self {
        Self {
            id: value.id,
            models: value
                .models
                .into_iter()
                .map(|m| Model::from_serde(m, materials, allocator))
                .collect(),
            name: value.name.clone(),
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct WorldSerde {
    pub textures: TextureManagerSerde,
    pub materials: MaterialManagerSerde,
    pub scenes: Vec<SceneSerde>,
}

impl WorldSerde {
    pub fn from(
        textures: &TextureManager,
        materials: &MaterialManager,
        scenes: Vec<Scene>,
    ) -> Self {
        Self {
            textures: TextureManagerSerde::from(textures),
            materials: MaterialManagerSerde::from(materials),
            scenes: scenes.into_iter().map(SceneSerde::from).collect(),
        }
    }

    pub fn parse(
        &mut self,
        allocator: &StandardMemoryAllocator,
        cmd_buf_builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
        root_dir: &Path,
    ) -> World {
        let textures =
            TextureManager::from_serde(&self.textures, allocator, cmd_buf_builder, root_dir);
        let materials = MaterialManager::from_serde(&self.materials, &textures, allocator);
        let scenes_serde = mem::take(&mut self.scenes);
        let scenes = scenes_serde
            .into_iter()
            .map(|scene| Scene::from_serde(scene, &materials, allocator))
            .collect();
        World {
            scenes,
            active_scene: 0,
            materials,
            textures,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct TextureManagerSerde {
    pub textures: Vec<TextureSerde>,
}

impl From<&TextureManager> for TextureManagerSerde {
    fn from(value: &TextureManager) -> Self {
        Self {
            textures: value
                .iter()
                .map(|tex| TextureSerde::from(tex.clone()))
                .collect(),
        }
    }
}

impl TextureManager {
    fn from_serde(
        value: &TextureManagerSerde,
        allocator: &StandardMemoryAllocator,
        cmd_buf_builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
        root_dir: &Path,
    ) -> Self {
        let mut manager = Self::new_ref(allocator, cmd_buf_builder);
        for tex in value
            .textures
            .iter()
            .map(|tex| Texture::from_serde(tex, allocator, cmd_buf_builder, root_dir))
        {
            let id = tex.id;
            let result_id = manager.add_texture(tex);
            assert_eq!(
                id, result_id,
                "Expected texture ID {} but got {}",
                result_id, id
            );
        }
        manager
    }
}

#[derive(Serialize, Deserialize)]
pub struct MaterialManagerSerde {
    pub materials: Vec<MaterialSerde>,
}

impl From<&MaterialManager> for MaterialManagerSerde {
    fn from(value: &MaterialManager) -> Self {
        Self {
            materials: value
                .iter()
                .map(|mat| MaterialSerde::from(mat.clone()))
                .collect(),
        }
    }
}

impl MaterialManager {
    fn from_serde(
        value: &MaterialManagerSerde,
        textures: &TextureManager,
        allocator: &StandardMemoryAllocator,
    ) -> MaterialManager {
        let mut manager = Self::new();
        for mat in value.materials.as_slice() {
            let id = mat.id;
            let result_id = manager.add_material(Material::from_serde(mat, textures, allocator));
            assert_eq!(
                result_id, id,
                "Expected material ID {} but got {}",
                result_id, id
            );
        }
        manager
    }
}
