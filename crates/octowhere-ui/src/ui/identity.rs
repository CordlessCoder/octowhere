//! The start-up's identity and logo card, played at 30 fps after a self-test that every part
//! passed, and again from the device page. The identity opens on a grid of blocks unlocking
//! beside BOOT, then types the title in outline over two scatter fields while registration marks
//! build and clear, flickers it filled, and settles with a small logo. The card follows in 19
//! frames of its own.
//!
//! `context/design/` has the design: G19's identity with G17's opening, marks and card. The
//! frame-by-frame timing is in its `renderer/concept/startup_s1_g17.py` and
//! `startup-g19-matched/matched_scatter.py`.

use core::fmt::Write as _;

use embedded_graphics::{
    draw_target::DrawTarget,
    prelude::{Dimensions, Point, Size},
    primitives::Rectangle,
    Pixel,
};

use super::{
    clock::ClockView,
    scatter::{Field, Look, Scatter, Shown},
    screens, smooth,
    startup::{self, Context, Mark, CARD_MARK, CARD_SCALE, CENTER, PAGE_RADIUS},
    text,
};
use crate::chrome::{
    self, Color, CoverageTarget, Dirty, FontdueRenderer, OnBackground, Round, Window, FRAKTION, INTERFERENCE_BOLD,
    SHAPIRO,
};

/// The identity's frames, then the card's.
pub const FRAMES: u32 = 120;
pub const CARD_FRAMES: u32 = 19;

const FRAME_MS: f32 = 1000.0 / 30.0;

// The opening: twelve blocks whose tongues slide right to meet their neighbours, beside BOOT,
// over a dim field of blocks.

const OPEN_FRAMES: u32 = 13;
const BLOCK: i32 = 48;
const BLOCK_PITCH: Point = Point::new(100, 79);
const FIRST_BLOCK: Point = Point::new(83, 68);
/// Each row of blocks is LIME at its own level, of 255.
const BLOCK_LEVELS: [u8; 4] = [245, 209, 232, 255];
/// How far a block's tongue has slid on each frame of its unlocking, the last where it meets
/// the next block.
const TONGUE_SHIFTS: [i32; 5] = [0, 9, 17, 24, 31];
const BACKDROP_BLOCKS: usize = 185;
const BACKDROP_LEVELS: [u8; 3] = [26, 33, 46];
const REGISTRATION: [Point; 4] = [Point::new(55, 48), Point::new(411, 48), Point::new(55, 418), Point::new(411, 418)];
const REGISTRATION_LEVEL: u8 = 74;
const BOOT: &str = "BOOT";
const BOOT_PX: u32 = 38;
const BOOT_CENTRE: Point = Point::new(407, 233);
/// BOOT shows dim from this frame, and at full from the next but one.
const BOOT_FROM: u32 = 2;
const BOOT_DIM: u8 = 140;

// After the opening.

const WORD: &str = "OCTOWHERE";
const WORD_PX: u32 = 49;
/// The title's outline, and the glyphs' type-in: each glyph's outline fades up over `RISE` ms,
/// a glyph every `INTERVAL` ms from `TYPE_FROM` ms.
const HOLLOW: u8 = 2;
const TYPE_FROM: f32 = 430.0;
const INTERVAL: f32 = 68.0;
const RISE: f32 = 55.0;
/// The outlined title and the ticks before they light: LIME at this level.
const DIM: u8 = 87;
const PARTIAL: u8 = 148;
/// Where the title shows only slices of its fill, as fractions of its ink's width.
const SLICES: [f32; 8] = [0.055, 0.15, 0.29, 0.42, 0.55, 0.69, 0.82, 0.95];
const SLICE: u32 = 2;
const FLICKER_FROM: u32 = 42;
const LOGO_FROM: u32 = 56;
const TICKS_FROM: u32 = 36;
const TICKS: [i32; 2] = [23, 443];
const LOGO_CORNER: Point = Point::new(407, 180);
/// The small logo's module, in tenths of a pixel.
const LOGO_MODULE: i32 = 13;
/// The mark as the microtext row and the small logo draw it, 15 × 15.
const MARK_ROWS: [u16; 15] = [
    0b111000111000111,
    0b100000000000001,
    0b100000000000001,
    0b000111111111000,
    0b000100000001000,
    0b000100000001000,
    0b100100000001001,
    0b100100000001001,
    0b100100000001001,
    0b000100000001000,
    0b000100000001000,
    0b000111111111000,
    0b100000000000001,
    0b100000000000001,
    0b111000111000111,
];

