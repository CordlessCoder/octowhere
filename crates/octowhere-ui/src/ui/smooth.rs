//! Antialiased primitives, which embedded-graphics does not draw.
//!
//! Coordinates are continuous, with pixel `(x, y)` covering `x..x + 1` and `y..y + 1`. Both
//! primitives are symmetric about `center`, which must be a pixel corner, so that each mirror or
//! quarter turn lands on whole pixels.

use alloc::vec::Vec;

use embedded_graphics::{
    prelude::{Point, Size},
    primitives::Rectangle,
};
use fontdue::{PathEvent, Transform, raster::Raster, rasterize_path_clipped};

use crate::chrome::CoverageTarget;

/// The coverage of a pixel whose centre lies `inside` units inside an edge, with a one-pixel
/// ramp across it.
fn coverage(inside: f32) -> u8 {
    ((inside + 0.5).clamp(0.0, 1.0) * 255.0 + 0.5) as u8
}

/// One row of a disc: its outermost solid offset left of the centre corner, negative when the
/// row has no solid pixel, and its edge coverage from the outside in.
type DiscRow = (i32, heapless::Vec<u8, 8>);

/// Row `y` of the disc of `radius` about the pixel corner `center`, or `None` if it misses it.
fn disc_row(y: i32, center: Point, radius: f32) -> Option<DiscRow> {
    let dy = y as f32 + 0.5 - center.y as f32;
    if dy.abs() >= radius + 0.5 {
        return None;
    }
    // Columns are offsets left of the centre corner; pixel `i` has its centre at `i + 0.5`.
    let covered = |i: i32| coverage(radius - libm::hypotf(i as f32 + 0.5, dy));
    let reach = libm::sqrtf((radius * radius - dy * dy).max(0.0));
    let mut i = libm::ceilf(reach + 1.0) as i32;
    let mut edge = heapless::Vec::new();
    while i >= 0 && covered(i) < u8::MAX {
        if covered(i) > 0 {
            // Past the band's few edge pixels the row is solid.
            if edge.push(covered(i)).is_err() {
                break;
            }
        }
        i -= 1;
    }
    Some((i, edge))
}

fn draw_disc_row<D: CoverageTarget>(
    target: &mut D,
    y: i32,
    center: Point,
    (i, edge): &DiscRow,
    color: D::Color,
) -> Result<(), D::Error> {
    let (left, right) = (center.x - 1 - i, center.x + i);
    if *i >= 0 {
        target.fill_solid(
            &Rectangle::with_corners(Point::new(left, y), Point::new(right, y)),
            color,
        )?;
    }
    let mut edge = edge.clone();
    target.blend_row(left - edge.len() as i32, y, &edge, color);
    edge.reverse();
    target.blend_row(right + 1, y, &edge, color);
    Ok(())
}

/// The rows of `rows` within the target's bounds.
fn bounded_rows<D: CoverageTarget>(
    target: &D,
    rows: core::ops::Range<i32>,
) -> core::ops::Range<i32> {
    let bounds = target.bounding_box();
    rows.start.max(bounds.top_left.y)..rows.end.min(bounds.top_left.y + bounds.size.height as i32)
}

/// Whether the target can show anything on row `y`.
fn row_visible<D: CoverageTarget>(target: &D, y: i32) -> bool {
    let bounds = target.bounding_box();
    target.visible(&Rectangle::new(
        Point::new(bounds.top_left.x, y),
        Size::new(bounds.size.width, 1),
    ))
}

/// Fills rows `rows` of the disc of `radius` about the pixel corner `center`, one span per row
/// with its end pixels blended over what is there.
pub fn disc_rows<D: CoverageTarget>(
    target: &mut D,
    rows: core::ops::Range<i32>,
    center: Point,
    radius: f32,
    color: D::Color,
) -> Result<(), D::Error> {
    for y in bounded_rows(target, rows) {
        if !row_visible(target, y) {
            continue;
        }
        if let Some(row) = disc_row(y, center, radius) {
            draw_disc_row(target, y, center, &row, color)?;
        }
    }
    Ok(())
}

/// [`disc_rows`] with each row worked out once, for a band drawn every frame.
pub struct DiscRows {
    center: Point,
    top: i32,
    rows: Vec<Option<DiscRow>>,
}

