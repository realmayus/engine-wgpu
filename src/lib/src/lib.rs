use wgpu::{Buffer};

pub mod scene;
pub mod scene_serde;
pub mod shader_types;
pub mod texture;
pub mod util;
pub mod buffer_array;
pub mod managers;

pub trait Dirtyable {
    /**
    Whether or not an object was modified and is due for update
    */
    fn dirty(&self) -> bool;

    /**
    Sets object due for update
    */
    fn set_dirty(&mut self, dirty: bool);
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

    pub fn dirty(&self) -> bool {
        match self {
            Material::Pbr(pbr) => pbr.dirty(),
        }
    }
}
