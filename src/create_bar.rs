use tempfile;
use std::fs::File;
use std::error::Error;
use std::{thread, cmp};
use std::io::{self, Write};
use byteorder::{WriteBytesExt, NativeEndian};
use std::sync::mpsc::{Sender, Receiver, channel};
use image::{ImageFormat, GenericImage, Pixel, DynamicImage};

use modules::Block;
use mouse::MouseEvent;
use parse_input::{self, Config};

// TODO: Look for actual changes in modules before requesting redraw!
pub fn start_bar_creator(bar_img_out: Sender<(File, i32)>,
                         resize_in: Receiver<u32>,
                         mouse_in: Receiver<MouseEvent>)
                         -> Result<(), Box<Error>> {
    let mut output_width = 0;
    let mut bg_img = DynamicImage::new_rgba8(0, 0);
    let (combined_out, combined_in) = channel();

    let mut config = parse_input::read_config()?;

    // Start interval notification callback for every block
    // This will spawn threads inside the start_interval methods
    for element in config.left_blocks
        .iter_mut()
        .chain(config.center_blocks.iter_mut())
        .chain(config.right_blocks.iter_mut()) {
        element.start_interval(combined_out.clone());
    }

    // Combine interval with output_width
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

    // Add mouse events to combined channel
    {
        let combined_out = combined_out.clone();
        thread::spawn(move || {
            while let Ok(event) = mouse_in.recv() {
                if combined_out.send((None, Some(event))).is_err() {
                    break;
                }
            }
        });
    }

    loop {
        match combined_in.recv() {
            Ok((width, mouse_event)) => {
                if let Some(width) = width {
                    if width != output_width {
                        bg_img = config.bg.crop(0, 0, width, config.bar_height);
                        output_width = width;
                    }
                } else if let Some(mouse_event) = mouse_event {
                    if !propagate_mouse_events(&mut config, output_width, mouse_event)? {
                        continue;
                    }
                }

                if output_width > 0 {
                    let bar = create_bar_from_config(&mut config, bg_img.clone(), output_width)?;
                    bar_img_out.send((img_to_file(bar)?, config.bar_height as i32))?;
                }
            }
            Err(_) => Err("Config or Resize channel disconnected.".to_owned())?,
        };
    }
}

fn propagate_mouse_events(config: &mut Config,
                          bar_width: u32,
                          mut mouse_event: MouseEvent)
                          -> Result<bool, Box<Error>> {
    let event_x = mouse_event.x as u32;

    let last_left_block_index = config.left_blocks.len();
    let last_center_block_index = last_left_block_index + config.center_blocks.len();

    let center_images = render_blocks(&mut config.center_blocks)?;
    let right_images = render_blocks(&mut config.right_blocks)?;

    let center_blocks_width: u32 = center_images.iter().map(|i| i.width()).sum();
    let center_offset_x = bar_width / 2 - center_blocks_width / 2;

    let right_blocks_width: u32 = right_images.iter().map(|i| i.width()).sum();
    let right_offset_x = bar_width - right_blocks_width;

    let mut redraw = false;

    let mut offset = 0;
    for (i, block) in config.left_blocks
        .iter_mut()
        .enumerate()
        .chain(config.center_blocks
            .iter_mut()
            .enumerate()
            .map(|(i, b)| (i + last_left_block_index, b)))
        .chain(config.right_blocks
            .iter_mut()
            .enumerate()
            .map(|(i, b)| (i + last_center_block_index, b))) {
        let image = block.render()?;
        let block_right = offset + image.width();

        if event_x >= offset && event_x <= block_right {
            mouse_event.x -= offset as f64;
            if block.mouse_event(Some(mouse_event.clone())) {
                redraw = true;
            }
        } else if block.mouse_event(None) {
            redraw = true;
        }

        if i == last_left_block_index - 1 {
            offset = center_offset_x;
        } else if i == last_center_block_index - 1 {
            offset = right_offset_x;
        } else {
            offset += block_right;
        }
    }

    Ok(redraw)
}

fn render_blocks(blocks: &mut [Box<Block>]) -> Result<Vec<DynamicImage>, Box<Error>> {
    Ok(blocks.iter_mut()
        .map(|block| block.render())
        .collect::<Result<Vec<DynamicImage>, Box<Error>>>()?)
}

fn create_bar_from_config(config: &mut Config,
                          mut bg_img: DynamicImage,
                          bar_width: u32)
                          -> Result<DynamicImage, Box<Error>> {
    if let Some(left_image) = combine_elements(&mut config.left_blocks, config.bar_height)? {
        combine_images(&mut bg_img, &left_image, 0);
    }

    if let Some(center_image) = combine_elements(&mut config.center_blocks, config.bar_height)? {
        let offset = bar_width / 2 - center_image.width() / 2;
        combine_images(&mut bg_img, &center_image, offset);
    }

    if let Some(right_image) = combine_elements(&mut config.right_blocks, config.bar_height)? {
        let offset = bar_width - right_image.width();
        combine_images(&mut bg_img, &right_image, offset);
    }

    Ok(bg_img)
}

fn combine_elements(blocks: &mut [Box<Block>],
                    bar_height: u32)
                    -> Result<Option<DynamicImage>, Box<Error>> {
    if blocks.is_empty() {
        Ok(None)
    } else {
        let images = render_blocks(blocks)?;
        let width = images.iter().map(|img| img.width()).sum();
        let mut result_img = DynamicImage::new_rgba8(width, bar_height);

        let mut offset = 0;
        for image in images {
            combine_images(&mut result_img, &image, offset);
            offset += image.width();
        }

        Ok(Some(result_img))
    }
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

fn img_to_file(img: DynamicImage) -> Result<File, io::Error> {
    let mut tmp = tempfile::tempfile()?;

    let mut bytes = Vec::new();
    for pixel in img.pixels() {
        let channels = pixel.2.channels();
        if channels.len() == 4 {
            bytes.push(channels[2]);    // Blue
            bytes.push(channels[1]);    // Green
            bytes.push(channels[0]);    // Red
            bytes.push(channels[3]);    // Transparency
        }
    }

    let _ = tmp.write(&bytes);
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
        bg_col: col.clone(),
        fg_col: col,
    };
    let font = parse_input::get_settings().font;

    let result = block.render(30, &font).unwrap();
    assert_eq!(result.get_pixel(0, 0), Rgba { data: [255, 0, 255, 255] });
}
