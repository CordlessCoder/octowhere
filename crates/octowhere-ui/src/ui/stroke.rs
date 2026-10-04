//! Antialiased strokes for the 2026-10-04 screens' symbols and rims: straight segments with
//! round ends, and arcs of a circle. Each pixel takes the most coverage any part of one stroke
//! gives it, so joints are not drawn twice.

use embedded_graphics::{
    prelude::{Point, Size},
    primitives::Rectangle,
};

use crate::chrome::{Color, CoverageTarget};

/// Positions and widths in quarter pixels, so that a stroke can be scaled without rounding it to
/// whole pixels.
pub const QUARTER: f32 = 4.0;

/// The widest stroke a row of coverage is kept for.
const ROW: usize = crate::board::LCD_WIDTH as usize;

/// A path of straight segments through `points`, in quarter pixels, `width` quarter pixels wide.
pub fn draw_path<D: CoverageTarget<Color = Color>>(
    points: &[(i16, i16)],
    width: u8,
    color: Color,
    target: &mut D,
) {
    let half = f32::from(width) / QUARTER / 2.0;
    let at = |&(x, y): &(i16, i16)| (f32::from(x) / QUARTER, f32::from(y) / QUARTER);
    let Some(bounds) = path_bounds(points, width) else {
        return;
    };
    if !target.visible(&bounds) {
        return;
    }
    fill_rows(bounds, color, target, |x, y| {
        points
            .windows(2)
            .map(|pair| {
                let distance = to_segment((x, y), at(&pair[0]), at(&pair[1]));
                (half + 0.5 - distance).clamp(0.0, 1.0)
            })
            .fold(0.0, f32::max)
    });
}

/// The pixels [`draw_path`] may touch.
#[must_use]
pub fn path_bounds(points: &[(i16, i16)], width: u8) -> Option<Rectangle> {
    let reach = i32::from(width) / 8 + 2;
    let xs = points.iter().map(|&(x, _)| i32::from(x) / 4);
    let ys = points.iter().map(|&(_, y)| i32::from(y) / 4);
    let (left, right) = (xs.clone().min()?, xs.max()?);
    let (top, bottom) = (ys.clone().min()?, ys.max()?);
    Some(Rectangle::new(
        Point::new(left - reach, top - reach),
        Size::new(
            (right - left + 2 * reach + 1) as u32,
            (bottom - top + 2 * reach + 1) as u32,
        ),
    ))
}

/// The arc of the circle about `center`, `radius` quarter pixels to the middle of its `width`,
/// from `from` to `to` tenths of a degree clockwise from the right.
pub fn draw_arc<D: CoverageTarget<Color = Color>>(
    center: Point,
    radius: u16,
    width: u8,
    (from, to): (i16, i16),
    color: Color,
    target: &mut D,
) {
    let bounds = arc_bounds(center, radius, width, (from, to));
    if !target.visible(&bounds) {
        return;
    }
    let radius = f32::from(radius) / QUARTER;
    let half = f32::from(width) / QUARTER / 2.0;
    let (from, to) = (
        f32::from(from).to_radians() / 10.0,
        f32::from(to).to_radians() / 10.0,
    );
    let middle = (from + to) / 2.0;
    let (cx, cy) = (center.x as f32, center.y as f32);
    fill_rows(bounds, color, target, |x, y| {
        let (dx, dy) = (x - cx, y - cy);
        let distance = libm::hypotf(dx, dy);
        let radial = (half + 0.5 - (distance - radius).abs()).clamp(0.0, 1.0);
        if radial <= 0.0 {
            return 0.0;
        }
        // The angle from the arc's middle, so that the arc may cross the right without a seam.
        let mut angle = libm::atan2f(dy, dx) - middle;
        while angle > core::f32::consts::PI {
            angle -= core::f32::consts::TAU;
        }
        while angle < -core::f32::consts::PI {
            angle += core::f32::consts::TAU;
        }
        let inside = ((to - from) / 2.0 - angle.abs()) * distance;
        radial * (inside + 0.5).clamp(0.0, 1.0)
    });
}

