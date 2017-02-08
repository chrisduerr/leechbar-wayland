use toml::Value;
use std::error::Error;
use image::DynamicImage;
use std::sync::mpsc::Sender;
use std::collections::HashMap;

use parse_input::Config;
use mouse::MouseEvent;

mod text;
mod command;

lazy_static! {
    pub static ref MODULES: HashMap<&'static str, fn(Config, &Value) -> Result<Box<Block>, Box<Error>>> = {
        let mut m: HashMap<&'static str
            , fn(Config, &Value) -> Result<Box<Block>, Box<Error>>> = HashMap::new();
        m.insert("text", text::TextBlock::create);
        m.insert("command", command::CommandBlock::create);
        m
    };
}

pub trait Block: Send + 'static {
    // Used to start notifier for bar updates
    fn start_interval(&mut self, Sender<(Option<u32>, Option<MouseEvent>)>);

    // Used to render the settings into an Image Block
    fn render(&mut self) -> Result<DynamicImage, Box<Error>>;

    // Used to update the settings based on mouse focus
    // Return true if it requires redraw
    fn mouse_event(&mut self, Option<MouseEvent>) -> bool;
}
