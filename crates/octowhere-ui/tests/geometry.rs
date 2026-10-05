use embedded_graphics::{prelude::*, primitives::Rectangle};
use octowhere_ui::ui::dirty::RowSpans;

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
    assert_eq!(
        spans.bounding_box(),
        Rectangle::new(Point::new(2, 2), Size::new(2, 2))
    );
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

#[test]
fn rectangles_cover_every_damaged_pixel_at_any_overhead() {
    type Wide = RowSpans<64, 32, 3>;
    let mut seed = 0x2545_f491_u32;
    let mut next = |below: u32| {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        seed % below
    };
    for _ in 0..200 {
        let mut spans = Wide::new();
        for _ in 0..next(12) {
            let (x, y) = (next(64) as i32, next(64) as i32);
            spans.add(Rectangle::new(
                Point::new(x, y),
                Size::new(next(20) + 1, next(20) + 1),
            ));
        }
        for overhead in [0, 16, 200, 2400, 1 << 20] {
            let rects: Vec<_> = spans.rectangles(overhead).collect();
            for y in 0..64 {
                for x in 0..64 {
                    let point = Point::new(x, y);
                    if spans.contains(point) {
                        assert!(
                            rects.iter().any(|rect| rect.contains(point)),
                            "{point:?} missed at overhead {overhead}: {rects:?}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn a_reflection_turns_damage_half_a_turn_about_the_centre() {
    let mut spans = Spans::new();
    spans.add(Rectangle::new(Point::new(2, 4), Size::new(4, 2)));
    let mut turned = Spans::new();
    turned.extend_reflected(&spans);
    // Pixel (x, y) lands on (15 - x, 15 - y) on a 16 × 16 panel.
    assert_eq!(
        turned.bounding_box(),
        Rectangle::new(Point::new(10, 10), Size::new(4, 2))
    );
}

#[test]
fn damaged_pieces_cover_the_area_once_within_a_band() {
    let mut spans = Spans::new();
    spans.add(Rectangle::new(Point::new(2, 1), Size::new(5, 6)));
    spans.add(Rectangle::new(Point::new(10, 3), Size::new(4, 4)));
    for area in [
        Rectangle::new(Point::new(-3, -2), Size::new(30, 30)),
        Rectangle::new(Point::new(3, 3), Size::new(9, 3)),
        Rectangle::new(Point::new(0, 5), Size::new(16, 1)),
        Rectangle::new(Point::new(4, 4), Size::zero()),
    ] {
        let mut hits = [[0u8; 16]; 16];
        spans.for_each_rect(&area, |part| {
            assert_eq!(
                part.top_left.y / 2,
                part.bottom_right().unwrap().y / 2,
                "{part:?}"
            );
            for point in part.points() {
                hits[point.y as usize][point.x as usize] += 1;
            }
        });
        for y in 0..16 {
            for x in 0..16 {
                let point = Point::new(x, y);
                let want = u8::from(area.contains(point) && spans.contains(point));
                assert_eq!(hits[y as usize][x as usize], want, "{point:?} in {area:?}");
            }
        }
    }
}
