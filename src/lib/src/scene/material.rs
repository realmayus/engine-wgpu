use crate::managers::{TexId, TextureManager};
use crate::texture::{Texture, TextureKind};
use glam::{Vec2, Vec3, Vec4};
use std::fmt::{Debug, Formatter};
use wgpu::{BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindingResource, Device};

pub struct PbrMaterial {
    pub dirty: bool,
    pub shader_id: u32,
    pub name: Option<Box<str>>,
    pub albedo_texture: Option<TexId>,
    pub albedo: Vec4,
    // this scales the RGBA components of the base_texture if defined; otherwise defines the color
    pub metallic_roughness_texture: Option<TexId>,
    pub metallic_roughness_factors: Vec2,
    // this scales the metallic & roughness components of the metallic_roughness_texture if defined; otherwise defines the reflection characteristics
    pub normal_texture: Option<TexId>,
    pub occlusion_texture: Option<TexId>,
    pub occlusion_factor: f32,
    pub emissive_texture: Option<TexId>,
    pub emissive_factors: Vec3,
    pub texture_bind_group: Option<wgpu::BindGroup>,
}

impl PbrMaterial {
    pub fn from_default(base_texture: Option<TexId>) -> Self {
        Self {
            dirty: true,
            shader_id: 0,
            name: Some(Box::from("Default material")),
            albedo_texture: base_texture,
            albedo: Vec4::from((1.0, 0.957, 0.859, 1.0)),
            metallic_roughness_texture: None,
            metallic_roughness_factors: Vec2::from((0.5, 0.5)),
            normal_texture: None,
            occlusion_texture: None,
            occlusion_factor: 1.0,
            emissive_texture: None,
            emissive_factors: Vec3::from((0.0, 0.0, 0.0)),
            texture_bind_group: None,
        }
    }

    pub fn create_texture_bind_group(&mut self, device: &Device, layout: &BindGroupLayout, tex_mgr: &TextureManager) {
        let mut entries = vec![];
        for Texture { view, sampler, .. } in [
            tex_mgr.unwrap_default(&self.albedo_texture, TextureKind::Albedo),
            tex_mgr.unwrap_default(&self.normal_texture, TextureKind::Normal),
            tex_mgr.unwrap_default(&self.metallic_roughness_texture, TextureKind::MetalRoughness),
            tex_mgr.unwrap_default(&self.occlusion_texture, TextureKind::Occlusion),
            tex_mgr.unwrap_default(&self.emissive_texture, TextureKind::Emission),
        ] {
            entries.push(BindGroupEntry {
                binding: entries.len() as u32,
                resource: BindingResource::TextureView(view),
            });
            entries.push(BindGroupEntry {
                binding: entries.len() as u32,
                resource: BindingResource::Sampler(sampler),
            });
        }
        self.texture_bind_group = Some(device.create_bind_group(&BindGroupDescriptor {
            label: Some("PBR Texture Bundle Bind Group"),
            layout,
            entries: &entries,
        }));
    }

    pub fn dirty(&self) -> bool {
        self.dirty
    }
}

impl Debug for PbrMaterial {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        // -1 means no texture, -2 means there is a texture but its ID is None fsr...
        write!(
            f,
            "{{MATERIAL: Name: {:?}, albedo: {:?}, metallic_roughness_factors: {:?}, occlusion_factor: {}, emissive_factors: {:?}, albedo_texture: {:?}, metallic_roughness_texture: {:?}, normal_texture: {:?}, occlusion_texture: {:?}, emissive_texture: {:?}}}",
            self.name,
            self.albedo,
            self.metallic_roughness_factors,
            self.occlusion_factor,
            self.emissive_factors,
            self.albedo_texture.is_some(),
            self.metallic_roughness_texture.is_some(),
            self.normal_texture.is_some(),
            self.occlusion_texture.is_some(),
            self.emissive_texture.is_some(),  // Todo debug print actual texture IDs
        )
    }
}