impl DiscRows {
    #[must_use]
    pub fn new(rows: core::ops::Range<i32>, center: Point, radius: f32) -> Self {
        Self {
            center,
            top: rows.start,
            rows: rows.map(|y| disc_row(y, center, radius)).collect(),
        }
    }

    pub fn draw<D: CoverageTarget>(&self, target: &mut D, color: D::Color) -> Result<(), D::Error> {
        let rows = self.top..self.top + self.rows.len() as i32;
        for y in bounded_rows(target, rows) {
            if !row_visible(target, y) {
                continue;
            }
            if let Some(row) = &self.rows[(y - self.top) as usize] {
                draw_disc_row(target, y, self.center, row, color)?;
            }
        }
        Ok(())
    }

    /// The largest rectangle every row fills solidly, which nothing under the band shows through.
    #[must_use]
    pub fn solid(&self) -> Rectangle {
        let inner = self
            .rows
            .iter()
            .map(|row| row.as_ref().map_or(-1, |(i, _)| *i))
            .min()
            .unwrap_or(-1);
        if inner < 0 {
            return Rectangle::zero();
        }
        Rectangle::with_corners(
            Point::new(self.center.x - 1 - inner, self.top),
            Point::new(self.center.x + inner, self.top + self.rows.len() as i32 - 1),
        )
    }
}

/// Fills the polygon through `corners`, then draws it turned by each number of quarters
/// clockwise about `center` whose bit is set in `quarters`, bit 0 unturned, so one fill serves
/// all four. `raster` and `coverage` are scratch, reused across calls.
pub fn polygon_quarters<D: CoverageTarget>(
    target: &mut D,
    raster: &mut Raster<'static>,
    coverage: &mut Vec<u8>,
    corners: &[(f32, f32)],
    center: Point,
    quarters: u8,
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
    // Offsets from the centre corner of the fill's first column and row, and of its last.
    let (dx0, dy0) = (left - center.x, top - center.y);
    let (dx1, dy1) = (dx0 + width as i32 - 1, dy0 + height as i32 - 1);
    let (along, across) = (
        Size::new(width as u32, height as u32),
        Size::new(height as u32, width as u32),
    );
    let turned = |quarter: u32| quarters & (1 << quarter) != 0;
    let places = [
        Rectangle::new(Point::new(left, top), along),
        Rectangle::new(Point::new(center.x - 1 - dy1, center.y + dx0), across),
        Rectangle::new(Point::new(center.x - 1 - dx1, center.y - 1 - dy1), along),
        Rectangle::new(Point::new(center.x + dy0, center.y - 1 - dx1), across),
    ];
    if !(0..4).any(|quarter| turned(quarter) && target.visible(&places[quarter as usize])) {
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
    rasterize_path_clipped(
        raster,
        path,
        Transform::IDENTITY,
        (-left as f32, -top as f32),
    );
    coverage.clear();
    raster
        .get_bitmap_iter()
        .for_each(|covered| coverage.push(covered));

    let mut line = Vec::with_capacity(width.max(height));
    // A clockwise quarter turn takes the pixel at offset (dx, dy) to (-1 - dy, dx), so each turn
    // of the fill is still whole rows: its own rows, reversed, or its columns.
    for (row, pixels) in coverage.chunks_exact(width).enumerate() {
        let dy = dy0 + row as i32;
        if turned(0) {
            target.blend_row(center.x + dx0, center.y + dy, pixels, color);
        }
        if turned(2) {
            line.clear();
            line.extend(pixels.iter().rev());
            target.blend_row(center.x - 1 - dx1, center.y - 1 - dy, &line, color);
        }
    }
    if !turned(1) && !turned(3) {
        return;
    }
    for column in 0..width {
        let dx = dx0 + column as i32;
        line.clear();
        line.extend(coverage[column..].iter().step_by(width));
        if turned(3) {
            target.blend_row(center.x + dy0, center.y - 1 - dx, &line, color);
        }
        line.reverse();
        if turned(1) {
            target.blend_row(center.x - 1 - dy1, center.y + dx, &line, color);
        }
    }
}