// The registration marks, one per corner, 145 px out from the centre on each axis.

const MARKS_FROM: u32 = 13;
const MARK_REACH: i32 = 145;
/// Each corner's frames for its horizontal arm, vertical arm, centre, and the square's
/// horizontal and vertical fragments, by the corner's side of centre.
const MARK_TIMING: [((i32, i32), [u32; 5]); 4] = [
    ((-1, 1), [13, 19, 14, 27, 35]),
    ((1, -1), [43, 16, 23, 37, 29]),
    ((1, 1), [27, 23, 28, 35, 31]),
    ((-1, -1), [41, 29, 29, 31, 38]),
];
/// The squares show over these frames, hopping out across then down, each hop settling over
/// four frames.
const SQUARES: core::ops::Range<u32> = 21..61;
const HOP: [i32; 4] = [16, 6, 2, 0];
const HOP_DOWN_FROM: u32 = 38;
const FRAGMENTS_UNTIL: [u32; 2] = [55, 58];
const ARMS_UNTIL: u32 = 64;
const MARK_LEVEL: u8 = 82;
const HAIR_LEVEL: u8 = 64;

// The microtext row, under the title.

const ROW_FROM: u32 = 21;
const MICRO_TOP: i32 = 268;
const BARCODE_LEFT: i32 = 70;
const MICRO_MARK_LEFT: i32 = 147;
const DIGITS_LEFT: i32 = 173;
const MICRO_GNSS_LEFT: i32 = 237;
const MICRO_TEXT_LEFT: i32 = 263;
const MICRO_TEXT_TOPS: [i32; 2] = [266, 282];
const MICRO_MODULE: i32 = 3;
const IDENTITY_DIGITS: Rectangle = Rectangle::new(Point::new(DIGITS_LEFT, MICRO_TOP), Size::new(51, 15));

// The scatter: an upper field that turns a step on five late frames, and a lower one that
// holds still, from the end of the opening.

const UPPER: Field = Field { center: Point::new(270, 145), radius: 150.0, seed: 0x6f63_7475 };
const LOWER: Field = Field { center: Point::new(190, 334), radius: 125.0, seed: 0x6f63_7476 };
const UPPER_LOOK: Look = Look { facing: -0.65, density: 0.65 };
const LOWER_LOOK: Look = Look { facing: 2.55, density: 0.40 };
const TURN_FROM: u32 = 66;
const TURN_EVERY: u32 = 11;
const TURNS: u32 = 5;
const TURN: f32 = 0.09;
const SCATTER_LEVEL: u8 = 69;

/// From this frame only the scatter's turns and the clock's digits change. Before it, a frame
/// redraws everything.
const SETTLED: u32 = LOGO_FROM + 11;
const _: () = assert!(
    ARMS_UNTIL <= SETTLED && FLICKER_FROM + 11 <= SETTLED && ROW_FROM + 5 <= SETTLED && TURN_FROM + TURN_EVERY >= SETTLED
);

/// How a flickering element shows, some frames after its flicker starts: on for two, off for
/// two, on for one, off for two, on for two, partly on for two, then on.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Lit {
    Before,
    On,
    Off,
    Partial,
}

fn lit(frame: u32, from: u32) -> Lit {
    match frame.checked_sub(from) {
        None => Lit::Before,
        Some(0..2 | 4 | 7..9) => Lit::On,
        Some(2..4 | 5..7) => Lit::Off,
        Some(9..11) => Lit::Partial,
        Some(_) => Lit::On,
    }
}

fn scatter() -> Scatter {
    Scatter {
        origin: Point::new(12, -2),
        gap: Some((197..=317, 2)),
        color: chrome::shade(chrome::PURPLE, SCATTER_LEVEL),
        fields: &[UPPER, LOWER],
    }
}

fn looks(frame: u32) -> [Look; 2] {
    let turns = frame.checked_sub(TURN_FROM).map_or(0, |since| (since / TURN_EVERY + 1).min(TURNS));
    [Look { facing: UPPER_LOOK.facing + TURN * turns as f32, ..UPPER_LOOK }, LOWER_LOOK]
}

