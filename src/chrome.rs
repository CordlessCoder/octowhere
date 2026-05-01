use embedded_graphics::{
    pixelcolor::Rgb888,
    prelude::{Point, RgbColor, Size},
    primitives::Rectangle,
};

use crate::board;

// Rgb888 is higher quality, Rgb565 cuts the size of the framebuffer by a third.
// Gray8 is 3x smaller than Rgb888... but I'm not sure we love monochrome.
pub type Color = embedded_graphics::pixelcolor::Rgb565;

pub const DISPLAY_SIZE: Size = Size::new(board::LCD_WIDTH as u32, board::LCD_HEIGHT as u32);
pub const DISPLAY_BBOX: Rectangle = Rectangle::new(Point::new_equal(0), DISPLAY_SIZE);

pub const UPSCALE: usize = 1;
pub const MEDIUM_FONT: &embedded_bitmap_fonts::BitmapFont<'static> = match UPSCALE {
    1 => &embedded_bitmap_fonts::terminus::FONT_16x32,
    2 => &embedded_bitmap_fonts::terminus::FONT_8x16,
    3 => &embedded_bitmap_fonts::terminus::FONT_6x12,
    4 => &embedded_bitmap_fonts::terminus::FONT_6x12,
    _ => todo!(),
};
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
pub const PURPLE: Color = color_from_hex("#5500e4");
pub const ORANGE_RED: Color = color_from_hex("#f15227");
pub const GRAY: Color = color_from_hex("#999999");
pub const WHITE: Color = color_from_hex("#ffffff");
pub const BLACK: Color = color_from_hex("#000000");

pub const ACCENT: Color = LIME;
// color-background: var(--color-black);
// color-foreground: var(--color-white);
