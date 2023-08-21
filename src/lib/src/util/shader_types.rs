use vulkano::buffer::BufferContents;

#[derive(BufferContents, Debug, Default)]
#[repr(C)]
pub struct MaterialInfo {
    pub base_color: [f32; 4],
    pub base_texture: u32, // index of texture
}

#[derive(BufferContents, Debug, Default)]
#[repr(C)]
pub struct MeshInfo {
    pub material: u32,
    _align: [u32; 3],
    pub model_transform: [[f32; 4]; 4],
}
impl MeshInfo {
    pub fn from_data(material: u32, model_transform: [[f32; 4]; 4]) -> Self {
        Self {
            material,
            _align: [0; 3],
            model_transform,
        }
    }
}
