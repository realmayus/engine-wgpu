use wgpu::{Buffer, Queue};

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
    fn update(&mut self, queue: &Queue);
}

// A buffer that also stores the number of elements in it.
pub struct SizedBuffer {
    pub buffer: Buffer,
    pub count: u32,
}


pub enum Material<'a> {
    Pbr(scene::PbrMaterial<'a>),
}
impl Material<'_> {
    pub fn id(&self) -> u32 {
        match self {
            Material::Pbr(pbr) => pbr.id,
        }
    }

    pub fn name(&self) -> &Option<Box<str>> {
        match self {
            Material::Pbr(pbr) => &pbr.name,
        }
    }
}