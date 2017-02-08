use wayland_client::protocol::wl_pointer;

#[derive(Clone)]
pub struct MouseEvent {
    pub state: Option<wl_pointer::ButtonState>,
    pub button: Option<u32>,
    pub x: f64,
    pub y: f64,
}
