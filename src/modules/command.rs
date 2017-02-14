use toml;
use image;
use std::cmp;
use rusttype;
use std::time;
use std::error;
use std::thread;
use std::process;
use std::sync::{self, mpsc};
use wayland_client::protocol::wl_pointer;

use mouse;
use modules;
use parse_input;
use modules::text;

pub struct CommandBlock {
    bar_height: u32,
    font_height: u32,
    font: rusttype::Font<'static>,
    bg_col: image::DynamicImage,
    fg_col: image::Rgba<u8>,
    width: u32,
    spacing: u32,
    command: String,
    interval: u32,
    cache: sync::Arc<sync::Mutex<Option<image::DynamicImage>>>,
    hover_bg_col: image::DynamicImage,
    hover_fg_col: image::Rgba<u8>,
    click_command: Option<String>,
    hover: bool,
}

// Unwraps cannot fail
impl CommandBlock {
    pub fn create(config: parse_input::Config,
                  value: &toml::Value)
                  -> Result<Box<modules::Block>, Box<error::Error>> {
        let command = value.lookup("command").ok_or("Could not find command in a command module.")?;
        let command = command.as_str()
            .ok_or("process::Command in command module is not a String.")?;
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
            cache: sync::Arc::new(sync::Mutex::new(None)),
            hover_bg_col: hover_bg_col,
            hover_fg_col: hover_fg_col,
            click_command: click_command,
            hover: false,
        }))
    }
}

impl modules::Block for CommandBlock {
    fn start_interval(&mut self,
                      interval_out: mpsc::Sender<(Option<u32>, Option<mouse::MouseEvent>)>) {
        if self.interval > 0 {
            let interval = self.interval as u64;
            let cache = self.cache.clone();
            thread::spawn(move || {
                loop {
                    thread::sleep(time::Duration::from_millis(interval));
                    let mut cache_lock = cache.lock().unwrap(); // TODO: Not unwrap?
                    *cache_lock = None;
                    interval_out.send((None, None)).unwrap(); // TODO: Not unwrap?
                }
            });
        }
    }

    fn mouse_event(&mut self, mouse_event: Option<mouse::MouseEvent>) -> bool {
        if let Some(ref mouse_event) = mouse_event {
            if let Some(wl_pointer::ButtonState::Released) = mouse_event.state {
                if let Some(ref command) = self.click_command {
                    let _ = process::Command::new("sh").arg("-c").arg(&command).spawn();
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

    fn render(&mut self) -> Result<image::DynamicImage, Box<error::Error>> {
        let mut cache = self.cache.lock().map_err(|e| e.to_string())?;
        if let Some(ref cache) = *cache {
            return Ok(cache.clone());
        }

        let output = process::Command::new("sh").arg("-c").arg(&self.command).output()?;
        let text = String::from_utf8_lossy(&output.stdout);

        let (fg_col, bg_col) = if self.hover {
            (self.hover_fg_col, &self.hover_bg_col)
        } else {
            (self.fg_col, &self.bg_col)
        };

        let mut text_block = text::TextBlock {
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