/// The scatter's points on the last frame [`IdentityMarks::changes`] saw, so each frame's damage
/// works out only its own, and the digits it saw.
#[derive(Clone, Debug, Default)]
pub struct IdentityMarks {
    scatter: Option<(u32, Shown)>,
    digits: Option<Option<[u8; 4]>>,
}

impl IdentityMarks {
    /// Adds what the identity changes from frame `before` to frame `after`, which may be the
    /// same: everything until it settles, then the marks that appear or go, and the digits if
    /// the clock moved them.
    pub fn changes(&mut self, before: u32, after: u32, clock: &ClockView, changed: &mut Dirty) {
        let digits = Some(startup::utc_digits(clock));
        if self.digits != digits {
            changed.add(IDENTITY_DIGITS);
            self.digits = digits;
        }
        if before == after {
            return;
        }
        if before.min(after) < SETTLED {
            changed.make_full();
            return;
        }
        let scatter = scatter();
        let earlier = match self.scatter.take() {
            Some((frame, marks)) if frame == before => marks,
            _ => scatter.shown(&looks(before)),
        };
        let later = scatter.shown(&looks(after));
        scatter.changed(&earlier, &later, changed);
        self.scatter = Some((after, later));
    }
}

fn style(font: &FontdueRenderer<'static, Color>, color: Color, size: u32, index: usize) -> FontdueRenderer<'static, Color> {
    text::style(font, color, size, index)
}

pub fn draw_identity<D: CoverageTarget<Color = Color>>(
    frame: u32,
    answered: usize,
    context: &Context<'_>,
    font: &FontdueRenderer<'static, Color>,
    target: &mut D,
) -> Result<(), D::Error> {
    screens::clear(target)?;
    if frame < OPEN_FRAMES {
        return draw_opening(frame, font, target);
    }
    scatter().draw(&looks(frame), target)?;
    let field = &mut OnBackground::new(&mut *target, chrome::BLACK);
    draw_marks(frame, field)?;
    draw_title(frame, font, field)?;
    draw_row(frame, answered, context, font, field)?;

    let lime = |lit: Lit| match lit {
        Lit::On => chrome::LIME,
        Lit::Partial => chrome::shade(chrome::LIME, PARTIAL),
        Lit::Before | Lit::Off => chrome::shade(chrome::LIME, DIM),
    };
    if frame >= TICKS_FROM {
        let lit = lit(frame, FLICKER_FROM);
        for x in TICKS {
            field.fill_solid(&Rectangle::new(Point::new(x - 5, 232), Size::new(11, 2)), lime(lit))?;
            if lit != Lit::Partial {
                field.fill_solid(&Rectangle::new(Point::new(x, 228), Size::new(2, 10)), lime(lit))?;
            }
        }
    }
    let logo = lit(frame, LOGO_FROM);
    if matches!(logo, Lit::On | Lit::Partial) {
        let at = |i: i32| (i * LOGO_MODULE + 5) / 10;
        for (j, bits) in MARK_ROWS.iter().enumerate() {
            let j = j as i32;
            for i in (0..15).filter(|i| bits & (1 << (14 - i)) != 0) {
                if logo == Lit::Partial && ![0, 7, 14].contains(&i) {
                    continue;
                }
                let corner = LOGO_CORNER + Point::new(at(i), at(j));
                let size = Size::new((at(i + 1) - at(i)) as u32, (at(j + 1) - at(j)) as u32);
                field.fill_solid(&Rectangle::new(corner, size), chrome::LIME)?;
            }
        }
    }
    Ok(())
}

/// A fixed sequence of numbers for the backdrop, the same every time.
struct Numbers(u32);

impl Numbers {
    fn below(&mut self, n: u32) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0 % n
    }
}

