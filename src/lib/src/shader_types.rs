use crate::scene::Material;
use crate::scene::PointLight;
use glam::Mat4;

trait Vertex {
    const ATTRIBS: [wgpu::VertexAttribute; 4];
    fn desc<'a>() -> wgpu::VertexBufferLayout<'a>;
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
}
impl CameraUniform {
    pub fn new() -> Self {
        Self {
            proj_view: Mat4::default().to_cols_array_2d(),
            view_position: [0.0; 4],
        }
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MaterialInfo {
    pub albedo: [f32; 4],
    pub albedo_texture: u32, // index of texture
    pub metal_roughness_factors: [f32; 2],
    pub metal_roughness_texture: u32,
    pub emission_factors: [f32; 3],
    pub emission_texture: u32,
    pub normal_texture: u32,
    pub occlusion_factor: f32,
    pub occlusion_texture: u32,
}

impl From<&Material> for MaterialInfo {
    fn from(material: &Material) -> Self {
        Self {
            albedo: material.albedo.to_array(),
            albedo_texture: material.albedo_texture.as_ref().unwrap().id,
            metal_roughness_factors: material.metallic_roughness_factors.to_array(),
            metal_roughness_texture: material
                .metallic_roughness_texture
                .as_ref()
                .map(|t| t.id)
                .unwrap_or(1),
            emission_factors: material.emissive_factors.to_array(),
            emission_texture: material
                .emissive_texture
                .as_ref()
                .map(|t| t.id)
                .unwrap_or(1),
            normal_texture: material.normal_texture.as_ref().map(|t| t.id).unwrap_or(0),
            occlusion_factor: 1.0,
            occlusion_texture: material
                .occlusion_texture
                .as_ref()
                .map(|t| t.id)
                .unwrap_or(0),
        }
    }
}
impl Default for MaterialInfo {
    fn default() -> Self {
        Self {
            albedo: [1.0; 4],
            albedo_texture: 0,
            metal_roughness_factors: [0.5; 2],
            metal_roughness_texture: 0,
            emission_factors: [0.0; 3],
            emission_texture: 0,
            normal_texture: 1,
            occlusion_factor: 0.0,
            occlusion_texture: 0,
        }
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
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
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LightInfo {
    pub transform: [[f32; 4]; 4],
    pub color: [f32; 3],
    pub light: u32,
    pub intensity: f32,
    pub range: f32,
    pub amount: u32,
}

impl From<&PointLight> for LightInfo {
    fn from(light: &PointLight) -> Self {
        Self {
            transform: light.global_transform.to_cols_array_2d(),
            color: light.color.to_array(),
            light: light.index as u32,
            intensity: light.intensity,
            range: light.range.unwrap_or(1.0),
            amount: light.amount,
        }
    }
}
