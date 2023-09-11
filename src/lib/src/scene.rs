use std::cell::RefCell;
use std::fmt::{Debug, Formatter};
use std::path::PathBuf;
use std::rc::Rc;
use std::slice::Iter;
use std::sync::Arc;

use crate::shader_types::{LightInfo, MaterialInfo, MeshInfo};
use crate::Dirtyable;
use glam::{Mat4, Vec2, Vec3, Vec4};
use log::{debug, info};
use rand::Rng;
use vulkano::buffer::{Buffer, BufferCreateInfo, BufferUsage, Subbuffer};
use vulkano::image::view::ImageView;
use vulkano::image::ImmutableImage;
use vulkano::memory::allocator::{AllocationCreateInfo, MemoryUsage, StandardMemoryAllocator};

use crate::shader_types::{MaterialInfo, MeshInfo};
use crate::{Dirtyable, VertexInputBuffer};

pub struct Texture {
    pub id: u32,
    pub name: Option<Box<str>>,
    pub view: Arc<ImageView<ImmutableImage>>,
    pub img_path: PathBuf, // relative to run directory
}

impl Texture {
    pub fn from(
        view: Arc<ImageView<ImmutableImage>>,
        name: Option<Box<str>>,
        id: u32,
        img_path: PathBuf,
    ) -> Self {
        Self {
            view,
            name,
            id,
            img_path,
        }
    }
}

impl Debug for Texture {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{{TEXTURE: name: {:?}, id: {}}}", self.name, self.id)
    }
}

/// PBR Material
pub struct Material {
    pub dirty: bool,
    pub id: u32,
    pub name: Option<Box<str>>,
    pub albedo_texture: Option<Rc<Texture>>,
    /// this scales the RGBA components of the base_texture if defined; otherwise defines the color
    pub albedo: Vec4,
    /// blue channel: metallicness, green channel: roughness
    pub metallic_roughness_texture: Option<Rc<Texture>>,
    /// this scales the metallic & roughness components of the metallic_roughness_texture if defined; otherwise defines the reflection characteristics
    pub metallic_roughness_factors: Vec2,
    pub normal_texture: Option<Rc<Texture>>,
    pub occlusion_texture: Option<Rc<Texture>>,
    pub occlusion_factor: f32,
    pub emissive_texture: Option<Rc<Texture>>,
    pub emissive_factors: Vec3,
    pub buffer: Subbuffer<MaterialInfo>,
}

impl Material {
    pub fn from_default(
        base_texture: Option<Rc<Texture>>,
        buffer: Subbuffer<MaterialInfo>,
    ) -> Self {
        Self {
            dirty: true,
            id: 0,
            name: Some(Box::from("Default material")),
            albedo_texture: base_texture,
            albedo: Vec4::from((1.0, 0.957, 0.859, 1.0)),
            metallic_roughness_texture: None,
            metallic_roughness_factors: Vec2::from((0.5, 0.5)),
            normal_texture: None,
            occlusion_texture: None,
            occlusion_factor: 0.0,
            emissive_texture: None,
            emissive_factors: Vec3::from((0.0, 0.0, 0.0)),
            buffer,
        }
    }
}

impl Dirtyable for Material {
    fn dirty(&self) -> bool {
        self.dirty
    }

    fn set_dirty(&mut self, dirty: bool) {
        self.dirty = dirty
    }

    fn update(&mut self) {
        debug!("Updated material #{}", self.id);
        self.set_dirty(false);
        let mut mapping = self.buffer.write().unwrap();

        mapping.albedo_texture = self.albedo_texture.as_ref().map(|t| t.id).unwrap_or(0);
        mapping.albedo = self.albedo.to_array();
        mapping.metal_roughness_texture = self
            .metallic_roughness_texture
            .as_ref()
            .map(|t| t.id)
            .unwrap_or(0);
        mapping.metal_roughness_factors = self.metallic_roughness_factors.to_array();
        mapping.normal_texture = self.normal_texture.as_ref().map(|t| t.id).unwrap_or(1);
        mapping.occlusion_texture = self.occlusion_texture.as_ref().map(|t| t.id).unwrap_or(0);
        mapping.occlusion_factor = self.occlusion_factor;
        mapping.emission_texture = self.emissive_texture.as_ref().map(|t| t.id).unwrap_or(0);
        mapping.emission_factors = self.emissive_factors.to_array();
    }
}

