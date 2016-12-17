extern crate wayland_scanner;

use std::env::var;
use std::path::Path;

use wayland_scanner::{Side, generate_code, generate_interfaces};

fn main() {
    // Location of the xml file, relative to the `Cargo.toml`
    let protocol_file = "./desktop-shell.xml";

    // Target directory for the generate files
    let out_dir_str = var("OUT_DIR").unwrap();
    let out_dir = Path::new(&out_dir_str);

    generate_code(protocol_file,
                  out_dir.join("desktop_shell.rs"),
                  Side::Client /* Replace by `Side::Server` for server-side code */);

    // interfaces are the same for client and server
    generate_interfaces(protocol_file, out_dir.join("desktop_shell_interfaces.rs"));
}