fn draw_opening<D: CoverageTarget<Color = Color>>(
    frame: u32,
    font: &FontdueRenderer<'static, Color>,
    target: &mut D,
) -> Result<(), D::Error> {
    let mut numbers = Numbers(17_420);
    for _ in 0..BACKDROP_BLOCKS {
        let x = (15 + numbers.below(430) as i32) / 8 * 8;
        let y = (15 + numbers.below(430) as i32) / 8 * 8;
        let size = Size::new([8, 16, 24][numbers.below(3) as usize], [2, 8][numbers.below(2) as usize]);
        let level = BACKDROP_LEVELS[numbers.below(3) as usize];
        target.fill_solid(&Rectangle::new(Point::new(x, y), size), chrome::shade(chrome::DEEP_BLUE, level))?;
    }
    let registration = chrome::shade(chrome::DEEP_BLUE, REGISTRATION_LEVEL);
    for centre in REGISTRATION {
        target.fill_solid(&Rectangle::new(centre - Point::new(6, 0), Size::new(13, 1)), registration)?;
        target.fill_solid(&Rectangle::new(centre - Point::new(0, 6), Size::new(1, 13)), registration)?;
    }
    for row in 0..4 {
        for column in 0..3 {
            let corner = FIRST_BLOCK + Point::new(column * BLOCK_PITCH.x, row * BLOCK_PITCH.y);
            let ink = chrome::shade(chrome::LIME, BLOCK_LEVELS[row as usize]);
            let phase = frame as i32 - (4 + row + column);
            target.fill_solid(&Rectangle::new(corner, Size::new_equal(BLOCK as u32)), ink)?;
            // The socket the tongue leaves: a notch as it opens, then clear to the block's edge.
            match phase {
                0 => target.fill_solid(&Rectangle::new(corner + Point::new(25, 17), Size::new(6, 14)), chrome::BLACK)?,
                1.. => target.fill_solid(&Rectangle::new(corner + Point::new(25, 17), Size::new(24, 14)), chrome::BLACK)?,
                _ => {}
            }
            let shift = TONGUE_SHIFTS[phase.clamp(0, 4) as usize];
            target.fill_solid(&Rectangle::new(corner + Point::new(31 + shift, 17), Size::new(38, 14)), ink)?;
        }
    }
    if frame >= BOOT_FROM {
        let color = if frame >= BOOT_FROM + 2 { chrome::DEEP_BLUE } else { chrome::shade(chrome::DEEP_BLUE, BOOT_DIM) };
        let style = style(font, color, BOOT_PX, INTERFERENCE_BOLD);
        // Upright, the ink's top-left is at (left, top) from the pen; turned, it runs down from
        // the pen with the ink's top to the right.
        let ink = style.baseline_bounds(BOOT, Point::zero());
        let (width, height) = (ink.size.width as f32, ink.size.height as f32);
        let pen = Point::new(
            libm::roundf(BOOT_CENTRE.x as f32 + ink.top_left.y as f32 + height / 2.0) as i32,
            libm::roundf(BOOT_CENTRE.y as f32 - ink.top_left.x as f32 - width / 2.0) as i32,
        );
        style.draw_turned_on_baseline(BOOT, pen, target)?;
    }
    Ok(())
}

fn draw_marks<D: DrawTarget<Color = Color>>(frame: u32, target: &mut D) -> Result<(), D::Error> {
    if frame < MARKS_FROM {
        return Ok(());
    }
    let (mark, hair) = (chrome::shade(chrome::GRAY, MARK_LEVEL), chrome::shade(chrome::GRAY, HAIR_LEVEL));
    let span = |target: &mut D, x0: i32, y0: i32, x1: i32, y1: i32, color: Color| {
        target.fill_solid(&Rectangle::with_corners(Point::new(x0.min(x1), y0.min(y1)), Point::new(x0.max(x1), y0.max(y1))), color)
    };
    let hop = |from: u32| HOP[frame.saturating_sub(from).min(3) as usize];
    for ((sx, sy), [across, down, centre, inner_across, inner_down]) in MARK_TIMING {
        let (cx, cy) = (CENTER.x + sx * MARK_REACH, CENTER.y + sy * MARK_REACH);
        let (qx, qy) = (cx - sx * (40 + hop(SQUARES.start)), cy - sy * (34 + hop(HOP_DOWN_FROM)));
        // Both fragments register to the square's corner that faces the centre.
        let (corner_x, corner_y) = (qx - sx * 5, qy - sy * 5);
        if (inner_across..FRAGMENTS_UNTIL[0]).contains(&frame) {
            span(target, qx + sx * 28, corner_y, qx + sx * 10, corner_y, hair)?;
        }
        if (inner_down..FRAGMENTS_UNTIL[1]).contains(&frame) {
            span(target, corner_x, qy + sy * 10, corner_x, qy + sy * 30, hair)?;
        }
        if SQUARES.contains(&frame) {
            target.fill_solid(&Rectangle::new(Point::new(qx - 5, qy - 5), Size::new_equal(10)), mark)?;
        }
        if (across..ARMS_UNTIL).contains(&frame) {
            span(target, cx - 28, cy, cx + 28, cy, hair)?;
        }
        if (down..ARMS_UNTIL).contains(&frame) {
            span(target, cx, cy - 28, cx, cy + 28, hair)?;
        }
        if (centre..ARMS_UNTIL).contains(&frame) {
            target.fill_solid(&Rectangle::new(Point::new(cx - 1, cy - 1), Size::new_equal(2)), mark)?;
        }
    }
    Ok(())
}

