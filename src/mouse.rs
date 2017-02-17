#[derive(Copy, Clone)]
pub enum ButtonState {
    PRESSED,
    RELEASED,
}

#[derive(Clone)]
pub struct MouseEvent {
    pub state: Option<ButtonState>,
    pub button: Option<u32>,
    pub x: f64,
    pub y: f64,
}
