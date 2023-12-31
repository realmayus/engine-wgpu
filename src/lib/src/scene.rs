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
use wgpu::{BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindingResource, Buffer, Device, Queue};
use crate::texture::{Texture, TextureKind};

pub struct PbrMaterial<'a> {
    pub dirty: bool,
    pub id: u32,
    pub name: Option<Box<str>>,
    pub albedo_texture: Option<&'a Texture>,
    pub albedo: Vec4, // this scales the RGBA components of the base_texture if defined; otherwise defines the color
    pub metallic_roughness_texture: Option<&'a Texture>,
    pub metallic_roughness_factors: Vec2, // this scales the metallic & roughness components of the metallic_roughness_texture if defined; otherwise defines the reflection characteristics
    pub normal_texture: Option<&'a Texture>,
    pub occlusion_texture: Option<&'a Texture>,
    pub occlusion_factor: f32,
    pub emissive_texture: Option<&'a Texture>,
    pub emissive_factors: Vec3,
    pub buffer: Buffer,
    pub texture_bind_group: Option<wgpu::BindGroup>,
}

impl<'a, 'b: 'a> PbrMaterial<'a> {
    pub fn from_default(
        base_texture: Option<&'b Texture>,
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
        for Texture{view, sampler, .. } in [
            tex_mgr.unwrap_default(&self.albedo_texture, TextureKind::Albedo),
            tex_mgr.unwrap_default(&self.normal_texture, TextureKind::Normal),
            tex_mgr.unwrap_default(&self.metallic_roughness_texture, TextureKind::MetalRoughness),
            tex_mgr.unwrap_default(&self.occlusion_texture, TextureKind::Occlusion),
            tex_mgr.unwrap_default(&self.emissive_texture, TextureKind::Emission),
        ] {
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

impl Dirtyable for PbrMaterial<'_> {
    fn dirty(&self) -> bool {
        self.dirty
    }

    fn set_dirty(&mut self, dirty: bool) {
        self.dirty = dirty
    }

    fn update(&mut self, queue: &Queue) {
        self.set_dirty(false);
        let mut uniform = MaterialInfo::default();
        uniform.albedo_texture = self.albedo_texture.as_ref().map(|t| t.id.unwrap()).unwrap_or(0);
        uniform.albedo = self.albedo.to_array();
        uniform.metal_roughness_texture = self
            .metallic_roughness_texture
            .as_ref()
            .map(|t| t.id.unwrap())
            .unwrap_or(0u16);
        uniform.metal_roughness_factors = self.metallic_roughness_factors.to_array();
        uniform.normal_texture = self.normal_texture.as_ref().map(|t| t.id.unwrap()).unwrap_or(1);
        uniform.occlusion_texture = self.occlusion_texture.as_ref().map(|t| t.id.unwrap()).unwrap_or(0);
        uniform.occlusion_factor = self.occlusion_factor;
        uniform.emission_texture = self.emissive_texture.as_ref().map(|t| t.id.unwrap()).unwrap_or(0);
        uniform.emission_factors = self.emissive_factors.to_array();
        info!("Updated material #{}", self.id);
    }
}

impl Debug for PbrMaterial<'_> {
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

pub struct Mesh<'a> {
    dirty: bool,
    pub id: u32, // for key purposes in GUIs and stuff
    pub vertices: Vec<Vec3>,
    pub indices: Vec<u32>,
    pub normals: Vec<Vec3>,
    pub tangents: Vec<Vec4>,
    pub material: &'a Material<'a>,
    pub uvs: Vec<Vec2>,
    pub global_transform: Mat4, // computed as product of the parent models' local transforms
    pub buffer: Buffer, // buffer containing the model transform and material info
    pub vertex_inputs: Option<VertexInputs>,
}
impl<'a, 'b: 'a> Mesh<'a> {
    pub fn from(
        vertices: Vec<Vec3>,
        indices: Vec<u32>,
        normals: Vec<Vec3>,
        tangents: Vec<Vec4>,
        material: &'b Material,
        uvs: Vec<Vec2>,
        global_transform: Mat4,
        device: &Device,
    ) -> Self {
        let buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("Mesh Buffer"),
            contents: bytemuck::cast_slice(&[MeshInfo::from_data(
                material.id(),
                global_transform.to_cols_array_2d(),
            )]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let vertex_inputs = VertexInputs::from_mesh(
            &vertices,
            &normals,
            &tangents,
            &uvs,
            &indices,
            device,
        );

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
            vertex_inputs: Some(vertex_inputs),
        }
    }
}
impl Dirtyable for Mesh<'_> {
    fn dirty(&self) -> bool {
        self.dirty
    }

    fn set_dirty(&mut self, dirty: bool) {
        self.dirty = dirty
    }