fn smoothstep(v: f32) -> f32 {
    let v = v.clamp(0.0, 1.0);
    v * v * (3.0 - 2.0 * v)
}

fn draw_title<D: CoverageTarget<Color = Color>>(
    frame: u32,
    font: &FontdueRenderer<'static, Color>,
    target: &mut D,
) -> Result<(), D::Error> {
    let filled = style(font, chrome::LIME, WORD_PX, SHAPIRO);
    let pen = Point::new(
        text::pen_x_for_ink_centre(&filled, WORD, CENTER.x as f32),
        text::baseline_for_ink_middle(&filled, WORD, CENTER.y as f32),
    );
    let hollow = |level: u8| style(font, chrome::shade(chrome::LIME, level), WORD_PX, SHAPIRO);
    match lit(frame, FLICKER_FROM) {
        Lit::Before => {
            let ms = frame as f32 * FRAME_MS;
            for (i, _) in WORD.char_indices() {
                let k = smoothstep((ms - TYPE_FROM - i as f32 * INTERVAL) / RISE);
                if k <= 0.0 {
                    break;
                }
                let at = pen + Point::new(libm::roundf(filled.advance(&WORD[..i])) as i32, 0);
                hollow(libm::roundf(f32::from(DIM) * k) as u8).draw_hollow_on_baseline(&WORD[i..=i], at, HOLLOW, target)?;
            }
            Ok(())
        }
        Lit::Off => hollow(DIM).draw_hollow_on_baseline(WORD, pen, HOLLOW, target),
        Lit::On => filled.draw_on_baseline(WORD, pen, target),
        Lit::Partial => {
            let ink = filled.baseline_bounds(WORD, pen);
            for fraction in SLICES {
                let left = ink.top_left.x + libm::roundf(ink.size.width as f32 * fraction) as i32;
                let slice = Rectangle::new(Point::new(left, ink.top_left.y), Size::new(SLICE, ink.size.height));
                filled.draw_on_baseline(WORD, pen, &mut Window::new(&mut *target, Point::zero(), slice))?;
            }
            Ok(())
        }
    }
}

fn draw_row<D: CoverageTarget<Color = Color>>(
    frame: u32,
    answered: usize,
    context: &Context<'_>,
    font: &FontdueRenderer<'static, Color>,
    target: &mut D,
) -> Result<(), D::Error> {
    let shown = |element: u32| frame >= ROW_FROM + element;
    if shown(0) {
        startup::barcode(context.firmware, BARCODE_LEFT, MICRO_TOP, 15, chrome::LIME, target)?;
    }
    if shown(1) {
        let origin = (MICRO_MARK_LEFT as f32, MICRO_TOP as f32);
        Mark { origin, module: 3.0, stroke: 1.0 }.draw(true, false, chrome::LIME, target)?;
    }
    if shown(2) {
        let digits = startup::utc_digits(context.clock);
        for i in 0..4 {
            let rows = digits.map_or(&startup::PIXEL_DASH, |digits| &startup::PIXEL_DIGITS[usize::from(digits[i])]);
            // A space between the hours and the minutes.
            let left = DIGITS_LEFT + 12 * i as i32 + if i >= 2 { 6 } else { 0 };
            startup::modules(rows, 3, Point::new(left, MICRO_TOP), MICRO_MODULE, chrome::LIME, target)?;
        }
    }
    if shown(3) {
        startup::modules(&startup::GNSS, 5, Point::new(MICRO_GNSS_LEFT, MICRO_TOP), MICRO_MODULE, chrome::LIME, target)?;
    }
    if shown(4) {
        let style = style(font, chrome::LIME, 14, FRAKTION);
        let mut version = heapless::String::<32>::new();
        let _ = write!(version, "VERSION {}", context.firmware);
        let mut count = heapless::String::<24>::new();
        let _ = write!(count, "SELF TEST {answered}/6 OK");
        for (line, top) in [version.as_str(), count.as_str()].into_iter().zip(MICRO_TEXT_TOPS) {
            let pen = Point::new(
                text::pen_x_for_ink_left(&style, line, MICRO_TEXT_LEFT),
                text::baseline_for_ink_top(&style, line, top),
            );
            style.draw_on_baseline(line, pen, target)?;
        }
    }
    Ok(())
}

