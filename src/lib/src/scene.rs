use std::cell::{Ref, RefCell};
use std::fmt::{Debug, Formatter};
use std::iter::Filter;
use std::path::PathBuf;
use std::rc::Rc;
use std::slice::Iter;
use std::sync::Arc;

use crate::shader_types::{LightInfo, MaterialInfo, MeshInfo, PbrVertex};
use crate::{Dirtyable, Material, SizedBuffer};
use glam::{Mat4, Vec2, Vec3, Vec4};
use itertools::izip;
use log::{debug, info, warn};
use rand::Rng;
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::{BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindingResource, Buffer, Device};
use crate::texture::{Texture, TextureKind};

pub struct PbrMaterial {
    pub dirty: bool,
    pub id: u32,
    pub name: Option<Box<str>>,
    pub albedo_texture: Option<Rc<Texture>>,
    pub albedo: Vec4, // this scales the RGBA components of the base_texture if defined; otherwise defines the color
    pub metallic_roughness_texture: Option<Rc<Texture>>,
    pub metallic_roughness_factors: Vec2, // this scales the metallic & roughness components of the metallic_roughness_texture if defined; otherwise defines the reflection characteristics
    pub normal_texture: Option<Rc<Texture>>,
    pub occlusion_texture: Option<Rc<Texture>>,
    pub occlusion_factor: f32,
    pub emissive_texture: Option<Rc<Texture>>,
    pub emissive_factors: Vec3,
    pub buffer: Buffer,
    pub texture_bind_group: Option<wgpu::BindGroup>,
}

impl PbrMaterial {
    pub fn from_default(
        base_texture: Option<Rc<Texture>>,
        buffer: Buffer,
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
            texture_bind_group: None,
        }
    }

    pub fn create_texture_bind_group(&mut self, device: &Device, layout: &BindGroupLayout, tex_mgr: &TextureManager) {
        let mut entries = vec![];
        for rc_tex in [
            tex_mgr.unwrap_default(&self.albedo_texture, TextureKind::Albedo),
            tex_mgr.unwrap_default(&self.normal_texture, TextureKind::Normal),
            tex_mgr.unwrap_default(&self.metallic_roughness_texture, TextureKind::MetalRoughness),
            tex_mgr.unwrap_default(&self.occlusion_texture, TextureKind::Occlusion),
            tex_mgr.unwrap_default(&self.emissive_texture, TextureKind::Emission),
        ] {
            let Texture{view, sampler, .. } = rc_tex.as_ref();
            entries.push(BindGroupEntry {
                binding: entries.len() as u32,
                resource: BindingResource::TextureView(&view),
            });
            entries.push(BindGroupEntry {
                binding: entries.len() as u32,
                resource: BindingResource::Sampler(&sampler),
            });
        }
        self.texture_bind_group = Some(device.create_bind_group(
            &BindGroupDescriptor{
                label: Some("PBR Texture Bundle Bind Group"),
                layout,
                entries: &entries,
            }
        ));
    }
}

impl Dirtyable for PbrMaterial {
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
            .map(|t| t.id.unwrap())
            .unwrap_or(0u16);
        mapping.metal_roughness_factors = self.metallic_roughness_factors.to_array();
        mapping.normal_texture = self.normal_texture.as_ref().map(|t| t.id.unwrap()).unwrap_or(1);
        mapping.occlusion_texture = self.occlusion_texture.as_ref().map(|t| t.id).unwrap_or(0);
        mapping.occlusion_factor = self.occlusion_factor;
        mapping.emission_texture = self.emissive_texture.as_ref().map(|t| t.id).unwrap_or(0);
        mapping.emission_factors = self.emissive_factors.to_array();
    }
}

