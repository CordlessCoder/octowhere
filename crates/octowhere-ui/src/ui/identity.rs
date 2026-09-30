//! The start-up's identity and logo card, played at 30 fps after a self-test that every part
//! passed, and again from the device page. The identity opens on a grid of blocks unlocking
//! beside BOOT, then types the title in outline over two scatter fields while registration marks
//! build and clear, flickers it filled, and settles with a small logo. The card follows in 19
//! frames of its own.
//!
//! `context/design/` has the design: G19's identity with G17's opening, marks and card. The
//! frame-by-frame timing is in its `renderer/concept/startup_s1_g17.py` and
//! `startup-g19-matched/matched_scatter.py`.

use alloc::{vec, vec::Vec};
use core::fmt::Write as _;

use embedded_graphics::{
    Pixel,
    draw_target::DrawTarget,
    prelude::{Dimensions, Point, Size},
    primitives::Rectangle,
};

use super::{
    clock::ClockView,
    scatter::{Field, Law, Look, Scatter, Shown},
    screens, smooth,
    startup::{self, CENTER, Context},
    text,
};
use crate::chrome::{
    self, Color, CoverageTarget, Dirty, FRAKTION, FontdueRenderer, INTERFERENCE_BOLD, MARATYPE,
    OnBackground,
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
const REGISTRATION: [Point; 4] = [
    Point::new(55, 48),
    Point::new(411, 48),
    Point::new(55, 418),
    Point::new(411, 418),
];
const REGISTRATION_LEVEL: u8 = 74;
const BOOT: &str = "BOOT";
const BOOT_PX: u32 = 38;
const BOOT_CENTRE: Point = Point::new(407, 233);
/// BOOT shows dim from this frame, and at full from the next but one.
const BOOT_FROM: u32 = 2;
const BOOT_DIM: u8 = 140;

// After the opening.

const WORD: &str = "OCTOWHERE";
/// Maratype at its natural proportions, its ink over `(33, 177)..(433, 289)`.
const WORD_PX: u32 = 112;
/// The title's outline, and the glyphs' type-in: each glyph's outline fades up over `RISE` ms,
/// a glyph every `INTERVAL` ms from `TYPE_FROM` ms.
const HOLLOW: u8 = 2;
const TYPE_FROM: f32 = 430.0;
const INTERVAL: f32 = 68.0;
const RISE: f32 = 55.0;
/// The outlined title and the pluses before they light: LIME at this level.
const DIM: u8 = 87;
const PARTIAL: u8 = 148;
/// Where the title shows only slices of its fill, as fractions of its ink's width.
const SLICES: [f32; 8] = [0.055, 0.15, 0.29, 0.42, 0.55, 0.69, 0.82, 0.95];
const SLICE: u32 = 2;
const FLICKER_FROM: u32 = 42;
const PIN_FROM: u32 = 56;
const PLUSES_FROM: u32 = 36;
/// The pluses' centres, 10 px out from the title ink's corners on the diagonal. Each arm reaches
/// 3 px from the centre pixel.
const PLUSES: [Point; 2] = [Point::new(23, 167), Point::new(442, 298)];
/// The small pin's top-left, just off the title ink's top-right corner, and its unit.
const PIN_CORNER: (f32, f32) = (437.0, 175.0);
const PIN_UNIT: f32 = 1.17;
/// While the pin flickers partly on, three upright stems of it show: each unit's column, top
/// row and height.
const PIN_STEMS: [(f32, f32, f32); 3] = [(0.0, 3.0, 4.0), (6.0, 3.0, 6.0), (11.0, 3.0, 4.0)];

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

// The row under the title: barcode, square, digits, GNSS and two lines of copy, each 25 px
// high and 8 px apart, together as wide as the title's ink.

const ROW_FROM: u32 = 21;
const ROW_TOP: i32 = 311;
const ROW_HEIGHT: f32 = 25.0;
const ROW_LEFT: f32 = 33.0;
const ROW_GAP: f32 = 8.0;
/// The version barcode, its 67 px of bars and spaces widened to this.
const BARCODE_WIDTH: f32 = 113.625;
const SQUARE_LEFT: i32 = 155;
const SQUARE: i32 = 25;
const DIGITS_LEFT: f32 = ROW_LEFT + BARCODE_WIDTH + ROW_GAP + SQUARE as f32 + ROW_GAP;
/// The digits' modules, and the step from one digit to the next, with twice the gap between
/// the hours and the minutes.
const DIGIT_MODULE: (f32, f32) = (4.0, 5.0);
const DIGIT_STEP: f32 = 16.0;
const GNSS_LEFT: f32 = DIGITS_LEFT + 4.0 * DIGIT_STEP + 8.0 + ROW_GAP;
const GNSS_MODULE: f32 = 5.0;
/// From the row's left to past the GNSS symbol, the columns the barcode, digits and symbol
/// share.
const ROW_LINE: usize = (GNSS_LEFT + 5.0 * GNSS_MODULE) as usize + 1 - ROW_LEFT as usize;
const COPY_LEFT: i32 = 300;
const COPY_TOPS: [i32; 2] = [309, 325];
/// Past the minutes' last module.
const DIGITS_RIGHT: f32 = DIGITS_LEFT + 3.0 * DIGIT_STEP + 8.0 + 3.0 * DIGIT_MODULE.0;
const IDENTITY_DIGITS: Rectangle = Rectangle::new(
    Point::new(DIGITS_LEFT as i32, ROW_TOP),
    Size::new(
        DIGITS_RIGHT as u32 + 1 - DIGITS_LEFT as u32,
        ROW_HEIGHT as u32,
    ),
);
/// The square's dot pulses from dark on this frame, over `DOT_PERIOD` frames.
const DOT_FROM: u32 = ROW_FROM + 1;
const DOT_PERIOD: u32 = 30;
const DOT: Rectangle = Rectangle::new(Point::new(SQUARE_LEFT + 10, ROW_TOP + 10), Size::new(5, 5));

// The scatter: an upper field that turns a step on five late frames, and a lower one that
// holds still, from the end of the opening.

const UPPER: Field = Field {
    center: Point::new(270, 145),
    radius: 150.0,
    seed: 0x6f63_7475,
    law: Law::Radial,
};
const LOWER: Field = Field {
    center: Point::new(190, 334),
    radius: 125.0,
    seed: 0x6f63_7476,
    law: Law::Radial,
};
const UPPER_LOOK: Look = Look {
    facing: -0.65,
    density: 0.65,
};
const LOWER_LOOK: Look = Look {
    facing: 2.55,
    density: 0.40,
};
const TURN_FROM: u32 = 66;
const TURN_EVERY: u32 = 11;
const TURNS: u32 = 5;
const TURN: f32 = 0.09;
const SCATTER_LEVEL: u8 = 69;

/// From this frame only the scatter's turns and the clock's digits change. Before it, a frame
/// redraws everything.
const SETTLED: u32 = PIN_FROM + 11;
const _: () = assert!(
    ARMS_UNTIL <= SETTLED
        && FLICKER_FROM + 11 <= SETTLED
        && ROW_FROM + 5 <= SETTLED
        && TURN_FROM + TURN_EVERY >= SETTLED
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
        gap: Some((162..=340, 2)),
        color: chrome::shade(chrome::PURPLE, SCATTER_LEVEL),
        tones: &[],
        fields: &[UPPER, LOWER],
    }
}

