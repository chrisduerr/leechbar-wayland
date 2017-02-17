use toml;
use std::cmp;
use rusttype;
use std::error;
use std::process;
use std::sync::mpsc;
use image::{self, GenericImage, Pixel};

use mouse;
use modules;
use parse_input;

pub struct TextBlock {
    pub bar_height: u32,
    pub font_height: u32,
    pub font: rusttype::Font<'static>,
    pub bg_col: image::DynamicImage,
    pub fg_col: image::Rgba<u8>,
    pub text: String,
    pub width: u32,
    pub spacing: u32,
    pub cache: Option<image::DynamicImage>,
    pub hover_bg_col: image::DynamicImage,
    pub hover_fg_col: image::Rgba<u8>,
    pub click_command: Option<String>,
    pub hover: bool,
}

// Unwraps cannot fail
impl TextBlock {
    pub fn create(config: parse_input::Config,
                  value: &toml::Value)
                  -> Result<Box<modules::Block>, Box<error::Error>> {
        let text = value.lookup("text").ok_or("Could not find text in a text module.")?;
        let text = text.as_str().ok_or("Text in text module is not a String.")?;
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

        Ok(Box::new(TextBlock {
            bar_height: config.bar_height,
            font_height: font_height,
            font: config.font.unwrap(),
            bg_col: config.bg,
            fg_col: config.fg,
            text: text.to_owned(),
            width: config.width,
            spacing: config.spacing,
            cache: None,
            hover_bg_col: hover_bg_col,
            hover_fg_col: hover_fg_col,
            click_command: click_command,
            hover: false,
        }))
    }
}

impl modules::Block for TextBlock {
    fn start_interval(&mut self,
                      _interval_out: mpsc::Sender<(Option<u32>, Option<mouse::MouseEvent>)>) {
        // TextBlock is never updated
    }

    fn mouse_event(&mut self, mouse_event: Option<mouse::MouseEvent>) -> bool {
        if let Some(ref mouse_event) = mouse_event {
            if let Some(mouse::ButtonState::RELEASED) = mouse_event.state {
                if let Some(ref command) = self.click_command {
                    let _ = process::Command::new("sh").arg("-c").arg(&command).spawn();
                }
            }
        }

        if self.hover != mouse_event.is_some() {
            self.hover = mouse_event.is_some();
            self.cache = None;
            return true;
        }

        false
    }

    fn render(&mut self) -> Result<image::DynamicImage, Box<error::Error>> {
        if let Some(ref cache) = self.cache {
            return Ok(cache.clone());
        }

        let (fg_col, bg_col) = if self.hover {
            (&self.hover_fg_col, &self.hover_bg_col)
        } else {
            (&self.fg_col, &self.bg_col)
        };

        let text = self.text.replace('\n', "").replace('\r', "").replace('\t', "");

        let scale = rusttype::Scale {
            x: self.font_height as f32,
            y: self.font_height as f32,
        };

        let v_metrics = self.font.v_metrics(scale);
        let offset = rusttype::point(0.0, v_metrics.ascent);

        let glyphs: Vec<rusttype::PositionedGlyph> = self.font
            .layout(&text, scale, offset)
            .collect();

        // Find the most visually pleasing width to display -> No idea what's going on exactly
        let mut width = glyphs.iter()
            .rev()
            .filter_map(|g| {
                g.pixel_bounding_box()
                    .map(|b| b.min.x as f32 + g.unpositioned().h_metrics().advance_width)
            })
            .next()
            .unwrap_or(0.0)
            .ceil() as u32;

        let mut x_offset = self.spacing;
        let y_offset = (self.bar_height - self.font_height) / 2;
        if width < self.width {
            x_offset += (self.width - width) / 2;
            width = self.width;
        }
        width += self.spacing * 2;

        let mut image = image::DynamicImage::new_rgba8(width, self.bar_height);
        for x in 0..width {
            for y in 0..self.bar_height {
                let bgcol_x = x % bg_col.width();
                let bgcol_y = y % bg_col.height();
                image.put_pixel(x, y, bg_col.get_pixel(bgcol_x, bgcol_y));
            }
        }

        // Render glyphs on top of background
        for glyph in glyphs {
            if let Some(bb) = glyph.pixel_bounding_box() {
                glyph.draw(|x, y, v| {
                    let x = x + bb.min.x as u32;
                    let y = y + bb.min.y as u32;
                    let mut current_pixel = image.get_pixel(x + x_offset, y + y_offset);
                    let mut pixel_col = *fg_col;
                    pixel_col.data[3] = (v * 255.0) as u8;
                    current_pixel.blend(&pixel_col);
                    image.put_pixel(x + x_offset, y + y_offset, current_pixel);
                });
            }
        }

        self.cache = Some(image.clone());
        Ok(image)
    }
}
