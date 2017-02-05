use std::cmp;
use std::thread;
use toml::Value;
use rusttype::Font;
use std::error::Error;
use std::time::Duration;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::Sender;
use image::{DynamicImage, Rgba};

use modules::Block;
use mouse::MouseEvent;
use parse_input::Config;
use modules::text::TextBlock;

pub struct CommandBlock {
    bar_height: u32,
    font_height: u32,
    font: Font<'static>,
    bg_col: DynamicImage,
    fg_col: Rgba<u8>,
    width: u32,
    spacing: u32,
    command: String,
    interval: u32,
    cache: Arc<Mutex<Option<DynamicImage>>>,
}

// Unwraps cannot fail
impl CommandBlock {
    pub fn create(config: Config, value: &Value) -> Result<Box<Block>, Box<Error>> {
        let command = value.lookup("command").ok_or("Could not find command in a command module.")?;
        let command = command.as_str().ok_or("Command in command module is not a String.")?;
        let font_height = cmp::min(config.bar_height, config.font_height.unwrap());
        Ok(Box::new(CommandBlock {
            bar_height: config.bar_height,
            font_height: font_height,
            font: config.font.unwrap(),
            bg_col: config.bg,
            fg_col: config.fg,
            width: config.width,
            spacing: config.spacing,
            command: command.to_owned(),
            interval: config.interval,
            cache: Arc::new(Mutex::new(None)),
        }))
    }
}

impl Block for CommandBlock {
    fn start_interval(&self, interval_out: Sender<(Option<u32>, Option<MouseEvent>)>) {
        if self.interval > 0 {
            let interval = self.interval as u64;
            let cache = self.cache.clone();
            thread::spawn(move || {
                loop {
                    thread::sleep(Duration::from_millis(interval));
                    let mut cache_lock = cache.lock().unwrap(); // TODO: Not unwrap?
                    *cache_lock = None;
                    interval_out.send((None, None)).unwrap(); // TODO: Not unwrap?
                }
            });
        }
    }

    fn mouse_event(&self, mouse_event: MouseEvent) -> bool {
        // TODO!!!
        println!("TODO: Command Mouse Event! {}-{}",
                 mouse_event.x,
                 mouse_event.y);

        if mouse_event.state.is_some() {
            println!("CLICK");
        }

        false
    }

    fn render(&mut self) -> Result<DynamicImage, Box<Error>> {
        let mut cache = self.cache.lock().map_err(|e| e.to_string())?;
        if let Some(ref cache) = *cache {
            return Ok(cache.clone());
        }

        let output = Command::new("sh").arg("-c").arg(&self.command).output()?;
        let text = String::from_utf8_lossy(&output.stdout);

        let mut text_block = TextBlock {
            bar_height: self.bar_height,
            font_height: self.font_height,
            font: self.font.clone(),
            bg_col: self.bg_col.clone(),
            fg_col: self.fg_col,
            text: text.to_string(),
            width: self.width,
            spacing: self.spacing,
            cache: None,
        };

        let image = text_block.render()?;
        *cache = Some(image.clone());
        Ok(image)
    }
}