fn looks(frame: u32) -> [Look; 2] {
    let turns = frame
        .checked_sub(TURN_FROM)
        .map_or(0, |since| (since / TURN_EVERY + 1).min(TURNS));
    [
        Look {
            facing: UPPER_LOOK.facing + TURN * turns as f32,
            ..UPPER_LOOK
        },
        LOWER_LOOK,
    ]
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
        if dot_level(before) != dot_level(after) {
            changed.add(DOT);
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

fn style(
    font: &FontdueRenderer<'static, Color>,
    color: Color,
    size: u32,
    index: usize,
) -> FontdueRenderer<'static, Color> {
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
        // The opening costs little, so it builds the title a few pieces a frame for the frames
        // after it.
        let pieces = (frame as usize + 1) * TITLE_PIECES / OPEN_FRAMES as usize;
        Title::take(pieces, font).keep();
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
    if frame >= PLUSES_FROM {
        let lit = lit(frame, FLICKER_FROM);
        for centre in PLUSES {
            field.fill_solid(
                &Rectangle::new(centre - Point::new(3, 0), Size::new(7, 1)),
                lime(lit),
            )?;
            if lit != Lit::Partial {
                field.fill_solid(
                    &Rectangle::new(centre - Point::new(0, 3), Size::new(1, 7)),
                    lime(lit),
                )?;
            }
        }
    }
    let (x, y) = PIN_CORNER;
    match lit(frame, PIN_FROM) {
        Lit::On => Pin {
            corner: PIN_CORNER,
            unit: PIN_UNIT,
        }
        .draw(chrome::LIME, field)?,
        Lit::Partial => {
            for (column, row, height) in PIN_STEMS {
                let left = x + column * PIN_UNIT;
                smooth::rect(
                    field,
                    (left, y + row * PIN_UNIT),
                    (left + 1.0, y + (row + height) * PIN_UNIT),
                    chrome::LIME,
                )?;
            }
        }
        Lit::Before | Lit::Off => {}
    }
    Ok(())
}

/// The map pin: a round head over a point, 12 units wide and 14 high, with an octagonal hole
/// centred in the head.
#[derive(Clone, Copy, Debug)]
struct Pin {
    corner: (f32, f32),
    unit: f32,
}

impl Pin {
    /// The pin as large as the impact frames show it, `scale` times over, about the centre.
    fn impact(scale: f32) -> Self {
        let unit = 17.5 * scale;
        Self {
            corner: (CENTER.x as f32 - 6.0 * unit, CENTER.y as f32 - 7.0 * unit),
            unit,
        }
    }

    fn draw<D: CoverageTarget<Color = Color>>(
        self,
        color: Color,
        target: &mut D,
    ) -> Result<(), D::Error> {
        let (left, top, unit) = (self.corner.0, self.corner.1, self.unit);
        // Half-widths about the centre line, in units down from the top: the head's half-circle
        // to 6, straight sides to 8, then the point at 14; the hole's octagon from 3 to 9.
        let units = |y: f32| (y - top) / unit;
        let outer = |y: f32| match units(y) {
            v if !(0.0..14.0).contains(&v) => 0.0,
            v if v < 6.0 => unit * libm::sqrtf(36.0 - (6.0 - v) * (6.0 - v)),
            v if v < 8.0 => 6.0 * unit,
            v => unit * (14.0 - v),
        };
        let hole = |y: f32| match units(y) {
            v if !(3.0..9.0).contains(&v) => 0.0,
            v if v < 5.0 => unit * (v - 2.0),
            v if v < 7.0 => 3.0 * unit,
            v => unit * (10.0 - v),
        };
        let rows = libm::floorf(top) as i32..libm::ceilf(top + 14.0 * unit) as i32;
        smooth::symmetric(target, left + 6.0 * unit, rows, outer, hole, color)
    }
}

/// How bright the square's dot is on `frame`, of 255: dark on its first frame, full half a
/// period later.
fn dot_level(frame: u32) -> u8 {
    let Some(since) = frame.checked_sub(DOT_FROM) else {
        return 0;
    };
    let phase = (since % DOT_PERIOD) as f32 / DOT_PERIOD as f32;
    libm::roundf(127.5 * (1.0 - libm::cosf(core::f32::consts::TAU * phase))) as u8
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
        let size = Size::new(
            [8, 16, 24][numbers.below(3) as usize],
            [2, 8][numbers.below(2) as usize],
        );
        let level = BACKDROP_LEVELS[numbers.below(3) as usize];
        target.fill_solid(
            &Rectangle::new(Point::new(x, y), size),
            chrome::shade(chrome::DEEP_BLUE, level),
        )?;
    }
    let registration = chrome::shade(chrome::DEEP_BLUE, REGISTRATION_LEVEL);
    for centre in REGISTRATION {
        target.fill_solid(
            &Rectangle::new(centre - Point::new(6, 0), Size::new(13, 1)),
            registration,
        )?;
        target.fill_solid(
            &Rectangle::new(centre - Point::new(0, 6), Size::new(1, 13)),
            registration,
        )?;
    }
    for row in 0..4 {
        for column in 0..3 {
            let corner = FIRST_BLOCK + Point::new(column * BLOCK_PITCH.x, row * BLOCK_PITCH.y);
            let ink = chrome::shade(chrome::LIME, BLOCK_LEVELS[row as usize]);
            let phase = frame as i32 - (4 + row + column);
            target.fill_solid(&Rectangle::new(corner, Size::new_equal(BLOCK as u32)), ink)?;
            // The socket the tongue leaves: a notch as it opens, then clear to the block's edge.
            match phase {
                0 => target.fill_solid(
                    &Rectangle::new(corner + Point::new(25, 17), Size::new(6, 14)),
                    chrome::BLACK,
                )?,
                1.. => target.fill_solid(
                    &Rectangle::new(corner + Point::new(25, 17), Size::new(24, 14)),
                    chrome::BLACK,
                )?,
                _ => {}
            }
            let shift = TONGUE_SHIFTS[phase.clamp(0, 4) as usize];
            target.fill_solid(
                &Rectangle::new(corner + Point::new(31 + shift, 17), Size::new(38, 14)),
                ink,
            )?;
        }
    }
    if frame >= BOOT_FROM {
        let color = if frame >= BOOT_FROM + 2 {
            chrome::DEEP_BLUE
        } else {
            chrome::shade(chrome::DEEP_BLUE, BOOT_DIM)
        };
        let style = style(font, color, BOOT_PX, INTERFERENCE_BOLD);
        // Upright, the ink's top-left is at (left, top) from the pen; turned, it runs down from
        // the pen with the ink's top to the right.
        let ink = style.baseline_bounds(BOOT, Point::zero());
        let (width, height) = (ink.size.width as f32, ink.size.height as f32);
        let pen = Point::new(
            libm::roundf(BOOT_CENTRE.x as f32 + ink.top_left.y as f32 + height / 2.0) as i32,
            libm::roundf(BOOT_CENTRE.y as f32 - ink.top_left.x as f32 - width / 2.0) as i32,
        );
        style.draw_turned(BOOT, (pen.x as f32, pen.y as f32), target)?;
    }
    Ok(())
}

