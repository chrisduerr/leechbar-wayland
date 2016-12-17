#[macro_use]
extern crate wayland_client;
#[macro_use]
extern crate wayland_sys;
extern crate tempfile;

extern crate byteorder;
use byteorder::{WriteBytesExt, NativeEndian};

use std::io::Write;
use std::os::unix::io::AsRawFd;

use wayland_client::{EventQueueHandle, EnvHandler};
use wayland_client::protocol::{wl_compositor, wl_shell, wl_shm, wl_shell_surface, wl_seat,
                               wl_pointer, wl_surface, wl_output};

pub use generated::client::desktop_shell;

mod generated {
    #![allow(dead_code,non_camel_case_types,unused_unsafe,unused_variables)]
    #![allow(non_upper_case_globals,non_snake_case,unused_imports)]

    pub mod interfaces {
        #[doc(hidden)]
        pub use wayland_client::protocol_interfaces::{wl_output_interface, wl_surface_interface};
        include!(concat!(env!("OUT_DIR"), "/desktop_shell_interfaces.rs"));
    }

    pub mod client {
        #[doc(hidden)]
        pub use wayland_client::{Proxy, Handler, EventQueueHandle, RequestResult};
        #[doc(hidden)]
        pub use super::interfaces;
        #[doc(hidden)]
        pub use wayland_client::protocol::{wl_surface, wl_region, wl_output};
        include!(concat!(env!("OUT_DIR"), "/desktop_shell.rs"));
    }
}

wayland_env!(WaylandEnv,
             compositor: wl_compositor::WlCompositor,
             seat: wl_seat::WlSeat,
             shell: wl_shell::WlShell,
             shm: wl_shm::WlShm,
             output: wl_output::WlOutput,
             desktop_shell: desktop_shell::DesktopShell);

struct MyHandler;

impl wl_shell_surface::Handler for MyHandler {
    fn ping(&mut self,
            _: &mut EventQueueHandle,
            me: &wl_shell_surface::WlShellSurface,
            serial: u32) {
        me.pong(serial);
    }

    // we ignore the other methods in this example, by default they do nothing
}

declare_handler!(MyHandler,
                 wl_shell_surface::Handler,
                 wl_shell_surface::WlShellSurface);

impl wl_pointer::Handler for MyHandler {
    fn enter(&mut self,
             _: &mut EventQueueHandle,
             _me: &wl_pointer::WlPointer,
             _serial: u32,
             _surface: &wl_surface::WlSurface,
             surface_x: f64,
             surface_y: f64) {
        println!("Pointer entered surface at ({},{}).", surface_x, surface_y);
    }
    fn leave(&mut self,
             _: &mut EventQueueHandle,
             _me: &wl_pointer::WlPointer,
             _serial: u32,
             _surface: &wl_surface::WlSurface) {
        println!("Pointer left surface.");
    }
    fn motion(&mut self,
              _: &mut EventQueueHandle,
              _me: &wl_pointer::WlPointer,
              _time: u32,
              surface_x: f64,
              surface_y: f64) {
        println!("Pointer moved to ({},{}).", surface_x, surface_y);
    }
    fn button(&mut self,
              _: &mut EventQueueHandle,
              _me: &wl_pointer::WlPointer,
              _serial: u32,
              _time: u32,
              button: u32,
              state: wl_pointer::ButtonState) {
        println!("Button {} ({}) was {:?}.",
                 match button {
                     272 => "Left",
                     273 => "Right",
                     274 => "Middle",
                     _ => "Unknown",
                 },
                 button,
                 state);
    }

    // we ignore the other methods in this example, by default they do nothing
}

declare_handler!(MyHandler, wl_pointer::Handler, wl_pointer::WlPointer);

fn main() {
    let (display, mut event_queue) = match wayland_client::default_connect() {
        Ok(ret) => ret,
        Err(e) => panic!("Cannot connect to wayland server: {:?}", e),
    };

    event_queue.add_handler(EnvHandler::<WaylandEnv>::new());
    event_queue.add_handler(MyHandler);
    let registry = display.get_registry().expect("Display cannot be already destroyed.");
    event_queue.register::<_, EnvHandler<WaylandEnv>>(&registry, 0);
    event_queue.sync_roundtrip().unwrap();

    // create a tempfile to write the conents of the window on
    let mut tmp = tempfile::tempfile().ok().expect("Unable to create a tempfile.");
    // write the contents to it, lets put a nice color gradient
    for i in 0..100_000 {
        let x = i % 1000 as u32;
        let r: u32 = (1000 - x) * 0xFF / 1000;
        let g: u32 = 0;
        let b: u32 = (1000 - x) * 0xFF / 1000;
        let _ = tmp.write_u32::<NativeEndian>((0xFF << 24) + (r << 16) + (g << 8) + b);
    }
    let _ = tmp.flush();

    // prepare the wayland surface
    let (shell_surface, pointer) = {
        // introduce a new scope because .state() borrows the event_queue
        let state = event_queue.state();
        // retrieve the EnvHandler
        let env = state.get_handler::<EnvHandler<WaylandEnv>>(0);
        let surface = env.compositor.create_surface().expect("Compositor cannot be destroyed");
        let shell_surface =
            env.shell.get_shell_surface(&surface).expect("Shell cannot be destroyed");

        env.desktop_shell.set_panel(&env.output, &surface);

        let pool = env.shm.create_pool(tmp.as_raw_fd(), 80_000).expect("Shm cannot be destroyed");
        // match a buffer on the part we wrote on
        let buffer = pool.create_buffer(0, 1000, 20, 4000, wl_shm::Format::Argb8888)
            .expect("The pool cannot be already dead");

        // make our surface as a toplevel one
        shell_surface.set_toplevel();
        // attach the buffer to it
        surface.attach(Some(&buffer), 0, 0);
        // commit
        surface.commit();

        let pointer = env.seat.get_pointer().expect("Seat cannot be destroyed.");

        // we can let the other objects go out of scope
        // their associated wyland objects won't automatically be destroyed
        // and we don't need them in this example
        (shell_surface, pointer)
    };

    event_queue.add_handler(MyHandler);
    event_queue.register::<_, MyHandler>(&shell_surface, 1);
    event_queue.register::<_, MyHandler>(&pointer, 1);

    loop {
        display.flush().unwrap();
        event_queue.dispatch().unwrap();
    }
}
