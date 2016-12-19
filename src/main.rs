#[macro_use]
extern crate wayland_client;
#[macro_use]
extern crate wayland_sys;
extern crate byteorder;
extern crate tempfile;
extern crate rand;

use std::thread;
use std::fs::File;
use std::os::unix::io::AsRawFd;
use std::sync::mpsc::{channel, Sender, Receiver, TryRecvError};
use wayland_client::{EventQueueHandle, EnvHandler, RequestResult, EventQueue};
use wayland_client::protocol::{wl_compositor, wl_shell, wl_shm, wl_shell_surface, wl_seat,
                               wl_pointer, wl_surface, wl_output, wl_display};

use generated::client::desktop_shell;

mod create_bar;
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
             desktop_shell: desktop_shell::DesktopShell,
             compositor: wl_compositor::WlCompositor,
             output: wl_output::WlOutput,
             shell: wl_shell::WlShell,
             seat: wl_seat::WlSeat,
             shm: wl_shm::WlShm);

struct EventHandler {
    event_out: Sender<(i32, i32)>,
}

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
    // TODO: Mouse Events not needed yet.
    fn enter(&mut self,
             _evqh: &mut EventQueueHandle,
             _proxy: &wl_pointer::WlPointer,
             _serial: u32,
             _surface: &wl_surface::WlSurface,
             _surface_x: f64,
             _surface_y: f64) {
        println!("Mouse Entered Bar!");
    }
}

declare_handler!(EventHandler, wl_pointer::Handler, wl_pointer::WlPointer);

impl wl_output::Handler for EventHandler {
    fn mode(&mut self,
            _evqh: &mut EventQueueHandle,
            _proxy: &wl_output::WlOutput,
            _flags: wl_output::Mode,
            width: i32,
            height: i32,
            _refresh: i32) {
        let _ = self.event_out.send((width, height));
    }
}
declare_handler!(EventHandler, wl_output::Handler, wl_output::WlOutput);

fn main() {
    let (bar_img_out, bar_img_in) = channel();
    thread::spawn(move || {
        create_bar::start_bar_creator(&bar_img_out);
    });

    // TODO: Implement logging, auto-revive
    start_wayland_panel(&bar_img_in).unwrap();
}

fn start_wayland_panel(bar_img_in: &Receiver<File>) -> Result<(), String> {
    let (display, mut event_queue) = match wayland_client::default_connect() {
        Ok(ret) => ret,
        Err(e) => return Err(format!("Cannot connect to wayland server: {:?}", e)),
    };

    let registry = request_result_to_result(display.get_registry(), "Proxy already destroyed.")?;
    event_queue.add_handler(EnvHandler::<WaylandEnv>::new());
    event_queue.register::<_, EnvHandler<WaylandEnv>>(&registry, 0);
    event_queue.sync_roundtrip().map_err(|e| e.to_string())?;

    let (shell_surface, pointer, surface, output) = {
        let state = event_queue.state();
        let env = state.get_handler::<EnvHandler<WaylandEnv>>(0);

        let surface = request_result_to_result(env.compositor.create_surface(),
                                               "Compositor already destroyed,")?;
        let shell_surface = request_result_to_result(env.shell.get_shell_surface(&surface),
                                                     "Surface already destroyed.")?;
        let pointer = request_result_to_result(env.seat.get_pointer(), "Seat already destroyed.")?;
        shell_surface.set_toplevel();

        // Make DesktopShell surface a bar
        env.desktop_shell.set_panel(&env.output, &surface);

        // Export output for registering an event handler later
        let mut output = None;
        for &(name, ref interface, version) in env.globals() {
            if interface == "wl_output" {
                output = Some(request_result_to_result(registry.bind::<wl_output::WlOutput>(version,
                                                                                        name),
                                                       "Unabled to find WlOutput in globals")?);
            }
        }
        let output = output.ok_or("Unable to find WlOutput in globals.".to_owned())?;

        (shell_surface, pointer, surface, output)
    };

    let (event_out, event_in) = channel();

    event_queue.add_handler(EventHandler { event_out: event_out });
    event_queue.register::<_, EventHandler>(&pointer, 1);
    event_queue.register::<_, EventHandler>(&shell_surface, 1);
    event_queue.register::<_, EventHandler>(&output, 1);

    let (mut width, mut height) = (0, 0);
    loop {
        match event_in.try_recv() {
            Ok((w, h)) => {
                width = w;
                height = h;
            }
            Err(TryRecvError::Empty) => (),
            Err(_) => return Err("Output resize channel disconnected".to_owned()),
        };

        {
            match bar_img_in.try_recv() {
                Ok(bar_img) => draw_bar(&bar_img, &mut event_queue, &surface, &display)?,
                Err(TryRecvError::Empty) => (), // TODO: Handle possible errors and retry?
                Err(_) => return Err("Bar creation channel disconnected.".to_owned()),
            };
        }
        event_queue.sync_roundtrip().map_err(|e| e.to_string())?;
    }
}

fn request_result_to_result<T>(request_result: RequestResult<T>,
                               error_msg: &str)
                               -> Result<T, String> {
    match request_result {
        RequestResult::Sent(result) => Ok(result),
        RequestResult::Destroyed => Err(error_msg.to_owned()),
    }
}

fn draw_bar(bar_img: &File,
            event_queue: &mut EventQueue,
            surface: &wl_surface::WlSurface,
            display: &wl_display::WlDisplay)
            -> Result<(), String> {
    let state = event_queue.state();
    let env = state.get_handler::<EnvHandler<WaylandEnv>>(0);
    let pool = match env.shm.create_pool(bar_img.as_raw_fd(), 80_000) {
        RequestResult::Sent(pool) => pool,
        RequestResult::Destroyed => return Err("SHM already destroyed.".to_owned()),
    };
    let buffer = match pool.create_buffer(0, 1000, 20, 4000, wl_shm::Format::Argb8888) {
        RequestResult::Sent(pool) => pool,
        RequestResult::Destroyed => return Err("Pool already destroyed.".to_owned()),
    };
    surface.attach(Some(&buffer), 0, 0);
    surface.commit();
    let _ = display.flush(); // Ignore if writing to display failed to try again next time
    Ok(())
}
