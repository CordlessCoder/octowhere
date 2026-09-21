#![allow(dead_code)]
#![allow(clippy::drop_non_drop)]

#[path = "../../src/util.rs"]
pub mod util;

/// `SwapThread` must not cross a thread boundary when its value is not `Send`.
///
/// ```compile_fail,E0277
/// use octowhere_host_tests::util::Swap;
/// use std::sync::{Mutex, MutexGuard};
///
/// fn assert_send<T: Send>(_: T) {}
/// fn assert_sync<T: Sync>() {}
/// let first_lock = Mutex::new(());
/// let second_lock = Mutex::new(());
/// let mut swap = Swap::new(first_lock.lock().unwrap(), second_lock.lock().unwrap());
/// let (first, _second) = swap.split();
/// assert_sync::<MutexGuard<'_, ()>>();
/// assert_send(first);
/// ```
pub const SWAP_SEND_BOUND: () = ();

#[path = "peripherals/mod.rs"]
pub mod peripherals;

#[path = "../../src/ui/dirty.rs"]
pub mod dirty;
#[path = "../../src/ui/geometry.rs"]
pub mod geometry;
#[path = "../../src/ui/input.rs"]
pub mod input;

#[cfg(test)]
mod tests {
    use super::{
        dirty::DirtyAreas,
        geometry::{clipped_fill_region, for_each_visible_color},
    };
    use embedded_graphics::{prelude::*, primitives::Rectangle};

    #[test]
    fn fill_region_clips_negative_coordinates() {
        assert_eq!(
            clipped_fill_region(4, 3, -1, -1, 3, 3),
            Some(super::geometry::FillRegion {
                x: 0,
                y: 0,
                width: 2,
                height: 2,
                source_x: 1,
                source_y: 1,
            })
        );
    }

    #[test]
    fn fill_consumes_area_and_ignores_excess_colors() {
        let mut pixels = Vec::new();
        for_each_visible_color(
            Size::new(3, 2),
            Rectangle::new(Point::new(-1, 0), Size::new(3, 1)),
            0..10,
            |x, y, color| pixels.push((x, y, color)),
        );
        assert_eq!(pixels, vec![(0, 0, 1), (1, 0, 2)]);
    }

    #[test]
    fn dirty_tracks_first_row_and_column() {
        let mut dirty = DirtyAreas::<8, 6, 2, 3, 6>::new();
        dirty.add(Rectangle::new(Point::new(0, 0), Size::new(1, 1)));
        assert_eq!(dirty.iter().count(), 1);
    }

    #[test]
    fn dirty_uses_x_stride_for_non_square_grid() {
        let mut dirty = DirtyAreas::<9, 6, 3, 2, 6>::new();
        dirty.add(Rectangle::new(Point::new(7, 1), Size::new(1, 1)));
        dirty.add(Rectangle::new(Point::new(1, 4), Size::new(1, 1)));
        assert_eq!(dirty.iter().count(), 2);
    }

    #[test]
    fn dirty_includes_non_divisible_tail_cells() {
        let mut dirty = DirtyAreas::<10, 7, 3, 2, 6>::new();
        dirty.add(Rectangle::new(Point::new(9, 6), Size::new(1, 1)));
        assert_eq!(dirty.iter().count(), 1);
    }
}
