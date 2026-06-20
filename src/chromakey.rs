use anyhow::Context;
use base64::Engine;
use image::{ImageFormat, RgbaImage};
use std::io::Cursor;

pub const CHROMAKEY_MIN_THRESHOLD: f64 = 800.0;
pub const CHROMAKEY_MAX_THRESHOLD: f64 = 3000.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RgbColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

pub fn transparent_png_base64(base64_image: &str) -> anyhow::Result<(String, RgbColor)> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(base64_image)
        .context("image result was not valid base64")?;
    let image = image::load_from_memory(&bytes)
        .context("image result could not be decoded")?
        .to_rgba8();
    let (png, color) = chromakey_image(image)?;
    let base64 = base64::engine::general_purpose::STANDARD.encode(png);
    Ok((base64, color))
}

fn chromakey_image(image: RgbaImage) -> anyhow::Result<(Vec<u8>, RgbColor)> {
    let color = detect_corner_color(&image)?;
    let (width, height) = image.dimensions();
    let mut pixels = image.into_raw();
    rustychroma::remove_range(
        &mut pixels,
        color.r,
        color.g,
        color.b,
        CHROMAKEY_MIN_THRESHOLD,
        CHROMAKEY_MAX_THRESHOLD,
    );
    let image = RgbaImage::from_raw(width, height, pixels)
        .context("chromakey output had invalid RGBA dimensions")?;

    let mut cursor = Cursor::new(Vec::new());
    image
        .write_to(&mut cursor, ImageFormat::Png)
        .context("could not encode transparent PNG")?;
    Ok((cursor.into_inner(), color))
}

pub fn detect_corner_color(image: &RgbaImage) -> anyhow::Result<RgbColor> {
    let (width, height) = image.dimensions();
    if width == 0 || height == 0 {
        anyhow::bail!("cannot detect background color from empty image");
    }

    let corners = [
        image.get_pixel(0, 0),
        image.get_pixel(width - 1, 0),
        image.get_pixel(0, height - 1),
        image.get_pixel(width - 1, height - 1),
    ];

    let (r, g, b) = corners.iter().fold((0u16, 0u16, 0u16), |(r, g, b), pixel| {
        (
            r + pixel[0] as u16,
            g + pixel[1] as u16,
            b + pixel[2] as u16,
        )
    });

    Ok(RgbColor {
        r: (r / 4) as u8,
        g: (g / 4) as u8,
        b: (b / 4) as u8,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};

    #[test]
    fn corner_color_detection_averages_four_corners() {
        let mut image = RgbaImage::from_pixel(3, 3, Rgba([0, 0, 0, 255]));
        image.put_pixel(0, 0, Rgba([10, 20, 30, 255]));
        image.put_pixel(2, 0, Rgba([20, 30, 40, 255]));
        image.put_pixel(0, 2, Rgba([30, 40, 50, 255]));
        image.put_pixel(2, 2, Rgba([40, 50, 60, 255]));

        assert_eq!(
            detect_corner_color(&image).unwrap(),
            RgbColor {
                r: 25,
                g: 35,
                b: 45
            }
        );
    }

    #[test]
    fn chromakey_makes_solid_background_corners_transparent() {
        let mut image = RgbaImage::from_pixel(4, 4, Rgba([255, 0, 150, 255]));
        image.put_pixel(1, 1, Rgba([255, 0, 0, 255]));
        image.put_pixel(2, 1, Rgba([255, 0, 0, 255]));

        let (png, color) = chromakey_image(image).unwrap();
        assert_eq!(
            color,
            RgbColor {
                r: 255,
                g: 0,
                b: 150
            }
        );

        let decoded = image::load_from_memory(&png).unwrap().to_rgba8();
        assert_eq!(decoded.get_pixel(0, 0)[3], 0);
        assert_eq!(decoded.get_pixel(3, 3)[3], 0);
    }
}
