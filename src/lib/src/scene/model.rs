use glam::Mat4;
use log::debug;
use rand::Rng;
use std::fmt::{Debug, Formatter};
use crate::Dirtyable;
use crate::scene::light::PointLight;
use crate::scene::mesh::Mesh;

pub struct Model {
    pub id: u32,
    pub meshes: Vec<Mesh>,
    pub children: Vec<Model>,
    pub name: Option<Box<str>>,
    pub local_transform: Mat4,
    pub light: Option<PointLight>,
}

impl Model {
    pub fn from(
        meshes: Vec<Mesh>,
        name: Option<Box<str>>,
        children: Vec<Model>,
        local_transform: Mat4,
        light: Option<PointLight>,
    ) -> Self {
        Self {
            id: rand::thread_rng().gen_range(0u32..1u32 << 31),
            meshes,
            name,
            children,
            local_transform,
            light,
        }
    }

    /**
    Call this after changing the local_transform of a model, it updates the computed global_transforms of all meshes.
    Sets dirty to true.
     */
    pub fn update_transforms(&mut self, parent: Mat4) {
        for mesh in self.meshes.as_mut_slice() {
            mesh.global_transform = parent * self.local_transform;
            mesh.normal_matrix = mesh.global_transform.inverse().transpose();
            mesh.set_dirty(true);
        }
        for child in self.children.as_mut_slice() {
            child.update_transforms(self.local_transform);
        }
        if let Some(ref mut light) = self.light {
            light.global_transform = parent * self.local_transform;
            light.set_dirty(true);
        }
    }
}
pub trait DeepIter<T> {
    fn iter_deep(&self) -> Box<dyn Iterator<Item = &T> + '_>;
}
impl DeepIter<Model> for Vec<Model> {
    fn iter_deep(&self) -> Box<(dyn Iterator<Item = &Model> + '_)> {
        Box::new(
            self.iter()
                .chain(self.iter().flat_map(|model| model.children.iter_deep())),
        )
    }
}

impl Debug for Model {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{{MODEL: Name: {:?}, # of meshes: {}, local transform: {}, children: [{}]}}",
            self.name,
            self.meshes.len(),
            self.local_transform,
            self.children
                .iter()
                .map(|c| format!("\n - {:?}", c))
                .collect::<String>(),
        )
    }
}
