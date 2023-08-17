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
    pub target: Vec3,
    pub up: Vec3,
    aspect: f32,
    pub fovy: f32,
    pub znear: f32,
    pub zfar: f32,
    pub buffer: Subbuffer<CameraUniform>,
    pub speed: f32,
    pub(crate) buffer_data: CameraUniform,
}

impl Camera {
    pub fn new_default(
        width: f32,
        height: f32,
        memory_allocator: &StandardMemoryAllocator,
    ) -> Self {
        let eye = (0.3, 0.3, 1.0).into();
        let target = (0.0, 0.0, 0.0).into();
        let up = (0.0, -1.0, 0.0).into();
        let aspect = width / height;
        let fovy = std::f32::consts::FRAC_PI_2;
        let znear = 0.1;
        let zfar = 100.0;

        let mut data = CameraUniform::new();
        let proj = Mat4::perspective_rh_gl(fovy, aspect, znear, zfar);
        let view = Mat4::look_at_rh(eye, target, up);
        println!("View proj: {:?}", proj * view);
        data.view_proj = (proj * view).to_cols_array_2d();
        // data.view_proj[1][1] *= -1.0;
        data.view_position = (Vec4::from((eye, 1.0))).into();

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

        Camera {
            eye,
            target,
            up,
            aspect,
            fovy,
            znear,
            zfar,
            buffer: camera_buffer,
            buffer_data: data,
            speed: 0.5,
        }
    }

    pub fn reset(&mut self) {
        self.eye = (0.3, 0.3, 1.0).into();
        self.target = (0.0, 0.0, 0.0).into();
        self.up = (0.0, -1.0, 0.0).into();
        self.fovy = std::f32::consts::FRAC_PI_2.to_radians();
        self.znear = 0.1;
        self.zfar = 100.0;
        self.speed = 0.5;
    }

    pub(crate) fn build_projection(&self) -> Mat4 {
        let view = Mat4::look_at_rh(self.eye, self.target, self.up);
        let proj =
            Mat4::perspective_rh_gl(self.fovy.to_radians(), self.aspect, self.znear, self.zfar);
        let scale = Mat4::from_scale((0.01, 0.01, 0.01).into());
        return proj * view * scale;
    }
    pub(crate) fn update_aspect(&mut self, width: f32, height: f32) {
        self.aspect = width / height;
    }
    pub fn update_view(&self) {
        let new_proj = self.build_projection();
        let mut mapping = self.buffer.write().unwrap();
        mapping.view_proj = (new_proj).to_cols_array_2d();
        // mapping.view_proj[1][1] *= -1.0;
        mapping.view_position = (Vec4::from((self.eye, 1.0))).into();
    }

    pub fn recv_input(
        &mut self,
        is_up_pressed: bool,
        is_down_pressed: bool,
        is_right_pressed: bool,
        is_left_pressed: bool,
    ) {
        let forward = self.target - self.eye;
        let forward_norm = forward.normalize();
        let forward_mag = forward.length();

        if is_up_pressed && forward_mag > self.speed {
            self.eye += forward_norm * self.speed;
        }
        if is_down_pressed {
            self.eye -= forward_norm * self.speed;
        }

        let right = forward_norm.cross(self.up);

        let forward = self.target - self.eye;
        let forward_mag = forward.length();

        if is_right_pressed {
            self.eye = self.target - (forward - right * self.speed).normalize() * forward_mag;
        }
        if is_left_pressed {
            self.eye = self.target - (forward + right * self.speed).normalize() * forward_mag;
        }
    }
}
