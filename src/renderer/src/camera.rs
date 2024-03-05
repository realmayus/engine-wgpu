use glam::{Mat4, Vec2, Vec3, Vec4, Vec4Swizzles};
use log::debug;
use wgpu::{Buffer, Device, Queue};
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use winit::event::{ElementState, ModifiersState, MouseButton, VirtualKeyCode};

use lib::shader_types::CameraUniform;

const GLOBAL_X: [f32; 4] = [1.0, 0.0, 0.0, 1.0];
const GLOBAL_Y: [f32; 4] = [0.0, -1.0, 0.0, 1.0];
const GLOBAL_Z: [f32; 4] = [0.0, 0.0, 1.0, 1.0];
const EPS: f32 = 0.01;

#[derive(Debug)]
enum InputDevice {
    Mouse {
        middle_pressed: bool,
    },
}

impl InputDevice {
    fn pan(&self) -> bool {
        match self {
            InputDevice::Mouse { middle_pressed } => *middle_pressed,
        }
    }
}

impl Default for InputDevice {
    fn default() -> Self {
        InputDevice::Mouse {
            middle_pressed: false,
        }
    }
}

#[derive(Default, Debug)]
pub struct KeyState {
    pub up_pressed: bool,
    pub down_pressed: bool,
    pub left_pressed: bool,
    pub right_pressed: bool,
    pub shift_pressed: bool,
    pub input_device: InputDevice,
    pub cmd_pressed: bool,
}

impl KeyState {
    pub(crate) fn update_keys(&mut self, keycode: VirtualKeyCode, state: ElementState) {
        let pressed = state == ElementState::Pressed;
        match keycode {
            VirtualKeyCode::W => self.up_pressed = pressed,
            VirtualKeyCode::S => self.down_pressed = pressed,
            VirtualKeyCode::A => self.left_pressed = pressed,
            VirtualKeyCode::D => self.right_pressed = pressed,
            // VirtualKeyCode::Space => self.middle_pressed = pressed,
            _ => (),
        }
    }

    pub(crate) fn update_mouse(&mut self, state: &ElementState, button: &MouseButton) {
        let pressed = state == &ElementState::Pressed;
        match button {
            MouseButton::Middle => self.input_device = InputDevice::Mouse { middle_pressed: pressed },
            MouseButton::Left if self.cmd_pressed || self.shift_pressed => self.input_device = InputDevice::Mouse { middle_pressed: pressed },
            _ => self.input_device = InputDevice::Mouse { middle_pressed: false },
        }
    }

    pub(crate) fn set_modifiers(&mut self, state: &ModifiersState) {
        self.shift_pressed = state.shift();
        self.cmd_pressed = state.logo();
    }
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
    dirty: bool,
    light_count: u32,
}

impl Camera {
    pub fn new_default(width: f32, height: f32, device: &Device) -> Self {
        let eye: Vec3 = (0.3, 0.3, 1.0).into();
        let target: Vec3 = (0.0, 0.0, 0.0).into();
        let up = Vec4::from(GLOBAL_Y).xyz();
        let aspect = width / height;
        let fovy = std::f32::consts::FRAC_PI_2;
        let znear = 0.1;
        let zfar = 100.0;

        let mut data = CameraUniform::new();
        let proj = Mat4::perspective_lh(fovy, aspect, znear, zfar);
        let view = Mat4::look_at_lh(eye, target, up);
        let scale = Mat4::from_scale((0.01, 0.01, 0.01).into());

        debug!("Creating view proj: {:?}", proj * view * scale);
        data.proj_view = (proj * view * scale).to_cols_array_2d();
        data.view_position = (Vec4::from((eye, 1.0))).into();

        let camera_buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("Camera Buffer"),
            contents: bytemuck::cast_slice(&[data]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

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
            dirty: false,
            light_count: 0,
        }
    }

    /**
    Call this whenever the number of lights in the scene changes. This value gets passed to the fragment shader.
     */
    pub fn update_light_count(&mut self, num_lights: u32) {
        if self.light_count == num_lights {
            return;
        }
        debug!("Light count updated to {}", num_lights);
        self.light_count = num_lights;
        self.dirty = true;
    }

    pub fn light_count(&self) -> u32 {
        self.light_count
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
        self.fps = false;
        self.view = Mat4::look_at_lh(self.eye, self.target, self.up);
        self.dirty = true;
    }

    pub(crate) fn build_projection(&self) -> Mat4 {
        let view = self.view;
        let proj =
            Mat4::perspective_lh(self.fovy.to_radians(), self.aspect, self.znear, self.zfar);
        let scale = Mat4::from_scale((0.01, 0.01, 0.01).into());
        proj * view * scale
    }

    pub fn update_aspect(&mut self, width: f32, height: f32) {
        self.aspect = width / height;
        self.dirty = true;
    }

    pub fn update_view(&mut self, queue: &Queue) {
        if !self.dirty { return; }
        self.dirty = false;
        let new_proj = self.build_projection();
        let mut uniform = CameraUniform::new();
        uniform.proj_view = new_proj.to_cols_array_2d();
        uniform.view_position = Vec4::from((self.eye, 1.0)).into();
        uniform.num_lights = self.light_count;
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
            self.dirty = true;
        }
        if keys.down_pressed {
            let translation = self.direction.normalize() * self.speed * delta_time * 10.;
            debug!("{translation}");
            self.eye -= translation;
            self.dirty = true;
        }
        if keys.left_pressed {
            let translation = right * self.speed * delta_time * 0.5;
            debug!("{translation}");
            self.eye -= translation;
            self.dirty = true;
        }
        if keys.right_pressed {
            let translation = right * self.speed * delta_time * 0.5;
            debug!("{translation}");
            self.eye += translation;
            self.dirty = true;
        }
        if cursor_delta.length() != 0.0 {
            let rotation_up =
                Mat4::from_axis_angle(global_up.xyz(), cursor_delta.x.to_degrees() * delta_time);
            let rotation_right =
                Mat4::from_axis_angle(right, -cursor_delta.y.to_degrees() * delta_time);

            self.direction = (rotation_right * rotation_up * as_4(self.direction)).xyz();
            self.dirty = true;
        }
        if self.dirty {
            self.view = Mat4::look_at_lh(
                self.eye,
                self.eye + self.direction.normalize(),
                global_up.xyz(),
            );
        }
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
            self.dirty = true;
        }
        if keys.down_pressed {
            self.eye -= forward_norm * self.speed * delta_time * 10.;
            self.dirty = true;
        }

        let translation = Mat4::from_translation(
            (self.view * Vec4::from((change * delta_time * 20., 0.0, 0.0))).xyz(),
        );

        if keys.input_device.pan() && change.length() != 0.0 {
            if keys.shift_pressed {
                self.target = transform(translation, self.target);
                self.eye = transform(translation, self.eye);
                self.dirty = true;
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
                self.dirty = true;
            }
        }
        if self.dirty {
            self.view = Mat4::look_at_lh(self.eye, self.target, global_up.xyz());
        }
    }
}

fn as_4(vec: Vec3) -> Vec4 {
    Vec4::from((vec, 1.0))
}

fn transform(mat: Mat4, vec: Vec3) -> Vec3 {
    (mat * as_4(vec)).xyz()
}
