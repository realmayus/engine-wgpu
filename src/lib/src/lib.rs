use wgpu::{Buffer, Queue};
use crate::scene::{MaterialManager, TextureManager};

pub mod scene;
pub mod scene_serde;
pub mod shader_types;
pub mod texture;
pub mod util;

pub trait Dirtyable {
    /**
    Whether or not an object was modified and is due for update
    */
    fn dirty(&self) -> bool;

    /**
    Sets object due for update
    */
    fn set_dirty(&mut self, dirty: bool);

    /**
    Call to update buffers. Sets dirty to false.
    */
    fn update(&mut self, queue: &Queue, texture_manager: &TextureManager, material_manager: &MaterialManager);
}

// A buffer that also stores the number of elements in it.
pub struct SizedBuffer {
    pub buffer: Buffer,
    pub count: u32,
}

pub enum Material {
    Pbr(scene::PbrMaterial),
}
impl Material {
    pub fn shader_id(&self) -> u32 {
        match self {
            Material::Pbr(pbr) => pbr.shader_id,
        }
    }

    pub fn set_shader_id(&mut self, id: u32) {
        match self {
            Material::Pbr(pbr) => pbr.shader_id = id,
        }
    }

    pub fn name(&self) -> &Option<Box<str>> {
        match self {
            Material::Pbr(pbr) => &pbr.name,
        }
    }

    pub fn buffer(&self) -> &Buffer {
        match self {
            Material::Pbr(pbr) => &pbr.buffer,
        }
    }

    pub fn update(&mut self, queue: &Queue) {
        match self {
            Material::Pbr(pbr) => pbr.update(queue),
        }
    }

    pub fn dirty(&self) -> bool {
        match self {
            Material::Pbr(pbr) => pbr.dirty(),
        }
    }
}