fn draw_marks<D: DrawTarget<Color = Color>>(frame: u32, target: &mut D) -> Result<(), D::Error> {
    if frame < MARKS_FROM {
        return Ok(());
    }
    let (mark, hair) = (
        chrome::shade(chrome::GRAY, MARK_LEVEL),
        chrome::shade(chrome::GRAY, HAIR_LEVEL),
    );
    let span = |target: &mut D, x0: i32, y0: i32, x1: i32, y1: i32, color: Color| {
        target.fill_solid(
            &Rectangle::with_corners(
                Point::new(x0.min(x1), y0.min(y1)),
                Point::new(x0.max(x1), y0.max(y1)),
            ),
            color,
        )
    };
    let hop = |from: u32| HOP[frame.saturating_sub(from).min(3) as usize];
    for ((sx, sy), [across, down, centre, inner_across, inner_down]) in MARK_TIMING {
        let (cx, cy) = (CENTER.x + sx * MARK_REACH, CENTER.y + sy * MARK_REACH);
        let (qx, qy) = (
            cx - sx * (40 + hop(SQUARES.start)),
            cy - sy * (34 + hop(HOP_DOWN_FROM)),
        );
        // Both fragments register to the square's corner that faces the centre.
        let (corner_x, corner_y) = (qx - sx * 5, qy - sy * 5);
        if (inner_across..FRAGMENTS_UNTIL[0]).contains(&frame) {
            span(target, qx + sx * 28, corner_y, qx + sx * 10, corner_y, hair)?;
        }
        if (inner_down..FRAGMENTS_UNTIL[1]).contains(&frame) {
            span(target, corner_x, qy + sy * 10, corner_x, qy + sy * 30, hair)?;
        }
        if SQUARES.contains(&frame) {
            target.fill_solid(
                &Rectangle::new(Point::new(qx - 5, qy - 5), Size::new_equal(10)),
                mark,
            )?;
        }
        if (across..ARMS_UNTIL).contains(&frame) {
            span(target, cx - 28, cy, cx + 28, cy, hair)?;
        }
        if (down..ARMS_UNTIL).contains(&frame) {
            span(target, cx, cy - 28, cx, cy + 28, hair)?;
        }
        if (centre..ARMS_UNTIL).contains(&frame) {
            target.fill_solid(
                &Rectangle::new(Point::new(cx - 1, cy - 1), Size::new_equal(2)),
                mark,
            )?;
        }
    }
    Ok(())
}

