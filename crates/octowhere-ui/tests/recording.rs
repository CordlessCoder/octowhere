use embedded_graphics::{prelude::*, primitives::Rectangle};
use octowhere_ui::chrome::{self, CoverageTarget, FB, Recording};

/// Rows of coverage: each row's start column, row and coverage. They carry uncovered ends, runs
/// of each kind longer than a run holds, and the last two reach off the panel's left and right.
fn rows(seed: usize) -> Vec<(i32, i32, Vec<u8>)> {
    let mut rows: Vec<_> = (0..40)
        .map(|i| {
            let y = 100 + i as i32;
            let coverage = (0..150 + i * 3)
                .map(|x| match (x / (i % 7 + 1) + seed + i) % 9 {
                    0..=1 => 0,
                    2..=4 => u8::MAX,
                    _ => ((x * 37 + i * 11 + seed) % 254 + 1) as u8,
                })
                .collect::<Vec<_>>();
            let mut row = vec![0; i % 5];
            row.extend(coverage);
            row.extend(vec![0; i % 3]);
            (20 + (i as i32 * 7 + seed as i32) % 90, y, row)
        })
        .collect();
    rows.push((-30, 150, vec![200; 80]));
    rows.push((430, 151, vec![u8::MAX; 80]));
    rows
}

fn blend_into(fb: &mut FB, rows: &[(i32, i32, Vec<u8>)], columns: core::ops::Range<i32>) {
    for (x, y, coverage) in rows {
        let (from, to) = (
            (*x).max(columns.start),
            (x + coverage.len() as i32).min(columns.end),
        );
        if from < to {
            fb.blend_row(
                from,
                *y,
                &coverage[(from - x) as usize..(to - x) as usize],
                chrome::LIME,
            );
        }
    }
}

fn background() -> Box<FB> {
    let mut fb = FB::boxed();
    fb.fill_solid(&chrome::DISPLAY_BBOX, chrome::BLUE).unwrap();
    fb.fill_solid(
        &Rectangle::new(Point::new(0, 120), Size::new(466, 10)),
        chrome::RED,
    )
    .unwrap();
    fb
}

#[test]
fn a_recording_draws_what_was_recorded() {
    let (first, second) = (rows(1), rows(4));
    let mut recording = Recording::with_capacity(0);
    for (x, y, coverage) in &first {
        recording.blend_row(*x, *y, coverage, chrome::WHITE);
    }
    let first_part = recording.part();
    for (x, y, coverage) in &second {
        recording.blend_row(*x, y + 60, coverage, chrome::WHITE);
    }
    let second_part = recording.part();
    let second: Vec<_> = second.into_iter().map(|(x, y, c)| (x, y + 60, c)).collect();

    for columns in [i32::MIN..i32::MAX, 65..131] {
        let mut drawn = background();
        recording.draw(
            core::slice::from_ref(&first_part),
            columns.clone(),
            chrome::LIME,
            &mut *drawn,
        );
        let mut expected = background();
        blend_into(&mut expected, &first, columns.clone());
        assert!(
            drawn.buffer() == expected.buffer(),
            "the first part in {columns:?}"
        );

        recording.draw(
            core::slice::from_ref(&second_part),
            columns.clone(),
            chrome::LIME,
            &mut *drawn,
        );
        blend_into(&mut expected, &second, columns.clone());
        assert!(
            drawn.buffer() == expected.buffer(),
            "both parts in {columns:?}"
        );

        let mut together = background();
        recording.draw(
            &[first_part.clone(), second_part.clone()],
            columns.clone(),
            chrome::LIME,
            &mut *together,
        );
        assert!(
            together.buffer() == expected.buffer(),
            "both parts together in {columns:?}"
        );
    }
}

#[test]
fn parts_side_by_side_draw_together_as_each_alone() {
    // Two glyphs' worth of rows on the same rows, apart in columns, each row starting a little
    // further in.
    let side = |left: i32, seed: usize| -> Vec<(i32, i32, Vec<u8>)> {
        (300..330)
            .map(|y| {
                let coverage = (0..60)
                    .map(|x| ((x * 13 + y as usize * 7 + seed) % 256) as u8)
                    .collect();
                (left + y % 4, y, coverage)
            })
            .collect()
    };
    let (left, right) = (side(20, 1), side(120, 2));
    let mut recording = Recording::with_capacity(0);
    for (x, y, coverage) in &left {
        recording.blend_row(*x, *y, coverage, chrome::WHITE);
    }
    let left_part = recording.part();
    for (x, y, coverage) in &right {
        recording.blend_row(*x, *y, coverage, chrome::WHITE);
    }
    let right_part = recording.part();
    for columns in [i32::MIN..i32::MAX, 50..150] {
        let mut together = background();
        recording.draw(
            &[left_part.clone(), right_part.clone()],
            columns.clone(),
            chrome::LIME,
            &mut *together,
        );
        let mut expected = background();
        blend_into(&mut expected, &left, columns.clone());
        blend_into(&mut expected, &right, columns.clone());
        assert!(
            together.buffer() == expected.buffer(),
            "side by side in {columns:?}"
        );
    }
}
