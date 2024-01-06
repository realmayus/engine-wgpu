use crate::scene::{MaterialManager, PbrMaterial};
use crate::scene::PointLight;
use glam::Mat4;

pub trait Vertex {
    const ATTRIBS: [wgpu::VertexAttribute; 4];
    fn desc() -> wgpu::VertexBufferLayout<'static>;
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PbrVertex {
    pub(crate) position: [f32; 3],
    pub(crate) normal: [f32; 3],
    pub(crate) tangent: [f32; 4],
    pub(crate) uv: [f32; 2],
}
impl Vertex for PbrVertex {
    const ATTRIBS: [wgpu::VertexAttribute; 4] =
        wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x4, 3 => Float32x2];
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<PbrVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }
}

// Uniforms

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraUniform {
    pub proj_view: [[f32; 4]; 4],
    pub view_position: [f32; 4],
    pub num_lights: u32,
    pub padding: [u32; 3],
}
impl CameraUniform {
    pub fn new() -> Self {
        Self {
            proj_view: Mat4::default().to_cols_array_2d(),
            view_position: [0.0; 4],
            num_lights: 0,
            padding: [0; 3],
        }
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MaterialInfo {
    pub albedo: [f32; 4],
    pub emission_factors: [f32; 3],
    padding1: f32,
    pub occlusion_factor: f32,
    padding2: [f32; 3],
    pub metal_roughness_factors: [f32; 2],
    padding3: [f32; 2],
}

impl Default for MaterialInfo {
    fn default() -> Self {
        Self {
            albedo: [1.0; 4],
            metal_roughness_factors: [0.5; 2],
            emission_factors: [0.0; 3],
            occlusion_factor: 1.0,
            padding1: 0.0,
            padding2: [0.0; 3],
            padding3: [0.0; 2],
        }
    }
}

#[repr(C)]
#[derive(Default, Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
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

#[repr(C)]
#[derive(Default, Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LightInfo {
    pub transform: [[f32; 4]; 4],
    pub color: [f32; 3],
    pub padding1: f32,
    pub intensity: f32,
    pub padding3: [f32; 3],
    pub range: f32,
    pub padding4: [f32; 3],
}

impl From<&PointLight> for LightInfo {
    fn from(light: &PointLight) -> Self {
        Self {
            transform: light.global_transform.to_cols_array_2d(),
            color: light.color.to_array(),
            intensity: light.intensity,
            range: light.range.unwrap_or(1.0),
            ..Default::default()
        }
    }
}
