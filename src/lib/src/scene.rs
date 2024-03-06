use std::fmt::{Debug, Formatter};

use glam::{Vec2, Vec3, Vec4};
use hashbrown::HashMap;
use itertools::izip;
use log::debug;
use rand::Rng;
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::{BindGroupLayout, Buffer, BufferUsages, Device, Queue};

use crate::buffer_array::DynamicBufferArray;
use crate::managers::{MaterialManager, TextureManager};
use crate::scene::mesh::Mesh;
use crate::scene::model::Model;
use crate::shader_types::{LightInfo, MeshInfo, PbrVertex};
use crate::{Dirtyable, Material, SizedBuffer};

mod light;
mod material;
mod mesh;
mod model;

pub struct Scene {
    pub id: u32,
    pub models: Vec<Model>,
    pub name: Option<Box<str>>,
    pub mesh_buffer: DynamicBufferArray<MeshInfo>,
    pub light_buffer: DynamicBufferArray<LightInfo>,
}

impl Scene {
    pub fn from(
        device: &Device,
        queue: &Queue,
        models: Vec<Model>,
        material_manager: &MaterialManager,
        name: Option<Box<str>>,
        mesh_bind_group_layout: &BindGroupLayout,
        light_bind_group_layout: &BindGroupLayout,
    ) -> Self {
        let mut mesh_buffer = DynamicBufferArray::new(
            device,
            Some("Mesh Buffer".to_string()),
            BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mesh_bind_group_layout,
        );
        let mut light_buffer = DynamicBufferArray::new(
            device,
            Some("Light Buffer".to_string()),
            BufferUsages::STORAGE | BufferUsages::COPY_DST,
            light_bind_group_layout,
        );
        for model in models.iter() {
            for mesh in model.meshes.iter() {
                mesh_buffer.push(
                    device,
                    queue,
                    &[MeshInfo::from_mesh(mesh, material_manager)],
                    mesh_bind_group_layout,
                );
            }
            if let Some(light) = &model.light {
                light_buffer.push(
                    device,
                    queue,
                    &[LightInfo::from(light)],
                    light_bind_group_layout,
                );
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

    /*
    Join another scene into this one, updating the mesh and light buffers accordingly. Note: you probably need to also update the light count in the camera.
     */
    pub fn join(
        &mut self,
        other: Scene,
        device: &Device,
        queue: &Queue,
        material_manager: &MaterialManager,
        mesh_bind_group_layout: &BindGroupLayout,
        light_bind_group_layout: &BindGroupLayout,
    ) {
        for model in other.models.iter() {
            for mesh in model.meshes.iter() {
                debug!(
                    "Inserting mesh {} with material {:?}",
                    mesh.id, mesh.material
                );
                self.mesh_buffer.push(
                    device,
                    queue,
                    &[MeshInfo::from_mesh(mesh, material_manager)],
                    mesh_bind_group_layout,
                );
            }
            if let Some(light) = &model.light {
                self.light_buffer.push(
                    device,
                    queue,
                    &[LightInfo::from(light)],
                    light_bind_group_layout,
                );
            }
        }
        self.models.extend(other.models);
        self.update_meshes(queue, material_manager);
        self.update_lights(queue);
    }

    /*
    Add a model to the scene, and update the mesh and light buffers accordingly. Note: you probably need to also update the light count in the camera.
     */
    pub fn add_model(
        &mut self,
        model: Model,
        parent_id: Option<u32>,
        device: &Device,
        queue: &Queue,
        material_manager: &MaterialManager,
        mesh_bind_group_layout: &BindGroupLayout,
        light_bind_group_layout: &BindGroupLayout,
    ) {
        for mesh in model.meshes.iter() {
            self.mesh_buffer.push(
                device,
                queue,
                &[MeshInfo::from_mesh(mesh, material_manager)],
                mesh_bind_group_layout,
            );
        }
        if let Some(light) = &model.light {
            self.light_buffer.push(
                device,
                queue,
                &[LightInfo::from(light)],
                light_bind_group_layout,
            );
        }
        if let Some(parent_id) = parent_id {
            self.models
                .iter_mut()
                .find(|m| m.id == parent_id)
                .unwrap()
                .children
                .push(model);
        } else {
            self.models.push(model);
        }
        self.update_meshes(queue, material_manager);
        self.update_lights(queue);
    }
    fn remove_model_deep(models: &mut Vec<Model>, model_id: u32) -> Option<Model> {
        let mut found_model = None;
        for (i, model) in models.iter_mut().enumerate() {
            if model.id == model_id {
                found_model = Some(i);
                break;
            }
        }
        if found_model.is_none() {
            for model in models.iter_mut() {
                if let Some(found_model) = Self::remove_model_deep(&mut model.children, model_id) {
                    return Some(found_model);
                }
            }
        }
        Some(models.remove(found_model?))
    }
    pub fn remove_model(
        &mut self,
        model_id: u32,
        queue: &Queue,
        material_manager: &MaterialManager,
    ) -> Option<Model> {
        let model = Self::remove_model_deep(&mut self.models, model_id);
        self.update_meshes(queue, material_manager);
        self.update_lights(queue);
        model
    }

    pub fn iter_meshes(&self) -> impl Iterator<Item = &Mesh> {
        self.models.iter().flat_map(|model| model.meshes.iter())
    }

    pub fn iter_models_deep(&self) -> impl Iterator<Item = &Model> {
        self.models.iter().chain(
            self.models
                .iter()
                .flat_map(|model| model.children.iter_deep()),
        )
    }

    pub fn update_meshes(&mut self, queue: &Queue, material_manager: &MaterialManager) {
        for (i, mesh) in self
            .models
            .iter()
            .flat_map(|model| model.meshes.iter())
            .enumerate()
            .filter(|(_, mesh)| mesh.dirty())
        {
            self.mesh_buffer
                .update(queue, i as u32, MeshInfo::from_mesh(mesh, material_manager));
        }
    }

    pub fn update_lights(&mut self, queue: &Queue) {
        for model in self
            .models
            .iter()
            .filter(|model| model.light.is_some() && model.light.as_ref().unwrap().dirty)
        {
            let light = model.light.as_ref().unwrap();
            self.light_buffer
                .update(queue, light.index as u32, LightInfo::from(light)); // TODO is light.index what we want here?
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
    pub scenes: HashMap<usize, Scene>,
    pub active_scene: usize,
    pub materials: MaterialManager,
    pub textures: TextureManager,
}

impl World {
    pub fn get_active_scene(&self) -> &Scene {
        self.scenes.get(&self.active_scene).unwrap()
    }

    // TODO Optimization: the performance of this must be terrible!
    pub fn pbr_meshes(&self) -> impl Iterator<Item = &Mesh> {
        self.get_active_scene().iter_meshes().filter(|mesh| {
            match *self.materials.get_material(mesh.material) {
                Material::Pbr(_) => true,
            }
        })
    }

    pub fn update_active_scene(&mut self, queue: &Queue) {
        let scene = &mut self
            .scenes
            .get_mut(&self.active_scene)
            .expect("Invalid active scene");
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
