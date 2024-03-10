use crate::commands::CommandResult;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

#[derive(Debug)]
pub enum Event {
    Click { x: u32, y: u32, mouse_button: MouseButton },
    CommandResult(CommandResult),
}
