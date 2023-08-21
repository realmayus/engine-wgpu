use std::cell::RefCell;
use std::fmt::{Debug, Formatter};
use std::rc::Rc;
use std::sync::Arc;

use crate::shader_types::{MaterialInfo, MeshInfo};
use crate::Dirtyable;
use glam::{Mat4, Vec2, Vec3, Vec4};
use rand::Rng;
use vulkano::buffer::Subbuffer;
use vulkano::image::view::ImageView;
use vulkano::image::ImmutableImage;
use vulkano::pipeline::GraphicsPipeline;

pub struct Texture {
    pub id: u32,
    pub name: Option<Box<str>>,
    pub view: Arc<ImageView<ImmutableImage>>,
}

impl Texture {
    pub fn from(view: Arc<ImageView<ImmutableImage>>, name: Option<Box<str>>, id: u32) -> Self {
        Self { view, name, id }
    }
}

impl Debug for Texture {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{{TEXTURE: name: {:?}, id: {}}}", self.name, self.id)
    }
}

pub struct Material {
    pub dirty: bool,
    pub id: u32,
    pub name: Option<Box<str>>,
    pub base_texture: Option<Rc<Texture>>,
    pub base_color: Vec4, // this scales the RGBA components of the base_texture if defined; otherwise defines the color
    pub metallic_roughness_texture: Option<Rc<Texture>>,
    pub metallic_roughness_factors: Vec2, // this scales the metallic & roughness components of the metallic_roughness_texture if defined; otherwise defines the reflection characteristics
    pub normal_texture: Option<Rc<Texture>>,
    pub occlusion_texture: Option<Rc<Texture>>,
    pub occlusion_strength: f32,
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
            base_texture,
            base_color: Vec4::from((1.0, 0.957, 0.859, 1.0)),
            metallic_roughness_texture: None,
            metallic_roughness_factors: Vec2::from((0.5, 0.5)),
            normal_texture: None,
            occlusion_texture: None,
            occlusion_strength: 0.0,
            emissive_texture: None,
            emissive_factors: Vec3::from((1.0, 1.0, 1.0)),
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
        println!("Updated material #{}", self.id);
        self.set_dirty(false);
        let mut mapping = self.buffer.write().unwrap();
        mapping.base_texture = self.base_texture.as_ref().map(|t| t.id).unwrap_or(0);
        mapping.base_color = self.base_color.to_array();
    }
}

impl Debug for Material {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{{MATERIAL: name: {}, base_texture: {}, base_color: {:?}, metallic_roughness_texture: {}, metallic_roughness_factors: {:?}, normal_texture: {}, occlusion_texture: {}, occlusion_strength: {}, emissive_texture: {}, emissive_factors: {:?}}}",
            self.name.clone().unwrap_or_default(),
            self.base_texture.clone().map(|t| t.id as i32).unwrap_or(-1),  // there really shouldn't be any int overflow :p
            self.base_color,
            self.metallic_roughness_texture.clone().map(|t| t.id as i32).unwrap_or(-1),
            self.metallic_roughness_factors,
            self.normal_texture.clone().map(|t| t.id as i32).unwrap_or(-1),
            self.occlusion_texture.clone().map(|t| t.id as i32).unwrap_or(-1),
            self.occlusion_strength,
            self.emissive_texture.clone().map(|t| t.id as i32).unwrap_or(-1),
            self.emissive_factors,
        )
    }
}

pub struct Mesh {
    dirty: bool,
    pub id: u32, // for key purposes in GUIs and stuff
    pub vertices: Vec<Vec3>,
    pub indices: Vec<u32>,
    pub normals: Vec<Vec3>,
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
            "{{MESH: # of vertices: {}, # of normals: {}, # of indices: {}, material: {}, global transform: {}}}",
            self.vertices.len(),
            self.normals.len(),
            self.indices.len(),
            self.material.borrow().name.clone().unwrap_or_default(),
            self.global_transform,
        )
    }
}

pub struct Model {
    pub id: u32,
    pub meshes: Vec<Mesh>,
    pub name: Option<Box<str>>,
    pub children: Vec<Model>,
    pub local_transform: Mat4,
}
impl Model {
    pub fn from(
        meshes: Vec<Mesh>,
        name: Option<Box<str>>,
        children: Vec<Model>,
        local_transform: Mat4,
    ) -> Self {
        Self {
            id: rand::thread_rng().gen_range(0u32..1u32 << 31),
            meshes,
            name,
            children,
            local_transform,
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
