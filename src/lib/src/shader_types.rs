use crate::scene::PointLight;
use crate::scene::Material;
use glam::Mat4;
use vulkano::buffer::BufferContents;
use vulkano::pipeline::graphics::vertex_input::Vertex;

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
pub struct MyTangent {
    #[format(R32G32B32A32_SFLOAT)]
    tangent: [f32; 4],
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

#[derive(BufferContents, Debug)]
#[repr(C)]
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
pub struct LineInfo {
    pub model_transform: [[f32; 4]; 4],
    pub color: [f32; 4],
}

#[derive(BufferContents, Debug, Default)]
#[repr(C)]
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