impl Debug for Material {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{{MATERIAL: name: {}, base_texture: {}, base_color: {:?}, metallic_roughness_texture: {}, metallic_roughness_factors: {:?}, normal_texture: {}, occlusion_texture: {}, occlusion_strength: {}, emissive_texture: {}, emissive_factors: {:?}}}",
            self.name.clone().unwrap_or_default(),
            self.albedo_texture.clone().map(|t| t.id).unwrap_or(0),  // there really shouldn't be any int overflow :p
            self.albedo,
            self.metallic_roughness_texture.clone().map(|t| t.id).unwrap_or(0),
            self.metallic_roughness_factors,
            self.normal_texture.clone().map(|t| t.id).unwrap_or(1),
            self.occlusion_texture.clone().map(|t| t.id).unwrap_or(0),
            self.occlusion_factor,
            self.emissive_texture.clone().map(|t| t.id).unwrap_or(0),
            self.emissive_factors,
        )
    }
}

#[derive(Clone)]
pub struct Mesh {
    dirty: bool,
    pub id: u32, // for key purposes in GUIs and stuff
    pub vertices: Vec<Vec3>,
    pub indices: Vec<u32>,
    pub normals: Vec<Vec3>,
    pub tangents: Vec<Vec4>,
    pub material: Rc<RefCell<Material>>,
    pub uvs: Vec<Vec2>,
    pub global_transform: Mat4, // computed as product of the parent models' local transforms
    pub buffer: Subbuffer<MeshInfo>,
}
impl Mesh {
    pub fn from(
        vertices: Vec<Vec3>,
        indices: Vec<u32>,
        normals: Vec<Vec3>,
        tangents: Vec<Vec4>,
        material: Rc<RefCell<Material>>,
        uvs: Vec<Vec2>,
        global_transform: Mat4,
        buffer: Subbuffer<MeshInfo>,
    ) -> Self {
        Self {
            id: rand::thread_rng().gen_range(0u32..1u32 << 31),
            dirty: true,
            vertices,
            indices,
            normals,
            tangents,
            material,
            uvs,
            global_transform,
            buffer,
        }
    }
}
impl Dirtyable for Mesh {
    fn dirty(&self) -> bool {
        self.dirty
    }

    fn set_dirty(&mut self, dirty: bool) {
        self.dirty = dirty
    }

    fn update(&mut self) {
        self.set_dirty(false);
        let mut mapping = self.buffer.write().unwrap();
        mapping.model_transform = self.global_transform.to_cols_array_2d();
        mapping.material = self.material.borrow().id;
    }
}
impl Debug for Mesh {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{{MESH: # of vertices: {}, # of normals: {}, # of tangents: {}, # of indices: {}, material: {}, global transform: {}}}",
            self.vertices.len(),
            self.normals.len(),
            self.tangents.len(),
            self.indices.len(),
            self.material.borrow().name.clone().unwrap_or_default(),
            self.global_transform,
        )
    }
}

#[derive(Clone)]
pub struct PointLight {
    pub dirty: bool,
    pub global_transform: Mat4,
    pub index: usize,
    pub color: Vec3,
    pub intensity: f32,
    pub range: Option<f32>,
    pub buffer: Subbuffer<LightInfo>,
    // TODO pass as set but fuck that right now
    pub amount: u32,
}
impl Dirtyable for PointLight {
    fn dirty(&self) -> bool {
        self.dirty
    }

    fn set_dirty(&mut self, dirty: bool) {
        self.dirty = dirty;
    }

    fn update(&mut self) {
        info!("Updated light {}", self.index);
        self.set_dirty(false);
        let mut mapping = self.buffer.write().unwrap();
        mapping.transform = self.global_transform.to_cols_array_2d();
        mapping.color = self.color.to_array();
        mapping.light = self.index as u32;
        mapping.intensity = self.intensity;
        mapping.amount = self.amount;
        mapping.range = self.range.unwrap_or(1.0);
    }
}

pub struct Model {
    pub id: u32,
    pub meshes: Vec<Mesh>,
    pub children: Vec<Model>,
    pub name: Option<Box<str>>,
    pub local_transform: Mat4,
    pub light: Option<PointLight>,
}
impl Model {
    pub fn from(
        meshes: Vec<Mesh>,
        name: Option<Box<str>>,
        children: Vec<Model>,
        local_transform: Mat4,
        light: Option<PointLight>,
    ) -> Self {
        Self {
            id: rand::thread_rng().gen_range(0u32..1u32 << 31),
            meshes,
            name,
            children,
            local_transform,
            light,
        }
    }

    /**
    Call this after changing the local_transform of a model, it updates the computed global_transforms of all meshes.
    Sets dirty to true.
    */
    pub fn update_transforms(&mut self, parent: Mat4) {
        for mesh in self.meshes.as_mut_slice() {
            mesh.global_transform = parent * self.local_transform;
            mesh.set_dirty(true);
        }
        for child in self.children.as_mut_slice() {
            child.update_transforms(self.local_transform);
        }
        if let Some(ref mut light) = self.light {
            light.global_transform = parent * self.local_transform;
            light.set_dirty(true);
        }
    }
}

