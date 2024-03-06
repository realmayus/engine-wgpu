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
