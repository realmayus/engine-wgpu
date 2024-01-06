use std::cell::{Ref, RefCell};
use std::fmt::{Debug, Formatter};
use std::iter::Filter;
use std::path::PathBuf;
use std::rc::Rc;
use std::slice::Iter;
use std::sync::{Arc, Mutex, RwLock};

use crate::shader_types::{LightInfo, MaterialInfo, MeshInfo, PbrVertex};
use crate::texture::{Texture, TextureKind};
use crate::{Dirtyable, Material, SizedBuffer};
use glam::{Mat4, Vec2, Vec3, Vec4};
use itertools::izip;
use log::{debug, info, warn};
use rand::Rng;
use slab::Slab;
use slotmap::{new_key_type, SlotMap};
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::{BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindingResource, Buffer, BufferUsages, Device, Queue};

pub struct PbrMaterial {
    pub dirty: bool,
    pub shader_id: u32,
    pub name: Option<Box<str>>,
    pub albedo_texture: Option<TexId>,
    pub albedo: Vec4, // this scales the RGBA components of the base_texture if defined; otherwise defines the color
    pub metallic_roughness_texture: Option<TexId>,
    pub metallic_roughness_factors: Vec2, // this scales the metallic & roughness components of the metallic_roughness_texture if defined; otherwise defines the reflection characteristics
    pub normal_texture: Option<TexId>,
    pub occlusion_texture: Option<TexId>,
    pub occlusion_factor: f32,
    pub emissive_texture: Option<TexId>,
    pub emissive_factors: Vec3,
    pub buffer: Buffer,
    pub texture_bind_group: Option<wgpu::BindGroup>,
}

impl PbrMaterial {
    pub fn from_default(base_texture: Option<TexId>, buffer: Buffer) -> Self {
        Self {
            dirty: true,
            shader_id: 0,
            name: Some(Box::from("Default material")),
            albedo_texture: base_texture,
            albedo: Vec4::from((1.0, 0.957, 0.859, 1.0)),
            metallic_roughness_texture: None,
            metallic_roughness_factors: Vec2::from((0.5, 0.5)),
            normal_texture: None,
            occlusion_texture: None,
            occlusion_factor: 1.0,
            emissive_texture: None,
            emissive_factors: Vec3::from((0.0, 0.0, 0.0)),
            buffer,
            texture_bind_group: None,
        }
    }

    pub fn create_texture_bind_group(
        &mut self,
        device: &Device,
        layout: &BindGroupLayout,
        tex_mgr: &TextureManager,
    ) {
        let mut entries = vec![];
        for Texture { view, sampler, .. } in [
            tex_mgr.unwrap_default(&self.albedo_texture, TextureKind::Albedo),
            tex_mgr.unwrap_default(&self.normal_texture, TextureKind::Normal),
            tex_mgr.unwrap_default(
                &self.metallic_roughness_texture,
                TextureKind::MetalRoughness,
            ),
            tex_mgr.unwrap_default(&self.occlusion_texture, TextureKind::Occlusion),
            tex_mgr.unwrap_default(&self.emissive_texture, TextureKind::Emission),
        ] {
            entries.push(BindGroupEntry {
                binding: entries.len() as u32,
                resource: BindingResource::TextureView(view),
            });
            entries.push(BindGroupEntry {
                binding: entries.len() as u32,
                resource: BindingResource::Sampler(sampler),
            });
        }
        self.texture_bind_group = Some(device.create_bind_group(&BindGroupDescriptor {
            label: Some("PBR Texture Bundle Bind Group"),
            layout,
            entries: &entries,
        }));
    }