impl Debug for Model {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{{MODEL: Name: {:?}, # of meshes: {}, local transform: {}, children: [{}]}}",
            self.name,
            self.meshes.len(),
            self.local_transform,
            self.children
                .iter()
                .map(|c| format!("\n - {:?}", c))
                .collect::<String>(),
        )
    }
}

impl Clone for Model {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            meshes: self.meshes.clone(),
            children: self.children.clone(),
            name: self.name.clone(),
            local_transform: self.local_transform,
            light: self.light.clone(),
        }
    }
}

pub struct Scene {
    pub id: u32,
    pub models: Vec<Model>,
    pub name: Option<Box<str>>,
}

impl Scene {
    pub fn from(models: Vec<Model>, name: Option<Box<str>>) -> Self {
        Self {
            id: rand::thread_rng().gen_range(0u32..1u32 << 31),
            models,
            name,
        }
    }
    pub fn iter_meshes(&self) -> impl Iterator<Item = &Mesh> {
        self.models.iter().flat_map(|model| model.meshes.iter())
    }
}

impl Debug for Scene {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{{SCENE: Name: {:?}, # of models: {}, models: [{}]}}",
            self.name,
            self.models.len(),
            self.models
                .iter()
                .map(|c| format!("\n - {:?}", c))
                .collect::<String>(),
        )
    }
}

impl Clone for Scene {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            name: self.name.clone(),
            models: self.models.clone(),
        }
    }
}

pub struct TextureManager {
    textures: Vec<Rc<Texture>>,
}

impl TextureManager {
    pub fn new() -> Self {
        Self { textures: vec![] }
    }
    pub fn add_texture(&mut self, mut texture: Texture) -> u32 {
        let id = self.textures.len();
        texture.id = id as u32;
        self.textures.push(Rc::from(texture));
        id as u32
    }

    pub fn get_texture(&self, id: u32) -> Rc<Texture> {
        self.textures[id as usize].clone()
    }

    pub fn iter(&self) -> Iter<'_, Rc<Texture>> {
        self.textures.iter()
    }
}

pub struct MaterialManager {
    materials: Vec<Rc<RefCell<Material>>>,
}

impl MaterialManager {
    pub fn new() -> Self {
        Self { materials: vec![] }
    }
    pub fn add_material(&mut self, mut material: Material) -> u32 {
        let id = self.materials.len();
        material.id = id as u32;
        self.materials.push(Rc::new(RefCell::new(material)));
        id as u32
    }

    pub fn get_material(&self, id: u32) -> Rc<RefCell<Material>> {
        self.materials[id as usize].clone()
    }

    pub fn get_default_material(&self) -> Rc<RefCell<Material>> {
        self.materials[0].clone()
    }

    pub fn iter(&self) -> Iter<'_, Rc<RefCell<Material>>> {
        self.materials.iter()
    }
}

pub struct World {
    pub scenes: Vec<Scene>,
    pub active_scene: usize,
    pub materials: MaterialManager,
    pub textures: TextureManager,
}

impl World {
    pub fn get_active_scene(&self) -> &Scene {
        self.scenes.get(self.active_scene).unwrap()
    }
}

pub struct DrawableVertexInputs {
    pub vertex_buffer: VertexInputBuffer,
    pub normal_buffer: VertexInputBuffer,
    pub uv_buffer: VertexInputBuffer,
    pub index_buffer: Subbuffer<[u32]>,
}

impl DrawableVertexInputs {
    pub fn from_mesh(mesh: &Mesh, memory_allocator: &StandardMemoryAllocator) -> Self {
        let vertex_buffer: Subbuffer<[[f32; 3]]> = Buffer::from_iter(
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

        let normal_buffer: Subbuffer<[[f32; 3]]> = Buffer::from_iter(
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

        let uv_buffer: Subbuffer<[[f32; 2]]> = Buffer::from_iter(
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

        let index_buffer = Buffer::from_iter(
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

        Self {
            vertex_buffer: VertexInputBuffer {
                subbuffer: vertex_buffer.into_bytes(),
                vertex_count: mesh.vertices.len() as u32,
            },
            normal_buffer: VertexInputBuffer {
                subbuffer: normal_buffer.into_bytes(),
                vertex_count: mesh.normals.len() as u32,
            },
            uv_buffer: VertexInputBuffer {
                subbuffer: uv_buffer.into_bytes(),
                vertex_count: mesh.uvs.len() as u32,
            },
            index_buffer,
        }
    }
}
