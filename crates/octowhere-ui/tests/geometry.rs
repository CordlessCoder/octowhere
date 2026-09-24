use octowhere_ui::ui::{
    dirty::RowSpans,
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

type Spans = RowSpans<16, 8, 2>;

fn covered(rects: &[Rectangle]) -> Vec<Point> {
    let mut points: Vec<Point> = rects.iter().flat_map(|rect| rect.points()).collect();
    points.sort_by_key(|point| (point.y, point.x));
    points
}

fn damaged(spans: &Spans) -> Vec<Point> {
    (0..16)
        .flat_map(|y| (0..16).map(move |x| Point::new(x, y)))
        .filter(|&point| spans.contains(point))
        .collect()
}

#[test]
fn spans_widen_to_the_panels_grain() {
    let mut spans = Spans::new();
    spans.add(Rectangle::new(Point::new(3, 3), Size::new(1, 1)));
    assert_eq!(spans.bounding_box(), Rectangle::new(Point::new(2, 2), Size::new(2, 2)));
    assert_eq!(spans.pixels(), 4);
}

#[test]
fn spans_past_the_limit_merge_across_the_smallest_gap() {
    let mut spans = Spans::new();
    for x in [0, 6, 14] {
        spans.add(Rectangle::new(Point::new(x, 0), Size::new(2, 2)));
    }
    assert_eq!(spans.spans(0).collect::<Vec<_>>(), vec![(0, 8), (14, 16)]);
}

#[test]
fn rectangles_cover_the_damage_and_nothing_else_at_no_overhead() {
    let mut spans = Spans::new();
    spans.add(Rectangle::new(Point::new(0, 0), Size::new(4, 6)));
    spans.add(Rectangle::new(Point::new(10, 2), Size::new(4, 2)));
    spans.add(Rectangle::new(Point::new(2, 6), Size::new(8, 2)));
    let rects: Vec<_> = spans.rectangles(0).collect();
    assert_eq!(covered(&rects), damaged(&spans), "{rects:?}");
}

#[test]
fn rectangles_absorb_small_steps_when_regions_cost_more() {
    let mut spans = Spans::new();
    spans.add(Rectangle::new(Point::new(0, 0), Size::new(8, 4)));
    spans.add(Rectangle::new(Point::new(0, 4), Size::new(10, 4)));
    assert_eq!(spans.rectangles(0).count(), 2);
    let rects: Vec<_> = spans.rectangles(64).collect();
    assert_eq!(rects, vec![Rectangle::new(Point::zero(), Size::new(10, 8))]);
}

#[test]
fn a_polygon_marks_every_pixel_it_touches() {
    let mut spans = Spans::new();
    let triangle = [(1.5, 1.5), (12.2, 4.0), (5.0, 13.7)];
    spans.add_polygon(&triangle, 0);
    // The centroid's row and the vertices are all inside.
    for (x, y) in triangle.into_iter().chain([(6.2, 6.4)]) {
        assert!(spans.contains(Point::new(x as i32, y as i32)), "({x}, {y})");
    }
    assert!(!spans.contains(Point::new(14, 14)));
}