/// The pixels [`draw_arc`] may touch.
#[must_use]
pub fn arc_bounds(center: Point, radius: u16, width: u8, (from, to): (i16, i16)) -> Rectangle {
    let outer = (f32::from(radius) + f32::from(width) / 2.0) / QUARTER + 1.0;
    let mut corners = [(f32::MAX, f32::MAX), (f32::MIN, f32::MIN)];
    let mut take = |angle: f32, reach: f32| {
        let (sin, cos) = libm::sincosf(angle);
        let (x, y) = (cos * reach, sin * reach);
        corners[0] = (corners[0].0.min(x), corners[0].1.min(y));
        corners[1] = (corners[1].0.max(x), corners[1].1.max(y));
    };
    let inner = ((f32::from(radius) - f32::from(width) / 2.0) / QUARTER - 1.0).max(0.0);
    for tenth in (i32::from(from)..=i32::from(to))
        .step_by(50)
        .chain([i32::from(to)])
    {
        let angle = (tenth as f32 / 10.0).to_radians();
        take(angle, outer);
        take(angle, inner);
    }
    let left = libm::floorf(corners[0].0) as i32 - 1;
    let top = libm::floorf(corners[0].1) as i32 - 1;
    let right = libm::ceilf(corners[1].0) as i32 + 1;
    let bottom = libm::ceilf(corners[1].1) as i32 + 1;
    Rectangle::new(
        center + Point::new(left, top),
        Size::new((right - left) as u32, (bottom - top) as u32),
    )
}

/// Blends `color` over `bounds` a row at a time, by `coverage` at each pixel's centre.
fn fill_rows<D: CoverageTarget<Color = Color>>(
    bounds: Rectangle,
    color: Color,
    target: &mut D,
    coverage: impl Fn(f32, f32) -> f32,
) {
    let mut row = [0u8; ROW];
    let width = (bounds.size.width as usize).min(ROW);
    for y in bounds.top_left.y..bounds.top_left.y + bounds.size.height as i32 {
        let line = Rectangle::new(Point::new(bounds.top_left.x, y), Size::new(width as u32, 1));
        if !target.visible(&line) {
            continue;
        }
        let mut any = false;
        for (i, cell) in row[..width].iter_mut().enumerate() {
            let x = bounds.top_left.x + i as i32;
            let c = coverage(x as f32 + 0.5, y as f32 + 0.5);
            *cell = libm::roundf(c * 255.0) as u8;
            any |= *cell > 0;
        }
        if any {
            target.blend_row(bounds.top_left.x, y, &row[..width], color);
        }
    }
}

/// The distance from `p` to the segment from `a` to `b`.
fn to_segment(p: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let length = dx * dx + dy * dy;
    let t = if length == 0.0 {
        0.0
    } else {
        (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / length).clamp(0.0, 1.0)
    };
    libm::hypotf(p.0 - (a.0 + t * dx), p.1 - (a.1 + t * dy))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chrome::{self, FB};

    fn lit(fb: &FB, x: i32, y: i32) -> bool {
        fb.pixel(Point::new(x, y)) != Some(chrome::BLACK)
    }

    #[test]
    fn a_path_covers_its_segment_and_nothing_far_from_it() {
        let mut fb = FB::boxed();
        let q = |v: i16| v * 4;
        draw_path(
            &[(q(10), q(10)), (q(30), q(10))],
            4,
            chrome::WHITE,
            &mut *fb,
        );
        assert!(lit(&fb, 20, 10));
        assert!(!lit(&fb, 20, 14));
        assert!(!lit(&fb, 40, 10));
    }

    #[test]
    fn the_unread_arc_lies_across_the_bottom() {
        let mut fb = FB::boxed();
        draw_arc(
            Point::new(233, 233),
            230 * 4,
            12,
            (800, 1000),
            chrome::WHITE,
            &mut *fb,
        );
        assert!(lit(&fb, 233, 463));
        assert!(!lit(&fb, 233, 455));
        assert!(!lit(&fb, 300, 455));
        let bounds = arc_bounds(Point::new(233, 233), 230 * 4, 12, (800, 1000));
        assert!(bounds.contains(Point::new(193, 459)));
        assert!(bounds.contains(Point::new(273, 459)));
    }

    #[test]
    fn an_arc_may_cross_the_right() {
        let mut fb = FB::boxed();
        draw_arc(
            Point::new(233, 233),
            225 * 4,
            4,
            (-410, 410),
            chrome::WHITE,
            &mut *fb,
        );
        assert!(lit(&fb, 458, 233));
        assert!(!lit(&fb, 233, 8));
    }
}
