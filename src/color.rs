use embedded_graphics::{pixelcolor::Rgb888, prelude::RgbColor};

// Rgb888 is higher quality, Rgb565 cuts the size of the framebuffer by a third.
// Gray8 is 3x smaller than Rgb888... but I'm not sure we love monochrome.
pub type Color = embedded_graphics::pixelcolor::Rgb565;

const fn color_from_rgb(r: u8, g: u8, b: u8) -> Color {
    Color::new(
        (r as f64 / 255. * Color::MAX_R as f64) as u8,
        (g as f64 / 255. * Color::MAX_G as f64) as u8,
        (b as f64 / 255. * Color::MAX_B as f64) as u8,
    )
}

const fn unhex(hex: u8) -> u8 {
    match hex {
        b'0'..=b'9' => hex - b'0',
        b'a'..=b'f' => hex - (b'a' - 10),
        b'A'..=b'F' => hex - (b'A' - 10),
        _ => panic!("Not a valid hex digit"),
    }
}

const fn color_from_hex(hex: &str) -> Color {
    let hex = hex.as_bytes();
    assert!(matches!(hex[0], b'#'), "Hex color must start with a #");
    match hex.len() {
        7 => color_from_rgb(
            unhex(hex[1]) * 16 + unhex(hex[2]),
            unhex(hex[3]) * 16 + unhex(hex[4]),
            unhex(hex[5]) * 16 + unhex(hex[6]),
        ),
        _ => unreachable!(),
    }
}

pub const LIME: Color = color_from_hex("#c0fe04");
pub const RED: Color = color_from_hex("#f24723");
pub const ACCENT: Color = LIME;
// color-background: var(--color-black);
// color-foreground: var(--color-white);
