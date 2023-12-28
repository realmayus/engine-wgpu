use std::cell::RefCell;
use std::fmt::Write;
use std::fmt::{Debug, Formatter};
use std::path::PathBuf;
use std::rc::Rc;
use std::slice::Iter;
use std::sync::Arc;

use glam::{Mat4, Vec2, Vec3, Vec4};
use image::DynamicImage;
use image::ImageFormat::Png;
use log::debug;
use rand::Rng;
use vulkano::buffer::{Buffer, BufferCreateInfo, BufferUsage, Subbuffer};
use vulkano::command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer};
use vulkano::device::Device;
use vulkano::format;
use vulkano::image::view::ImageView;
use vulkano::image::{ImageViewAbstract, ImmutableImage};
use vulkano::memory::allocator::{AllocationCreateInfo, MemoryUsage, StandardMemoryAllocator};
use vulkano::sampler::{Sampler, SamplerCreateInfo};

use crate::shader_types::{MaterialInfo, MeshInfo};
use crate::texture::create_texture;
use crate::util::extract_image_to_file;
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
        mapping.base_texture = self.base_texture.as_ref().map(|t| t.id).unwrap_or(1);
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
            self.children.iter().fold(String::new(), |mut acc, n| {
                let _ = write!(acc, "\n - {:?}", n);
                acc
            }) //.collect::<String>(),
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

#[derive(Default)]
pub struct TextureManager {
    textures: Vec<Rc<Texture>>,
}

impl TextureManager {
    pub fn new(
        memory_allocator: &Arc<StandardMemoryAllocator>,
        cmd_buf_builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
    ) -> Self {
        let mut textures = vec![];
        {
            let img = image::open("assets/textures/white.png")
                .expect("Couldn't load default texture")
                .to_rgba8();
            let width = img.width();
            let height = img.height();
            let dyn_img = DynamicImage::from(img);

            let path = extract_image_to_file("white", &dyn_img, Png);

            let tex = create_texture(
                dyn_img.into_bytes(),
                format::Format::R8G8B8A8_UNORM,
                width,
                height,
                memory_allocator,
                cmd_buf_builder,
            );

            let texture = Texture::from(tex, Some(Box::from("Default texture")), 0, path);
            textures.push(Rc::from(texture));
        }

        {
            let img = image::open("assets/textures/default_normal.png")
                .expect("Couldn't load white texture")
                .to_rgba8();
            let width = img.width();
            let height = img.height();
            let dyn_img = DynamicImage::from(img);

            let path = extract_image_to_file("default_normal", &dyn_img, Png);

            let tex = create_texture(
                dyn_img.into_bytes(),
                format::Format::R8G8B8A8_UNORM,
                width,
                height,
                memory_allocator,
                cmd_buf_builder,
            );

            let texture = Texture::from(tex, Some(Box::from("Default normal texture")), 1, path);
            textures.push(Rc::from(texture));
        }

        {
            let img = image::open("assets/textures/no_texture.png")
                .expect("Couldn't load white texture")
                .to_rgba8();
            let width = img.width();
            let height = img.height();
            let dyn_img = DynamicImage::from(img);

            let path = extract_image_to_file("no_texture", &dyn_img, Png);

            let tex = create_texture(
                dyn_img.into_bytes(),
                format::Format::R8G8B8A8_UNORM,
                width,
                height,
                memory_allocator,
                cmd_buf_builder,
            );

            let texture = Texture::from(tex, Some(Box::from("No texture")), 2, path);
            textures.push(Rc::from(texture));
        }
        Self { textures }
    }
    pub fn new_ref(
        memory_allocator: &StandardMemoryAllocator,
        cmd_buf_builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
    ) -> Self {
        let mut textures = vec![];
        {
            let img = image::open("assets/textures/white.png")
                .expect("Couldn't load default texture")
                .to_rgba8();
            let width = img.width();
            let height = img.height();
            let dyn_img = DynamicImage::from(img);

            let path = extract_image_to_file("white", &dyn_img, Png);

            let tex = create_texture(
                dyn_img.into_bytes(),
                format::Format::R8G8B8A8_UNORM,
                width,
                height,
                memory_allocator,
                cmd_buf_builder,
            );

            let texture = Texture::from(tex, Some(Box::from("Default texture")), 0, path);
            textures.push(Rc::from(texture));
        }

        {
            let img = image::open("assets/textures/default_normal.png")
                .expect("Couldn't load white texture")
                .to_rgba8();
            let width = img.width();
            let height = img.height();
            let dyn_img = DynamicImage::from(img);

            let path = extract_image_to_file("default_normal", &dyn_img, Png);

            let tex = create_texture(
                dyn_img.into_bytes(),
                format::Format::R8G8B8A8_UNORM,
                width,
                height,
                memory_allocator,
                cmd_buf_builder,
            );

            let texture = Texture::from(tex, Some(Box::from("Default normal texture")), 1, path);
            textures.push(Rc::from(texture));
        }

        {
            let img = image::open("assets/textures/no_texture.png")
                .expect("Couldn't load white texture")
                .to_rgba8();
            let width = img.width();
            let height = img.height();
            let dyn_img = DynamicImage::from(img);

            let path = extract_image_to_file("no_texture", &dyn_img, Png);

            let tex = create_texture(
                dyn_img.into_bytes(),
                format::Format::R8G8B8A8_UNORM,
                width,
                height,
                memory_allocator,
                cmd_buf_builder,
            );

            let texture = Texture::from(tex, Some(Box::from("No texture")), 2, path);
            textures.push(Rc::from(texture));
        }
        Self { textures }
    }

    pub fn add_texture(&mut self, mut texture: Texture) -> u32 {
        let id = self.textures.len();
        texture.id = id as u32;
        self.textures.push(Rc::from(texture));
        id as u32
    }

    pub fn get_default_texture(&self, default_texture_type: DefaultTextureType) -> Rc<Texture> {
        use DefaultTextureType as dtt;
        match default_texture_type {
            dtt::Default => self.textures[0].clone(),
            dtt::DefaultNormal => self.textures[1].clone(),
            dtt::NoTexture => self.textures[2].clone(),
        }
    }

    pub fn get_texture(&self, id: u32) -> Rc<Texture> {
        self.textures[id as usize].clone()
    }

    pub fn iter(&self) -> Iter<'_, Rc<Texture>> {
        self.textures.iter()
    }

    pub fn get_view_sampler_array(
        &self,
        device: Arc<Device>,
    ) -> Vec<(Arc<dyn ImageViewAbstract>, Arc<Sampler>)> {
        //TODO Optimization: work out if we really need to enforce Vec everywhere or if slices are sufficient
        self.iter()
            .map(|t| {
                (
                    t.view.clone() as Arc<dyn ImageViewAbstract>,
                    Sampler::new(device.clone(), SamplerCreateInfo::simple_repeat_linear())
                        .unwrap(),
                )
            })
            .collect()
    }
}
pub enum DefaultTextureType {
    Default,
    DefaultNormal,
    NoTexture,
}

#[derive(Default)]
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

    pub fn get_buffer_array(&self) -> Vec<Subbuffer<MaterialInfo>> {
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
