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
    pub fg_col: Rgba<u8>,
    pub font: String,
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

pub fn read_stdin(stdin_out: &Sender<Vec<Element>>) -> Result<(), Box<Error>> {
    loop {
        let mut buffer = String::new();
        if let Ok(out_length) = io::stdin().read_line(&mut buffer) {
            if out_length > 0 {
                stdin_out.send(parse_stdin(&buffer)?)?;
            }
        }
    }
}

pub fn get_settings() -> Settings {
    // TODO: ARGPARSE OR SOMETHING?
    let mut bg_col = DynamicImage::new_rgba8(1, 1);
    bg_col.put_pixel(0, 0, Rgba { data: [255, 0, 0, 255] });
    Settings {
        bar_height: 30,
        bg_col: bg_col,
        fg_col: Rgba { data: [61, 61, 61, 255] },
        font: "./src/font.ttf".to_owned(),
    }
}

fn parse_stdin(stdin: &str) -> Result<Vec<Element>, Box<Error>> {
    let mut elements = Vec::new();

    let mut current_bg_col = DynamicImage::new_rgba8(1, 1);
    let mut current_fg_col = Rgba { data: [0, 0, 0, 0] };
    let mut current_text = String::new();

    let mut setting_start_index = -1;
    let mut skip_char = false;

    // Iterate over each character in the string extracting settings
    // whenever a single { is found until a single } is found
    for (i, character) in stdin.chars().enumerate() {
        current_text.push(character);

        if skip_char {
            skip_char = false;
            continue;
        } else if character == '{' && setting_start_index == -1 {
            if stdin.len() >= (i + 2) && &stdin[i + 1..i + 2] == "{" {
                skip_char = true;
            } else {
                setting_start_index = i as i32 + 1;
            }
        } else if character == '}' && setting_start_index != -1 {
            if stdin.len() >= (i + 2) && &stdin[i + 1..i + 2] == "}" {
                skip_char = true;
            } else {
                let setting_end_index = i as i32;
                let setting_str = &stdin[setting_start_index as usize..setting_end_index as usize];

                current_text = current_text[..current_text.len() - (setting_str.len() + 2)]
                    .to_owned();
                if !current_text.is_empty() {
                    elements.push(Element {
                        bg_col: current_bg_col.clone(),
                        fg_col: current_fg_col,
                        text: current_text.to_owned(),
                    });
                }

                match setting_str[0..1].to_lowercase().as_ref() {
                    "b" => {
                        if &setting_str[1..2] == "#" {
                            current_bg_col.put_pixel(0, 0, string_to_rgba(setting_str)?);
                        } else {
                            let home_dir =
                                env::home_dir().ok_or("Could not find home dir.".to_owned())?;
                            let home_str = home_dir.to_string_lossy();
                            let path = setting_str[1..]
                                .replace('~', &home_str)
                                .replace("$HOME", &home_str);
                            current_bg_col = image::open(&Path::new(&path))?;
                        }
                    }
                    "f" => current_fg_col = string_to_rgba(setting_str)?,
                    _ => (),
                };

                current_text = String::new();
                setting_start_index = -1;
            }
        }
    }

    if !current_text.is_empty() {
        elements.push(Element {
            bg_col: current_bg_col,
            fg_col: current_fg_col,
            text: current_text.to_owned(),
        });
    }

    Ok(elements)
}

fn string_to_rgba(col_string: &str) -> Result<Rgba<u8>, ParseIntError> {
    let red_string = col_string[2..4].to_lowercase();
    let blue_string = col_string[6..8].to_lowercase();
    let green_string = col_string[4..6].to_lowercase();
    let alpha_string = {
        if col_string.len() > 8 {
            col_string[8..10].to_lowercase()
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
    let stdin = "{B#ff00ff}TestString{F#00ff0000}aaa{F#ffffffff}99{F#00000000}";
    let result = parse_stdin(stdin).unwrap();

    assert_eq!(result.len(), 3);

    assert_eq!(result[0].bg_col.get_pixel(0, 0),
               Rgba { data: [255, 0, 255, 255] });
    assert_eq!(result[0].fg_col, Rgba { data: [0, 0, 0, 0] });
    assert_eq!(result[0].text, "TestString".to_owned());

    assert_eq!(result[1].bg_col.get_pixel(0, 0),
               Rgba { data: [255, 0, 255, 255] });
    assert_eq!(result[1].fg_col, Rgba { data: [0, 255, 0, 0] });
    assert_eq!(result[1].text, "aaa".to_owned());

    assert_eq!(result[2].bg_col.get_pixel(0, 0),
               Rgba { data: [255, 0, 255, 255] });
    assert_eq!(result[2].fg_col, Rgba { data: [255, 255, 255, 255] });
    assert_eq!(result[2].text, "99".to_owned());
}

#[test]
fn stdin_parser_single_element() {
    let stdin = "{B#ff00ff}{F#00ff00}TEST1TEST2TEST3TEST4TEST5TEST6";
    let result = parse_stdin(stdin).unwrap();

    assert_eq!(result.len(), 1);

    assert_eq!(result[0].bg_col.get_pixel(0, 0),
               Rgba { data: [255, 0, 255, 255] });
    assert_eq!(result[0].fg_col, Rgba { data: [0, 255, 0, 255] });
    assert_eq!(result[0].text, "TEST1TEST2TEST3TEST4TEST5TEST6".to_owned());
}
