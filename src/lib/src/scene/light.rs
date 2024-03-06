use crate::shader_types::LightInfo;
use crate::Dirtyable;
use glam::{Mat4, Vec3};
use wgpu::util::BufferInitDescriptor;
use wgpu::{Buffer, BufferUsages, Device};

#[derive(Debug)]
pub struct PointLight {
    pub dirty: bool,
    pub global_transform: Mat4,
    pub index: usize,
    pub color: Vec3,
    pub intensity: f32,
    pub range: Option<f32>,
    pub buffer: Buffer,
    // pub shadow_view: Option<Texture>,
}

impl PointLight {
    pub fn new(
        global_transform: Mat4,
        index: usize,
        color: Vec3,
        intensity: f32,
        range: Option<f32>,
        device: &Device,
    ) -> Self {
        let buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("Point Light Buffer"),
            contents: bytemuck::cast_slice(&[LightInfo {
                transform: global_transform.to_cols_array_2d(),
                color: color.to_array(),
                intensity,
                range: range.unwrap_or(10.0),
                ..Default::default()
            }]),
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
        });
        Self {
            dirty: true,
            global_transform,
            index,
            color,
            intensity,
            range,
            buffer,
        }
    }
}

impl Dirtyable for PointLight {
    fn dirty(&self) -> bool {
        self.dirty
    }

    fn set_dirty(&mut self, dirty: bool) {
        self.dirty = dirty;
    }
}
