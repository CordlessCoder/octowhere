//! Antialiased primitives, which embedded-graphics does not draw.
//!
//! Coordinates are continuous, with pixel `(x, y)` covering `x..x + 1` and `y..y + 1`. Both
//! primitives are symmetric about `center`, which must be a pixel corner, so that each mirror or
//! quarter turn lands on whole pixels.

use alloc::vec::Vec;

use embedded_graphics::prelude::Point;
use fontdue::{PathEvent, Transform, raster::Raster, rasterize_path_clipped};

use crate::chrome::CoverageTarget;

/// The coverage of a pixel whose centre lies `inside` units inside an edge, with a one-pixel
/// ramp across it.
fn coverage(inside: f32) -> u8 {
    ((inside + 0.5).clamp(0.0, 1.0) * 255.0 + 0.5) as u8
}

/// A ring between `inner` and `outer` radius about `center`. One eighth of it is computed and
/// mirrored to the rest, and each column visits only the pixels the ring covers. The distance
/// from the ring's middle comes from the squared distance, `(d² - m²) / 2m`, which is a few
/// hundredths of a pixel from exact across a band a few pixels wide.
pub fn ring<D: CoverageTarget>(
    target: &mut D,
    center: Point,
    inner: f32,
    outer: f32,
    color: D::Color,
) {
    let middle = (inner + outer) / 2.0;
    let (half, scale) = ((outer - inner) / 2.0, 1.0 / (2.0 * middle));
    // Coverage is nonzero only within half a pixel of either edge.
    let (low, high) = (inner - 0.5, outer + 0.5);
    // Offsets (i, j) are whole pixels from the centre corner, with the pixel's centre at
    // (i + 0.5, j + 0.5). The octant is 0 <= i <= j.
    let last_column = libm::floorf(high / core::f32::consts::SQRT_2) as i32;
    for i in 0..=last_column {
        let x = i as f32 + 0.5;
        let top = libm::sqrtf((high * high - x * x).max(0.0));
        let bottom = libm::sqrtf((low * low - x * x).max(0.0));
        let first = (libm::floorf(bottom - 0.5) as i32).max(i);
        let last = libm::ceilf(top - 0.5) as i32;
        for j in first..=last {
            let y = j as f32 + 0.5;
            let off = (x * x + y * y - middle * middle) * scale;
            let covered = coverage(half - off.abs());
            if covered == 0 {
                continue;
            }
            let (a, b) = (-1 - i, -1 - j);
            let mirrors = [(i, j), (a, j), (i, b), (a, b), (j, i), (b, i), (j, a), (b, a)];
            // On the diagonal the last four repeat the first four.
            let count = if i == j { 4 } else { 8 };
            for &(dx, dy) in &mirrors[..count] {
                target.blend_pixel(center + Point::new(dx, dy), covered, color);
            }
        }
    }
}

/// Fills the polygon through `corners`, then draws it and its turns by one, two and three
/// quarters clockwise about `center`, so one fill serves all four. `raster` and `coverage` are
/// scratch, reused across calls.
pub fn polygon_quarters<D: CoverageTarget>(
    target: &mut D,
    raster: &mut Raster<'static>,
    coverage: &mut Vec<u8>,
    corners: &[(f32, f32)],
    center: Point,
    color: D::Color,
) {
    let bound = |pick: fn(f32, f32) -> f32, axis: fn(&(f32, f32)) -> f32| {
        corners.iter().map(axis).reduce(pick).unwrap_or(0.0)
    };
    let left = libm::floorf(bound(f32::min, |p| p.0)) as i32;
    let top = libm::floorf(bound(f32::min, |p| p.1)) as i32;
    let width = (libm::ceilf(bound(f32::max, |p| p.0)) as i32 - left).max(0) as usize;
    let height = (libm::ceilf(bound(f32::max, |p| p.1)) as i32 - top).max(0) as usize;
    if width == 0 || height == 0 {
        return;
    }
    raster.resize(width, height);
    let path = corners.iter().enumerate().map(|(index, &(x, y))| {
        if index == 0 {
            PathEvent::MoveTo([x, y])
        } else {
            PathEvent::LineTo([x, y])
        }
    });
    rasterize_path_clipped(raster, path, Transform::IDENTITY, (-left as f32, -top as f32));
    coverage.clear();
    raster
        .get_bitmap_iter()
        .for_each(|covered| coverage.push(covered));

    // Unturned, the fill's rows are the target's rows.
    for (row, line) in coverage.chunks_exact(width).enumerate() {
        target.blend_row(left, top + row as i32, line, color);
    }
    for (index, &covered) in coverage.iter().enumerate() {
        if covered == 0 {
            continue;
        }
        let dx = left + (index % width) as i32 - center.x;
        let dy = top + (index / width) as i32 - center.y;
        // Each clockwise quarter takes the pixel at offset (dx, dy) to (-1 - dy, dx).
        for (dx, dy) in [(-1 - dy, dx), (-1 - dx, -1 - dy), (dy, -1 - dx)] {
            target.blend_pixel(center + Point::new(dx, dy), covered, color);
        }
    }
}
