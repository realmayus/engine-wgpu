use glam::{Vec2, Vec3, Vec4};
use std::fmt::{Debug, Formatter};

pub struct Texture {
    id: u32,
}
impl Debug for Texture {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{{TEXTURE: id: {}}}", self.id)
    }
}

pub struct Material<'a> {
    name: Option<Box<str>>,
    base_texture: Option<&'a Texture>,
    base_color: Vec4, // this scales the RGBA components of the base_texture if defined; otherwise defines the color
    metallic_roughness_texture: Option<&'a Texture>,
    metallic_roughness_factors: Vec2, // this scales the metallic & roughness components of the metallic_roughness_texture if defined; otherwise defines the reflection characteristics
    normal_texture: Option<&'a Texture>,
    occlusion_texture: Option<&'a Texture>,
    occlusion_strength: f32,
    emissive_texture: Option<&'a Texture>,
    emissive_factors: Vec3,
}

impl<'a> Default for Material<'a> {
    fn default() -> Self {
        Self {
            name: Some(Box::from("Default material")),
            base_texture: None,
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

impl<'a> Debug for Material<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{{MATERIAL: name: {}, base_texture: {}, base_color: {:?}, metallic_roughness_texture: {}, metallic_roughness_factors: {:?}, normal_texture: {}, occlusion_texture: {}, occlusion_strength: {}, emissive_texture: {}, emissive_factors: {:?}}}",
            self.name.unwrap_or_default(),
            self.base_texture.map(|t| t.id as i32).unwrap_or(-1),  // there really shouldn't be any int overflow :p
            self.base_color,
            self.metallic_roughness_texture.map(|t| t.id as i32).unwrap_or(-1),
            self.metallic_roughness_factors,
            self.normal_texture.map(|t| t.id as i32).unwrap_or(-1),
            self.occlusion_texture.map(|t| t.id as i32).unwrap_or(-1),
            self.occlusion_strength,
            self.emissive_texture.map(|t| t.id as i32).unwrap_or(-1),
            self.emissive_factors,
        )
    }
}

pub struct Mesh<'a> {
    vertices: Vec<Vec3>,
    indices: Vec<u32>,
    normals: Vec<Vec3>,
    material: &'a Material<'a>,
}
impl<'a> Mesh<'a> {
    pub fn from(vertices: Vec<Vec3>, indices: Vec<u32>, normals: Vec<Vec3>) -> Self {
        Self {
            vertices,
            indices,
            normals,
            material: &Default::default(),
        }
    }
}

impl<'a> Debug for Mesh<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{{MESH: # of vertices: {}, # of normals: {}, # of indices: {}, material: {}}}",
            self.vertices.len(),
            self.normals.len(),
            self.indices.len(),
            self.material.name.unwrap_or_default()
        )
    }
}

pub struct Model<'a> {
    meshes: Vec<Mesh<'a>>,
    name: Option<Box<str>>,
    children: Vec<Model<'a>>,
}
impl<'a> Debug for Model<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{{MODEL: Name: {:?}, # of meshes: {}, children: [{}]}}",
            self.name,
            self.meshes.len(),
            self.children
                .iter()
                .map(|c| format!("\n - {:?}", c))
                .collect::<String>(),
        )
    }
}

pub struct Scene<'a> {
    models: Vec<Model<'a>>,
    name: Option<Box<str>>,
}

impl<'a> Scene<'a> {
    pub fn from(models: Vec<Model>, name: Option<Box<str>>) -> Self {
        Self { models, name }
    }
}
impl<'a> Debug for Scene<'a> {
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
