use regex::Regex;
use std::io::Read;
use std::fs::File;
use std::{io, env};
use std::path::Path;
use std::boxed::Box;
use std::error::Error;
use std::num::ParseIntError;
use std::sync::mpsc::Sender;
use rusttype::{Font, FontCollection};
use image::{self, Rgba, DynamicImage, GenericImage};

use create_bar::{TextElement, ImageElement, Blockable, Alignment};

pub struct Settings {
    pub bar_height: i32,
    pub bg_col: DynamicImage,
    pub fg_col: Rgba<u8>,
    pub font: Font<'static>,
}

impl Clone for Settings {
    fn clone(&self) -> Settings {
        Settings {
            bar_height: self.bar_height,
            bg_col: self.bg_col.clone(),
            fg_col: self.fg_col,
            font: self.font.clone(),
        }
    }
}

pub fn read_stdin(settings: Settings,
                  stdin_out: &Sender<Vec<Box<Blockable>>>)
                  -> Result<(), Box<Error>> {
    loop {
        let mut buffer = String::new();
        if let Ok(out_length) = io::stdin().read_line(&mut buffer) {
            if out_length > 0 {
                let mut blank_bg = DynamicImage::new_rgba8(1, 1);
                blank_bg.put_pixel(0, 0, Rgba::<u8> { data: [0, 0, 0, 0] });
                stdin_out.send(parse_stdin(settings.fg_col, blank_bg, Alignment::LEFT, &buffer)?)?;
            }
        }
    }
}

pub fn get_settings() -> Settings {
    // TODO: ARGPARSE OR SOMETHING?
    let mut bg_col = DynamicImage::new_rgba8(1, 1);
    bg_col.put_pixel(0, 0, Rgba { data: [255, 0, 0, 255] });

    let font_file = File::open("./font.ttf").unwrap();
    let font_data = font_file.bytes().collect::<Result<Vec<u8>, io::Error>>().unwrap();
    let collection = FontCollection::from_bytes(font_data);
    let font = collection.into_font().ok_or("Invalid font type.".to_owned()).unwrap();

    Settings {
        bar_height: 30,
        bg_col: bg_col,
        fg_col: Rgba { data: [255, 0, 255, 255] },
        font: font,
    }
}

fn parse_stdin(fg_col: Rgba<u8>,
               bg_col: DynamicImage,
               alignment: Alignment,
               stdin: &str)
               -> Result<Vec<Box<Blockable>>, Box<Error>> {
    if stdin.is_empty() {
        return Ok(Vec::new());
    }

    if stdin.starts_with('{') && &stdin[1..2] != "{" {
        return parse_block(fg_col, bg_col, alignment, stdin);
    }

    let mut next_block_index = stdin.len();
    let block_match = Regex::new("[^\\{]\\{[^\\{]").unwrap().find(stdin);
    if block_match.is_some() {
        next_block_index = block_match.unwrap().start() + 1;
    }

    let element = TextElement {
        alignment: alignment,
        bg_col: bg_col.clone(),
        fg_col: fg_col,
        text: stdin[..next_block_index].to_owned(),
    };

    let mut elements: Vec<Box<Blockable>> =
        parse_stdin(fg_col, bg_col, alignment, &stdin[next_block_index..])?;
    elements.insert(0, Box::new(element));
    Ok(elements)
}

fn parse_block(fg_col: Rgba<u8>,
               bg_col: DynamicImage,
               alignment: Alignment,
               stdin: &str)
               -> Result<Vec<Box<Blockable>>, Box<Error>> {
    let mut next_text_index = stdin.len() - 1;
    let text_match = Regex::new("[^}]}[^}]").unwrap().find(stdin);
    if let Some(text_match) = text_match {
        next_text_index = text_match.start() + 1;
    }

    if let Some(alignment) = match &stdin[1..2] {
        "l" => Some(Alignment::LEFT),
        "c" => Some(Alignment::CENTER),
        "r" => Some(Alignment::RIGHT),
        _ => None,
    } {
        return parse_stdin(fg_col, bg_col, alignment, &stdin[next_text_index + 1..]);
    }

    match stdin[2..3].to_lowercase().as_ref() {
        "#" => {
            let rgba = string_to_rgba(&stdin[3..next_text_index])?;
            match stdin[1..2].to_lowercase().as_ref() {
                "b" => {
                    let mut bg_col = DynamicImage::new_rgba8(1, 1);
                    bg_col.put_pixel(0, 0, rgba);
                    parse_stdin(fg_col, bg_col, alignment, &stdin[next_text_index + 1..])
                }
                _ => {
                    parse_stdin(string_to_rgba(&stdin[3..next_text_index])?,
                                bg_col,
                                alignment,
                                &stdin[next_text_index + 1..])
                }
            }
        }
        _ => {
            let image = load_image(&stdin[2..next_text_index])?;
            match stdin[1..2].to_lowercase().as_ref() {
                "b" => parse_stdin(fg_col, image, alignment, &stdin[next_text_index + 1..]),
                _ => {
                    let element = ImageElement {
                        alignment: alignment,
                        bg_col: bg_col.clone(),
                        fg_col: image,
                    };
                    let mut elements =
                        parse_stdin(fg_col, bg_col, alignment, &stdin[next_text_index + 1..])?;
                    elements.insert(0, Box::new(element));
                    Ok(elements)
                }
            }
        }
    }
}

