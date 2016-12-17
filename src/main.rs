#[macro_use]
extern crate wayland_client;
#[macro_use]
extern crate wayland_sys;
extern crate byteorder;
extern crate tempfile;
extern crate rand;

use std::fs::File;
use std::io::Write;
use std::{thread, time};
use std::os::unix::io::AsRawFd;
use byteorder::{WriteBytesExt, NativeEndian};
use rand::distributions::{IndependentSample, Range};
use wayland_client::{EventQueueHandle, EnvHandler, RequestResult};
use wayland_client::protocol::{wl_compositor, wl_shell, wl_shm, wl_shell_surface, wl_seat,
                               wl_pointer, wl_surface, wl_output};

use generated::client::desktop_shell;

mod generated {
    #![allow(dead_code,non_camel_case_types,unused_unsafe,unused_variables)]
    #![allow(non_upper_case_globals,non_snake_case,unused_imports)]

    #[doc(hidden)]
    pub mod interfaces {
        #[doc(hidden)]
        pub use wayland_client::protocol_interfaces::{wl_output_interface, wl_surface_interface};
        include!(concat!(env!("OUT_DIR"), "/desktop_shell_interfaces.rs"));
    }

    #[doc(hidden)]
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

struct EventHandler;

impl wl_shell_surface::Handler for EventHandler {
    fn ping(&mut self,
            _: &mut EventQueueHandle,
            me: &wl_shell_surface::WlShellSurface,
            serial: u32) {
        me.pong(serial);
    }
}
declare_handler!(EventHandler,
                 wl_shell_surface::Handler,
                 wl_shell_surface::WlShellSurface);

impl wl_pointer::Handler for EventHandler {
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
}
declare_handler!(EventHandler, wl_pointer::Handler, wl_pointer::WlPointer);

fn main() {
    // TODO: Implement logging
    start_wayland_panel().unwrap();
}

fn start_wayland_panel() -> Result<(), String> {
    let (display, mut event_queue) = match wayland_client::default_connect() {
        Ok(ret) => ret,
        Err(e) => return Err(format!("Cannot connect to wayland server: {:?}", e)),
    };

    event_queue.add_handler(EnvHandler::<WaylandEnv>::new());
    let registry = match display.get_registry() {
        RequestResult::Sent(registry) => registry,
        RequestResult::Destroyed => return Err("Proxy already destroyed.".to_owned()),
    };
    event_queue.register::<_, EnvHandler<WaylandEnv>>(&registry, 0);
    event_queue.sync_roundtrip().map_err(|e| e.to_string())?;

    let (shell_surface, pointer, surface) = {
        let state = event_queue.state();
        let env = state.get_handler::<EnvHandler<WaylandEnv>>(0);

        let surface = match env.compositor.create_surface() {
            RequestResult::Sent(surface) => surface,
            RequestResult::Destroyed => return Err("Compositor already destroyed.".to_owned()),
        };

        let shell_surface = match env.shell.get_shell_surface(&surface) {
            RequestResult::Sent(shell_surface) => shell_surface,
            RequestResult::Destroyed => return Err("Surface already destroyed.".to_owned()),
        };
        shell_surface.set_toplevel();

        let pointer = match env.seat.get_pointer() {
            RequestResult::Sent(pointer) => pointer,
            RequestResult::Destroyed => return Err("Seat already destroyed.".to_owned()),
        };

        // Make DesktopShell surface a bar
        env.desktop_shell.set_panel(&env.output, &surface);

        (shell_surface, pointer, surface)
    };

    event_queue.add_handler(EventHandler);
    event_queue.register::<_, EventHandler>(&shell_surface, 1);
    event_queue.register::<_, EventHandler>(&pointer, 1);

    loop {
        {
            let state = event_queue.state();
            let env = state.get_handler::<EnvHandler<WaylandEnv>>(0);
            let tmp = get_tmp().map_err(|e| e.to_string())?;
            let pool = match env.shm.create_pool(tmp.as_raw_fd(), 80_000) {
                RequestResult::Sent(pool) => pool,
                RequestResult::Destroyed => return Err("SHM already destroyed.".to_owned()),
            };
            let buffer = match pool.create_buffer(0, 1000, 20, 4000, wl_shm::Format::Argb8888) {
                RequestResult::Sent(pool) => pool,
                RequestResult::Destroyed => return Err("Pool already destroyed.".to_owned()),
            };
            surface.attach(Some(&buffer), 0, 0);
            surface.commit();
        }

        display.flush().map_err(|e| e.to_string())?;
        event_queue.dispatch_pending().map_err(|e| e.to_string())?;

        thread::sleep(time::Duration::from_millis(1000));
    }
}

// TODO: This is kinda where the actual bar would come in
fn get_tmp() -> Result<File, std::io::Error> {
    let mut tmp = tempfile::tempfile()?;
    let between = Range::new(0, 0xFF);
    let mut rng = rand::thread_rng();
    for _ in 0..20_000 {
        let r: u32 = between.ind_sample(&mut rng);
        let g: u32 = between.ind_sample(&mut rng);
        let b: u32 = between.ind_sample(&mut rng);
        let _ = tmp.write_u32::<NativeEndian>((0xFF << 24) + (r << 16) + (g << 8) + b);
    }
    let _ = tmp.flush();
    Ok(tmp)
}
