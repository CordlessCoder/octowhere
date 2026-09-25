use embedded_graphics::{pixelcolor::Rgb565, prelude::*, primitives::Rectangle};
use octowhere_ui::chrome::{self, CoverageTarget, FB, Knockout};

const ROWS: core::ops::Range<i32> = 10..30;
const SKIP: core::ops::Range<i32> = 18..22;

/// A glyph's box and the rows it sends: each row's start column, row and coverage.
type Glyph = (Rectangle, Vec<(i32, i32, Vec<u8>)>);

/// Two glyphs whose boxes overlap and a third after a gap, each sending rows that start part
/// way into its box.
fn glyphs() -> [Glyph; 3] {
    let glyph = |left: i32, top: i32, width: u32, height: u32, seed: usize| {
        let bounds = Rectangle::new(Point::new(left, top), Size::new(width, height));
        let rows = (top..top + height as i32)
            .map(|y| {
                let skip = (y - top) as usize % 3;
                let coverage = (skip..width as usize)
                    .map(|i| match (i * 7 + y as usize * 13 + seed) % 5 {
                        0 => 0,
                        1 => u8::MAX,
                        _ => ((i * 31 + seed) % 256) as u8,
                    })
                    .collect();
                (left + skip as i32, y, coverage)
            })
            .collect();
        (bounds, rows)
    };
    [glyph(100, 5, 40, 30, 1), glyph(130, 8, 40, 20, 2), glyph(200, 12, 30, 25, 3)]
}

fn channels(color: Rgb565) -> [i32; 3] {
    [color.r().into(), color.g().into(), color.b().into()]
}

#[test]
fn a_knockout_matches_filling_then_blending_and_leaves_skipped_rows() {
    let mut fb = FB::boxed();
    fb.fill_solid(&chrome::DISPLAY_BBOX, chrome::RED).unwrap();
    let mut knockout = Knockout::new(&mut *fb, ROWS, SKIP, chrome::BLACK);
    for (bounds, rows) in glyphs() {
        knockout.begin_glyph(bounds);
        for (x, y, coverage) in rows {
            knockout.blend_row(x, y, &coverage, chrome::WHITE);
        }
    }
    knockout.finish();

    let mut expected = FB::boxed();
    expected.fill_solid(&chrome::DISPLAY_BBOX, chrome::RED).unwrap();
    let band = Rectangle::new(Point::new(0, ROWS.start), Size::new(466, ROWS.len() as u32));
    expected.fill_solid(&band, chrome::BLACK).unwrap();
    for (_, rows) in glyphs() {
        for (x, y, coverage) in rows.into_iter().filter(|(_, y, _)| ROWS.contains(y)) {
            expected.blend_row(x, y, &coverage, chrome::WHITE);
        }
    }

    for y in 0..466 {
        for x in 0..466 {
            let point = Point::new(x, y);
            let got = fb.pixel(point).unwrap();
            if !ROWS.contains(&y) || SKIP.contains(&y) {
                assert_eq!(got, chrome::RED, "at {point}");
                continue;
            }
            let want = expected.pixel(point).unwrap();
            let apart = channels(got).iter().zip(channels(want)).map(|(a, b)| (a - b).abs()).max().unwrap();
            assert!(apart <= 1, "at {point}: {got:?} against {want:?}");
        }
    }
}
