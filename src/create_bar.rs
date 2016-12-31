use tempfile;
use std::thread;
use std::fs::File;
use std::io::{self, Write, Read};
use byteorder::{WriteBytesExt, NativeEndian};
use std::sync::mpsc::{Sender, Receiver, channel};
use image::{GenericImage, Pixel, Rgba, DynamicImage, FilterType};
use rusttype::{FontCollection, Scale, PositionedGlyph, point};

use parse_input::{Settings, Element};

pub fn start_bar_creator(settings: &Settings,
                         bar_img_out: &Sender<File>,
                         resize_in: Receiver<i32>,
                         stdin_in: Receiver<Vec<Element>>)
                         -> Result<(), String> {
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
                                                           &settings.font);
                    bar_img_out.send(img_to_file(bar_img).map_err(|e| e.to_string())?)
                        .map_err(|e| e.to_string())?;
                }
            }
            Err(_) => return Err("Stdin or Resize channel disconnected.".to_owned()),
        };
    }
}

fn create_bar_from_elements(elements: &[Element],
                            bar_width: u32,
                            bar_height: u32,
                            bg_col: &DynamicImage,
                            font: &str)
                            -> DynamicImage {
    let mut bar_img = bg_col.clone().resize_exact(bar_width, bar_height, FilterType::Lanczos3);

    let mut rendered_elements = Vec::new();
    for element in elements {
        rendered_elements.push(render_block(&element.text,
                                            &element.fg_col,
                                            &element.bg_col,
                                            bar_height as f32,
                                            font));
    }

    let mut x_offset = 0;
    for element in rendered_elements {
        let (ele_width, ele_height) = element.dimensions();
        for x in 0..ele_width {
            if x + x_offset >= bar_width {
                break;
            }

            for y in 0..ele_height {
                let mut element_pixel = bar_img.get_pixel(x + x_offset, y);
                element_pixel.blend(&element.get_pixel(x, y));
                bar_img.put_pixel(x + x_offset, y, element_pixel);
            }
        }
        x_offset += ele_width;
    }
    bar_img
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

fn render_block(text: &str,
                fg_col: &Rgba<u8>,
                bg_col: &DynamicImage,
                height: f32,
                font_path: &str)
                -> DynamicImage {
    let text = text.replace('\n', "").replace('\r', "").replace('\t', "");

    let font_data: Vec<u8> = File::open(font_path).unwrap().bytes().map(|b| b.unwrap()).collect();
    let collection = FontCollection::from_bytes(font_data);
    let font = collection.into_font().unwrap();

    let scale = Scale {
        x: height,
        y: height,
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

    let mut image = bg_col.clone().resize_exact(width as u32, height as u32, FilterType::Lanczos3);

    // Render glyphs on top of background
    for glyph in glyphs {
        if let Some(bb) = glyph.pixel_bounding_box() {
            glyph.draw(|x, y, v| {
                let x = x + bb.min.x as u32;
                let y = y + bb.min.y as u32;
                let mut current_pixel = image.get_pixel(x, y);
                let mut pixel_col = *fg_col;
                pixel_col.data[3] = (v * 255.0) as u8;
                current_pixel.blend(&pixel_col);
                image.put_pixel(x, y, current_pixel);
            });
        }
    }

    image
}

#[test]
fn render_block_prevent_escape_sequences() {
    let result = render_block("TEXT\t\n\rx",
                              &Rgba { data: [255, 0, 255, 255] },
                              &Rgba { data: [255, 0, 255, 255] },
                              30.0,
                              "./src/font.ttf");
    assert_eq!(result.get_pixel(0, 0), Rgba { data: [255, 0, 255, 255] });
}
