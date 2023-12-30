use wgpu::Buffer;

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
    fn update(&mut self);
}

// A buffer that also stores the number of elements in it.
pub struct SizedBuffer {
    pub buffer: Buffer,
    pub count: u32,
}
