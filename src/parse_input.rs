use toml;
use std::fs;
use rusttype;
use std::num;
use std::path;
use std::error;
use std::io::Read;
use std::{io, env};
use std::boxed::Box;
use image::{self, GenericImage};

use modules::{MODULES, Block};

pub struct Config {
    // Defaults for each element:
    pub bg: image::DynamicImage,
    pub fg: image::Rgba<u8>,
    pub font: Option<rusttype::Font<'static>>,
    pub font_height: Option<u32>,
    pub resize: bool, // TODO: Currently never used by anything
    pub width: u32,
    pub spacing: u32,
    pub interval: u32,

    // Exclusive to bar:
    pub bar_height: u32,
    pub top: bool, // TODO: Currently not implemented in Wayland
    pub left_blocks: Vec<Box<Block>>,
    pub center_blocks: Vec<Box<Block>>,
    pub right_blocks: Vec<Box<Block>>,
}

// Blocks are dropped on clone since this is never needed after a clone
// If needed they need to be wrapped in Arc<Box<> instead of Box<>
impl Clone for Config {
    fn clone(&self) -> Config {
        Config {
            bg: self.bg.clone(),
            fg: self.fg,
            font: self.font.clone(),
            font_height: self.font_height,
            resize: self.resize,
            width: self.width,
            spacing: self.spacing,
            interval: self.interval,

            bar_height: self.bar_height,
            top: self.top,
            left_blocks: Vec::new(),
            center_blocks: Vec::new(),
            right_blocks: Vec::new(),
        }
    }
}

// TODO: FAIL MORE OFTEN!
// It should not be possible to specify an image as foreground without error
pub fn read_config() -> Result<Config, Box<error::Error>> {
    let mut config_buf = String::new();
    let mut config_file = fs::File::open(format!("{}/.config/leechbar/config.toml",
                                                 get_home_dir()?))?;
    config_file.read_to_string(&mut config_buf)?;
    config_buf = config_buf.replace("\\", "\\\\"); // Escape escape characters in config file
    let config_val: toml::Value = config_buf.parse().map_err(|_| "Unable to parse config.")?;

    Ok(parse_settings(&config_val)?)
}

fn parse_settings(config_val: &toml::Value) -> Result<Config, Box<error::Error>> {
    let general = config_val.lookup("general").ok_or("Unable to find [general] in the config.")?;

    let mut black_img = image::DynamicImage::new_rgba8(1, 1);
    black_img.put_pixel(0, 0, image::Rgba::<u8> { data: [0, 0, 0, 255] });
    let mut config = Config {
        fg: image::Rgba::<u8> { data: [255, 255, 255, 255] },
        bg: black_img,
        font: None,
        font_height: None,
        resize: false,
        width: 0,
        spacing: 0,
        interval: 0,
        bar_height: 0,
        top: true,
        left_blocks: Vec::new(),
        center_blocks: Vec::new(),
        right_blocks: Vec::new(),
    };
    config = block_from_toml(general, &config)?;

    config.bar_height = toml_value_to_integer(general, "bar_height")? as u32;
    config.top = toml_value_to_bool(general, "top").unwrap_or(true);

    config.left_blocks = toml_value_to_blocks(general, config_val, "left_blocks", &config)?;
    config.center_blocks = toml_value_to_blocks(general, config_val, "center_blocks", &config)?;
    config.right_blocks = toml_value_to_blocks(general, config_val, "right_blocks", &config)?;

    Ok(config)
}

// Creates a Block from a toml field
// Depending on "is_config" it raises errors if certain fields are missing
// TODO: Fall back to already existing values instead of using fallback.fg etc
fn block_from_toml(general_val: &toml::Value,
                   fallback: &Config)
                   -> Result<Config, Box<error::Error>> {
    let mut config = fallback.clone();

    config.bg = toml_value_to_image(general_val, "bg").unwrap_or_else(|_| fallback.bg.clone());
    config.fg = toml_value_to_rgba(general_val, "fg").unwrap_or(fallback.fg);
    config.resize = toml_value_to_bool(general_val, "resize").unwrap_or(fallback.resize);
    config.width = toml_value_to_integer(general_val, "width").unwrap_or(fallback.width);
    config.spacing = toml_value_to_integer(general_val, "spacing").unwrap_or(fallback.spacing);
    config.interval = toml_value_to_integer(general_val, "interval").unwrap_or(fallback.interval);

    // Unwrap because if these missing it's over anyways.
    config.font = Some(toml_value_to_font(general_val, "font").unwrap_or_else(|_| {
        fallback.font.clone().ok_or("rusttype::Font required in [general].").unwrap()
    }));
    config.font_height = Some(toml_value_to_integer(general_val, "font_height")
        .unwrap_or_else(|_| {
            fallback.font_height.ok_or("rusttype::Font Height required in [general].").unwrap()
        }));

    Ok(config)
}

