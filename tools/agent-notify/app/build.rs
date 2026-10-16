//! Writes the app icon when it is missing, then generates the Tauri context
//! from `tauri.conf.json` and the capabilities.

use std::error::Error;
use std::fs::{self, File};
use std::io::BufWriter;
use std::path::Path;

const ICON: &str = "icons/icon.png";
const ICON_SIZE: u32 = 128;
const CENTRE: f64 = 63.5;
const RADIUS: f64 = 60.0;
const AMBER: [u8; 4] = [0xe0, 0xa0, 0x30, 0xff];
const CLEAR: [u8; 4] = [0, 0, 0, 0];

fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-changed={ICON}");
    let icon = Path::new(ICON);
    if !icon.exists() {
        write_icon(icon)?;
    }
    tauri_build::build();
    Ok(())
}

fn amber_disc() -> Vec<u8> {
    (0..ICON_SIZE)
        .flat_map(|row| (0..ICON_SIZE).map(move |column| (f64::from(row), f64::from(column))))
        .flat_map(|(y, x)| if (x - CENTRE).powi(2) + (y - CENTRE).powi(2) <= RADIUS.powi(2) { AMBER } else { CLEAR })
        .collect()
}

fn write_icon(icon: &Path) -> Result<(), Box<dyn Error>> {
    if let Some(directory) = icon.parent() {
        fs::create_dir_all(directory)?;
    }
    let mut encoder = png::Encoder::new(BufWriter::new(File::create(icon)?), ICON_SIZE, ICON_SIZE);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(&amber_disc())?;
    Ok(())
}
