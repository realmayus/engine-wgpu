use std::f32::consts::PI;

use egui::Ui;
use glam::{Vec3, Vec4};

pub(crate) struct SparseScene {
    pub(crate) id: u32,
    pub(crate) name: Option<Box<str>>,
}

pub(crate) struct SparseModel {
    pub(crate) id: u32,
    pub(crate) name: Option<Box<str>>,
}

#[derive(PartialEq)]
pub(crate) enum CameraModes {
    Arcball,
    FPS,
}

pub(crate) trait Editable<T> {
    fn editable(&mut self, label: Option<String>, ui: &mut egui::Ui, min: T, max: T);
}

impl Editable<f32> for f32 {
    fn editable(&mut self, label: Option<String>, ui: &mut egui::Ui, min: f32, max: f32) {
        ui.horizontal(|ui| {
            if let Some(label) = label {
                ui.label(label);
            }
            ui.add(egui::DragValue::new(self).clamp_range(min..=max));
        });
    }
}

impl Editable<glam::Vec3> for glam::Vec3 {
    fn editable(&mut self, label: Option<String>, ui: &mut egui::Ui, min: Vec3, max: Vec3) {
        ui.horizontal(|ui| {
            if let Some(label) = label {
                ui.label(label);
            }
            ui.add(egui::DragValue::new(&mut self.x).clamp_range(min.x..=max.x));
            ui.add(egui::DragValue::new(&mut self.y).clamp_range(min.y..=max.y));
            ui.add(egui::DragValue::new(&mut self.z).clamp_range(min.z..=max.z));
        });
    }
}

impl Editable<glam::Vec4> for glam::Vec4 {
    fn editable(&mut self, label: Option<String>, ui: &mut egui::Ui, min: Vec4, max: Vec4) {
        ui.horizontal(|ui| {
            if let Some(label) = label {
                ui.label(label);
            }
            ui.add(egui::DragValue::new(&mut self.x).clamp_range(min.x..=max.x));
            ui.add(egui::DragValue::new(&mut self.y).clamp_range(min.y..=max.y));
            ui.add(egui::DragValue::new(&mut self.z).clamp_range(min.z..=max.z));
            ui.add(egui::DragValue::new(&mut self.w).clamp_range(min.w..=max.w));
        });
    }
}

impl Editable<bool> for bool {
    fn editable(&mut self, label: Option<String>, ui: &mut Ui, min: bool, max: bool) {
        ui.checkbox(self, label.unwrap_or_default());
    }
}

#[macro_export]
macro_rules! observe {
    ($field:expr, $code:block, |$model:ident| $update:block) => {
        let before = $field.clone();
        $code
        if before != $field {
            $update
        }
    };
}

#[macro_export]
macro_rules! mutate_indirect {
    ($field:expr, |$copy:ident| $code:block, |$model:ident, $copy2:ident| $update:block) => {
        let mut $copy = $field.clone();
        $code
        if $copy != $field {
            let $copy2 = $copy;
            $update
        }
    };
}

pub(crate) struct RainbowAnimation {
    current_color: [u8; 3],
    time_elapsed: i32,
    transition_duration: u32,
}

impl RainbowAnimation {
    pub fn new() -> Self {
        RainbowAnimation {
            current_color: [0, 0, 0], // Start with red
            time_elapsed: 0,
            transition_duration: 150, // Transition duration in milliseconds
        }
    }

    pub fn update(&mut self, delta_time: u32) {
        self.time_elapsed += 1;

        // Calculate the interpolation factor
        let t = self.time_elapsed.abs_diff(0) as f32 / 1000.0;
        let t = t * 2. * PI;
        if self.time_elapsed.abs_diff(0) >= self.transition_duration {
            self.time_elapsed = -self.time_elapsed;
        }

        // Interpolate between colors of the rainbow
        let red = (t.sin() * 176.0) as u8;
        let green = (t.sin() * 84.0) as u8;
        let blue = (t.sin() * 39.0) as u8;

        self.current_color = [red, green, blue];
    }

    pub fn get_current_color(&self) -> [u8; 3] {
        self.current_color
    }

    pub fn reset(&mut self) {
        self.time_elapsed = 0;
        self.current_color = [0, 0, 0];
    }
}
