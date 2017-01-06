use tempfile;
use std::fs::File;
use std::error::Error;
use std::{thread, cmp};
use std::io::{self, Write};
use byteorder::{WriteBytesExt, NativeEndian};
use std::sync::mpsc::{Sender, Receiver, channel};
use image::{GenericImage, Pixel, Rgba, DynamicImage, FilterType};
use rusttype::{Scale, PositionedGlyph, point, Font};

use parse_input::Settings;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Alignment {
    LEFT,
    CENTER,
    RIGHT,
}

pub trait Blockable: Send + 'static {
    fn render(&self, height: u32, font: &Font) -> Result<DynamicImage, Box<Error>>;
    fn alignment(&self) -> Alignment;
    #[cfg(test)]
    fn as_textelement(&self) -> TextElement;
}

pub struct TextElement {
    pub alignment: Alignment,
    pub bg_col: DynamicImage,
    pub fg_col: Rgba<u8>,
    pub text: String,
}

impl Clone for TextElement {
    fn clone(&self) -> TextElement {
        TextElement {
            alignment: self.alignment,
            bg_col: self.bg_col.clone(),
            fg_col: self.fg_col,
            text: self.text.clone(),
        }
    }
}

impl Blockable for TextElement {
    fn render(&self, height: u32, font: &Font) -> Result<DynamicImage, Box<Error>> {
        let text = self.text.replace('\n', "").replace('\r', "").replace('\t', "");

        let scale = Scale {
            x: height as f32,
            y: height as f32,
        };

        let v_metrics = font.v_metrics(scale);
        let offset = point(0.0, v_metrics.ascent);

        let glyphs: Vec<PositionedGlyph> = font.layout(&text, scale, offset)
            .collect();

        // Find the most visually pleasing width to display -> No idea what's going on exactly
        let width = glyphs.iter()
            .rev()
            .filter_map(|g| {
                g.pixel_bounding_box()
                    .map(|b| b.min.x as f32 + g.unpositioned().h_metrics().advance_width)
            })
            .next()
            .unwrap_or(0.0)
            .ceil() as usize;

        let mut image = DynamicImage::new_rgba8(width as u32, height);
        for x in 0..cmp::min(self.bg_col.width(), width as u32) {
            for y in 0..cmp::min(self.bg_col.height(), height) {
                image.put_pixel(x, y, self.bg_col.get_pixel(x, y));
            }
        }

        // Render glyphs on top of background
        for glyph in glyphs {
            if let Some(bb) = glyph.pixel_bounding_box() {
                glyph.draw(|x, y, v| {
                    let x = x + bb.min.x as u32;
                    let y = y + bb.min.y as u32;
                    let mut current_pixel = image.get_pixel(x, y);
                    let mut pixel_col = self.fg_col;
                    pixel_col.data[3] = (v * 255.0) as u8;
                    current_pixel.blend(&pixel_col);
                    image.put_pixel(x, y, current_pixel);
                });
            }
        }

        Ok(image)
    }

    fn alignment(&self) -> Alignment {
        self.alignment
    }

    #[cfg(test)]
    fn as_textelement(&self) -> TextElement {
        self.clone()
    }
}

pub struct ImageElement {
    pub alignment: Alignment,
    pub bg_col: DynamicImage,
    pub fg_col: DynamicImage,
}

impl Clone for ImageElement {
    fn clone(&self) -> ImageElement {
        ImageElement {
            alignment: self.alignment,
            bg_col: self.bg_col.clone(),
            fg_col: self.fg_col.clone(),
        }
    }
}

impl Blockable for ImageElement {
    fn render(&self, height: u32, _font: &Font) -> Result<DynamicImage, Box<Error>> {
        let mut image = DynamicImage::new_rgba8(self.fg_col.width(), height);
        for x in 0..cmp::min(self.bg_col.width(), self.fg_col.width()) {
            for y in 0..cmp::min(self.bg_col.height(), height) {
                image.put_pixel(x, y, self.bg_col.get_pixel(x, y));
            }
        }

        for x in 0..self.fg_col.width() {
            for y in 0..cmp::min(self.fg_col.height(), height) {
                let mut current_pixel = image.get_pixel(x, y);
                current_pixel.blend(&self.fg_col.get_pixel(x, y));
                image.put_pixel(x, y, current_pixel);
            }
        }

        Ok(image)
    }

    fn alignment(&self) -> Alignment {
        self.alignment
    }

    #[cfg(test)]
    fn as_textelement(&self) -> TextElement {
        panic!("This is not a TextElement.");
    }
}

