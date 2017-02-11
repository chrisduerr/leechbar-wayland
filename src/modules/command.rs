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
use wayland_client::protocol::wl_pointer::ButtonState;

use modules::Block;
use mouse::MouseEvent;
use modules::text::TextBlock;
use parse_input::{self, Config};

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
    hover_bg_col: DynamicImage,
    hover_fg_col: Rgba<u8>,
    click_command: Option<String>,
    hover: bool,
}

// Unwraps cannot fail
impl CommandBlock {
    pub fn create(config: Config, value: &Value) -> Result<Box<Block>, Box<Error>> {
        let command = value.lookup("command").ok_or("Could not find command in a command module.")?;
        let command = command.as_str().ok_or("Command in command module is not a String.")?;
        let font_height = cmp::min(config.bar_height, config.font_height.unwrap());

        // Read mouse values from toml
        let mut hover_bg_col = config.bg.clone();
        let mut hover_fg_col = config.fg;
        let mut click_command = None;

        if let Some(hover_table) = value.lookup("mouse") {
            hover_bg_col = parse_input::toml_value_to_image(hover_table, "hover_bg")
                .unwrap_or(hover_bg_col);
            hover_fg_col = parse_input::toml_value_to_rgba(hover_table, "hover_fg")
                .unwrap_or(hover_fg_col);
            click_command = parse_input::toml_value_to_string(hover_table, "command").ok();
        }

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
            hover_bg_col: hover_bg_col,
            hover_fg_col: hover_fg_col,
            click_command: click_command,
            hover: false,
        }))
    }
}

impl Block for CommandBlock {
    fn start_interval(&mut self, interval_out: Sender<(Option<u32>, Option<MouseEvent>)>) {
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

    fn mouse_event(&mut self, mouse_event: Option<MouseEvent>) -> bool {
        if let Some(ref mouse_event) = mouse_event {
            if let Some(ButtonState::Released) = mouse_event.state {
                if let Some(ref command) = self.click_command {
                    let _ = Command::new("sh").arg("-c").arg(&command).spawn();
                }
            }
        }

        if self.hover != mouse_event.is_some() {
            self.hover = mouse_event.is_some();
            let mut cache_lock = self.cache.lock().unwrap(); // TODO: Not unwrap?
            *cache_lock = None;
            return true;
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

        let (fg_col, bg_col) = if self.hover {
            (self.hover_fg_col, &self.hover_bg_col)
        } else {
            (self.fg_col, &self.bg_col)
        };

        let mut text_block = TextBlock {
            bar_height: self.bar_height,
            font_height: self.font_height,
            font: self.font.clone(),
            bg_col: bg_col.clone(),
            fg_col: fg_col,
            text: text.to_string(),
            width: self.width,
            spacing: self.spacing,
            cache: None,
            hover_bg_col: self.hover_bg_col.clone(),
            hover_fg_col: self.hover_fg_col,
            click_command: self.click_command.clone(),
            hover: false,
        };

        let image = text_block.render()?;
        *cache = Some(image.clone());
        Ok(image)
    }
}