fn smoothstep(v: f32) -> f32 {
    let v = v.clamp(0.0, 1.0);
    v * v * (3.0 - 2.0 * v)
}

/// The title's coverage, filled and hollow, worked out on the first frame that draws it and
/// kept until the card: rasterizing 112 px glyphs, and hollowing them, every frame cost more
/// than a frame. The hollow title's glyphs are placed as the type-in places them one at a time.
struct Title {
    area: Rectangle,
    /// The filled word's ink, which the partly lit frames slice.
    ink: Rectangle,
    filled: Vec<u8>,
    hollow: Vec<u8>,
    /// Each glyph's columns in the hollow title.
    glyphs: [core::ops::Range<i32>; WORD.len()],
    /// Where each glyph's pen sits.
    pens: [Point; WORD.len()],
    /// How many of the [`TITLE_PIECES`] are drawn into the buffers: each glyph filled, then each
    /// hollow.
    built: usize,
}

/// The pieces of work that build the title: each glyph filled, then each glyph hollow.
const TITLE_PIECES: usize = 2 * WORD.len();

static TITLE: embassy_sync::blocking_mutex::Mutex<
    embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex,
    core::cell::RefCell<Option<Title>>,
> = embassy_sync::blocking_mutex::Mutex::new(core::cell::RefCell::new(None));