fn load_image(path: &str) -> Result<DynamicImage, Box<Error>> {
    let home_dir = env::home_dir().ok_or("Could not find home dir.".to_owned())?;
    let home_str = home_dir.to_string_lossy();
    let path = path.replace('~', &home_str).replace("$HOME", &home_str);
    Ok(image::open(&Path::new(&path))?)
}

fn string_to_rgba(col_string: &str) -> Result<Rgba<u8>, ParseIntError> {
    let red_string = col_string[..2].to_lowercase();
    let blue_string = col_string[4..6].to_lowercase();
    let green_string = col_string[2..4].to_lowercase();
    let alpha_string = {
        if col_string.len() > 6 {
            col_string[6..8].to_lowercase()
        } else {
            "ff".to_owned()
        }
    };

    let red = u8::from_str_radix(&red_string, 16)?;
    let blue = u8::from_str_radix(&blue_string, 16)?;
    let green = u8::from_str_radix(&green_string, 16)?;
    let alpha = u8::from_str_radix(&alpha_string, 16)?;

    Ok(Rgba { data: [red, green, blue, alpha] })
}

#[test]
fn stdin_parser_result_correct() {
    let mut default_col = DynamicImage::new_rgba8(1, 1);
    default_col.put_pixel(0, 0, Rgba::<u8> { data: [0, 0, 0, 0] });
    let stdin = "{B#ff00ff}TestString{F#00ff0000}{r}aaa{F#ffffffff}99{F#00000000}";

    let mut result = Vec::new();
    for r in parse_stdin(Rgba { data: [0, 0, 0, 0] },
                         default_col,
                         Alignment::LEFT,
                         stdin)
        .unwrap() {
        result.push(r.as_textelement());
    }

    assert_eq!(result.len(), 3);

    assert_eq!(result[0].bg_col.get_pixel(0, 0),
               Rgba { data: [255, 0, 255, 255] });
    assert_eq!(result[0].fg_col, Rgba { data: [0, 0, 0, 0] });
    assert_eq!(result[0].text, "TestString".to_owned());
    assert_eq!(result[0].alignment, Alignment::LEFT);

    assert_eq!(result[1].bg_col.get_pixel(0, 0),
               Rgba { data: [255, 0, 255, 255] });
    assert_eq!(result[1].fg_col, Rgba { data: [0, 255, 0, 0] });
    assert_eq!(result[1].text, "aaa".to_owned());
    assert_eq!(result[1].alignment, Alignment::RIGHT);

    assert_eq!(result[2].bg_col.get_pixel(0, 0),
               Rgba { data: [255, 0, 255, 255] });
    assert_eq!(result[2].fg_col, Rgba { data: [255, 255, 255, 255] });
    assert_eq!(result[2].text, "99".to_owned());
    assert_eq!(result[2].alignment, Alignment::RIGHT);
}

#[test]
fn stdin_parser_single_element() {
    let mut default_col = DynamicImage::new_rgba8(1, 1);
    default_col.put_pixel(0, 0, Rgba::<u8> { data: [0, 0, 0, 0] });

    let stdin = "{B#ff00ff}{F#00ff00}TEST1TEST2TEST3TEST4TEST5TEST6";
    let mut result = Vec::new();
    for r in parse_stdin(Rgba { data: [0, 0, 0, 0] },
                         default_col,
                         Alignment::LEFT,
                         stdin)
        .unwrap() {
        result.push(r.as_textelement());
    }

    assert_eq!(result.len(), 1);

    assert_eq!(result[0].bg_col.get_pixel(0, 0),
               Rgba { data: [255, 0, 255, 255] });
    assert_eq!(result[0].fg_col, Rgba { data: [0, 255, 0, 255] });
    assert_eq!(result[0].text, "TEST1TEST2TEST3TEST4TEST5TEST6".to_owned());
    assert_eq!(result[0].alignment, Alignment::LEFT);
}
