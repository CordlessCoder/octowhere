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
    Pixel,
    draw_target::DrawTarget,
    prelude::{Dimensions, Point, Size},
    primitives::Rectangle,
};

use super::{
    clock::ClockView,
    scatter::{Field, Look, Scatter, Shown},
    screens, smooth,
    startup::{self, CENTER, Context},
    text,
};
use crate::chrome::{
    self, Color, CoverageTarget, Dirty, FRAKTION, FontdueRenderer, INTERFERENCE_BOLD, MARATYPE,
    OnBackground, Window,
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
};
const LOWER: Field = Field {
    center: Point::new(190, 334),
    radius: 125.0,
    seed: 0x6f63_7476,
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
        .draw(chrome::LIME, field),
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
    /// Points on the head's upper half-circle, left to right.
    const ARC: usize = 33;
    const HOLE: [(f32, f32); 8] = [
        (5.0, 3.0),
        (7.0, 3.0),
        (9.0, 5.0),
        (9.0, 7.0),
        (7.0, 9.0),
        (5.0, 9.0),
        (3.0, 7.0),
        (3.0, 5.0),
    ];

    /// The pin as large as the impact frames show it, `scale` times over, about the centre.
    fn impact(scale: f32) -> Self {
        let unit = 17.5 * scale;
        Self {
            corner: (CENTER.x as f32 - 6.0 * unit, CENTER.y as f32 - 7.0 * unit),
            unit,
        }
    }

    fn draw<D: CoverageTarget<Color = Color>>(self, color: Color, target: &mut D) {
        let place =
            |(x, y): (f32, f32)| (self.corner.0 + x * self.unit, self.corner.1 + y * self.unit);
        let mut outline: heapless::Vec<(f32, f32), { Self::ARC + 3 }> = (0..Self::ARC)
            .map(|i| {
                let angle = core::f32::consts::PI * (1.0 + i as f32 / (Self::ARC - 1) as f32);
                place((6.0 + 6.0 * libm::cosf(angle), 6.0 + 6.0 * libm::sinf(angle)))
            })
            .collect();
        for point in [(12.0, 8.0), (6.0, 14.0), (0.0, 8.0)] {
            let _ = outline.push(place(point));
        }
        let hole = Self::HOLE.map(place);
        smooth::contours(
            target,
            &mut fontdue::raster::Raster::empty(),
            &mut alloc::vec::Vec::new(),
            &[&outline, &hole],
            color,
        );
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

fn draw_title<D: CoverageTarget<Color = Color>>(
    frame: u32,
    font: &FontdueRenderer<'static, Color>,
    target: &mut D,
) -> Result<(), D::Error> {
    let filled = style(font, chrome::LIME, WORD_PX, MARATYPE);
    let pen = Point::new(
        text::pen_x_for_ink_centre(&filled, WORD, CENTER.x as f32),
        text::baseline_for_ink_middle(&filled, WORD, CENTER.y as f32),
    );
    let hollow = |level: u8| style(font, chrome::shade(chrome::LIME, level), WORD_PX, MARATYPE);
    match lit(frame, FLICKER_FROM) {
        Lit::Before => {
            let ms = frame as f32 * FRAME_MS;
            for (i, _) in WORD.char_indices() {
                let k = smoothstep((ms - TYPE_FROM - i as f32 * INTERVAL) / RISE);
                if k <= 0.0 {
                    break;
                }
                let at = pen + Point::new(libm::roundf(filled.advance(&WORD[..i])) as i32, 0);
                hollow(libm::roundf(f32::from(DIM) * k) as u8).draw_hollow_on_baseline(
                    &WORD[i..=i],
                    at,
                    HOLLOW,
                    target,
                )?;
            }
            Ok(())
        }
        Lit::Off => hollow(DIM).draw_hollow_on_baseline(WORD, pen, HOLLOW, target),
        Lit::On => filled.draw_on_baseline(WORD, pen, target),
        Lit::Partial => {
            let ink = filled.baseline_bounds(WORD, pen);
            for fraction in SLICES {
                let left = ink.top_left.x + libm::roundf(ink.size.width as f32 * fraction) as i32;
                let slice = Rectangle::new(
                    Point::new(left, ink.top_left.y),
                    Size::new(SLICE, ink.size.height),
                );
                filled.draw_on_baseline(
                    WORD,
                    pen,
                    &mut Window::new(&mut *target, Point::zero(), slice),
                )?;
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
    let (top, bottom) = (ROW_TOP as f32, ROW_TOP as f32 + ROW_HEIGHT);
    if shown(0) {
        let scale = BARCODE_WIDTH / startup::bars_advance(context.firmware) as f32;
        for (at, width) in startup::bars(context.firmware) {
            let left = ROW_LEFT + at as f32 * scale;
            smooth::rect(
                target,
                (left, top),
                (left + width as f32 * scale, bottom),
                chrome::LIME,
            )?;
        }
    }
    if shown(1) {
        let (x, y) = (SQUARE_LEFT as f32, top);
        let side = SQUARE as f32;
        let square = [(x, y), (x + side, y), (x + side, y + side), (x, y + side)];
        let octagon = [
            (8.0, 3.0),
            (17.0, 3.0),
            (22.0, 8.0),
            (22.0, 17.0),
            (17.0, 22.0),
            (8.0, 22.0),
            (3.0, 17.0),
            (3.0, 8.0),
        ]
        .map(|(dx, dy)| (x + dx, y + dy));
        smooth::contours(
            target,
            &mut fontdue::raster::Raster::empty(),
            &mut alloc::vec::Vec::new(),
            &[&square, &octagon],
            chrome::LIME,
        );
        let level = dot_level(frame);
        if level > 0 {
            target.fill_solid(&DOT, chrome::shade(chrome::LIME, level))?;
        }
    }
    let modules = |target: &mut D, rows: &[u8], columns: u32, left: f32, (w, h): (f32, f32)| {
        // Each run of set modules is one span, since two blended edges meeting mid-pixel would
        // leave a seam.
        for (row, bits) in rows.iter().enumerate() {
            let set = |column: u32| column < columns && bits & (1 << (columns - 1 - column)) != 0;
            let y = top + row as f32 * h;
            let mut column = 0;
            while column < columns {
                let start = column;
                while set(column) {
                    column += 1;
                }
                if column > start {
                    let x = |column: u32| left + column as f32 * w;
                    smooth::rect(target, (x(start), y), (x(column), y + h), chrome::LIME)?;
                }
                column += 1;
            }
        }
        Ok(())
    };
    if shown(2) {
        let digits = startup::utc_digits(context.clock);
        for i in 0..4 {
            let rows = digits.map_or(&startup::PIXEL_DASH, |digits| {
                &startup::PIXEL_DIGITS[usize::from(digits[i])]
            });
            // A space between the hours and the minutes.
            let left = DIGITS_LEFT + DIGIT_STEP * i as f32 + if i >= 2 { 8.0 } else { 0.0 };
            modules(target, rows, 3, left, DIGIT_MODULE)?;
        }
    }
    if shown(3) {
        modules(
            target,
            &startup::GNSS,
            5,
            GNSS_LEFT,
            (GNSS_MODULE, GNSS_MODULE),
        )?;
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
    screens::clear_to(target, if page { chrome::LIME } else { chrome::BLACK })?;
    let (pin, scaled) = (Pin::impact(1.0), Pin::impact(CARD_SCALE));
    match frame {
        0..3 => pin.draw(chrome::BLACK, &mut Stripes(&mut *target)),
        3..9 => pin.draw(chrome::BLACK, target),
        9..13 => pin.draw(chrome::LIME, target),
        13..17 => scaled.draw(chrome::LIME, target),
        17 => scaled.draw(chrome::BLACK, target),
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