fn toml_value_to_blocks(general_val: &toml::Value,
                        config_val: &toml::Value,
                        name: &str,
                        config: &Config)
                        -> Result<Vec<Box<Block>>, Box<error::Error>> {
    let blocks_text = toml_value_to_string(general_val, name)?;
    let blocks_split = blocks_text.split(' ');

    let mut blocks = Vec::new();
    for mut block_name in blocks_split {
        block_name = block_name.trim();
        if block_name.is_empty() {
            continue;
        }

        let block_val = config_val.lookup(block_name)
            .ok_or_else(|| format!("Could not find toml value {}.", block_name))?;
        let block_config = block_from_toml(block_val, config)?;

        let module_name = toml_value_to_string(block_val, "module")?;
        blocks.push(MODULES.get(module_name.as_str())
            .ok_or_else(|| format!("Unable to find module {}.", module_name))?(block_config,
                                                                               block_val)?);
    }

    Ok(blocks)
}

pub fn toml_value_to_bool(general_val: &toml::Value, name: &str) -> Result<bool, String> {
    let value = general_val.lookup(name)
        .ok_or_else(|| format!("Could not find toml value {}.", name))?;
    Ok(value.as_bool().ok_or("Toml value not an integer.")?)
}

pub fn toml_value_to_integer(general_val: &toml::Value, name: &str) -> Result<u32, String> {
    let value = general_val.lookup(name)
        .ok_or_else(|| format!("Could not find toml value {}.", name))?;
    Ok(value.as_integer().ok_or("Toml value not an integer.")? as u32)
}

pub fn toml_value_to_string(general_val: &toml::Value, name: &str) -> Result<String, String> {
    let value = general_val.lookup(name)
        .ok_or_else(|| format!("Could not find toml value {}.", name))?;
    Ok(value.as_str().ok_or("Toml value not a string.")?.to_owned())
}

pub fn toml_value_to_rgba(general_val: &toml::Value,
                          name: &str)
                          -> Result<image::Rgba<u8>, Box<error::Error>> {
    let col_string = toml_value_to_string(general_val, name)?;
    Ok(string_to_rgba(&col_string)?)
}

pub fn toml_value_to_image(general_val: &toml::Value,
                           name: &str)
                           -> Result<image::DynamicImage, Box<error::Error>> {
    let path = toml_value_to_string(general_val, name)?;

    if path.starts_with('#') {
        let mut img = image::DynamicImage::new_rgba8(1, 1);
        let str_rgba = string_to_rgba(&path)?;
        img.put_pixel(0, 0, str_rgba);

        Ok(img)
    } else {
        let home = get_home_dir()?;
        let path = path.replace('~', &home).replace("$HOME", &home);

        Ok(image::open(&path::Path::new(&path))?)
    }
}

// Uses string as path to load a font file
pub fn toml_value_to_font(general_val: &toml::Value,
                          name: &str)
                          -> Result<rusttype::Font<'static>, Box<error::Error>> {
    let home = get_home_dir()?;
    let font_string =
        toml_value_to_string(general_val, name)?.replace("$", &home).replace("~", &home);

    let font_file = fs::File::open(&font_string)?;
    let font_data = font_file.bytes().collect::<Result<Vec<u8>, io::Error>>()?;
    let collection = rusttype::FontCollection::from_bytes(font_data);
    let font = collection.into_font().ok_or("Please only use valid TTF fonts.")?;

    Ok(font)
}

fn string_to_rgba(col_string: &str) -> Result<image::Rgba<u8>, num::ParseIntError> {
    let red_string = col_string[1..3].to_lowercase();
    let blue_string = col_string[5..7].to_lowercase();
    let green_string = col_string[3..5].to_lowercase();
    let alpha_string = {
        if col_string.len() > 7 {
            col_string[7..9].to_lowercase()
        } else {
            "ff".to_owned()
        }
    };

    let red = u8::from_str_radix(&red_string, 16)?;
    let blue = u8::from_str_radix(&blue_string, 16)?;
    let green = u8::from_str_radix(&green_string, 16)?;
    let alpha = u8::from_str_radix(&alpha_string, 16)?;

    Ok(image::Rgba { data: [red, green, blue, alpha] })
}

fn get_home_dir() -> Result<String, String> {
    let home_dir = env::home_dir().ok_or("Could not find home dir.")?;
    let home_str = home_dir.to_string_lossy();
    Ok(home_str.to_string())
}
