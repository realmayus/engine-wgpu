use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::{Debug, Formatter};
use std::iter::{Flatten, Map};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::slice::Iter;
use std::sync::Arc;

use crate::shader_types::{MaterialInfo, MeshInfo};
use crate::{Dirtyable, VertexBuffer};
use glam::{Mat4, Vec2, Vec3, Vec4};
use log::debug;
use rand::Rng;
use vulkano::buffer::{Buffer, BufferCreateInfo, BufferUsage, Subbuffer};
use vulkano::image::view::ImageView;
use vulkano::image::ImmutableImage;
use vulkano::memory::allocator::{AllocationCreateInfo, MemoryUsage, StandardMemoryAllocator};

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
        debug!("Updated material #{}", self.id);
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

#[derive(Clone)]
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
    pub children: Vec<Model>,
    pub name: Option<Box<str>>,
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

impl Clone for Model {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            meshes: self.meshes.clone(),
            children: self.children.clone(),
            name: self.name.clone(),
            local_transform: self.local_transform,
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

pub struct World {
    pub textures: HashMap<u32, Rc<Texture>>,
    pub materials: HashMap<u32, Rc<RefCell<Material>>>,
    pub scenes: Vec<Scene>,
    pub cached_vertex_buffers: Option<Vec<VertexBuffer>>,
    pub cached_normal_buffers: Option<Vec<VertexBuffer>>,
    pub cached_uv_buffers: Option<Vec<VertexBuffer>>,
    pub cached_index_buffers: Option<Vec<Subbuffer<[u32]>>>,
}

impl World {
    pub fn get_vertex_buffers(
        &mut self,
        memory_allocator: &StandardMemoryAllocator,
    ) -> Vec<VertexBuffer> {
        if self.cached_vertex_buffers.is_none() {
            self.cached_vertex_buffers = Some(
                self.scenes
                    .iter()
                    .flat_map(|s| s.iter_meshes())
                    .map(|mesh| VertexBuffer {
                        vertex_count: mesh.vertices.len() as u32,
                        subbuffer: Buffer::from_iter(
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
                        .expect("Couldn't allocate vertex buffer")
                        .into_bytes(),
                    })
                    .collect(),
            );
        }
        self.cached_vertex_buffers.clone().unwrap()
    }

    pub fn get_normal_buffers(
        &mut self,
        memory_allocator: &StandardMemoryAllocator,
    ) -> Vec<VertexBuffer> {
        if self.cached_normal_buffers.is_none() {
            self.cached_normal_buffers = Some(
                self.scenes
                    .iter()
                    .flat_map(|s| s.iter_meshes())
                    .map(|mesh| VertexBuffer {
                        vertex_count: mesh.vertices.len() as u32,
                        subbuffer: Buffer::from_iter(
                            memory_allocator,
                            BufferCreateInfo {
                                usage: BufferUsage::VERTEX_BUFFER,
                                ..Default::default()
                            },
                            AllocationCreateInfo {
                                usage: MemoryUsage::Upload,
                                ..Default::default()
                            },
                            mesh.normals.iter().map(|v| v.to_array()),
                        )
                        .expect("Couldn't allocate normal buffer")
                        .into_bytes(),
                    })
                    .collect(),
            );
        }
        self.cached_normal_buffers.clone().unwrap()
    }

    pub fn get_uv_buffers(
        &mut self,
        memory_allocator: &StandardMemoryAllocator,
    ) -> Vec<VertexBuffer> {
        if self.cached_uv_buffers.is_none() {
            self.cached_uv_buffers = Some(
                self.scenes
                    .iter()
                    .flat_map(|s| s.iter_meshes())
                    .map(|mesh| VertexBuffer {
                        vertex_count: mesh.vertices.len() as u32,
                        subbuffer: Buffer::from_iter(
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
                        .expect("Couldn't allocate UV buffer")
                        .into_bytes(),
                    })
                    .collect(),
            );
        }
        self.cached_uv_buffers.clone().unwrap()
    }

    pub fn get_index_buffers(
        &mut self,
        memory_allocator: &StandardMemoryAllocator,
    ) -> Vec<Subbuffer<[u32]>> {
        if self.cached_index_buffers.is_none() {
            self.cached_index_buffers = Some(
                self.scenes
                    .iter()
                    .flat_map(|s| s.iter_meshes())
                    .map(|mesh| {
                        Buffer::from_iter(
                            memory_allocator,
                            BufferCreateInfo {
                                usage: BufferUsage::INDEX_BUFFER,
                                ..Default::default()
                            },
                            AllocationCreateInfo {
                                usage: MemoryUsage::Upload,
                                ..Default::default()
                            },
                            mesh.indices.clone().into_iter(),
                        )
                        .expect("Couldn't allocate index buffer")
                    })
                    .collect(),
            );
        }
        self.cached_index_buffers.clone().unwrap()
    }
}
