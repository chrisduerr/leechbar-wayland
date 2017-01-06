#[macro_use]
extern crate wayland_client;
#[macro_use]
extern crate wayland_sys;
extern crate byteorder;
extern crate tempfile;
extern crate rusttype;
extern crate image;
extern crate regex;

use std::thread;
use std::sync::mpsc::channel;

mod wayland;
mod create_bar;
mod parse_input;

// TODO: Logging instead of unwrapping
// TODO: Immortality -> Auto-Revive
fn main() {
    let (bar_img_out, bar_img_in) = channel();
    let (resize_out, resize_in) = channel();
    let (stdin_out, stdin_in) = channel();

    let settings = parse_input::get_settings();

    {
        let settings = settings.clone();
        thread::spawn(move || {
            create_bar::start_bar_creator(settings, bar_img_out, resize_in, stdin_in).unwrap();
        });
    }

    {
        let settings = settings.clone();
        thread::spawn(move || {
            parse_input::read_stdin(settings, &stdin_out).unwrap();
        });
    }

    wayland::start_wayland_panel(&settings, bar_img_in, resize_out).unwrap();
}