/// A coverage buffer over `area` that text draws into, each layer over the last.
struct Coverage<'a> {
    area: Rectangle,
    cells: &'a mut [u8],
}

impl Dimensions for Coverage<'_> {
    fn bounding_box(&self) -> Rectangle {
        self.area
    }
}

impl Coverage<'_> {
    fn add(&mut self, x: i32, y: i32, cover: u8) {
        let (dx, dy) = (x - self.area.top_left.x, y - self.area.top_left.y);
        if (0..self.area.size.width as i32).contains(&dx)
            && (0..self.area.size.height as i32).contains(&dy)
        {
            let cell = &mut self.cells[(dy * self.area.size.width as i32 + dx) as usize];
            *cell = cell.saturating_add(((255 - u16::from(*cell)) * u16::from(cover) / 255) as u8);
        }
    }
}

impl DrawTarget for Coverage<'_> {
    type Color = Color;
    type Error = core::convert::Infallible;

    fn draw_iter<I: IntoIterator<Item = Pixel<Color>>>(
        &mut self,
        pixels: I,
    ) -> Result<(), Self::Error> {
        for Pixel(point, _) in pixels {
            self.add(point.x, point.y, u8::MAX);
        }
        Ok(())
    }
}

impl CoverageTarget for Coverage<'_> {
    fn blend_row(&mut self, x: i32, y: i32, coverage: &[u8], _: Color) {
        for (i, &cover) in coverage.iter().enumerate() {
            self.add(x + i as i32, y, cover);
        }
    }
}