    pub(crate) fn update(&mut self, queue: &Queue) {
        self.set_dirty(false);
        let mut uniform = MaterialInfo::default();
        uniform.albedo = self.albedo.to_array();
        uniform.metal_roughness_factors = self.metallic_roughness_factors.to_array();
        uniform.occlusion_factor = self.occlusion_factor;
        uniform.emission_factors = self.emissive_factors.to_array();
        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&[uniform]));
        info!("Updated material #{}", self.shader_id);
    }
    pub fn dirty(&self) -> bool {
        self.dirty
    }

    fn set_dirty(&mut self, dirty: bool) {
        self.dirty = dirty
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
            // self.albedo_texture.as_ref().map(|t| t.id.map(|i| i as i16).unwrap_or(-2)).unwrap_or(-1),
            // self.metallic_roughness_texture.as_ref().map(|t| t.id.map(|i| i as i16).unwrap_or(-2)).unwrap_or(-1),
            // self.normal_texture.as_ref().map(|t| t.id.map(|i| i as i16).unwrap_or(-2)).unwrap_or(-1),
            // self.occlusion_texture.as_ref().map(|t| t.id.map(|i| i as i16).unwrap_or(-2)).unwrap_or(-1),
            // self.emissive_texture.as_ref().map(|t| t.id.map(|i| i as i16).unwrap_or(-2)).unwrap_or(-1),
            self.albedo_texture.is_some(),
            self.metallic_roughness_texture.is_some(),
            self.normal_texture.is_some(),
            self.occlusion_texture.is_some(),
            self.emissive_texture.is_some(),  // Todo debug print actual texture IDs
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
    pub material: MatId,
    pub uvs: Vec<Vec2>,
    pub global_transform: Mat4, // computed as product of the parent models' local transforms
    pub buffer: Buffer,         // buffer containing the model transform and material info
    pub vertex_inputs: Option<VertexInputs>,
}
impl Mesh {
    pub fn from(
        vertices: Vec<Vec3>,
        indices: Vec<u32>,
        normals: Vec<Vec3>,
        tangents: Vec<Vec4>,
        material: MatId,
        material_manager: &MaterialManager,
        uvs: Vec<Vec2>,
        global_transform: Mat4,
        device: &Device,
    ) -> Self {
        let buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("Mesh Buffer"),
            contents: bytemuck::cast_slice(&[MeshInfo::from_data(
                material_manager.get_material(material).shader_id(),
                global_transform.to_cols_array_2d(),
            )]),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });

        let vertex_inputs =
            VertexInputs::from_mesh(&vertices, &normals, &tangents, &uvs, &indices, device);

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
impl Dirtyable for Mesh {
    fn dirty(&self) -> bool {
        self.dirty
    }

    fn set_dirty(&mut self, dirty: bool) {
        self.dirty = dirty
    }

    fn update(&mut self, queue: &Queue, _texture_manager: &TextureManager, material_manager: &MaterialManager) {
        self.set_dirty(false);
        let uniform =
            MeshInfo::from_data(material_manager.get_material(self.material).shader_id(), self.global_transform.to_cols_array_2d());
        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&[uniform]));
        info!("Updated mesh {}", self.id);
    }
}
impl Debug for Mesh {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{{MESH: # of vertices: {}, # of normals: {}, # of tangents: {}, # of indices: {}, material: {:?}, global transform: {}}}",
            self.vertices.len(),
            self.normals.len(),
            self.tangents.len(),
            self.indices.len(),
            self.material,  // todo proper debugging of material (only printing MatId right now)
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
}

impl PointLight {
    pub fn new(
        global_transform: Mat4,
        index: usize,
        color: Vec3,
        intensity: f32,
        range: Option<f32>,
        device: &Device,
    ) -> Self {
        let buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("Point Light Buffer"),
            contents: bytemuck::cast_slice(&[LightInfo {
                transform: global_transform.to_cols_array_2d(),
                color: color.to_array(),
                intensity,
                range: range.unwrap_or(10.0),
                ..Default::default()
            }]),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });
        Self {
            dirty: true,
            global_transform,
            index,
            color,
            intensity,
            range,
            buffer,
        }
    }
}
impl Dirtyable for PointLight {
    fn dirty(&self) -> bool {
        self.dirty
    }

    fn set_dirty(&mut self, dirty: bool) {
        self.dirty = dirty;
    }

    fn update(&mut self, queue: &Queue, _texture_manager: &TextureManager, _material_manager: &MaterialManager) {
        self.set_dirty(false);
        let uniform = LightInfo {
            transform: self.global_transform.to_cols_array_2d(),
            color: self.color.to_array(),
            intensity: self.intensity,
            range: self.range.unwrap_or(10.0),
            ..Default::default()
        };
        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&[uniform]));
        println!("Updated light {}: {:?}", self.index, uniform);
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

new_key_type! { pub struct TexId; }
#[derive(Default)]
pub struct TextureManager {
    textures: SlotMap<TexId, Texture>,
    default_albedo: TexId,
}

impl TextureManager {
    pub fn new(device: &Device, queue: &Queue) -> Self {
        let mut textures = SlotMap::with_key();
        let default_albedo = Texture::from_image(
            device,
            queue,
            &image::load_from_memory(include_bytes!("../../../assets/textures/default.png")).unwrap(),
            Some("Default Albedo Texture"),
            TextureKind::Albedo,
        ).expect("Couldn't load default texture");
        Self {
            default_albedo: textures.insert(default_albedo),
            textures,
        }
    }
    pub fn add_texture(&mut self, mut texture: Texture) -> TexId {
        let id = self.textures.len();
        texture.id = Some(id as u32);
        let id = self.textures.insert(texture);
        id
    }

