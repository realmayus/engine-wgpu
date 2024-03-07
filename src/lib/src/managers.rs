use log::{debug, info, warn};
use slotmap::basic::SlotMap;
use slotmap::new_key_type;
use wgpu::{BindGroupLayout, BufferUsages, Device, Queue};

use crate::buffer_array::DynamicBufferArray;
use crate::shader_types::MaterialInfo;
use crate::texture::{Texture, TextureKind};
use crate::Material;
use crate::scene::material::PbrMaterial;

new_key_type! { pub struct TexId; }

new_key_type! { pub struct MatId; }

#[derive(Default)]
pub struct TextureManager {
    textures: SlotMap<TexId, Texture>,
    default_albedo: TexId,
    default_normal: TexId,
}

impl TextureManager {
    pub fn new(device: &Device, queue: &Queue, x: &BindGroupLayout) -> Self {
        let mut textures = SlotMap::with_key();
        let default_albedo = Texture::from_image(
            device,
            queue,
            &image::load_from_memory(include_bytes!("../../../assets/textures/default.png"))
                .unwrap(),
            Some("Default Albedo Texture"),
            TextureKind::Albedo,
        )
        .expect("Couldn't load default texture");

        let default_normal = Texture::from_image(
            device,
            queue,
            &image::load_from_memory(include_bytes!(
                "../../../assets/textures/default_normal.png"
            ))
            .unwrap(),
            Some("Default Normal Texture"),
            TextureKind::Normal,
        )
        .expect("Couldn't load default normal texture");

        Self {
            default_albedo: textures.insert(default_albedo),
            default_normal: textures.insert(default_normal),
            textures,
        }
    }
    pub fn add_texture(&mut self, mut texture: Texture) -> TexId {
        let id = self.textures.len();
        texture.id = Some(id as u32);
        let id = self.textures.insert(texture);
        id
    }

    pub fn get_texture(&self, id: &TexId) -> &Texture {
        &self.textures[*id]
    }

    pub fn iter(&self) -> impl Iterator<Item = &Texture> {
        self.textures.values()
    }

    pub fn iter_with_ids(&self) -> impl Iterator<Item = (TexId, &Texture)> {
        self.textures.iter()
    }

    pub fn default_tex(&self, texture_kind: TextureKind) -> &Texture {
        match texture_kind {
            TextureKind::Albedo => &self.textures[self.default_albedo],
            TextureKind::Normal => &self.textures[self.default_normal], // TODO support other default texture kinds
            TextureKind::MetalRoughness => &self.textures[self.default_albedo],
            TextureKind::Occlusion => &self.textures[self.default_albedo],
            TextureKind::Emission => &self.textures[self.default_albedo],
            TextureKind::Depth => &self.textures[self.default_albedo],
            TextureKind::Other => {
                warn!("No default texture for texture kind {:?}", texture_kind);
                &self.textures[self.default_albedo]
            }
        }
    }

    pub fn unwrap_default(&self, tex_id: &Option<TexId>, texture_kind: TextureKind) -> &Texture {
        let texture = tex_id.map(|t_id| &self.textures[t_id]);
        texture.unwrap_or_else(|| self.default_tex(texture_kind))
    }
}

pub struct MaterialManager {
    materials: SlotMap<MatId, Material>,
    pub default_material: MatId,
    pub buffer: DynamicBufferArray<MaterialInfo>,
}

impl MaterialManager {
    pub fn new(
        device: &Device,
        queue: &Queue,
        mat_bind_group_layout: &BindGroupLayout,
        tex_bind_group_layout: &BindGroupLayout,
        texture_manager: &TextureManager,
    ) -> Self {
        let mut materials = SlotMap::with_key();
        let mut pbr_mat = PbrMaterial::from_default(None);
        pbr_mat.create_texture_bind_group(device, tex_bind_group_layout, texture_manager);
        let mut buffer = DynamicBufferArray::new(
            device,
            Some("Material Buffer".to_string()),
            BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mat_bind_group_layout,
        );
        buffer.push(
            device,
            queue,
            &[MaterialInfo::from(&pbr_mat)],
            mat_bind_group_layout,
        );
        let default_material = materials.insert(Material::Pbr(pbr_mat));

        Self {
            materials,
            default_material,
            buffer,
        }
    }
    pub fn add_material(
        &mut self,
        mut material: Material,
        device: &Device,
        queue: &Queue,
        bind_group_layout: &BindGroupLayout,
    ) -> MatId {
        debug!("Adding material: {:?}", material.name());
        let shader_id = self.materials.len();
        material.set_shader_id(shader_id as u32);
        match &material {
            Material::Pbr(pbr) => {
                self.buffer
                    .push(device, queue, &[MaterialInfo::from(pbr)], bind_group_layout);
            }
        }
        self.materials.insert(material)
    }

    pub fn get_material(&self, id: MatId) -> &Material {
        &self.materials[id]
    }

    pub fn get_default_material(&self) -> &Material {
        &self.materials[self.default_material]
    }

    pub fn iter(&self) -> impl Iterator<Item = &Material> {
        self.materials.values()
    }

    pub fn iter_with_ids(&self) -> impl Iterator<Item = (MatId, &Material)> {
        self.materials.iter()
    }

    pub fn update_dirty(&mut self, queue: &Queue) {
        for (_, mat) in self.materials.iter_mut().filter(|(_, m)| m.dirty()) {
            debug!("Updating material {:?}...", mat.name());
            let Material::Pbr(mat) = mat;
            mat.dirty = false;
            let mat_id = mat.shader_id;
            let uniform = MaterialInfo::from(mat);
            self.buffer.update(queue, mat_id as u64, uniform);
            info!("Updated material #{}", mat_id);
        }
    }
}