impl Debug for PbrMaterial {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        // -1 means no texture, -2 means there is a texture but its ID is None fsr...
        write!(
            f,
            "{{MATERIAL: Name: {:?}, albedo: {:?}, metallic_roughness_factors: {:?}, occlusion_factor: {}, emissive_factors: {:?}, albedo_texture: {:?}, metallic_roughness_texture: {:?}, normal_texture: {:?}, occlusion_texture: {:?}, emissive_texture: {:?}}}",
            self.name,
            self.albedo,
            self.metallic_roughness_factors,
            self.occlusion_factor,
            self.emissive_factors,
            self.albedo_texture.as_ref().map(|t| t.id.map(|i| i as i16).unwrap_or(-2)).unwrap_or(-1),
            self.metallic_roughness_texture.as_ref().map(|t| t.id.map(|i| i as i16).unwrap_or(-2)).unwrap_or(-1),
            self.normal_texture.as_ref().map(|t| t.id.map(|i| i as i16).unwrap_or(-2)).unwrap_or(-1),
            self.occlusion_texture.as_ref().map(|t| t.id.map(|i| i as i16).unwrap_or(-2)).unwrap_or(-1),
            self.emissive_texture.as_ref().map(|t| t.id.map(|i| i as i16).unwrap_or(-2)).unwrap_or(-1),
        )
    }
}

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
    pub buffer: Buffer, // buffer containing the model transform and material info
    pub vertex_inputs: Option<VertexInputs>,
}
impl Mesh {
    pub fn from(
        vertices: Vec<Vec3>,
        indices: Vec<u32>,
        normals: Vec<Vec3>,
        tangents: Vec<Vec4>,
        material: Rc<RefCell<PbrMaterial>>,
        uvs: Vec<Vec2>,
        global_transform: Mat4,
        device: &Device,
    ) -> Self {
        let buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("Mesh Buffer"),
            contents: bytemuck::cast_slice(&[MeshInfo::from_data(
                material.borrow().id,
                global_transform.to_cols_array_2d(),
            )]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let vertex_inputs = VertexInputs::from_mesh()
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
    pub buffer: Buffer,
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

#[derive(Default)]
pub struct TextureManager {
    textures: Vec<Rc<Texture>>,
}

impl TextureManager {
    pub fn new() -> Self {
        Self { textures: vec![] }
    }
    pub fn add_texture(&mut self, mut texture: Texture) -> u32 {
        let id = self.textures.len();
        texture.id = Some(id as u16);
        self.textures.push(Rc::from(texture));
        id as u32
    }

    pub fn get_texture(&self, id: u32) -> Rc<Texture> {
        self.textures[id as usize].clone()
    }

    pub fn iter(&self) -> Iter<'_, Rc<Texture>> {
        self.textures.iter()
    }

    pub fn default_tex(&self, texture_kind: TextureKind) -> Rc<Texture> {
        match texture_kind {
            TextureKind::Albedo => {
                self.textures[0].clone()
            }
            TextureKind::Normal => {
                self.textures[0].clone()
            }
            TextureKind::MetalRoughness => {
                self.textures[0].clone()
            }
            TextureKind::Occlusion => {
                self.textures[0].clone()
            }
            TextureKind::Emission => {
                self.textures[0].clone()
            }
            TextureKind::Depth => {
                self.textures[0].clone()
            }
            TextureKind::Other => {
                warn!("No default texture for texture kind {:?}", texture_kind);
                self.textures[0].clone()
            }
        }
    }

    pub fn unwrap_default(&self, texture: &Option<Rc<Texture>>, texture_kind: TextureKind) -> Rc<Texture> {
        texture.unwrap_or_else(|| self.default_tex(texture_kind))
    }
}

#[derive(Default)]
pub struct MaterialManager {
    materials: Vec<Rc<RefCell<PbrMaterial>>>,
}

impl MaterialManager {
    pub fn new() -> Self {
        Self { materials: vec![] }
    }
    pub fn add_material(&mut self, mut material: PbrMaterial) -> u32 {
        let id = self.materials.len();
        material.id = id as u32;
        self.materials.push(Rc::new(RefCell::new(material)));
        id as u32
    }

    pub fn get_material(&self, id: u32) -> Rc<RefCell<PbrMaterial>> {
        self.materials[id as usize].clone()
    }

    pub fn get_default_material(&self) -> Rc<RefCell<PbrMaterial>> {
        self.materials[0].clone()
    }

    pub fn iter(&self) -> Iter<'_, Rc<RefCell<PbrMaterial>>> {
        self.materials.iter()
    }

    pub fn get_buffer_array(&self) -> Vec<Buffer> {
        self.iter().map(|mat| mat.borrow().buffer.clone()).collect()
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

    pub fn get_active_scene_mut(&mut self) -> &mut Scene {
        self.scenes.get_mut(self.active_scene).unwrap()
    }

    // TODO Optimization: the performance of this must be terrible!
    pub fn pbr_meshes(&self) -> impl Iterator<Item = &Mesh> {
        self.get_active_scene().iter_meshes().filter(|mesh| match *mesh.material.borrow() {
            Material::Pbr(_) => true,
        })
    }
}

// Data passed to the vertex shader as vertex inputs, contains the vertex positions, normals, tangents, UVs and indices for a mesh
pub struct VertexInputs {
    pub vertex_buffer: SizedBuffer,
    pub index_buffer: SizedBuffer,
}

impl VertexInputs {
    pub fn from_mesh(vertices: &Vec<Vec3>, normals: &Vec<Vec3>, tangents: &Vec<Vec4>, uvs: &Vec<Vec2>, indices: &Vec<u32>, device: &Device) -> Self {
        let mut buffers = vec![];
        for (position, normal, tangent, uv) in izip!(vertices, normals, tangents, uvs)
        {
            buffers.push(PbrVertex {
                position: (*position).into(),
                normal: (*normal).into(),
                tangent: (*tangent).into(),
                uv: (*uv).into(),
            });
        }

        let vertex_buffer: Buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(&buffers),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("Index Buffer"),
            contents: bytemuck::cast_slice(
                &indices.iter().map(|i| *i as u16).collect::<Vec<u16>>(),
            ),
            usage: wgpu::BufferUsages::INDEX,
        });

        Self {
            vertex_buffer: SizedBuffer {
                buffer: vertex_buffer,
                count: vertices.len() as u32,
            },
            index_buffer: SizedBuffer {
                buffer: index_buffer,
                count: indices.len() as u32,
            },
        }
    }
}
