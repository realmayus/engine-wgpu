use crate::scene::{Mesh, PbrMaterial};
use crate::scene::PointLight;
use glam::Mat4;
use crate::managers::MaterialManager;

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
    pub proj_view: [[f32; 4]; 4], // s64 o0
    pub view_position: [f32; 4], // s16 o64
    pub num_lights: u32,  // s4 o80
    pub padding: [u32; 3],  // total size: 96
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
    pub albedo: [f32; 4],  // s16 o0
    pub emission_factors: [f32; 3], // s12 o16
    pub occlusion_factor: f32,  // s4 o28
    pub metal_roughness_factors: [f32; 2],  // s8 o32
    padding3: [f32; 2],  // total size: 48
}

impl From<&PbrMaterial> for MaterialInfo {
    fn from(material: &PbrMaterial) -> Self {
        Self {
            albedo: material.albedo.to_array(),
            emission_factors: material.emissive_factors.into(),
            occlusion_factor: material.occlusion_factor,
            metal_roughness_factors: material.metallic_roughness_factors.into(),
            ..Default::default()
        }
    }
}

impl From<&mut PbrMaterial> for MaterialInfo {
    fn from(material: &mut PbrMaterial) -> Self {
        Self {
            albedo: material.albedo.to_array(),
            emission_factors: material.emissive_factors.into(),
            occlusion_factor: material.occlusion_factor,
            metal_roughness_factors: material.metallic_roughness_factors.into(),
            ..Default::default()
        }
    }
}

impl Default for MaterialInfo {
    fn default() -> Self {
        Self {
            albedo: [1.0; 4],
            metal_roughness_factors: [0.5; 2],
            emission_factors: [0.0; 3],
            occlusion_factor: 1.0,
            padding3: [0.0; 2],
        }
    }
}

#[repr(C)]
#[derive(Default, Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MeshInfo {
    pub material: u32,  // s4 o0
    _align: [u32; 3],
    pub model_transform: [[f32; 4]; 4],  // s64 o16
    pub normal_matrix: [[f32; 4]; 4],  // s36 o80
    // _align2: [u32; 3],  // total size: 128
}
impl MeshInfo {
    pub fn from_mesh(mesh: &Mesh, material_manager: &MaterialManager) -> Self {
        Self {
            material: material_manager.get_material(mesh.material).shader_id(),
            _align: [0; 3],
            model_transform: mesh.global_transform.to_cols_array_2d(),
            normal_matrix: mesh.normal_matrix.to_cols_array_2d(),
            // _align2: [0; 3],
        }
    }
}


#[repr(C)]
#[derive(Default, Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LightInfo {
    pub transform: [[f32; 4]; 4],  // s64 o0
    pub color: [f32; 3],  // s12 o64
    pub intensity: f32,  // s4 o76
    pub range: f32,  // s4 o80
    pub padding4: [f32; 3],  // total size: 96
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
