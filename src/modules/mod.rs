use toml;
use image;
use std::error;
use std::sync::mpsc;
use std::collections;

use mouse;
use parse_input;

mod text;
mod command;

lazy_static! {
    pub static ref MODULES: collections::HashMap<&'static str, fn(parse_input::Config, &toml::Value) -> Result<Box<Block>, Box<error::Error>>> = {
        let mut m: collections::HashMap<&'static str
            , fn(parse_input::Config, &toml::Value) -> Result<Box<Block>, Box<error::Error>>> = collections::HashMap::new();
        m.insert("text", text::TextBlock::create);
        m.insert("command", command::CommandBlock::create);
        m
    };
}

pub trait Block: Send + 'static {
    // Used to start notifier for bar updates
    fn start_interval(&mut self, mpsc::Sender<(Option<u32>, Option<mouse::MouseEvent>)>);

    // Used to render the settings into an Image Block
    fn render(&mut self) -> Result<image::DynamicImage, Box<error::Error>>;

    // Used to update the settings based on mouse focus
    // Return true if it requires redraw
    fn mouse_event(&mut self, Option<mouse::MouseEvent>) -> bool;
}
