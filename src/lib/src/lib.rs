pub mod scene;
pub mod util;

pub trait Dirtyable {
    fn dirty(&self) -> bool;
    fn set_dirty(&mut self, dirty: bool);
}