// The card.

/// The columns the card's opening frames show the mark through: one in every five.
const STRIPE_PITCH: i32 = 5;
const STRIPE_PHASE: i32 = CENTER.x % STRIPE_PITCH;
/// On those frames the tile is a thick frame: its outline and the hatch's margin.
const TILE_FRAME: (i32, i32) = (170, 195);

/// A view of `parent` that draws only on the card's stripe columns.
struct Stripes<'a, T>(&'a mut T);

impl<T: DrawTarget> Dimensions for Stripes<'_, T> {
    fn bounding_box(&self) -> Rectangle {
        self.0.bounding_box()
    }
}

impl<T: DrawTarget> DrawTarget for Stripes<'_, T> {
    type Color = T::Color;
    type Error = T::Error;

    fn draw_iter<I: IntoIterator<Item = Pixel<Self::Color>>>(&mut self, pixels: I) -> Result<(), Self::Error> {
        self.0.draw_iter(pixels.into_iter().filter(|Pixel(point, _)| point.x.rem_euclid(STRIPE_PITCH) == STRIPE_PHASE))
    }

    fn fill_solid(&mut self, area: &Rectangle, color: Self::Color) -> Result<(), Self::Error> {
        let first = area.top_left.x + (STRIPE_PHASE - area.top_left.x).rem_euclid(STRIPE_PITCH);
        for x in (first..area.top_left.x + area.size.width as i32).step_by(STRIPE_PITCH as usize) {
            self.0.fill_solid(&Rectangle::new(Point::new(x, area.top_left.y), Size::new(1, area.size.height)), color)?;
        }
        Ok(())
    }
}

pub fn draw_card<D: CoverageTarget<Color = Color>>(frame: u32, target: &mut D) -> Result<(), D::Error> {
    screens::clear(target)?;
    let page = |target: &mut D| smooth::disc_rows(target, 0..466, CENTER, PAGE_RADIUS, chrome::LIME);
    let scaled = CARD_MARK.scaled(CARD_SCALE);
    match frame {
        0..3 => {
            page(target)?;
            let stripes = &mut Stripes(&mut *target);
            CARD_MARK.draw(true, false, chrome::BLACK, stripes)?;
            let (outer, inner) = TILE_FRAME;
            let far = 2 * CENTER.x - outer;
            let near = 2 * CENTER.x - inner;
            for (x0, y0, x1, y1) in [(outer, outer, far, inner), (outer, near, far, far), (outer, inner, inner, near), (near, inner, far, near)] {
                stripes.fill_solid(&Rectangle::with_corners(Point::new(x0, y0), Point::new(x1 - 1, y1 - 1)), chrome::BLACK)?;
            }
        }
        3..9 => {
            page(target)?;
            CARD_MARK.draw(true, true, chrome::BLACK, target)?;
        }
        9..13 => CARD_MARK.draw(true, true, chrome::LIME, target)?,
        13..17 => scaled.draw(true, true, chrome::LIME, &mut Round::new(&mut *target, CENTER, PAGE_RADIUS))?,
        17 => {
            page(target)?;
            scaled.draw(true, true, chrome::BLACK, &mut Round::new(&mut *target, CENTER, PAGE_RADIUS))?;
        }
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_flicker_follows_the_reference_cadence() {
        let cadence: alloc::vec::Vec<Lit> = (0..13).map(|frame| lit(frame + 10, 10)).collect();
        use Lit::{Off, On, Partial};
        assert_eq!(cadence, [On, On, Off, Off, On, Off, Off, On, On, Partial, Partial, On, On]);
        assert_eq!(lit(9, 10), Lit::Before);
    }

    #[test]
    fn the_upper_field_turns_on_five_frames_and_the_lower_never() {
        let turned: alloc::vec::Vec<u32> = (1..FRAMES).filter(|&frame| looks(frame) != looks(frame - 1)).collect();
        assert_eq!(turned, [66, 77, 88, 99, 110]);
    }
}
