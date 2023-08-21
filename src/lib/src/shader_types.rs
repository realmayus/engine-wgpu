use glam::Mat4;
use vulkano::buffer::BufferContents;
use vulkano::pipeline::graphics::vertex_input::Vertex;
use crate::scene::PointLight;

// Vertex buffers

#[derive(BufferContents, Vertex)]
#[repr(C)]
pub struct MyVertex {
    #[format(R32G32B32_SFLOAT)]
    position: [f32; 3],
}

#[derive(BufferContents, Vertex)]
#[repr(C)]
pub struct MyNormal {
    #[format(R32G32B32_SFLOAT)]
    normal: [f32; 3],
}

#[derive(BufferContents, Vertex)]
#[repr(C)]
pub struct MyUV {
    #[format(R32G32_SFLOAT)]
    uv: [f32; 2],
}

// Uniforms

#[derive(BufferContents, Debug, Default, Copy, Clone)]
#[repr(C)]
pub struct CameraUniform {
    pub proj_view: [[f32; 4]; 4],
    pub view_position: [f32; 4],
}
impl CameraUniform {
    pub fn new() -> Self {
        Self {
            proj_view: Mat4::default().to_cols_array_2d(),
            view_position: [0.0; 4],
        }
    }
}

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

#[derive(BufferContents, Debug, Default)]
#[repr(C)]
pub struct LightInfo {
    pub transform: [[f32; 4]; 4],
    pub light: u32,
    pub intensity: f32,
}
impl LightInfo {
    pub fn from_light(light: &PointLight) -> Self {
        Self {
            transform: light.global_transform.to_cols_array_2d(),
            light: light.index as u32,
            intensity: light.intensity
        }
    }
}