impl Title {
    fn new(font: &FontdueRenderer<'static, Color>) -> Self {
        let filled = style(font, chrome::LIME, WORD_PX, MARATYPE);
        let pen = Point::new(
            text::pen_x_for_ink_centre(&filled, WORD, CENTER.x as f32),
            text::baseline_for_ink_middle(&filled, WORD, CENTER.y as f32),
        );
        let pens: [Point; WORD.len()] = core::array::from_fn(|i| {
            pen + Point::new(libm::roundf(filled.advance(&WORD[..i])) as i32, 0)
        });
        let area = pens
            .iter()
            .enumerate()
            .fold(filled.baseline_bounds(WORD, pen), |area, (i, &at)| {
                let glyph = filled.baseline_bounds(&WORD[i..=i], at);
                let corner = |r: &Rectangle| r.bottom_right().unwrap_or(r.top_left);
                Rectangle::with_corners(
                    area.top_left.component_min(glyph.top_left),
                    corner(&area).component_max(corner(&glyph)),
                )
            })
            .offset(1);
        let cells = area.size.width as usize * area.size.height as usize;
        Self {
            area,
            ink: filled.baseline_bounds(WORD, pen),
            filled: vec![0; cells],
            hollow: vec![0; cells],
            glyphs: core::array::from_fn(|i| {
                let ink = filled.baseline_bounds(&WORD[i..=i], pens[i]);
                ink.top_left.x..ink.top_left.x + ink.size.width as i32
            }),
            pens,
            built: 0,
        }
    }

    /// Draws the pieces up to `pieces` into the buffers, if they are not yet.
    fn build(&mut self, pieces: usize, font: &FontdueRenderer<'static, Color>) {
        let style = style(font, chrome::LIME, WORD_PX, MARATYPE);
        while self.built < pieces.min(TITLE_PIECES) {
            let i = self.built % WORD.len();
            let (glyph, at) = (&WORD[i..=i], self.pens[i]);
            if self.built < WORD.len() {
                let cells = &mut Coverage {
                    area: self.area,
                    cells: &mut self.filled,
                };
                let _ = style.draw_on_baseline(glyph, at, cells);
            } else {
                let cells = &mut Coverage {
                    area: self.area,
                    cells: &mut self.hollow,
                };
                let _ = style.draw_hollow_on_baseline(glyph, at, HOLLOW, cells);
            }
            self.built += 1;
        }
    }

    /// Takes the title out of its slot, begun if there was none, and builds it to `pieces`. Put
    /// it back with [`Title::keep`].
    fn take(pieces: usize, font: &FontdueRenderer<'static, Color>) -> Self {
        let mut title = TITLE
            .lock(|title| title.borrow_mut().take())
            .unwrap_or_else(|| Self::new(font));
        title.build(pieces, font);
        title
    }

    fn keep(self) {
        TITLE.lock(|slot| *slot.borrow_mut() = Some(self));
    }

    /// Blends `columns` of `cells` onto the target in `color`.
    fn draw<D: CoverageTarget<Color = Color>>(
        &self,
        cells: &[u8],
        columns: core::ops::Range<i32>,
        color: Color,
        target: &mut D,
    ) {
        let left = self.area.top_left.x;
        let (from, to) = (
            columns.start.max(left),
            columns.end.min(left + self.area.size.width as i32),
        );
        if from >= to {
            return;
        }
        let height = self.area.size.height;
        if !target.visible(&Rectangle::new(
            Point::new(from, self.area.top_left.y),
            Size::new((to - from) as u32, height),
        )) {
            return;
        }
        let width = self.area.size.width as usize;
        for (row, line) in cells.chunks_exact(width).enumerate() {
            let line = &line[(from - left) as usize..(to - left) as usize];
            target.blend_row(from, self.area.top_left.y + row as i32, line, color);
        }
    }
}

/// Lets the title's coverage go, once the identity is over.
fn forget_title() {
    TITLE.lock(|title| title.borrow_mut().take());
}

fn draw_title<D: CoverageTarget<Color = Color>>(
    frame: u32,
    font: &FontdueRenderer<'static, Color>,
    target: &mut D,
) -> Result<(), D::Error> {
    // Out of the lock while it draws, so drawing holds no critical section.
    let title = Title::take(TITLE_PIECES, font);
    let all = i32::MIN..i32::MAX;
    match lit(frame, FLICKER_FROM) {
        Lit::Before => {
            let ms = frame as f32 * FRAME_MS;
            for (i, columns) in title.glyphs.iter().enumerate() {
                let k = smoothstep((ms - TYPE_FROM - i as f32 * INTERVAL) / RISE);
                if k <= 0.0 {
                    break;
                }
                let color = chrome::shade(chrome::LIME, libm::roundf(f32::from(DIM) * k) as u8);
                title.draw(&title.hollow, columns.clone(), color, target);
            }
        }
        Lit::Off => title.draw(&title.hollow, all, chrome::shade(chrome::LIME, DIM), target),
        Lit::On => title.draw(&title.filled, all, chrome::LIME, target),
        Lit::Partial => {
            let ink = title.ink;
            for fraction in SLICES {
                let left = ink.top_left.x + libm::roundf(ink.size.width as f32 * fraction) as i32;
                title.draw(
                    &title.filled,
                    left..left + SLICE as i32,
                    chrome::LIME,
                    target,
                );
            }
        }
    }
    title.keep();
    Ok(())
}

