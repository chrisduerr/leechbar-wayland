#[macro_use]
extern crate wayland_client;
#[macro_use]
extern crate wayland_sys;
#[macro_use]
extern crate lazy_static;
extern crate tempfile;
extern crate rusttype;
extern crate xcb_util;
extern crate image;
extern crate regex;
extern crate toml;
extern crate xcb;

use std::thread;
use std::sync::mpsc;

mod xorg;
mod mouse;
mod modules;
mod wayland;
mod create_bar;
mod parse_input;

// TODO: Logging instead of unwrapping
// TODO: Immortality -> Auto-Revive
// TODO: Don't use libs without prefix, so image::GenericImage instead of GenericImage
fn main() {
    let (bar_img_out, bar_img_in) = mpsc::channel();
    let (resize_out, resize_in) = mpsc::channel();
    let (mouse_out, mouse_in) = mpsc::channel();

    {
        thread::spawn(move || {
            create_bar::start_bar_creator(bar_img_out, resize_in, mouse_in).unwrap();
        });
    }

    if wayland::wayland_server_available() {
        wayland::start_wayland_panel(bar_img_in, resize_out, mouse_out).unwrap();
    } else {
        xorg::start_xorg_panel(bar_img_in, resize_out, mouse_out).unwrap();
    }
}
