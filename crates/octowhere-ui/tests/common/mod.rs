//! What the stage tests share: drawing a stage into two framebuffers in turn as the frame loop
//! does, and counting the pixels that differ from drawing it whole.

#![allow(dead_code)]

use embedded_graphics::{
    prelude::{Point, Size},
    primitives::Rectangle,
};
use octowhere_ui::{
    chrome::{Clip, Dirty, FB},
    ui::stage::Stage,
};

/// Two framebuffers drawn in turn as the frame loop draws them: each repaints the damage of the
/// step before as well as its own, since it last saw the frame before that.
pub struct Buffers {
    fbs: [Box<FB>; 2],
    next: usize,
    pub previous: Dirty,
    drawn: [bool; 2],
    /// Pixels repainted, over every step.
    pub pixels: u64,
    /// What the panel shows: each buffer's flush rectangles copied in after it is drawn.
    pub panel: Box<FB>,
    flushed: bool,
}

impl Buffers {
    pub fn new() -> Self {
        Self {
            fbs: [FB::boxed(), FB::boxed()],
            next: 0,
            previous: Dirty::new_full(),
            drawn: [false; 2],
            pixels: 0,
            panel: FB::boxed(),
            flushed: false,
        }
    }

    /// Draws `stage` after a step that changed `changed`, and returns the buffer drawn into.
    pub fn draw(&mut self, stage: &Stage, changed: &Dirty) -> &FB {
        let mut repaint = self.previous.clone();
        repaint.extend(changed);
        self.previous = changed.clone();
        let index = self.next;
        self.next ^= 1;
        let fb = &mut *self.fbs[index];
        if repaint.is_full() || !self.drawn[index] {
            self.drawn[index] = true;
            self.pixels += 466 * 466;
            stage.draw(fb);
        } else if !repaint.is_empty() {
            self.pixels += u64::from(repaint.pixels());
            stage.draw(&mut Clip::new(fb, &repaint));
        }
        // The panel shows the step before, so the flush sends only this step's damage.
        let flush = if self.flushed {
            changed.clone()
        } else {
            Dirty::new_full()
        };
        self.flushed = true;
        let rows = |rect: Rectangle| {
            (rect.top_left.y..rect.top_left.y + rect.size.height as i32).map(move |y| {
                let start = (y * 466 + rect.top_left.x) as usize * 2;
                start..start + rect.size.width as usize * 2
            })
        };
        let whole = Rectangle::new(Point::zero(), Size::new(466, 466));
        let rects: Vec<_> = if flush.is_full() {
            vec![whole]
        } else {
            flush
                .rectangles(octowhere_ui::chrome::FLUSH_OVERHEAD)
                .collect()
        };
        for rect in rects {
            for range in rows(rect) {
                self.panel.buffer_mut()[range.clone()].copy_from_slice(&fb.buffer()[range]);
            }
        }
        &self.fbs[index]
    }
}

/// How many pixels differ on the round panel, out to as far past its edge as the pixel shift
/// can bring onto it. The square's corners beyond are never seen or cleared, and a page moving
/// across them leaves what it drew there.
pub fn differing(a: &FB, b: &FB) -> usize {
    differing_within(a, b, 233.0 + octowhere_ui::ui::shift::REACH as f32)
}

/// [`differing`], but on the glass alone while the start-up shows, since it always shows
/// unshifted.
pub fn differing_as_shown(stage: &Stage, a: &FB, b: &FB) -> usize {
    let reach = if stage.starting_up() {
        0.0
    } else {
        octowhere_ui::ui::shift::REACH as f32
    };
    differing_within(a, b, 233.0 + reach)
}

pub fn differing_within(a: &FB, b: &FB, radius: f32) -> usize {
    let inside = |point: Point| {
        let (x, y) = (point.x as f32 + 0.5 - 233.0, point.y as f32 + 0.5 - 233.0);
        x * x + y * y <= radius * radius
    };
    // Pixel by pixel, unoptimised, this was most of every damage test's time, so each row's bytes
    // are compared first: whole, then from its first pixel inside to its last, which holds every
    // pixel inside.
    let row = 466 * 2;
    a.buffer()
        .chunks_exact(row)
        .zip(b.buffer().chunks_exact(row))
        .enumerate()
        .filter(|(_, (a_row, b_row))| a_row != b_row)
        .map(|(y, (a_row, b_row))| {
            let point = |x| Point::new(x, y as i32);
            let Some(first) = (0..466).find(|&x| inside(point(x))) else {
                return 0;
            };
            let end = (0..466).rfind(|&x| inside(point(x))).unwrap() + 1;
            let span = first as usize * 2..end as usize * 2;
            if a_row[span.clone()] == b_row[span] {
                return 0;
            }
            (first..end)
                .map(point)
                .filter(|&point| inside(point) && a.pixel(point) != b.pixel(point))
                .count()
        })
        .sum()
}