fn draw_row<D: CoverageTarget<Color = Color>>(
    frame: u32,
    answered: usize,
    context: &Context<'_>,
    font: &FontdueRenderer<'static, Color>,
    target: &mut D,
) -> Result<(), D::Error> {
    let shown = |element: u32| frame >= ROW_FROM + element;
    if !shown(0) {
        return Ok(());
    }
    // The barcode, digits and GNSS symbol change only every 5 px down, where the modules do, so
    // each band of five rows is one line of coverage blended five times.
    let left = ROW_LEFT as i32;
    let mut line = [0u8; ROW_LINE];
    let add = |line: &mut [u8; ROW_LINE], from: f32, to: f32| {
        for x in libm::floorf(from) as i32..libm::ceilf(to) as i32 {
            let covered = (to.min(x as f32 + 1.0) - from.max(x as f32)).clamp(0.0, 1.0);
            let cell = &mut line[(x - left) as usize];
            *cell = cell.saturating_add(libm::roundf(covered * 255.0) as u8);
        }
    };
    let scale = BARCODE_WIDTH / startup::bars_advance(context.firmware) as f32;
    let digits = startup::utc_digits(context.clock);
    for band in 0..5 {
        line.fill(0);
        for (at, width) in startup::bars(context.firmware) {
            let from = ROW_LEFT + at as f32 * scale;
            add(&mut line, from, from + width as f32 * scale);
        }
        // Each run of set modules is one span, since two edges meeting mid-pixel would leave a
        // seam.
        let modules = |line: &mut [u8; ROW_LINE], bits: u8, columns: u32, from: f32, w: f32| {
            let set = |column: u32| column < columns && bits & (1 << (columns - 1 - column)) != 0;
            let mut column = 0;
            while column < columns {
                let start = column;
                while set(column) {
                    column += 1;
                }
                if column > start {
                    add(line, from + start as f32 * w, from + column as f32 * w);
                }
                column += 1;
            }
        };
        if shown(2) {
            for i in 0..4 {
                let rows = digits.map_or(&startup::PIXEL_DASH, |digits| {
                    &startup::PIXEL_DIGITS[usize::from(digits[i])]
                });
                // A space between the hours and the minutes.
                let from = DIGITS_LEFT + DIGIT_STEP * i as f32 + if i >= 2 { 8.0 } else { 0.0 };
                modules(&mut line, rows[band], 3, from, DIGIT_MODULE.0);
            }
        }
        if shown(3) {
            modules(&mut line, startup::GNSS[band], 5, GNSS_LEFT, GNSS_MODULE);
        }
        for y in 0..DIGIT_MODULE.1 as i32 {
            target.blend_row(left, ROW_TOP + band as i32 * 5 + y, &line, chrome::LIME);
        }
    }
    if shown(1) {
        // The square, less its octagonal hole: 4.5 px either side of the centre on its fourth
        // row, widening at 45° to 9.5 px by its ninth, straight to its eighteenth, then in again.
        let top = ROW_TOP as f32;
        let hole = |y: f32| match y - top {
            v if !(3.0..22.0).contains(&v) => 0.0,
            v if v < 8.0 => v + 1.5,
            v if v < 17.0 => 9.5,
            v => 26.5 - v,
        };
        let half = SQUARE as f32 / 2.0;
        smooth::symmetric(
            target,
            SQUARE_LEFT as f32 + half,
            ROW_TOP..ROW_TOP + SQUARE,
            |_| half,
            hole,
            chrome::LIME,
        )?;
        let level = dot_level(frame);
        if level > 0 {
            target.fill_solid(&DOT, chrome::shade(chrome::LIME, level))?;
        }
    }
    if shown(4) {
        let style = style(font, chrome::LIME, 14, FRAKTION);
        let mut version = heapless::String::<32>::new();
        let _ = write!(version, "VERSION {}", context.firmware);
        let mut count = heapless::String::<24>::new();
        let _ = write!(count, "SELF TEST {answered}/6 OK");
        for (line, top) in [version.as_str(), count.as_str()]
            .into_iter()
            .zip(COPY_TOPS)
        {
            let pen = Point::new(
                text::pen_x_for_ink_left(&style, line, COPY_LEFT),
                text::baseline_for_ink_top(&style, line, top),
            );
            style.draw_on_baseline(line, pen, target)?;
        }
    }
    Ok(())
}

