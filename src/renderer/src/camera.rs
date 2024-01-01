use glam::{Mat4, Vec2, Vec3, Vec4, Vec4Swizzles};
use log::debug;
use wgpu::{Buffer, Device, Queue};
use wgpu::util::{BufferInitDescriptor, DeviceExt};

use lib::shader_types::CameraUniform;

const GLOBAL_X: [f32; 4] = [1.0, 0.0, 0.0, 1.0];
const GLOBAL_Y: [f32; 4] = [0.0, -1.0, 0.0, 1.0];
const GLOBAL_Z: [f32; 4] = [0.0, 0.0, 1.0, 1.0];
const EPS: f32 = 0.01;

#[derive(Default)]
pub struct KeyState {
    pub up_pressed: bool,
    pub down_pressed: bool,
    pub left_pressed: bool,
    pub right_pressed: bool,
    pub middle_pressed: bool,
    pub shift_pressed: bool,
}

pub struct Camera {
    /// camera position
    pub eye: Vec3,
    /// target position, used by arcball camera
    pub target: Vec3,
    /// direction vector, used by fps cam
    pub direction: Vec3,
    pub up: Vec3,
    aspect: f32,
    pub fovy: f32,
    pub znear: f32,
    pub zfar: f32,
    pub buffer: Buffer,
    pub speed: f32,
    pub fps: bool,
    /// the camera's transform matrix / world to view matrix
    pub view: Mat4,
}

impl Camera {
    pub fn new_default(
        width: f32,
        height: f32,
        device: &Device,
    ) -> Self {
        let eye: Vec3 = (0.3, 0.3, 1.0).into();
        let target: Vec3 = (0.0, 0.0, 0.0).into();
        let up = Vec4::from(GLOBAL_Y).xyz();
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

        let camera_buffer = device.create_buffer_init(
            &BufferInitDescriptor {
                label: Some("Camera Buffer"),
                contents: bytemuck::cast_slice(&[data]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            }
        );

        Camera {
            eye,
            target,
            direction: target - eye,
            up,
            aspect,
            fovy,
            znear,
            zfar,
            buffer: camera_buffer,
            speed: 0.5,
            fps: false,
            view,
        }
    }

    pub fn reset(&mut self) {
        self.eye = (0.3, 0.3, 1.0).into();
        self.target = (0.0, 0.0, 0.0).into();
        self.direction = (self.target - self.eye).normalize();
        self.up = Vec4::from(GLOBAL_Y).xyz();
        self.fovy = std::f32::consts::FRAC_PI_2;
        self.znear = 0.1;
        self.zfar = 100.0;
        self.speed = 0.5;
        self.fps = !self.fps;
        self.view = Mat4::look_at_rh(self.eye, self.target, self.up);
    }

    pub(crate) fn build_projection(&self) -> Mat4 {
        let view = self.view;
        let proj =
            Mat4::perspective_rh_gl(self.fovy.to_radians(), self.aspect, self.znear, self.zfar);
        let scale = Mat4::from_scale((0.01, 0.01, 0.01).into());
        proj * view * scale
    }

    pub fn update_aspect(&mut self, width: f32, height: f32) {
        self.aspect = width / height;
    }

    pub fn update_view(&self, queue: &Queue) {
        let new_proj = self.build_projection();
        let mut uniform = CameraUniform::new();
        uniform.proj_view = new_proj.to_cols_array_2d();
        uniform.view_position = Vec4::from((self.eye, 1.0)).into();
        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&[uniform]))
    }

    pub fn recv_input(&mut self, keys: &KeyState, change: Vec2, delta_time: f32) {
        // TODO clamp right rotation between 90° and -90° to avoid the jittering at the top
        if self.fps {
            self.update_fps(keys, change, delta_time)
        } else {
            self.update_arcball(keys, change, delta_time)
        }
    }

    /// FPS cam
    fn update_fps(&mut self, keys: &KeyState, cursor_delta: Vec2, delta_time: f32) {
        let global_up = Vec4::from(GLOBAL_Y);
        let direction = self.direction;
        // TODO can't compute right vector if direction.abs() == global_up.abs()
        let right = direction.cross(global_up.xyz()).normalize();

        if keys.up_pressed {
            let translation = self.direction.normalize() * self.speed * delta_time * 10.;
            debug!("{translation}");
            self.eye += translation;
        }
        if keys.down_pressed {
            let translation = self.direction.normalize() * self.speed * delta_time * 10.;
            debug!("{translation}");
            self.eye -= translation;
        }
        if keys.left_pressed {
            let translation = right * self.speed * delta_time * 0.5;
            debug!("{translation}");
            self.eye -= translation;
        }
        if keys.right_pressed {
            let translation = right * self.speed * delta_time * 0.5;
            debug!("{translation}");
            self.eye += translation;
        }
        if cursor_delta.length() != 0.0 {
            let rotation_up =
                Mat4::from_axis_angle(global_up.xyz(), cursor_delta.x.to_degrees() * delta_time);
            let rotation_right =
                Mat4::from_axis_angle(right, -cursor_delta.y.to_degrees() * delta_time);

            self.direction = (rotation_right * rotation_up * as_4(self.direction)).xyz();
        }
        self.view = Mat4::look_at_rh(
            self.eye,
            self.eye + self.direction.normalize(),
            global_up.xyz(),
        );
    }

    /// __Moves the camera using arcball rotation and panning__.
    ///
    /// Middle mouse button: Arcball rotation around target point.
    ///
    /// Shift + Middle mouse button: Translate target and eye on the view plane.
    fn update_arcball(&mut self, keys: &KeyState, change: Vec2, delta_time: f32) {
        let global_up = Vec4::from(GLOBAL_Y);
        let direction = self.target - self.eye;
        let forward_norm = direction.normalize();
        let distance = direction.length();

        if keys.up_pressed && distance > self.speed {
            self.eye += forward_norm * self.speed * delta_time * 10.;
        }
        if keys.down_pressed {
            self.eye -= forward_norm * self.speed * delta_time * 10.;
        }

        let translation = Mat4::from_translation(
            (self.view * Vec4::from((change * delta_time * 20., 0.0, 0.0))).xyz(),
        );

        if keys.middle_pressed && change.length() != 0.0 {
            if keys.shift_pressed {
                self.target = transform(translation, self.target);
                self.eye = transform(translation, self.eye);
            } else {
                let target_to_cam = self.eye - self.target;
                let right = target_to_cam.cross(global_up.xyz()).normalize();

                let rotation_up = Mat4::from_axis_angle(
                    global_up.xyz(),
                    change.x.to_degrees() * delta_time * 20.,
                );
                let rotation_right =
                    Mat4::from_axis_angle(right, change.y.to_degrees() * delta_time * 20.);
                let new_focus_to_cam = rotation_up * rotation_right * as_4(target_to_cam);

                self.eye = new_focus_to_cam.xyz() + self.target;
                self.direction = self.target - self.eye;
                let x_axis = new_focus_to_cam.xyz().cross(global_up.xyz()).normalize();
                self.up = new_focus_to_cam.xyz().cross(x_axis).normalize();
            }
        }
        self.view = Mat4::look_at_rh(self.eye, self.target, global_up.xyz());
    }
}

fn as_4(vec: Vec3) -> Vec4 {
    Vec4::from((vec, 1.0))
}

fn transform(mat: Mat4, vec: Vec3) -> Vec3 {
    (mat * as_4(vec)).xyz()
}
