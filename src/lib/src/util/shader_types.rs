use std::rc::Rc;
use crate::scene::{Material, Mesh, Model};
use vulkano::buffer::BufferContents;

#[derive(BufferContents, Debug, Default)]
#[repr(C)]
pub struct MaterialInfo {
    pub base_color: [f32; 4],
    pub base_texture: u32, // index of texture
}

impl From<Rc<Material>> for MaterialInfo {
    fn from(value: Rc<Material>) -> Self {
        MaterialInfo {
            base_color: value.base_color.to_array(),
            base_texture: value.base_texture.as_ref().map(|t| t.id).unwrap_or(0),
        }
    }
}

#[derive(BufferContents, Debug, Default)]
#[repr(C)]
pub struct DrawCallInfo {
    pub material: u32,
    _align: [u32; 3],
    pub model_transform: [[f32; 4]; 4],
}
impl DrawCallInfo {
    pub fn from_data(material: u32, model_transform: [[f32; 4]; 4]) -> Self {
        Self {
            material,
            _align: [0; 3],
            model_transform,
        }
    }
}

impl From<&Mesh> for DrawCallInfo {
    fn from(value: &Mesh) -> Self {
        DrawCallInfo {
            material: value.material.id,
            _align: [0; 3],
            model_transform: value.global_transform.to_cols_array_2d(),
        }
    }
}