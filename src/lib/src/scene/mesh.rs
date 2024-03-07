use crate::managers::MatId;
use crate::scene::VertexInputs;
use crate::Dirtyable;
use glam::{Mat4, Vec2, Vec3, Vec4};
use rand::Rng;
use std::fmt::{Debug, Formatter};
use wgpu::Device;

pub struct Mesh {
    dirty: bool,
    pub id: u32,
    // for key purposes in GUIs and stuff
    pub vertices: Vec<Vec3>,
    pub indices: Vec<u32>,
    pub normals: Vec<Vec3>,
    pub tangents: Vec<Vec4>,
    pub material: MatId,
    pub uvs: Vec<Vec2>,
    pub global_transform: Mat4,
    // computed as product of the parent models' local transforms
    pub normal_matrix: Mat4,
    // computed as inverse transpose of the global transform
    pub vertex_inputs: Option<VertexInputs>,
}

impl Mesh {
    pub fn from(
        vertices: Vec<Vec3>,
        indices: Vec<u32>,
        normals: Vec<Vec3>,
        tangents: Vec<Vec4>,
        material: MatId,
        uvs: Vec<Vec2>,
        global_transform: Mat4,
        device: &Device,
    ) -> Self {
        let id = rand::thread_rng().gen_range(0u32..1u32 << 31);
        let vertex_inputs =
            VertexInputs::from_mesh(id, &vertices, &normals, &tangents, &uvs, &indices, device);

        Self {
            id,
            dirty: true,
            vertices,
            indices,
            normals,
            tangents,
            material,
            uvs,
            global_transform,
            normal_matrix: global_transform.inverse().transpose(),
            vertex_inputs: Some(vertex_inputs),
        }
    }

    pub fn clone(&self, device: &Device) -> Self {
        let vertices = self.vertices.clone();
        let indices = self.indices.clone();
        let normals = self.normals.clone();
        let tangents = self.tangents.clone();
        let uvs = self.uvs.clone();
        let id = rand::thread_rng().gen_range(0u32..1u32 << 31);
        let vertex_inputs =
            VertexInputs::from_mesh(id, &vertices, &normals, &tangents, &uvs, &indices, device);

        Self {
            id,
            dirty: true,
            vertices,
            indices,
            normals,
            tangents,
            uvs,
            material: self.material,
            global_transform: self.global_transform,
            normal_matrix: self.normal_matrix,
            vertex_inputs: Some(vertex_inputs),
        }
    }
}

impl Dirtyable for Mesh {
    fn dirty(&self) -> bool {
        self.dirty
    }

    fn set_dirty(&mut self, dirty: bool) {
        self.dirty = dirty
    }
}

impl Debug for Mesh {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{{MESH: # of vertices: {}, # of normals: {}, # of tangents: {}, # of indices: {}, material: {:?}, global transform: {}}}",
            self.vertices.len(),
            self.normals.len(),
            self.tangents.len(),
            self.indices.len(),
            self.material,  // todo proper debugging of material (only printing MatId right now)
            self.global_transform,
        )
    }
}
