use glam::{Mat4, Vec3, Vec4};
use vulkano::buffer::{Buffer, BufferContents, BufferCreateInfo, BufferUsage, Subbuffer};
use vulkano::command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer};
use vulkano::memory::allocator::{AllocationCreateInfo, MemoryUsage, StandardMemoryAllocator};


#[derive(BufferContents, Debug, Default, Copy, Clone)]
#[repr(C)]
pub struct CameraUniform {
    pub view_proj: [[f32; 4]; 4],
    pub view_position: [f32; 4],
}
impl CameraUniform {
    fn new() -> Self {
        Self {
            view_proj: Mat4::default().to_cols_array_2d(),
            view_position: [0.0; 4],
        }
    }
}

pub struct Camera {
    pub eye: Vec3,
    pub(crate) target: Vec3,
    pub(crate) up: Vec3,
    aspect: f32,
    fovy: f32,
    znear: f32,
    zfar: f32,
    pub buffer: Subbuffer<CameraUniform>,
    pub(crate) buffer_data: CameraUniform,
}

impl Camera {
    pub fn new_default(width: f32, height: f32, memory_allocator: &StandardMemoryAllocator) -> Self {
        let data = CameraUniform::new();
        let camera_buffer = Buffer::from_data(
            memory_allocator,
            BufferCreateInfo {
                usage: BufferUsage::STORAGE_BUFFER,
                ..Default::default()
            },
            AllocationCreateInfo {
                usage: MemoryUsage::Upload,
                ..Default::default()
            },
            data,
        )
            .expect("Couldn't create camera buffer");

        let mut camera = Camera {
            eye: (0.0, 1.0, 2.0).into(),
            target: (0.0, 0.0, 0.0).into(),
            up: (0.0, 1.0, 0.0).into(),
            aspect: width / height,
            fovy: std::f32::consts::PI/2f32,
            znear: 0.1,
            zfar: 200.0,
            buffer: camera_buffer,
            buffer_data: data,
        };
        camera.update_view();
        camera
    }

    pub(crate) fn build_projection(&self) -> Mat4 {
        let view = Mat4::look_at_rh(self.eye, self.target, self.up);
        let proj = Mat4::perspective_rh(self.fovy, self.aspect, self.znear, self.zfar);
        return proj * view;
    }
    fn update_aspect(&mut self, width: f32, height: f32) {
        self.aspect = width / height;
        self.update_view();
    }
    pub fn update_view(&self) {
        let new_proj = self.build_projection();
        let mut mapping = self.buffer.write().unwrap();
        mapping.view_proj = new_proj.to_cols_array_2d();
        mapping.view_position = (Vec4::from((self.eye, 1.0))).into();
    }
}