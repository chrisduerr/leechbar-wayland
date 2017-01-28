use std::thread;
use std::fs::File;
use std::error::Error;
use std::os::unix::io::AsRawFd;
use std::sync::mpsc::{Receiver, Sender, channel, TryRecvError};
use wayland_client::{self, EventQueueHandle, EnvHandler, RequestResult};
use wayland_client::protocol::{wl_compositor, wl_shell, wl_shm, wl_shell_surface, wl_seat,
                               wl_pointer, wl_surface, wl_output, wl_display, wl_registry};

use self::generated::client::desktop_shell;

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
    event_out: Sender<i32>,
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
            _height: i32,
            _refresh: i32) {
        let _ = self.event_out.send(width);
    }
}
declare_handler!(EventHandler, wl_output::Handler, wl_output::WlOutput);

pub fn start_wayland_panel(bar_img_in: Receiver<(File, i32)>,
                           resize_out: Sender<i32>)
                           -> Result<(), Box<Error>> {
    let (display, mut event_queue) = match wayland_client::default_connect() {
        Ok(ret) => ret,
        Err(e) => Err(format!("Cannot connect to wayland server: {:?}", e))?,
    };

    let registry = request_result_to_result(display.get_registry(), "Proxy already destroyed.")?;
    event_queue.add_handler(EnvHandler::<WaylandEnv>::new());
    event_queue.register::<_, EnvHandler<WaylandEnv>>(&registry, 0);
    event_queue.sync_roundtrip()?;

    let (shell_surface, pointer, surface, output) = {
        let state = event_queue.state();
        let env = state.get_handler::<EnvHandler<WaylandEnv>>(0);

        let surface = request_result_to_result(env.compositor.create_surface(),
                                               "Compositor already destroyed.")?;
        let shell_surface = request_result_to_result(env.shell.get_shell_surface(&surface),
                                                     "Surface already destroyed.")?;
        let pointer = request_result_to_result(env.seat.get_pointer(), "Seat already destroyed.")?;
        shell_surface.set_toplevel();

        // Make DesktopShell surface a bar
        env.desktop_shell.set_panel(&env.output, &surface);

        // Export output for registering an event handler later
        let output: wl_output::WlOutput = reexport(env, &registry, "wl_output")?;

        (shell_surface, pointer, surface, output)
    };

    let (event_out, event_in) = channel();

    event_queue.add_handler(EventHandler { event_out: event_out });
    event_queue.register::<_, EventHandler>(&pointer, 1);
    event_queue.register::<_, EventHandler>(&shell_surface, 1);
    event_queue.register::<_, EventHandler>(&output, 1);


    let (draw_resize_out, draw_resize_in) = channel();
    {
        let state = event_queue.state();
        let env = state.get_handler::<EnvHandler<WaylandEnv>>(0);
        let shm = reexport(env, &registry, "wl_shm")?;

        thread::spawn(move || {
            let mut output_width = 0;
            while let Ok((bar_img, bar_height)) = bar_img_in.recv() {
                while let Ok(width) = draw_resize_in.try_recv() {
                    output_width = width;
                }

                if let Err(TryRecvError::Disconnected) = draw_resize_in.try_recv() {
                    break;
                }

                if output_width > 0 &&
                   draw_bar(&bar_img, &shm, &surface, &display, output_width, bar_height).is_err() {
                    break;
                }
            }
        });
    }

    loop {
        // TODO: Fix SHM buffer error when resizing sometimes
        event_queue.dispatch()?;

        while let Ok(width) = event_in.try_recv() {
            resize_out.send(width)?;
            draw_resize_out.send(width)?;
        }

        if let Err(TryRecvError::Disconnected) = event_in.try_recv() {
            Err("Wayland event channel disconnected".to_owned())?;
        };
    }
}

fn draw_bar(bar_img: &File,
            shm: &wl_shm::WlShm,
            surface: &wl_surface::WlSurface,
            display: &wl_display::WlDisplay,
            bar_width: i32,
            bar_height: i32)
            -> Result<(), Box<Error>> {
    let pool = match shm.create_pool(bar_img.as_raw_fd(), bar_height * bar_width * 4) {
        RequestResult::Sent(pool) => pool,
        RequestResult::Destroyed => Err("SHM already destroyed.".to_owned())?,
    };

    let buffer = match pool.create_buffer(0,
                                          bar_width,
                                          bar_height,
                                          bar_width * 4,
                                          wl_shm::Format::Argb8888) {
        RequestResult::Sent(pool) => pool,
        RequestResult::Destroyed => Err("Pool already destroyed.".to_owned())?,
    };

    surface.attach(Some(&buffer), 0, 0);
    surface.commit();

    // Ignore if writing to display failed to try again next time
    let _ = display.flush();

    Ok(())
}

fn reexport<T: wayland_client::Proxy>(env: &EnvHandler<WaylandEnv>,
                                      registry: &wl_registry::WlRegistry,
                                      interface_name: &str)
                                      -> Result<T, Box<Error>> {
    for &(name, ref interface, version) in env.globals() {
        if interface == interface_name {
            return Ok(request_result_to_result(registry.bind::<T>(version, name),
                                               "Unabled to find WlOutput in globals.")?);
        }
    }
    Err(format!("Unable to find {} in globals.", interface_name))?
}

fn request_result_to_result<T>(request_result: RequestResult<T>,
                               error_msg: &str)
                               -> Result<T, String> {
    match request_result {
        RequestResult::Sent(result) => Ok(result),
        RequestResult::Destroyed => Err(error_msg.to_owned()),
    }
}
