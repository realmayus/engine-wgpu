use vulkano::buffer::BufferContents;
use vulkano::pipeline::graphics::vertex_input::Vertex;

#[derive(BufferContents, Vertex)]
#[repr(C)]
struct VertexPosition {
    #[format(R32G32B32_SFLOAT)]
    position: [f32; 3],
}

#[derive(BufferContents, Vertex)]
#[repr(C)]
struct VertexNormal {
    #[format(R32G32B32_SFLOAT)]
    normal: [f32; 3],
}

#[derive(BufferContents, Debug, Default)]
#[repr(C)]
pub struct MaterialUniform {
    pub base_color: [f32; 4],
    pub base_texture: u32, // index of texture
}

#[derive(BufferContents, Debug, Default)]
#[repr(C)]
pub struct MeshRenderSettingsUniform {
    pub uv: [f32; 2],
}

// #[derive(BufferContents, Debug, Default)]
// #[repr(C)]
// pub struct MeshRenderSettings {
//     pub materials: Vec<f32>,
// }

#[derive(BufferContents, Debug, Default)]
#[repr(C)]
pub struct DrawCallInfo {
    pub material: u32,
    _align: [u32; 3],
    pub model_transform: [[f32; 4]; 4],
}
impl DrawCallInfo {
    pub fn from(material: u32, model_transform: [[f32; 4]; 4]) -> Self {
        Self {
            material,
            _align: [0; 3],
            model_transform,
        }
    }
}
