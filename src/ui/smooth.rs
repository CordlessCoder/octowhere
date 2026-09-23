//! Antialiased primitives, which embedded-graphics does not draw.
//!
//! Coordinates are continuous, with pixel `(x, y)` covering `x..x + 1` and `y..y + 1`. Edge
//! pixels are blended toward `background`, since a draw target cannot be read back: draw these
//! only over a plain field of that colour.

use alloc::vec::Vec;

use embedded_graphics::{Pixel, draw_target::DrawTarget, pixelcolor::PixelColor, prelude::Point};
use fontdue::{PathEvent, Transform, raster::Raster, rasterize_path_clipped};

use crate::chrome::RgbColorExt;

/// The coverage of a pixel whose centre lies `inside` units inside an edge, with a one-pixel
/// ramp across it.
fn coverage(inside: f32) -> u8 {
    ((inside + 0.5).clamp(0.0, 1.0) * 255.0 + 0.5) as u8
}

/// A ring between `inner` and `outer` radius about `center`.
pub fn ring<C, D>(
    target: &mut D,
    center: (f32, f32),
    inner: f32,
    outer: f32,
    color: C,
    background: C,
) -> Result<(), D::Error>
where
    C: PixelColor + RgbColorExt,
    D: DrawTarget<Color = C>,
{
    let (cx, cy) = center;
    let (reach, hollow) = (outer + 1.0, (inner - 1.0).max(0.0));
    let pixel = move |x: i32, y: i32| {
        let (dx, dy) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
        let distance = libm::sqrtf(dx * dx + dy * dy);
        let covered = coverage((outer - distance).min(distance - inner));
        (covered > 0).then(|| Pixel(Point::new(x, y), background.lerp(&color, covered)))
    };
    let rows = libm::floorf(cy - reach) as i32..=libm::ceilf(cy + reach) as i32;
    target.draw_iter(rows.flat_map(move |y| {
        let dy = y as f32 + 0.5 - cy;
        let across = libm::sqrtf((reach * reach - dy * dy).max(0.0));
        let within = if dy.abs() < hollow {
            libm::sqrtf(hollow * hollow - dy * dy)
        } else {
            0.0
        };
        let left = libm::floorf(cx - across) as i32..=libm::ceilf(cx - within) as i32;
        // With no hollow on this row the two spans meet, so the right one starts past the left.
        let right_start = (libm::floorf(cx + within) as i32).max(*left.end() + 1);
        let right = right_start..=libm::ceilf(cx + across) as i32;
        left.chain(right).filter_map(move |x| pixel(x, y))
    }))
}

/// Fills the polygon through `corners`, then draws it and its turns by one, two and three
/// quarters clockwise about `center`. `center` is a pixel corner, so each turn lands on whole
/// pixels and one fill serves all four. `raster` and `coverage` are scratch, reused across calls.
pub fn polygon_quarters<C, D>(
    target: &mut D,
    raster: &mut Raster<'static>,
    coverage: &mut Vec<u8>,
    corners: &[(f32, f32)],
    center: Point,
    color: C,
    background: C,
) -> Result<(), D::Error>
where
    C: PixelColor + RgbColorExt,
    D: DrawTarget<Color = C>,
{
    let bound = |pick: fn(f32, f32) -> f32, axis: fn(&(f32, f32)) -> f32| {
        corners.iter().map(axis).reduce(pick).unwrap_or(0.0)
    };
    let left = libm::floorf(bound(f32::min, |p| p.0)) as i32;
    let top = libm::floorf(bound(f32::min, |p| p.1)) as i32;
    let width = (libm::ceilf(bound(f32::max, |p| p.0)) as i32 - left).max(0) as usize;
    let height = (libm::ceilf(bound(f32::max, |p| p.1)) as i32 - top).max(0) as usize;
    raster.resize(width, height);
    let path = corners.iter().enumerate().map(|(index, &(x, y))| {
        if index == 0 {
            PathEvent::MoveTo([x, y])
        } else {
            PathEvent::LineTo([x, y])
        }
    });
    rasterize_path_clipped(
        raster,
        path,
        Transform::IDENTITY,
        (-left as f32, -top as f32),
    );
    coverage.clear();
    coverage.extend(raster.get_bitmap_iter());

    for quarter in 0..4 {
        target.draw_iter(coverage.iter().enumerate().filter(|&(_, &c)| c != 0).map(
            |(index, &c)| {
                let dx = left + (index % width) as i32 - center.x;
                let dy = top + (index / width) as i32 - center.y;
                // Each clockwise quarter takes the pixel at offset (dx, dy) to (-1 - dy, dx).
                let (dx, dy) = match quarter {
                    0 => (dx, dy),
                    1 => (-1 - dy, dx),
                    2 => (-1 - dx, -1 - dy),
                    _ => (dy, -1 - dx),
                };
                Pixel(center + Point::new(dx, dy), background.lerp(&color, c))
            },
        ))?;
    }
    Ok(())
}
