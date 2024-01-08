use std::fmt::{Debug, Formatter};
use crate::shader_types::{LightInfo, MeshInfo, PbrVertex};
use crate::texture::{Texture, TextureKind};
use crate::{Dirtyable, Material, SizedBuffer};
use glam::{Mat4, Vec2, Vec3, Vec4};
use itertools::izip;
use rand::Rng;
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::{BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindingResource, Buffer, BufferUsages, Device, Queue};
use crate::buffer_array::DynamicBufferArray;
use crate::managers::{MaterialManager, MatId, TexId, TextureManager};

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
    pub texture_bind_group: Option<wgpu::BindGroup>,
}

impl PbrMaterial {
    pub fn from_default(base_texture: Option<TexId>) -> Self {
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
    pub vertex_inputs: Option<VertexInputs>,
}
impl Mesh {
    pub fn from(
        vertices: Vec<Vec3>,
        indices: Vec<u32>,
        normals: Vec<Vec3>,
        tangents: Vec<Vec4>,
        material: MatId,
        uvs: Vec<Vec2>,
        global_transform: Mat4,
        device: &Device,
    ) -> Self {

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
    // pub shadow_view: Option<Texture>,
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
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
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
    pub mesh_buffer: DynamicBufferArray<MeshInfo>,
    pub light_buffer: DynamicBufferArray<LightInfo>,
}

impl Scene {
    pub fn from(device: &Device, queue: &Queue, models: Vec<Model>, material_manager: &MaterialManager, name: Option<Box<str>>) -> Self {
        let mut mesh_buffer = DynamicBufferArray::new(
            device,
            Some("Mesh Buffer".to_string()),
            BufferUsages::STORAGE | BufferUsages::COPY_DST,
        );
        let mut light_buffer = DynamicBufferArray::new(
            device,
            Some("Light Buffer".to_string()),
            BufferUsages::STORAGE | BufferUsages::COPY_DST,
        );
        for model in models.iter() {
            for mesh in model.meshes.iter() {
                mesh_buffer.push(
                    device,
                    queue,
                    &[MeshInfo::from_mesh(mesh, material_manager)],
                );
            }
            if let Some(light) = &model.light {
                light_buffer.push(device, queue, &[LightInfo::from(light)]);
            }
        }

        Self {
            id: rand::thread_rng().gen_range(0u32..1u32 << 31),
            models,
            name,
            mesh_buffer,
            light_buffer,
        }
    }

    pub fn iter_meshes(&self) -> impl Iterator<Item = &Mesh> {
        self.models.iter().flat_map(|model| model.meshes.iter())
    }

    pub fn update_meshes(&mut self, queue: &Queue, material_manager: &MaterialManager) {
        for (i, mesh) in self.models.iter().flat_map(|model| model.meshes.iter()).enumerate().filter(|(_, mesh)| mesh.dirty()) {
            self.mesh_buffer.update(queue, i as u32, MeshInfo::from_mesh(mesh, material_manager));
        }
    }

    pub fn update_lights(&mut self, queue: &Queue) {
        for model in self.models.iter().filter(|model| model.light.is_some() && model.light.as_ref().unwrap().dirty) {
            let light = model.light.as_ref().unwrap();
            self.light_buffer.update(queue, light.index as u32, LightInfo::from(light));  // TODO is light.index what we want here?
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

    pub fn update_active_scene(&mut self, queue: &Queue) {
        let scene = &mut self.scenes[self.active_scene];
        scene.update_meshes(queue, &self.materials);
        scene.update_lights(queue);
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