    fn update(&mut self, queue: &Queue) {
        self.set_dirty(false);
        let uniform = MeshInfo::from_data(
            self.material.id(),
            self.global_transform.to_cols_array_2d(),
        );
        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&[uniform]));
        info!("Updated mesh {}", self.id);
    }
}
impl Debug for Mesh<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{{MESH: # of vertices: {}, # of normals: {}, # of tangents: {}, # of indices: {}, material: {}, global transform: {}}}",
            self.vertices.len(),
            self.normals.len(),
            self.tangents.len(),
            self.indices.len(),
            self.material.name().clone().unwrap_or_default(),
            self.global_transform,
        )
    }
}

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

    fn update(&mut self, queue: &Queue) {
        self.set_dirty(false);
        let uniform = LightInfo {
            transform: self.global_transform.to_cols_array_2d(),
            color: self.color.to_array(),
            light: self.index as u32,
            intensity: self.intensity,
            amount: self.amount,
            range: self.range.unwrap_or(1.0),
        };
        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&[uniform]));
        info!("Updated light {}", self.index);
    }
}

pub struct Model<'a> {
    pub id: u32,
    pub meshes: Vec<Mesh<'a>>,
    pub children: Vec<Model<'a>>,
    pub name: Option<Box<str>>,
    pub local_transform: Mat4,
    pub light: Option<PointLight>,
}
impl<'a, 'b: 'a> Model<'a> {
    pub fn from(
        meshes: Vec<Mesh<'b>>,
        name: Option<Box<str>>,
        children: Vec<Model<'b>>,
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

impl Debug for Model<'_> {
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


pub struct Scene<'a> {
    pub id: u32,
    pub models: Vec<Model<'a>>,
    pub name: Option<Box<str>>,
}

impl<'a, 'b: 'a> Scene<'a> {
    pub fn from(models: Vec<Model<'b>>, name: Option<Box<str>>) -> Self {
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

impl Debug for Scene<'_> {
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


#[derive(Default)]
pub struct TextureManager {
    textures: Vec<Texture>,
}

impl TextureManager {
    pub fn new() -> Self {
        Self { textures: vec![] }
    }
    pub fn add_texture(&mut self, mut texture: Texture) -> u32 {
        let id = self.textures.len();
        texture.id = Some(id as u16);
        self.textures.push(texture);
        id as u32
    }

    pub fn get_texture(&self, id: u32) -> &Texture {
        &self.textures[id as usize]
    }

    pub fn iter(&self) -> Iter<'_, Texture> {
        self.textures.iter()
    }

    pub fn default_tex(&self, texture_kind: TextureKind) -> &Texture {
        match texture_kind {
            TextureKind::Albedo => {
                &self.textures[0]
            }
            TextureKind::Normal => {
                &self.textures[0]
            }
            TextureKind::MetalRoughness => {
                &self.textures[0]
            }
            TextureKind::Occlusion => {
                &self.textures[0]
            }
            TextureKind::Emission => {
                &self.textures[0]
            }
            TextureKind::Depth => {
                &self.textures[0]
            }
            TextureKind::Other => {
                warn!("No default texture for texture kind {:?}", texture_kind);
                &self.textures[0]
            }
        }
    }

    pub fn unwrap_default<'a>(&'a self, texture: &Option<&'a Texture>, texture_kind: TextureKind) -> &Texture {
        texture.unwrap_or_else(|| self.default_tex(texture_kind))
    }
}

#[derive(Default)]
pub struct MaterialManager<'a> {
    materials: Vec<PbrMaterial<'a>>,
}

impl<'a, 'b: 'a> MaterialManager<'a> {
    pub fn new() -> Self {
        Self { materials: vec![] }
    }
    pub fn add_material(&mut self, mut material: PbrMaterial<'b>) -> u32 {
        let id = self.materials.len();
        material.id = id as u32;
        self.materials.push(material);
        id as u32
    }

    pub fn get_material(&self, id: u32) -> &PbrMaterial {
        &self.materials[id as usize]
    }

    pub fn get_default_material(&self) -> &PbrMaterial {
        &self.materials[0]
    }

    pub fn iter(&self) -> Iter<'_, PbrMaterial> {
        self.materials.iter()
    }

    pub fn create_bind_group(&self, device: &Device, layout: &BindGroupLayout) -> wgpu::BindGroup {
        let mut entries = vec![];
        for rc_mat in self.materials.iter() {
            entries.push(rc_mat.buffer.as_entire_buffer_binding());
        }
        device.create_bind_group(
            &BindGroupDescriptor{
                label: Some("PBR Materials Bind Group"),
                layout,
                entries: &[
                    BindGroupEntry {
                        binding: 0,
                        resource: BindingResource::BufferArray(&entries),
                    }
                ],
            }
        )
    }
}

pub struct World<'a> {
    pub scenes: Vec<Scene<'a>>,
    pub active_scene: usize,
    pub materials: MaterialManager<'a>,
    pub textures: TextureManager,
}

impl<'a> World<'a> {
    pub fn get_active_scene(&self) -> &Scene {
        self.scenes.get(self.active_scene).unwrap()
    }

    // TODO Optimization: the performance of this must be terrible!
    pub fn pbr_meshes(&self) -> impl Iterator<Item = &Mesh> {
        self.get_active_scene().iter_meshes().filter(|mesh| match *mesh.material {
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
