use toml::Value;
use std::error::Error;
use image::DynamicImage;
use std::collections::HashMap;

use parse_input::Config;

mod text;
mod command;

lazy_static! {
    pub static ref MODULES: HashMap<&'static str, fn(Config, &Value) -> Result<Box<Block>, Box<Error>>> = {
        let mut m: HashMap<&'static str
            , fn(Config, &Value) -> Result<Box<Block>, Box<Error>>> = HashMap::new();
        m.insert("text", text::TextBlock::new);
        m.insert("command", command::CommandBlock::new);
        m
    };
}

pub trait Block: Send + 'static {
    fn render(&mut self) -> Result<DynamicImage, Box<Error>>;
}
