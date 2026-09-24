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

/// A ring between two radii about a pixel corner, with its coverage computed once. It is stored
/// as one run per row of the top-left quarter, and each draw blends every run at its four mirror
/// positions.
pub struct Ring {
    center: Point,
    /// Per row of the quarter: its offset above the centre, the leftmost pixel's offset left of
    /// the centre, and the run's place in `forward`.
    runs: Vec<(i32, i32, usize, usize)>,
    /// Coverage left to right across the top-left quarter's runs, and the same runs reversed
    /// for the right half.
    forward: Vec<u8>,
    reversed: Vec<u8>,
}

impl Ring {
    /// The distance from the ring's middle comes from the squared distance, `(d² - m²) / 2m`,
    /// which is a few hundredths of a pixel from exact across a band a few pixels wide.
    #[must_use]
    pub fn new(center: Point, inner: f32, outer: f32) -> Self {
        let middle = (inner + outer) / 2.0;
        let (half, scale) = ((outer - inner) / 2.0, 1.0 / (2.0 * middle));
        // Coverage is nonzero only within half a pixel of either edge.
        let (low, high) = (inner - 0.5, outer + 0.5);
        let (mut runs, mut forward) = (Vec::new(), Vec::new());
        // Offsets (i, j) are whole pixels left of and above the centre corner, with the pixel's
        // centre at (i + 0.5, j + 0.5) from it.
        for j in 0..=libm::ceilf(high) as i32 {
            let y = j as f32 + 0.5;
            if y >= high {
                break;
            }
            let first = libm::floorf(libm::sqrtf((low * low - y * y).max(0.0)) - 0.5).max(0.0) as i32;
            let last = libm::ceilf(libm::sqrtf(high * high - y * y) - 0.5) as i32;
            let start = forward.len();
            // Left to right is from the outermost offset in.
            forward.extend((first..=last).rev().map(|i| {
                let x = i as f32 + 0.5;
                coverage(half - ((x * x + y * y - middle * middle) * scale).abs())
            }));
            runs.push((j, last, start, forward.len() - start));
        }
        let reversed = runs
            .iter()
            .flat_map(|&(_, _, start, length)| forward[start..start + length].iter().rev().copied())
            .collect();
        Self {
            center,
            runs,
            forward,
            reversed,
        }
    }

    pub fn draw<D: CoverageTarget>(&self, target: &mut D, color: D::Color) {
        let Point { x: cx, y: cy } = self.center;
        for &(j, last, start, length) in &self.runs {
            let (left, right) = (cx - 1 - last, cx + last + 1 - length as i32);
            let forward = &self.forward[start..start + length];
            let reversed = &self.reversed[start..start + length];
            for y in [cy - 1 - j, cy + j] {
                target.blend_row(left, y, forward, color);
                target.blend_row(right, y, reversed, color);
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

    // Offsets from the centre corner of the fill's first column and row, and of its last.
    let (dx0, dy0) = (left - center.x, top - center.y);
    let (dx1, dy1) = (dx0 + width as i32 - 1, dy0 + height as i32 - 1);
    let mut line = Vec::with_capacity(width.max(height));
    // A clockwise quarter turn takes the pixel at offset (dx, dy) to (-1 - dy, dx), so each turn
    // of the fill is still whole rows: its own rows, reversed, or its columns.
    for (row, pixels) in coverage.chunks_exact(width).enumerate() {
        let dy = dy0 + row as i32;
        target.blend_row(center.x + dx0, center.y + dy, pixels, color);
        line.clear();
        line.extend(pixels.iter().rev());
        target.blend_row(center.x - 1 - dx1, center.y - 1 - dy, &line, color);
    }
    for column in 0..width {
        let dx = dx0 + column as i32;
        line.clear();
        line.extend(coverage[column..].iter().step_by(width));
        target.blend_row(center.x + dy0, center.y - 1 - dx, &line, color);
        line.reverse();
        target.blend_row(center.x - 1 - dy1, center.y + dx, &line, color);
    }
}
