use std::env;
use std::thread;
use std::fs::File;
use std::error::Error;
use std::os::unix::io::AsRawFd;
use std::sync::mpsc::{Receiver, Sender};
use wayland_client::{self, EventQueueHandle, EnvHandler, RequestResult, cursor};
use wayland_client::protocol::{wl_compositor, wl_shell, wl_shm, wl_shell_surface, wl_seat,
                               wl_pointer, wl_surface, wl_output, wl_display, wl_registry};

use mouse::MouseEvent;
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
    resize_out: Sender<u32>,
    mouse_out: Sender<MouseEvent>,
    cursor_theme: cursor::CursorTheme,
    cursor_surface: wl_surface::WlSurface,
    last_x: f64,
    last_y: f64,
}

impl EventHandler {
    fn new(resize_out: Sender<u32>,
           mouse_out: Sender<MouseEvent>,
           cursor_theme: cursor::CursorTheme,
           cursor_surface: wl_surface::WlSurface)
           -> Result<EventHandler, Box<Error>> {
        Ok(EventHandler {
            resize_out: resize_out,
            mouse_out: mouse_out,
            cursor_theme: cursor_theme,
            cursor_surface: cursor_surface,
            last_x: 0f64,
            last_y: 0f64,
        })
    }
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
    fn motion(&mut self,
              _evqh: &mut EventQueueHandle,
              _proxy: &wl_pointer::WlPointer,
              _time: u32,
              surface_x: f64,
              surface_y: f64) {
        self.last_x = surface_x;
        self.last_y = surface_y;

        let _ = self.mouse_out.send(MouseEvent {
            button: None,
            state: None,
            x: surface_x,
            y: surface_y,
        });
    }

    fn button(&mut self,
              _evqh: &mut EventQueueHandle,
              _proxy: &wl_pointer::WlPointer,
              _serial: u32,
              _time: u32,
              button: u32,
              state: wl_pointer::ButtonState) {
        let _ = self.mouse_out.send(MouseEvent {
            button: Some(button),
            state: Some(state),
            x: self.last_x,
            y: self.last_y,
        });
    }

    fn leave(&mut self,
             _evqh: &mut EventQueueHandle,
             _proxy: &wl_pointer::WlPointer,
             _serial: u32,
             _surface: &wl_surface::WlSurface) {
        let _ = self.mouse_out.send(MouseEvent {
            button: None,
            state: None,
            x: -1f64,
            y: -1f64,
        });
    }

    fn enter(&mut self,
             _evqh: &mut EventQueueHandle,
             proxy: &wl_pointer::WlPointer,
             serial: u32,
             _surface: &wl_surface::WlSurface,
             _surface_x: f64,
             _surface_y: f64) {
        let cursor = self.cursor_theme.get_cursor("left_ptr").unwrap();
        let cursor_buffer = cursor.frame_buffer(0).unwrap();
        self.cursor_surface.attach(Some(&cursor_buffer), 0, 0);
        self.cursor_surface.commit();
        proxy.set_cursor(serial, Some(&self.cursor_surface), 0, 0);
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
        let _ = self.resize_out.send(width as u32);
    }
}
declare_handler!(EventHandler, wl_output::Handler, wl_output::WlOutput);

pub fn start_wayland_panel(bar_img_in: Receiver<(File, i32)>,
                           resize_out: Sender<u32>,
                           mouse_out: Sender<MouseEvent>)
                           -> Result<(), Box<Error>> {
    let (display, mut event_queue) = match wayland_client::default_connect() {
        Ok(ret) => ret,
        Err(e) => Err(format!("Cannot connect to wayland server: {:?}", e))?,
    };

    let registry = request_result_to_result(display.get_registry(), "Proxy already destroyed.")?;
    event_queue.add_handler(EnvHandler::<WaylandEnv>::new());
    event_queue.register::<_, EnvHandler<WaylandEnv>>(&registry, 0);
    event_queue.sync_roundtrip()?;

    let (shell_surface, pointer, surface, output, cursor_surface, cursor_theme) = {
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

        // Create a surface for the cursor
        let cursor_surface = request_result_to_result(env.compositor.create_surface(),
                                                      "Compositor already destroyed.")?;
        let cursor_theme = load_cursor_theme(&env.shm);

        // Export output for registering an event handler later
        let output: wl_output::WlOutput = reexport(env, &registry, "wl_output")?;

        (shell_surface, pointer, surface, output, cursor_surface, cursor_theme)
    };

    event_queue.add_handler(EventHandler::new(resize_out, mouse_out, cursor_theme, cursor_surface)?);

    event_queue.register::<_, EventHandler>(&shell_surface, 1);
    event_queue.register::<_, EventHandler>(&pointer, 1);
    event_queue.register::<_, EventHandler>(&output, 1);

    {
        let state = event_queue.state();
        let env = state.get_handler::<EnvHandler<WaylandEnv>>(0);
        let shm: wl_shm::WlShm = reexport(env, &registry, "wl_shm")?;

        let mut wlc_unbugged = false;
        thread::spawn(move || {
            while let Ok((bar_img, bar_height)) = bar_img_in.recv() {
                let output_width = match bar_img.metadata() {
                    Ok(meta) => meta.len() as i32 / bar_height / 4,
                    _ => 0,
                };

                if output_width > 0 {
                    let _ = draw_bar(&bar_img, &shm, &surface, &display, output_width, bar_height);
                }

                if !wlc_unbugged {
                    surface.commit();
                    wlc_unbugged = true;
                }
            }
        });
    }

    // Dispatch all Wayland Events until the end of Dawn
    loop {
        event_queue.dispatch()?;
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

fn load_cursor_theme(shm: &wl_shm::WlShm) -> cursor::CursorTheme {
    let name = env::var("SWAY_CURSOR_THEME").unwrap_or_else(|_| String::from("default"));
    let size = env::var("SWAY_CURSOR_SIZE").unwrap_or_else(|_| String::from("16"));
    let size = size.parse().unwrap_or(16);
    cursor::load_theme(Some(&name), size, shm)
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
