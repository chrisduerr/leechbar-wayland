use xcb;
use std::fs;
use std::error;
use std::sync::mpsc;
use xcb_util::{icccm, ewmh};

use mouse::MouseEvent;

pub fn start_xorg_panel(bar_img_in: mpsc::Receiver<(fs::File, i32)>,
                        resize_out: mpsc::Sender<u32>,
                        mouse_out: mpsc::Sender<MouseEvent>)
                        -> Result<(), Box<error::Error>> {
    let (connection, screen) =
        xcb::Connection::connect(None).map_err(|_| "X Connection error::Error.")?;
    let connection =
        ewmh::Connection::connect(connection).map_err(|_| "X Connection error::Error.")?;
    let screen = connection.get_setup().roots().nth(screen as usize).ok_or("No screen found.")?;

    let (w, h) = (100, 100);

    let x = 0;
    let y = 0;

    let wid = connection.generate_id();
    xcb::create_window(&connection,
                       xcb::COPY_FROM_PARENT as u8,
                       wid,
                       screen.root(),
                       x,
                       y,
                       w,
                       h,
                       10, // border_width
                       xcb::WINDOW_CLASS_INPUT_OUTPUT as u16,
                       screen.root_visual(),
                       &[(xcb::CW_BACKING_PIXEL, screen.black_pixel())]);

    icccm::set_wm_class(&connection, wid, "leechbar", "Bar");
    icccm::set_wm_name(&connection, wid, "leechbar");
    ewmh::set_wm_name(&connection, wid, "leechbar");
    ewmh::set_wm_state(&connection,
                       wid,
                       &[connection.WM_STATE_STICKY(), connection.WM_STATE_ABOVE()]);
    ewmh::set_wm_window_type(&connection, wid, &[connection.WM_WINDOW_TYPE_DOCK()]);

    xcb::map_window(&connection, wid);
    connection.flush();

    loop {}
}
