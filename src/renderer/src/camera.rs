use glam::{Mat4, Vec2, Vec3, Vec4, Vec4Swizzles};
use lib::shader_types::CameraUniform;
use log::debug;
use vulkano::buffer::{Buffer, BufferCreateInfo, BufferUsage, Subbuffer};
use vulkano::memory::allocator::{AllocationCreateInfo, MemoryUsage, StandardMemoryAllocator};

const GLOBAL_X: [f32; 4] = [1.0, 0.0, 0.0, 1.0];
const GLOBAL_Y: [f32; 4] = [0.0, -1.0, 0.0, 1.0];
const GLOBAL_Z: [f32; 4] = [0.0, 0.0, 1.0, 1.0];

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
}

impl Camera {
    pub fn new_default(
        width: f32,
        height: f32,
        memory_allocator: &StandardMemoryAllocator,
    ) -> Self {
        let eye: Vec3 = (0.3, 0.3, 1.0).into();
        let target: Vec3 = (0.0, 0.0, 0.0).into();
        let up = (eye - target)
            .cross(Vec4::from(GLOBAL_Y).xyz())
            .cross(eye - target)
            .normalize();
        let aspect = width / height;
        let fovy = std::f32::consts::FRAC_PI_2;
        let znear = 0.1;
        let zfar = 100.0;

        let mut data = CameraUniform::new();
        let proj = Mat4::perspective_rh_gl(fovy, aspect, znear, zfar);
        let view = Mat4::look_at_rh(eye, target, up);
        let scale = Mat4::from_scale((0.01, 0.01, 0.01).into());

        debug!("Creating view proj: {:?}", proj * view * scale);
        data.proj_view = (proj * view * scale).to_cols_array_2d();
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
            speed: 0.5,
        }
    }

    pub fn reset(&mut self) {
        self.eye = (0.3, 0.3, 1.0).into();
        self.target = (0.0, 0.0, 0.0).into();
        self.up = (self.eye - self.target)
            .cross(Vec4::from(GLOBAL_Y).xyz())
            .cross(self.eye - self.target)
            .normalize();
        self.fovy = std::f32::consts::FRAC_PI_2;
        self.znear = 0.1;
        self.zfar = 100.0;
        self.speed = 0.5;
    }

    pub(crate) fn build_projection(&self) -> Mat4 {
        let view = Mat4::look_at_rh(self.eye, self.target, self.up); //Vec4::from(GLOBAL_Y).xyz()
                                                                     // info!("{}", self.up);
        let proj =
            Mat4::perspective_rh_gl(self.fovy.to_radians(), self.aspect, self.znear, self.zfar);
        let scale = Mat4::from_scale((0.01, 0.01, 0.01).into());
        proj * view * scale
    }
    pub(crate) fn update_aspect(&mut self, width: f32, height: f32) {
        self.aspect = width / height;
    }
    pub fn update_view(&self) {
        let new_proj = self.build_projection();
        let mut mapping = self.buffer.write().unwrap();
        mapping.proj_view = (new_proj).to_cols_array_2d();
        mapping.view_position = (Vec4::from((self.eye, 1.0))).into();
    }

    /// __Moves the camera__.
    ///
    /// Middle mouse button: Arcball rotation around target point.
    ///
    /// Shift + Middle mouse button: Translate target and eye on the view plane.
    pub fn recv_input(
        &mut self,
        is_up_pressed: bool,
        is_down_pressed: bool,
        mouse_middle: bool,
        shift: bool,
        change: Vec2,
        delta_time: f32,
    ) {
        let global_up = Vec4::from(GLOBAL_Y);
        let global_z = Vec4::from(GLOBAL_Z);
        let global_x = Vec4::from(GLOBAL_X);
        let direction = self.target - self.eye;
        let forward_norm = direction.normalize();
        let forward_mag = direction.length();

        if is_up_pressed && forward_mag > self.speed {
            self.eye += forward_norm * self.speed;
        }
        if is_down_pressed {
            self.eye -= forward_norm * self.speed;
        }

        let translation = Vec4::from((change, 0.0, 0.0));
        let view = Mat4::look_at_rh(self.eye, self.target, self.up);
        let translation = view * translation;

        if mouse_middle && shift {
            self.target += translation.xyz() * delta_time * 20.;
            self.eye += translation.xyz() * delta_time * 20.;
        } else if mouse_middle && change.length() != 0.0 {
            let focus_to_cam = self.eye - self.target;
            let is_up = focus_to_cam.normalize().abs() == global_up.xyz().abs();
            let right = if !is_up {
                focus_to_cam.cross(global_up.xyz()).normalize()
            } else {
                // this case never happens for some reason
                // basically pray that self.up was set to something meaningful before
                focus_to_cam.cross(self.up).normalize()
            };

            // debug!("up: {}, right: {}, is up: {}", self.up, right, is_up);
            let rotation_up =
                Mat4::from_axis_angle(global_up.xyz(), change.x.to_degrees() * delta_time * 20.);
            let rotation_right =
                Mat4::from_axis_angle(right, change.y.to_degrees() * delta_time * 20.);

            let new_focus_to_cam = rotation_up * rotation_right * Vec4::from((focus_to_cam, 1.0));
            self.eye = new_focus_to_cam.xyz() + self.target;
            let right = new_focus_to_cam.xyz().cross(global_up.xyz());
            self.up = -new_focus_to_cam.xyz().cross(right).normalize();
        }
    }
}