pub fn start_bar_creator(settings: Settings,
                         bar_img_out: &Sender<File>,
                         resize_in: Receiver<i32>,
                         stdin_in: Receiver<Vec<Box<Blockable>>>)
                         -> Result<(), Box<Error>> {
    let mut output_width = 0;
    let mut bar_elements = Vec::new();
    let (combined_out, combined_in) = channel();

    {
        let combined_out = combined_out.clone();
        thread::spawn(move || {
            while let Ok(output_width) = resize_in.recv() {
                if combined_out.send((Some(output_width), None)).is_err() {
                    break;
                }
            }
        });
    }

    {
        let combined_out = combined_out.clone();
        thread::spawn(move || {
            while let Ok(elements) = stdin_in.recv() {
                if combined_out.send((None, Some(elements))).is_err() {
                    break;
                }
            }
        });
    }

    loop {
        match combined_in.recv() {
            Ok((width, elements)) => {
                if let Some(width) = width {
                    output_width = width;
                } else if let Some(elements) = elements {
                    bar_elements = elements;
                }

                if output_width > 0 && !bar_elements.is_empty() {
                    let bar_img = create_bar_from_elements(&bar_elements,
                                                           output_width as u32,
                                                           settings.bar_height as u32,
                                                           &settings.bg_col,
                                                           &settings.font)?;
                    bar_img_out.send(img_to_file(bar_img)?)?;
                }
            }
            Err(_) => Err("Stdin or Resize channel disconnected.".to_owned())?,
        };
    }
}

fn create_bar_from_elements(elements: &[Box<Blockable>],
                            bar_width: u32,
                            bar_height: u32,
                            bg_col: &DynamicImage,
                            font: &Font)
                            -> Result<DynamicImage, Box<Error>> {
    let mut bar_img = bg_col.resize_exact(bar_width, bar_height, FilterType::Triangle);

    let mut left_elements = Vec::new();
    let mut center_elements = Vec::new();
    let mut right_elements = Vec::new();
    for element in elements {
        match element.alignment() {
            Alignment::LEFT => left_elements.push(element.render(bar_height, font)?),
            Alignment::CENTER => center_elements.push(element.render(bar_height, font)?),
            Alignment::RIGHT => right_elements.push(element.render(bar_height, font)?),
        }
    }

    if let Some(mut left_image) = combine_elements(&left_elements) {
        combine_images(&mut bar_img, &mut left_image, 0);
    }

    if let Some(mut center_image) = combine_elements(&center_elements) {
        let offset = bar_width / 2 - center_image.width() / 2;
        combine_images(&mut bar_img, &mut center_image, offset);
    }

    if let Some(mut right_image) = combine_elements(&right_elements) {
        let offset = bar_width - right_image.width();
        combine_images(&mut bar_img, &mut right_image, offset);
    }

    Ok(bar_img)
}

// Draws the second image on top of the first one with an x-offset
fn combine_images(first: &mut DynamicImage, second: &DynamicImage, offset: u32) {
    for x in offset..cmp::min(offset + second.width(), first.width()) {
        for y in 0..second.height() {
            let mut pixel = first.get_pixel(x, y);
            pixel.blend(&second.get_pixel(x - offset, y));
            first.put_pixel(x, y, pixel);
        }
    }
}

fn combine_elements(elements: &[DynamicImage]) -> Option<DynamicImage> {
    if elements.is_empty() {
        None
    } else {
        let width = elements.iter().map(|img| img.width()).sum();
        let height = elements[0].height();
        let mut img = DynamicImage::new_rgba8(width, height);

        let mut offset = 0;
        for element in elements {
            combine_images(&mut img, &element, offset);
            offset += element.width();
        }

        Some(img)
    }
}

fn img_to_file(img: DynamicImage) -> Result<File, io::Error> {
    let mut tmp = tempfile::tempfile()?;

    for pixel in img.pixels() {
        let channels = pixel.2.channels();
        if channels.len() == 4 {
            let _ = tmp.write_u32::<NativeEndian>((0xFF << 24) + ((channels[0] as u32) << 16) +
                                                  ((channels[1] as u32) << 8) +
                                                  channels[2] as u32);
        }
    }

    let _ = tmp.flush();
    Ok(tmp)
}

#[cfg(test)]
use parse_input;
#[test]
fn render_block_prevent_escape_sequences() {
    let mut col = DynamicImage::new_rgba8(1, 1);
    col.put_pixel(0, 0, Rgba { data: [255, 0, 255, 255] });
    let block = ImageElement {
        alignment: Alignment::LEFT,
        bg_col: col.clone(),
        fg_col: col,
    };
    let font = parse_input::get_settings().font;

    let result = block.render(30, &font).unwrap();
    assert_eq!(result.get_pixel(0, 0), Rgba { data: [255, 0, 255, 255] });
}
