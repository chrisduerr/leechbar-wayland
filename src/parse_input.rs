use regex::Regex;
use std::{io, env};
use std::error::Error;
use std::path::Path;
use std::num::ParseIntError;
use std::sync::mpsc::Sender;
use image::{self, Rgba, DynamicImage, GenericImage};

use create_bar::Element;

pub struct Settings {
    pub bar_height: i32,
    pub bg_col: DynamicImage,
    pub fg_col: DynamicImage,
    pub font: String,
}

impl Clone for Settings {
    fn clone(&self) -> Settings {
        Settings {
            bar_height: self.bar_height,
            bg_col: self.bg_col.clone(),
            fg_col: self.fg_col.clone(),
            font: self.font.clone(),
        }
    }
}

pub fn read_stdin(settings: Settings, stdin_out: &Sender<Vec<Element>>) -> Result<(), Box<Error>> {
    loop {
        let mut buffer = String::new();
        if let Ok(out_length) = io::stdin().read_line(&mut buffer) {
            if out_length > 0 {
                let mut blank_bg = DynamicImage::new_rgba8(1, 1);
                blank_bg.put_pixel(0, 0, Rgba::<u8> { data: [0, 0, 0, 0] });
                stdin_out.send(parse_stdin(settings.fg_col.clone(), blank_bg, &buffer)?)?;
            }
        }
    }
}

pub fn get_settings() -> Settings {
    // TODO: ARGPARSE OR SOMETHING?
    let mut bg_col = DynamicImage::new_rgba8(1, 1);
    bg_col.put_pixel(0, 0, Rgba { data: [255, 0, 0, 255] });
    let mut fg_col = DynamicImage::new_rgba8(1, 1);
    fg_col.put_pixel(0, 0, Rgba { data: [255, 0, 255, 255] });
    Settings {
        bar_height: 30,
        bg_col: bg_col,
        fg_col: fg_col,
        font: "./src/font.ttf".to_owned(),
    }
}

fn parse_stdin(fg_col: DynamicImage,
               bg_col: DynamicImage,
               stdin: &str)
               -> Result<Vec<Element>, Box<Error>> {
    if stdin.is_empty() {
        return Ok(Vec::new());
    }

    if stdin.starts_with('{') && &stdin[1..2] != "{" {
        let mut next_text_index = stdin.len() - 1;
        let text_match = Regex::new("[^}]}[^}]").unwrap().find(stdin);
        if let Some(text_match) = text_match {
            next_text_index = text_match.start() + 1;
        }

        match stdin[1..2].to_lowercase().as_ref() {
            "b" => {
                let bg_col = path_or_color_to_img(&stdin[2..next_text_index])?;
                return parse_stdin(fg_col, bg_col, &stdin[next_text_index + 1..]);
            }
            "f" => {
                let fg_col = path_or_color_to_img(&stdin[2..next_text_index])?;
                return parse_stdin(fg_col, bg_col, &stdin[next_text_index + 1..]);
            }
            _ => return Err("Invalid stdin configuration.".to_owned())?,
        }
    }

    let mut next_block_index = stdin.len();
    let block_match = Regex::new("[^\\{]\\{[^\\{]").unwrap().find(stdin);
    if block_match.is_some() {
        next_block_index = block_match.unwrap().start() + 1;
    }

    let element = Element {
        bg_col: bg_col.clone(),
        fg_col: fg_col.clone(),
        text: stdin[..next_block_index].to_owned(),
    };

    let mut elements = parse_stdin(fg_col, bg_col, &stdin[next_block_index..])?;
    elements.insert(0, element);
    Ok(elements)
}

fn path_or_color_to_img(text: &str) -> Result<DynamicImage, Box<Error>> {
    match text[0..1].to_lowercase().as_ref() {
        "#" => Ok(string_to_rgba_image(&text[1..])?),
        _ => Ok(load_image(text)?),
    }
}

fn load_image(path: &str) -> Result<DynamicImage, Box<Error>> {
    let home_dir = env::home_dir().ok_or("Could not find home dir.".to_owned())?;
    let home_str = home_dir.to_string_lossy();
    let path = path.replace('~', &home_str).replace("$HOME", &home_str);
    Ok(image::open(&Path::new(&path))?)
}

fn string_to_rgba_image(col_string: &str) -> Result<DynamicImage, ParseIntError> {
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

    let mut image = DynamicImage::new_rgba8(1, 1);
    image.put_pixel(0, 0, Rgba { data: [red, green, blue, alpha] });
    Ok(image)
}

#[test]
fn stdin_parser_result_correct() {
    let mut default_col = DynamicImage::new_rgba8(1, 1);
    default_col.put_pixel(0, 0, Rgba::<u8> { data: [0, 0, 0, 0] });

    let stdin = "{B#ff00ff}TestString{F#00ff0000}aaa{F#ffffffff}99{F#00000000}";
    let result = parse_stdin(default_col.clone(), default_col, stdin).unwrap();

    assert_eq!(result.len(), 3);

    assert_eq!(result[0].bg_col.get_pixel(0, 0),
               Rgba { data: [255, 0, 255, 255] });
    assert_eq!(result[0].fg_col.get_pixel(0, 0),
               Rgba { data: [0, 0, 0, 0] });
    assert_eq!(result[0].text, "TestString".to_owned());

    assert_eq!(result[1].bg_col.get_pixel(0, 0),
               Rgba { data: [255, 0, 255, 255] });
    assert_eq!(result[1].fg_col.get_pixel(0, 0),
               Rgba { data: [0, 255, 0, 0] });
    assert_eq!(result[1].text, "aaa".to_owned());

    assert_eq!(result[2].bg_col.get_pixel(0, 0),
               Rgba { data: [255, 0, 255, 255] });
    assert_eq!(result[2].fg_col.get_pixel(0, 0),
               Rgba { data: [255, 255, 255, 255] });
    assert_eq!(result[2].text, "99".to_owned());
}

#[test]
fn stdin_parser_single_element() {
    let mut default_col = DynamicImage::new_rgba8(1, 1);
    default_col.put_pixel(0, 0, Rgba::<u8> { data: [0, 0, 0, 0] });

    let stdin = "{B#ff00ff}{F#00ff00}TEST1TEST2TEST3TEST4TEST5TEST6";
    let result = parse_stdin(default_col.clone(), default_col, stdin).unwrap();

    assert_eq!(result.len(), 1);

    assert_eq!(result[0].bg_col.get_pixel(0, 0),
               Rgba { data: [255, 0, 255, 255] });
    assert_eq!(result[0].fg_col.get_pixel(0, 0),
               Rgba { data: [0, 255, 0, 255] });
    assert_eq!(result[0].text, "TEST1TEST2TEST3TEST4TEST5TEST6".to_owned());
}