// The card.

/// The columns the card's opening frames show the pin through: one in every five.
const STRIPE_PITCH: i32 = 5;
const STRIPE_PHASE: i32 = CENTER.x % STRIPE_PITCH;
/// The pin grows to this for the card's last frames.
const CARD_SCALE: f32 = 1.9;

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

    fn draw_iter<I: IntoIterator<Item = Pixel<Self::Color>>>(
        &mut self,
        pixels: I,
    ) -> Result<(), Self::Error> {
        self.0.draw_iter(
            pixels
                .into_iter()
                .filter(|Pixel(point, _)| point.x.rem_euclid(STRIPE_PITCH) == STRIPE_PHASE),
        )
    }

    fn fill_solid(&mut self, area: &Rectangle, color: Self::Color) -> Result<(), Self::Error> {
        let first = area.top_left.x + (STRIPE_PHASE - area.top_left.x).rem_euclid(STRIPE_PITCH);
        for x in (first..area.top_left.x + area.size.width as i32).step_by(STRIPE_PITCH as usize) {
            self.0.fill_solid(
                &Rectangle::new(
                    Point::new(x, area.top_left.y),
                    Size::new(1, area.size.height),
                ),
                color,
            )?;
        }
        Ok(())
    }
}

impl<T: CoverageTarget> CoverageTarget for Stripes<'_, T> {
    fn blend_row(&mut self, x: i32, y: i32, coverage: &[u8], color: Self::Color) {
        let first = (STRIPE_PHASE - x).rem_euclid(STRIPE_PITCH) as usize;
        for (i, &cover) in coverage
            .iter()
            .enumerate()
            .skip(first)
            .step_by(STRIPE_PITCH as usize)
        {
            self.0
                .blend_pixel(Point::new(x + i as i32, y), cover, color);
        }
    }

    fn visible(&self, area: &Rectangle) -> bool {
        self.0.visible(area)
    }
}

pub fn draw_card<D: CoverageTarget<Color = Color>>(
    frame: u32,
    target: &mut D,
) -> Result<(), D::Error> {
    // The lime frames clear to the page's colour rather than painting it over black.
    let page = matches!(frame, 0..9 | 17);
    forget_title();
    screens::clear_to(target, if page { chrome::LIME } else { chrome::BLACK })?;
    let (pin, scaled) = (Pin::impact(1.0), Pin::impact(CARD_SCALE));
    match frame {
        0..3 => pin.draw(chrome::BLACK, &mut Stripes(&mut *target))?,
        3..9 => pin.draw(chrome::BLACK, target)?,
        9..13 => pin.draw(chrome::LIME, target)?,
        13..17 => scaled.draw(chrome::LIME, target)?,
        17 => scaled.draw(chrome::BLACK, target)?,
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
        assert_eq!(
            cadence,
            [
                On, On, Off, Off, On, Off, Off, On, On, Partial, Partial, On, On
            ]
        );
        assert_eq!(lit(9, 10), Lit::Before);
    }

    #[test]
    fn the_upper_field_turns_on_five_frames_and_the_lower_never() {
        let turned: alloc::vec::Vec<u32> = (1..FRAMES)
            .filter(|&frame| looks(frame) != looks(frame - 1))
            .collect();
        assert_eq!(turned, [66, 77, 88, 99, 110]);
    }
}
