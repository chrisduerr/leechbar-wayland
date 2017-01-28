use std::cmp;
use toml::Value;
use std::error::Error;
use std::sync::{Arc, Mutex};
use rusttype::{Font, Scale, point, PositionedGlyph};
use image::{DynamicImage, GenericImage, Rgba, Pixel};

use modules::Block;
use parse_input::Config;

pub struct TextBlock {
    pub bar_height: u32,
    pub font_height: u32,
    pub font: Font<'static>,
    pub bg_col: DynamicImage,
    pub fg_col: Rgba<u8>,
    pub text: String,
    pub width: u32,
    pub spacing: u32,
    pub cache: Option<DynamicImage>,
}

// Unwraps cannot fail
impl TextBlock {
    pub fn new(config: Config, value: &Value) -> Result<Arc<Mutex<Block>>, Box<Error>> {
        let text = value.lookup("text").ok_or("Could not find text in a text module.")?;
        let text = text.as_str().ok_or("Text in text module is not a String.")?;
        let font_height = cmp::min(config.bar_height, config.font_height.unwrap());
        Ok(Arc::new(Mutex::new(TextBlock {
            bar_height: config.bar_height,
            font_height: font_height,
            font: config.font.unwrap(),
            bg_col: config.bg,
            fg_col: config.fg,
            text: text.to_owned(),
            width: config.width,
            spacing: config.spacing,
            cache: None,
        })))
    }
}

impl Block for TextBlock {
    fn render(&mut self) -> Result<DynamicImage, Box<Error>> {
        if let Some(ref cache) = self.cache {
            return Ok(cache.clone());
        }

        let text = self.text.replace('\n', "").replace('\r', "").replace('\t', "");

        let scale = Scale {
            x: self.font_height as f32,
            y: self.font_height as f32,
        };

        let v_metrics = self.font.v_metrics(scale);
        let offset = point(0.0, v_metrics.ascent);

        let glyphs: Vec<PositionedGlyph> = self.font
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

        let mut image = DynamicImage::new_rgba8(width, self.bar_height);
        for x in 0..width {
            for y in 0..self.bar_height {
                let bgcol_x = x % self.bg_col.width();
                let bgcol_y = y % self.bg_col.height();
                image.put_pixel(x, y, self.bg_col.get_pixel(bgcol_x, bgcol_y));
            }
        }

        // Render glyphs on top of background
        for glyph in glyphs {
            if let Some(bb) = glyph.pixel_bounding_box() {
                glyph.draw(|x, y, v| {
                    let x = x + bb.min.x as u32;
                    let y = y + bb.min.y as u32;
                    let mut current_pixel = image.get_pixel(x + x_offset, y + y_offset);
                    let mut pixel_col = self.fg_col;
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
