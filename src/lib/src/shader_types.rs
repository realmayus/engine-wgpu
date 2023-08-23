use crate::scene::{Material, PointLight};
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
    /// index of the albedo texture, panics if None
    pub albedo_texture: u32,
    /// scales albedo texture if defined, otherwise defines color
    pub albedo: [f32; 4],
    /// index of the metal_roughness texture
    pub metal_roughness_texture: u32,
    /// scales metal_roughness texture if defined, otherwise defines reflection properties
    pub metal_roughness_factors: [f32; 2],
    /// index of the normal texture
    pub normal_texture: u32,
    /// index of the occlusion texture
    pub occlusion_texture: u32,
    /// scales occlusion texture if defined, otherwise defines constant occlusion value
    pub occlusion_factor: f32,
    /// index of the emission texture
    pub emission_texture: u32,
    /// scales emission texture if defined, otherwise defines the emission color
    pub emission_factors: [f32; 3],
}
impl MaterialInfo {
    pub fn from_material(material: &Material) -> Self {
        Self {
            albedo_texture: material.albedo_texture.as_ref().unwrap().id,
            albedo: material.albedo.to_array(),
            metal_roughness_texture: material
                .metallic_roughness_texture
                .as_ref()
                .map(|t| t.id)
                .unwrap_or(0),
            metal_roughness_factors: material.metallic_roughness_factors.to_array(),
            normal_texture: material.normal_texture.as_ref().map(|t| t.id).unwrap_or(0),
            occlusion_texture: material
                .occlusion_texture
                .as_ref()
                .map(|t| t.id)
                .unwrap_or(0),
            occlusion_factor: material.occlusion_factor,
            emission_texture: material
                .emissive_texture
                .as_ref()
                .map(|t| t.id)
                .unwrap_or(0),
            emission_factors: material.emissive_factors.to_array(),
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
pub struct LightInfo {
    transform: [[f32; 4]; 4],
    color: [f32; 3],
    light: u32,
    intensity: f32,
    range: f32,
}
impl LightInfo {
    pub fn from_light(light: &PointLight) -> Self {
        Self {
            transform: light.global_transform.to_cols_array_2d(),
            color: {
                let color = light.color.to_array();
                [color[0], color[1], color[2]]
            },
            light: light.index as u32,
            intensity: light.intensity,
            range: light.range.unwrap_or(1.),
        }
    }
}
