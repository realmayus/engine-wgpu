use std::fmt::{Debug, Formatter};
use std::rc::Rc;
use std::sync::Arc;

use crate::util::shader_types::MaterialInfo;
use crate::Dirtyable;
use glam::{Mat4, Vec2, Vec3, Vec4};
use vulkano::image::view::ImageView;
use vulkano::image::ImmutableImage;

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
    // pub pipeline: Arc<GraphicsPipeline>,
}

impl Material {
    pub fn from_default(base_texture: Option<Rc<Texture>>) -> Self {
        Self {
            dirty: false,
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
    pub vertices: Vec<Vec3>,
    pub indices: Vec<u32>,
    pub normals: Vec<Vec3>,
    pub material: Rc<Material>,
    pub uvs: Vec<Vec2>,
    pub global_transform: Mat4, // computed as product of the parent models' local transforms
}
impl Mesh {
    pub fn from(
        vertices: Vec<Vec3>,
        indices: Vec<u32>,
        normals: Vec<Vec3>,
        material: Rc<Material>,
        uvs: Vec<Vec2>,
        global_transform: Mat4,
    ) -> Self {
        Self {
            dirty: false,
            vertices,
            indices,
            normals,
            material,
            uvs,
            global_transform,
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
}
impl Debug for Mesh {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{{MESH: # of vertices: {}, # of normals: {}, # of indices: {}, material: {}, global transform: {}}}",
            self.vertices.len(),
            self.normals.len(),
            self.indices.len(),
            self.material.name.clone().unwrap_or_default(),
            self.global_transform,
        )
    }
}

pub struct Model {
    dirty: bool,
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
            dirty: false,
            meshes,
            name,
            children,
            local_transform,
        }
    }

    /**
    Call this after changing the local_transform of a model, it updates the computed global_transforms of all meshes
    */
    pub fn update_transforms(&mut self, parent: Mat4) {
        for mesh in self.meshes.as_mut_slice() {
            mesh.global_transform = self.local_transform * parent;
        }
        for child in self.children.as_mut_slice() {
            child.update_transforms(self.local_transform);
        }
    }
}
impl Dirtyable for Model {
    fn dirty(&self) -> bool {
        self.dirty
    }

    fn set_dirty(&mut self, dirty: bool) {
        self.dirty = dirty
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
    pub models: Vec<Model>,
    pub name: Option<Box<str>>,
}

impl Scene {
    pub fn from(models: Vec<Model>, name: Option<Box<str>>) -> Self {
        Self { models, name }
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
