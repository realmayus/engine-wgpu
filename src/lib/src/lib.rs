use vulkano::buffer::Subbuffer;

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

pub struct VertexInputBuffer {
    pub subbuffer: Subbuffer<[u8]>,
    pub vertex_count: u32,
}