    pub fn get_texture(&self, id: &TexId) -> &Texture {
        &self.textures[*id]
    }

    pub fn iter(&self) -> impl Iterator<Item = &Texture> {
        self.textures.values()
    }

    pub fn default_tex(&self, texture_kind: TextureKind) -> &Texture {
        match texture_kind {
            TextureKind::Albedo => &self.textures[self.default_albedo],
            TextureKind::Normal => &self.textures[self.default_albedo],  // TODO support other default texture kinds
            TextureKind::MetalRoughness => &self.textures[self.default_albedo],
            TextureKind::Occlusion => &self.textures[self.default_albedo],
            TextureKind::Emission => &self.textures[self.default_albedo],
            TextureKind::Depth => &self.textures[self.default_albedo],
            TextureKind::Other => {
                warn!("No default texture for texture kind {:?}", texture_kind);
                &self.textures[self.default_albedo]
            }
        }
    }

    pub fn unwrap_default(
        &self,
        tex_id: &Option<TexId>,
        texture_kind: TextureKind,
    ) -> &Texture {
        let texture = tex_id.map(|t_id| &self.textures[t_id]);
        texture.unwrap_or_else(|| self.default_tex(texture_kind))
    }
}

new_key_type! { pub struct MatId; }

#[derive(Default)]
pub struct MaterialManager {
    materials: SlotMap<MatId, Material>,
    pub default_material: MatId,
}

impl MaterialManager {
    pub fn new(device: &Device) -> Self {
        let mut materials = SlotMap::with_key();
        let default_material = materials.insert(Material::Pbr(PbrMaterial::from_default(
            None,
            device.create_buffer_init(&BufferInitDescriptor {
                label: Some("Default Material Buffer"),
                contents: bytemuck::cast_slice(&[MaterialInfo::default()]),
                usage: BufferUsages::STORAGE | BufferUsages::COPY_DST
            }),
        )));
        Self {
            materials,
            default_material
        }
    }
    pub fn add_material(&mut self, mut material: Material) -> MatId {
        let shader_id = self.materials.len();
        material.set_shader_id(shader_id as u32);
        let mat_id = self.materials.insert(material);
        mat_id
    }

    pub fn get_material(&self, id: MatId) -> &Material {
        &self.materials[id]
    }

    pub fn get_default_material(&self) -> &Material {
        &self.materials[self.default_material]
    }

    pub fn iter(&self) -> impl Iterator<Item = &Material> {
        self.materials.values()
    }

    pub fn create_bind_group(&self, device: &Device, layout: &BindGroupLayout) -> wgpu::BindGroup {
        let mut entries = vec![];
        for mat in self.iter() {
            entries.push(mat.buffer().as_entire_buffer_binding());
        }
        device.create_bind_group(&BindGroupDescriptor {
            label: Some("PBR Materials Bind Group"),
            layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: BindingResource::BufferArray(&entries),
            }],
        })
    }

    pub fn update_dirty(&mut self, queue: &Queue) {
        for (_, mat) in self.materials.iter_mut().filter(|(_, m)| m.dirty()) {
            mat.update(queue);
        }
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

    // TODO Optimization: the performance of this must be terrible!
    pub fn pbr_meshes(&self) -> impl Iterator<Item = &Mesh> {
        self.get_active_scene()
            .iter_meshes()
            .filter(|mesh| match *self.materials.get_material(mesh.material) {
                Material::Pbr(_) => true,
            })
    }

    pub fn update_lights(&mut self, queue: &Queue) {  // TODO optimization: more efficient way of keeping track of lights
        let lights = self.scenes[self.active_scene]
            .models
            .iter_mut()
            .filter(|model| model.light.is_some())
            .filter_map(|model| model.light.as_mut()).collect::<Vec<_>>();
        for light in lights {
            if light.dirty() {
                light.update(queue, &self.textures, &self.materials);
            }
        }
    }
}

// Data passed to the vertex shader as vertex inputs, contains the vertex positions, normals, tangents, UVs and indices for a mesh
pub struct VertexInputs {
    pub vertex_buffer: SizedBuffer,
    pub index_buffer: SizedBuffer,
}

impl VertexInputs {
    pub fn from_mesh(
        vertices: &Vec<Vec3>,
        normals: &Vec<Vec3>,
        tangents: &Vec<Vec4>,
        uvs: &Vec<Vec2>,
        indices: &Vec<u32>,
        device: &Device,
    ) -> Self {
        let mut buffers = vec![];
        for (position, normal, tangent, uv) in izip!(vertices, normals, tangents, uvs) {
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
