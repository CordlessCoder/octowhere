use octowhere_ui::ui::{
    dirty::DirtyAreas,
    geometry::{FillRegion, clipped_fill_region, for_each_visible_color},
};
use embedded_graphics::{prelude::*, primitives::Rectangle};

#[test]
fn fill_region_clips_negative_coordinates() {
    assert_eq!(
        clipped_fill_region(4, 3, -1, -1, 3, 3),
        Some(FillRegion {
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
