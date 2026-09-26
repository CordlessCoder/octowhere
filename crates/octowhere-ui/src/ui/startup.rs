//! The start-up sequence: a self-test that shows each part boot brings up as it answers, then
//! either the identity and the logo card, which hand over to the clock face, or the fault
//! screen. Section 2 of `context/design/specs/DISPLAY-AND-MOTION-SPEC.md` is its design.
//!
//! The self-test runs on the stage's clock in ms, like the faces. The identity, the card and the
//! fault screen are timed in frames at 30 fps, counted from the clock rather than from frames
//! drawn, so a late frame shows the step due at its own time.

use core::fmt::Write as _;

use embedded_graphics::{
    draw_target::DrawTarget,
    prelude::{Point, Size},
    primitives::Rectangle,
};

use super::{
    clock::ClockView,
    gesture::Micros,
    icon::{self, Glyph, Tile},
    identity, screens,
    smooth,
    text,
};
use crate::chrome::{
    self, Color, CoverageTarget, FontdueRenderer, Knockout, OnBackground, Window, FRAKTION, FRAKTION_BOLD,
    SHAPIRO,
};

/// A part the self-test waits for, in the order its cells run.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Part {
    Power,
    Clock,
    Touch,
    Motion,
    Magnet,
    Gnss,
}

impl Part {
    pub const ALL: [Self; 6] = [Self::Power, Self::Clock, Self::Touch, Self::Motion, Self::Magnet, Self::Gnss];

    #[must_use]
    pub fn index(self) -> usize {
        self as usize
    }

    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Power => "POWER",
            Self::Clock => "CLOCK",
            Self::Touch => "TOUCH",
            Self::Motion => "MOTION",
            Self::Magnet => "MAGNET",
            Self::Gnss => "GNSS",
        }
    }

    /// The part's self-test glyph.
    #[must_use]
    pub fn glyph(self) -> &'static Glyph {
        match self {
            Self::Power => &[0b01110, 0b11111, 0b10001, 0b11111, 0b11111],
            Self::Clock => &[0b11111, 0b10001, 0b10101, 0b10001, 0b11111],
            Self::Touch => &[0b00100, 0b00100, 0b11011, 0b00100, 0b00100],
            Self::Motion => &[0b10000, 0b10000, 0b10000, 0b10000, 0b11111],
            Self::Magnet => &[0b11011, 0b11011, 0b11011, 0b11111, 0b01110],
            Self::Gnss => &GNSS,
        }
    }
}

/// What a replay from the device page plays: the identity and the card, or a demonstration of
/// one part failing, from the self-test through the fault screen.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Replay {
    Good,
    Failing(Part),
}

impl Replay {
    pub const ALL: [Self; 7] = [
        Self::Good,
        Self::Failing(Part::Power),
        Self::Failing(Part::Clock),
        Self::Failing(Part::Touch),
        Self::Failing(Part::Motion),
        Self::Failing(Part::Magnet),
        Self::Failing(Part::Gnss),
    ];

    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Good => "GOOD",
            Self::Failing(Part::Power) => "POWER FAIL",
            Self::Failing(Part::Clock) => "CLOCK FAIL",
            Self::Failing(Part::Touch) => "TOUCH FAIL",
            Self::Failing(Part::Motion) => "MOTION FAIL",
            Self::Failing(Part::Magnet) => "MAGNET FAIL",
            Self::Failing(Part::Gnss) => "GNSS FAIL",
        }
    }

    /// The failing part's self-test glyph. A good start-up has the mark instead, which is not
    /// a 5 × 5 glyph: [`draw_mark_icon`] draws it.
    #[must_use]
    pub fn glyph(self) -> Option<&'static Glyph> {
        match self {
            Self::Good => None,
            Self::Failing(part) => Some(part.glyph()),
        }
    }
}

/// How a part's bring-up ended. Passing claims only that the part answered its driver.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Outcome {
    Answered,
    /// Nothing came back by the part's deadline, or the bus carried no reply.
    NoReply,
    /// The part replied, but not as its driver expects.
    BadReply,
}

impl Outcome {
    fn reason(self) -> &'static str {
        match self {
            Self::Answered => "",
            Self::NoReply => "NO REPLY BY DEADLINE",
            Self::BadReply => "REPLY NOT AS EXPECTED",
        }
    }
}

/// One part's bring-up, as boot reports it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Report {
    pub part: Part,
    pub outcome: Outcome,
}

/// The panel's brightness climbs from dark to its level over this, from the first frame.
const RAMP: Micros = 200_000;
/// A passed cell's glyph lands a row per this.
const ROW: Micros = 30_000;
/// How long the finished self-test holds before what follows it.
const PASS_HOLD: Micros = 200_000;
const FAIL_HOLD: Micros = 300_000;
/// Frames are counted at 30 fps, from the end of the self-test's hold.
const FRAMES_PER_SECOND: Micros = 30;
/// The identity's frames, then the card's to the clock's.
const CARD_FROM: u32 = identity::FRAMES;
const CLOCK_FROM: u32 = CARD_FROM + identity::CARD_FRAMES;
const FAULT_FRAMES: u32 = 120;
/// When each part reports in a demonstration, from its start, in cell order.
const DEMO_REPORTS: [Micros; 6] = [150_000, 250_000, 500_000, 600_000, 750_000, 1_300_000];

/// The whole sequence, from the first frame after power-on.
#[derive(Clone, Debug, Default)]
pub struct Startup {
    began: Option<Micros>,
    outcomes: [Option<(Outcome, Micros)>; 6],
    /// A touch cut the identity, the card or the fault screen short.
    skipped: bool,
    /// Set on a replay, which starts the identity here and shows no self-test or fault screen.
    identity_from: Option<Micros>,
    /// A demonstration: its outcomes are scripted, not reported, and it says so.
    demo: bool,
}

/// Where the sequence is.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Phase {
    SelfTest,
    /// The identity and then the card, by frame.
    Identity(u32),
    Fault(u32),
    /// Over. The clock face shows next: settled if a touch skipped the identity, otherwise
    /// running its entry, and typing its time in after the card.
    Done { entry: bool, after_card: bool },
}

/// What one cell of the self-test shows.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Cell {
    Waiting,
    /// How many rows of the glyph have landed.
    Answered(u8),
    Failed,
}

/// What a frame of the sequence draws. Two equal views draw the same pixels.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum View {
    SelfTest([Cell; 6]),
    Identity(u32),
    /// The card, by its own frame.
    Card(u32),
    Fault(u32),
}

impl Startup {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Takes a part's outcome. Only the first report for a part counts.
    pub fn report(&mut self, report: Report, now: Micros) {
        let slot = &mut self.outcomes[report.part.index()];
        if slot.is_none() {
            *slot = Some((report.outcome, now));
        }
    }

    /// The identity and the card again from `now`, after a boot that left `self` finished. The
    /// identity's count is this boot's, whether or not every part answered.
    #[must_use]
    pub fn replay(&self, now: Micros) -> Self {
        Self {
            began: Some(now.saturating_sub(RAMP)),
            outcomes: self.outcomes,
            skipped: false,
            identity_from: Some(now),
            demo: false,
        }
    }

    /// A demonstration of `failing` not answering, from a self-test that starts at `now`. The
    /// parts report on a fixed schedule.
    #[must_use]
    pub fn demo(failing: Part, now: Micros) -> Self {
        Self {
            began: Some(now.saturating_sub(RAMP)),
            outcomes: core::array::from_fn(|i| {
                let outcome = if i == failing.index() { Outcome::NoReply } else { Outcome::Answered };
                Some((outcome, now + DEMO_REPORTS[i]))
            }),
            skipped: false,
            identity_from: None,
            demo: true,
        }
    }

    /// Whether this is the boot itself, rather than a replay or a demonstration.
    #[must_use]
    pub fn is_boot(&self) -> bool {
        self.identity_from.is_none() && !self.demo
    }

    /// Starts the sequence's clock, on the first frame.
    pub fn begin(&mut self, now: Micros) {
        self.began.get_or_insert(now);
    }

    /// A touch skips the identity, the card and the fault screen. The self-test ignores it.
    pub fn touch(&mut self, now: Micros) {
        if !matches!(self.phase(now), Phase::SelfTest) {
            self.skipped = true;
        }
    }

    fn failed(&self) -> impl Iterator<Item = (Part, Outcome)> + '_ {
        Part::ALL.into_iter().filter_map(|part| match self.outcomes[part.index()] {
            Some((outcome, _)) if outcome != Outcome::Answered => Some((part, outcome)),
            _ => None,
        })
    }

    pub(super) fn answered(&self) -> usize {
        self.outcomes.iter().flatten().filter(|(outcome, _)| *outcome == Outcome::Answered).count()
    }

    /// When the identity or the fault screen starts: once every cell has been decided, the last
    /// glyph has landed, and the hold is over.
    fn frames_from(&self) -> Option<Micros> {
        if self.identity_from.is_some() {
            return self.identity_from;
        }
        let mut built = 0;
        for outcome in &self.outcomes {
            let (outcome, at) = (*outcome)?;
            let lands = if outcome == Outcome::Answered { at + 4 * ROW } else { at };
            built = built.max(lands);
        }
        let hold = if self.failed().next().is_some() { FAIL_HOLD } else { PASS_HOLD };
        Some(built + hold)
    }

    #[must_use]
    pub fn phase(&self, now: Micros) -> Phase {
        let failed = self.identity_from.is_none() && self.failed().next().is_some();
        let Some(from) = self.frames_from().filter(|&from| now >= from) else {
            return Phase::SelfTest;
        };
        let frame = ((now - from) * FRAMES_PER_SECOND / 1_000_000) as u32;
        match (failed, self.skipped) {
            (true, true) => Phase::Done { entry: true, after_card: false },
            (false, true) => Phase::Done { entry: false, after_card: false },
            (true, false) if frame >= FAULT_FRAMES => Phase::Done { entry: true, after_card: false },
            (true, false) => Phase::Fault(frame),
            (false, false) if frame >= CLOCK_FROM => Phase::Done { entry: true, after_card: true },
            (false, false) => Phase::Identity(frame),
        }
    }

    #[must_use]
    pub fn view(&self, now: Micros) -> Option<View> {
        Some(match self.phase(now) {
            Phase::SelfTest => View::SelfTest(core::array::from_fn(|i| match self.outcomes[i] {
                None => Cell::Waiting,
                Some((_, at)) if at > now => Cell::Waiting,
                Some((Outcome::Answered, at)) => Cell::Answered(((now.saturating_sub(at)) / ROW + 1).min(5) as u8),
                Some(_) => Cell::Failed,
            })),
            Phase::Identity(frame) if frame < CARD_FROM => View::Identity(frame),
            Phase::Identity(frame) => View::Card(frame - CARD_FROM),
            Phase::Fault(frame) => View::Fault(frame),
            Phase::Done { .. } => return None,
        })
    }

    /// The panel's brightness now, given its stored `level`.
    #[must_use]
    pub fn brightness(&self, now: Micros, level: u8) -> u8 {
        let since = self.began.map_or(0, |began| now.saturating_sub(began));
        if since < RAMP { (u64::from(level) * since / RAMP) as u8 } else { level }
    }

    /// When the sequence next changes on its own, if nothing else arrives first.
    #[must_use]
    pub fn next_change(&self, now: Micros) -> Option<Micros> {
        let ramping = self.began.is_none_or(|began| now < began + RAMP);
        let next = match self.phase(now) {
            Phase::SelfTest => {
                let rows = self.outcomes.iter().flatten().filter_map(|&(outcome, at)| {
                    if at > now {
                        return Some(at);
                    }
                    let row = at + (now.saturating_sub(at) / ROW + 1) * ROW;
                    (outcome == Outcome::Answered && row <= at + 4 * ROW).then_some(row)
                });
                rows.chain(self.frames_from()).min()
            }
            Phase::Identity(_) | Phase::Fault(_) => {
                let from = self.frames_from()?;
                let frame = (now - from) * FRAMES_PER_SECOND / 1_000_000;
                Some(from + ((frame + 1) * 1_000_000).div_ceil(FRAMES_PER_SECOND))
            }
            Phase::Done { .. } => None,
        };
        if ramping { Some(now) } else { next }
    }
}

pub(super) const CENTER: Point = Point::new(233, 233);
pub(super) const GNSS: Glyph = [0b00100, 0b01010, 0b10101, 0b01010, 0b00100];

/// Everything the sequence shows that is not its own: the time, and the version.
pub struct Context<'a> {
    pub clock: &'a ClockView,
    pub firmware: &'a str,
}

pub fn draw<D>(
    view: View,
    startup: &Startup,
    context: &Context<'_>,
    font: &FontdueRenderer<'static, Color>,
    target: &mut D,
) -> Result<(), D::Error>
where
    D: CoverageTarget<Color = Color>,
{
    match view {
        View::SelfTest(cells) => {
            let mut version = heapless::String::<32>::new();
            let _ = if startup.demo { write!(version, "DEMO, NOT A HARDWARE TEST") } else { write!(version, "VERSION {}", context.firmware) };
            draw_self_test(&cells, &version, font, target)
        }
        View::Identity(frame) => identity::draw_identity(frame, startup.answered(), context, font, target),
        View::Card(frame) => identity::draw_card(frame, target),
        View::Fault(frame) => draw_fault(frame, startup, context.firmware, font, target),
    }
}

// The self-test: six rows, one a part, each with its index, glyph, name and state.

const TITLE: &str = "SELF TEST";
const TITLE_TOP: i32 = 30;
const COUNTER_TOP: i32 = 70;
/// Each row's rule, the top of the row below it.
const ROW_TOP: i32 = 89;
const ROW_PITCH: i32 = 45;
/// The rows run between these columns, the first and last narrower, as the glass is there.
const WIDE: (i32, i32) = (62, 404);
const NARROW: (i32, i32) = (82, 384);
const LAST_RULE: i32 = 359;
const VERSION_TOP: i32 = 380;

fn row_top(index: usize) -> i32 {
    ROW_TOP + ROW_PITCH * index as i32
}

fn row_columns(index: usize) -> (i32, i32) {
    if index == 0 || index == 5 { NARROW } else { WIDE }
}

fn row_tile(index: usize) -> Tile {
    Tile { corner: Point::new(row_columns(index).0 + 38, row_top(index) + 6), module: 5, padding: 4 }
}

/// Everything in one row below its rule.
#[must_use]
pub fn cell_bounds(index: usize) -> Rectangle {
    let (left, right) = row_columns(index);
    Rectangle::with_corners(Point::new(left, row_top(index) + 1), Point::new(right - 1, row_top(index) + ROW_PITCH - 1))
}

fn counter(cells: &[Cell; 6]) -> heapless::String<16> {
    let decided = cells.iter().filter(|cell| **cell != Cell::Waiting).count();
    let mut text = heapless::String::new();
    let _ = write!(text, "DECIDED  {decided}/6");
    text
}

fn small(font: &FontdueRenderer<'static, Color>, color: Color, size: u32, index: usize) -> FontdueRenderer<'static, Color> {
    text::style(font, color, size, index)
}

/// Draws `text` with its ink centred on the panel's middle column and its ink top on `top`.
fn centred<D: CoverageTarget<Color = Color>>(
    style: &FontdueRenderer<'static, Color>,
    text: &str,
    top: i32,
    target: &mut D,
) -> Result<(), D::Error> {
    let pen = Point::new(
        text::pen_x_for_ink_centre(style, text, CENTER.x as f32),
        text::baseline_for_ink_top(style, text, top),
    );
    style.draw_on_baseline(text, pen, target)
}

fn counter_style(font: &FontdueRenderer<'static, Color>) -> FontdueRenderer<'static, Color> {
    small(font, chrome::GRAY, 13, FRAKTION)
}

/// Where the counter's ink can land, whatever it counts.
#[must_use]
pub fn counter_bounds(font: &FontdueRenderer<'static, Color>, cells: &[Cell; 6]) -> Rectangle {
    let style = counter_style(font);
    let text = counter(cells);
    let pen = Point::new(
        text::pen_x_for_ink_centre(&style, &text, CENTER.x as f32),
        text::baseline_for_ink_top(&style, &text, COUNTER_TOP),
    );
    style.baseline_bounds(&text, pen)
}

fn draw_self_test<D: CoverageTarget<Color = Color>>(
    cells: &[Cell; 6],
    version: &str,
    font: &FontdueRenderer<'static, Color>,
    target: &mut D,
) -> Result<(), D::Error> {
    screens::clear(target)?;
    let field = &mut OnBackground::new(&mut *target, chrome::BLACK);
    smooth::perimeter().draw(field, chrome::GRAY);
    centred(&small(font, chrome::WHITE, 27, SHAPIRO), TITLE, TITLE_TOP, field)?;
    centred(&counter_style(font), &counter(cells), COUNTER_TOP, field)?;
    let index_style = small(font, chrome::GRAY, 13, FRAKTION);
    for (i, (part, cell)) in Part::ALL.into_iter().zip(cells).enumerate() {
        let (left, right) = row_columns(i);
        let top = row_top(i);
        field.fill_solid(&Rectangle::with_corners(Point::new(left, top), Point::new(right - 1, top)), chrome::GRAY)?;
        if !field.visible(&cell_bounds(i)) {
            continue;
        }
        let mut index = heapless::String::<2>::new();
        let _ = write!(index, "{:02}", i + 1);
        let pen = Point::new(
            text::pen_x_for_ink_left(&index_style, &index, left + 9),
            text::baseline_for_ink_top(&index_style, &index, top + 14),
        );
        index_style.draw_on_baseline(&index, pen, field)?;
        let tile = row_tile(i);
        let (status, color, name_color) = match *cell {
            Cell::Waiting => {
                tile.draw(part.glyph(), chrome::GRAY, 0, field)?;
                ("--", chrome::GRAY, chrome::GRAY)
            }
            Cell::Answered(rows) => {
                tile.draw(part.glyph(), chrome::WHITE, rows, field)?;
                ("OK", chrome::WHITE, chrome::WHITE)
            }
            Cell::Failed => {
                tile.draw(&icon::NO_DATA, chrome::RED, 5, field)?;
                field.fill_solid(&Rectangle::new(Point::new(left, top + 4), Size::new(4, 36)), chrome::RED)?;
                ("FAIL", chrome::RED, chrome::RED)
            }
        };
        let name_style = small(font, name_color, 17, FRAKTION_BOLD);
        let name = part.name();
        let pen = Point::new(
            text::pen_x_for_ink_left(&name_style, name, left + 82),
            text::baseline_for_ink_top(&name_style, name, top + 12),
        );
        name_style.draw_on_baseline(name, pen, field)?;
        let status_style = small(font, color, 16, FRAKTION_BOLD);
        let pen = Point::new(
            text::pen_x_for_ink_right(&status_style, status, right - 13),
            text::baseline_for_ink_middle(&status_style, status, (top + 23) as f32),
        );
        status_style.draw_on_baseline(status, pen, field)?;
    }
    let (left, right) = NARROW;
    field.fill_solid(&Rectangle::with_corners(Point::new(left, LAST_RULE), Point::new(right - 1, LAST_RULE)), chrome::GRAY)?;
    centred(&small(font, chrome::GRAY, 13, FRAKTION), version, VERSION_TOP, field)
}

// Shared by the identity and the fault screen.

/// Fills the set modules of `rows`, `columns` wide with the leftmost the highest bit, as
/// squares of `module` from `corner`.
pub(super) fn modules<D: DrawTarget<Color = Color>>(
    rows: &[u8],
    columns: i32,
    corner: Point,
    module: i32,
    color: Color,
    target: &mut D,
) -> Result<(), D::Error> {
    for (row, bits) in rows.iter().enumerate() {
        for column in (0..columns).filter(|column| bits & (1 << (columns - 1 - column)) != 0) {
            let at = corner + Point::new(column * module, row as i32 * module);
            target.fill_solid(&Rectangle::new(at, Size::new_equal(module as u32)), color)?;
        }
    }
    Ok(())
}

/// Draws the version as bars from `left`, each character's four low bits least first: 2 px
/// for a one, 1 px for a zero, 2 px apart. Returns the column past the last bar.
pub(super) fn barcode<D: DrawTarget<Color = Color>>(
    version: &str,
    left: i32,
    top: i32,
    height: u32,
    color: Color,
    target: &mut D,
) -> Result<i32, D::Error> {
    let mut x = left;
    for byte in version.bytes() {
        for bit in 0..4 {
            let width = if byte & (1 << bit) != 0 { 2 } else { 1 };
            target.fill_solid(&Rectangle::new(Point::new(x, top), Size::new(width, height)), color)?;
            x += width as i32 + 2;
        }
    }
    Ok(x - 2)
}

/// Stripes at 45°, rising to the right, `width` px of every `pitch` along each row of `area`,
/// shifted left by `shift`.
pub(super) fn hatch<D: DrawTarget<Color = Color>>(
    area: Rectangle,
    pitch: i32,
    width: i32,
    shift: i32,
    color: Color,
    target: &mut D,
) -> Result<(), D::Error> {
    let Some(bottom_right) = area.bottom_right() else {
        return Ok(());
    };
    for y in area.top_left.y..=bottom_right.y {
        let mut x = area.top_left.x;
        while x <= bottom_right.x {
            let phase = (x + y + shift).rem_euclid(pitch);
            let run = if phase < width { width - phase } else { pitch - phase };
            let end = (x + run).min(bottom_right.x + 1);
            if phase < width {
                target.fill_solid(&Rectangle::new(Point::new(x, y), Size::new((end - x) as u32, 1)), color)?;
            }
            x = end;
        }
    }
    Ok(())
}

/// Pixel digits, three modules wide, and a dash for a time the clock cannot vouch for.
pub(super) const PIXEL_DIGITS: [[u8; 5]; 10] = [
    [0b111, 0b101, 0b101, 0b101, 0b111],
    [0b010, 0b110, 0b010, 0b010, 0b111],
    [0b111, 0b001, 0b111, 0b100, 0b111],
    [0b111, 0b001, 0b111, 0b001, 0b111],
    [0b101, 0b101, 0b111, 0b001, 0b001],
    [0b111, 0b100, 0b111, 0b001, 0b111],
    [0b111, 0b100, 0b111, 0b101, 0b111],
    [0b111, 0b001, 0b010, 0b010, 0b010],
    [0b111, 0b101, 0b111, 0b101, 0b111],
    [0b111, 0b101, 0b111, 0b001, 0b111],
];
pub(super) const PIXEL_DASH: [u8; 5] = [0b000, 0b000, 0b111, 0b000, 0b000];

/// The UTC hours and minutes as four digits, while the clock has a time it trusts.
pub(super) fn utc_digits(clock: &ClockView) -> Option<[u8; 4]> {
    let state = clock.clock();
    let utc = state.utc.filter(|_| !state.stopped)?;
    let minutes = utc.div_euclid(60);
    let (hours, minutes) = (minutes.div_euclid(60).rem_euclid(24) as u8, minutes.rem_euclid(60) as u8);
    Some([hours / 10, hours % 10, minutes / 10, minutes % 10])
}

// The mark and the logo card.

pub(super) const PAGE_RADIUS: f32 = 232.0;
/// The mark on the card: 42 px modules, the icons' frame rule's 11 px strokes, top-left at 128.
pub(super) const CARD_MARK: Mark = Mark { origin: (128.0, 128.0), module: 42.0, stroke: 11.0 };
/// The card's mark scaled about the centre for the scaled and impact frames.
pub(super) const CARD_SCALE: f32 = 1.9;
/// The hatch on the card at 1×: inset 5 px inside the tile's outline, 5 px stripes on a 12 px
/// pitch, all measured across the stripes. They scale with the mark.
const HATCH_INSET: f32 = 5.0;
const HATCH_STRIPE: f32 = 5.0;
const HATCH_PITCH: f32 = 12.0;

/// The OCTOWHERE mark on the icons' 5 × 5 grid: a tile's outline over modules 1–4, a corner
/// bracket in each corner module, and a line along the outer edge of each middle module.
#[derive(Clone, Copy, Debug)]
pub(super) struct Mark {
    /// The top-left corner, and the sizes, in continuous pixels.
    pub(super) origin: (f32, f32),
    pub(super) module: f32,
    pub(super) stroke: f32,
}

impl Mark {
    pub(super) fn scaled(self, scale: f32) -> Self {
        let about = |v: f32, c: i32| c as f32 + (v - c as f32) * scale;
        Self {
            origin: (about(self.origin.0, CENTER.x), about(self.origin.1, CENTER.y)),
            module: self.module * scale,
            stroke: self.stroke * scale,
        }
    }

    /// Half-open spans `[x0, x1) × [y0, y1)` from the origin, for the tile's outline, then
    /// for the corners and edge lines.
    fn tile(self) -> [(f32, f32, f32, f32); 4] {
        let (m, s) = (self.module, self.stroke);
        let (a, b) = (m, 4.0 * m);
        [(a, a, b, a + s), (a, b - s, b, b), (a, a + s, a + s, b - s), (b - s, a + s, b, b - s)]
    }

    fn surround(self) -> [(f32, f32, f32, f32); 12] {
        let (m, s) = (self.module, self.stroke);
        let far = 5.0 * m;
        [
            (0.0, 0.0, m, s),
            (0.0, 0.0, s, m),
            (4.0 * m, 0.0, far, s),
            (far - s, 0.0, far, m),
            (0.0, far - s, m, far),
            (0.0, 4.0 * m, s, far),
            (4.0 * m, far - s, far, far),
            (far - s, 4.0 * m, far, far),
            (2.0 * m, 0.0, 3.0 * m, s),
            (2.0 * m, far - s, 3.0 * m, far),
            (0.0, 2.0 * m, s, 3.0 * m),
            (far - s, 2.0 * m, far, 3.0 * m),
        ]
    }

    fn fill<D: DrawTarget<Color = Color>>(self, span: (f32, f32, f32, f32), color: Color, target: &mut D) -> Result<(), D::Error> {
        let at = |v: f32, o: f32| libm::roundf(o + v) as i32;
        let (x0, y0) = (at(span.0, self.origin.0), at(span.1, self.origin.1));
        let (x1, y1) = (at(span.2, self.origin.0), at(span.3, self.origin.1));
        if x1 > x0 && y1 > y0 {
            let area = Rectangle::new(Point::new(x0, y0), Size::new((x1 - x0) as u32, (y1 - y0) as u32));
            target.fill_solid(&area, color)?;
        }
        Ok(())
    }

    /// Draws the tile, the rest of the mark if `whole`, and the hatch inside the tile if
    /// `hatched`, scaling the hatch with the mark from its size on the card.
    pub(super) fn draw<D: DrawTarget<Color = Color>>(self, whole: bool, hatched: bool, color: Color, target: &mut D) -> Result<(), D::Error> {
        for span in self.tile() {
            self.fill(span, color, target)?;
        }
        if whole {
            for span in self.surround() {
                self.fill(span, color, target)?;
            }
        }
        if hatched {
            let scale = self.module / CARD_MARK.module;
            let inset = self.module + self.stroke + HATCH_INSET * scale;
            let from = |o: f32| libm::roundf(o + inset) as i32;
            let to = |o: f32| libm::roundf(o + 5.0 * self.module - inset) as i32;
            let (pitch, stripe) = (HATCH_PITCH * scale, HATCH_STRIPE * scale);
            // Lit where the distance across the stripes, from the panel's centre, falls in the
            // first `stripe` of each `pitch`. Along a row that distance grows by 1/√2 a pixel,
            // so each stripe's run is worked out from where it starts and ends.
            let root2 = core::f32::consts::SQRT_2;
            for y in from(self.origin.1)..to(self.origin.1) {
                let (start, end) = (from(self.origin.0), to(self.origin.0));
                let k = (y - CENTER.x - CENTER.y + 1) as f32;
                let mut n = libm::floorf((start as f32 + k) / root2 / pitch);
                loop {
                    let first = (libm::ceilf(n * pitch * root2 - k) as i32).max(start);
                    let last = (libm::ceilf((n * pitch + stripe) * root2 - k) as i32).min(end);
                    if first >= end {
                        break;
                    }
                    if last > first {
                        let run = Rectangle::new(Point::new(first, y), Size::new((last - first) as u32, 1));
                        target.fill_solid(&run, color)?;
                    }
                    n += 1.0;
                }
            }
        }
        Ok(())
    }
}

/// Draws the mark filling the square `bounds`, as an icon: strokes by the icons' frame rule and
/// the hatch inside the tile.
pub fn draw_mark_icon<D: DrawTarget<Color = Color>>(bounds: Rectangle, color: Color, target: &mut D) -> Result<(), D::Error> {
    let module = bounds.size.width as f32 / 5.0;
    let stroke = libm::roundf((module + 2.0) / 4.0).max(2.0);
    let origin = (bounds.top_left.x as f32, bounds.top_left.y as f32);
    Mark { origin, module, stroke }.draw(true, true, color, target)
}

// The fault screen.

const DASH_ROWS: [i32; 4] = [86, 150, 366, 430];
const DASH: Size = Size::new(9, 2);
const DASH_PITCH: i32 = 46;
const DASH_FROM: i32 = 20;
const PART_CENTRES: [Point; 4] = [Point::new(110, 118), Point::new(356, 118), Point::new(110, 348), Point::new(356, 348)];
const PART_MODULE: i32 = 9;
const BAND_ROWS: core::ops::Range<i32> = 198..318;
const STRIP_ROWS: core::ops::Range<i32> = 234..282;
const NAME_PX: u32 = 200;
/// The running line and the giant name are centred on this row's middle.
const LINE_MIDDLE: f32 = 258.0;
/// How far right of centred the giant name's ink starts, and how fast both move left, in px a
/// frame.
const NAME_START: i32 = 60;
const NAME_SPEED: i32 = 1;
const LINE_PX: u32 = 32;
const LINE_SCALE: f32 = 1.3;
const LINE_SPEED: i32 = 4;
/// Where the running line's first repeat starts its ink on the first frame.
const LINE_START: i32 = 231;
const FAULT_HATCH: Rectangle = Rectangle::new(Point::new(290, 164), Size::new(29, 27));
const FAULT_HATCH_SPEED: i32 = 2;
const MICRO_LEFT: i32 = 162;
const ABOVE_TOPS: [i32; 2] = [164, 178];
const REASON_TOP: i32 = 328;
const FAULT_BARCODE_TOP: i32 = 345;
const FAULT_VERSION_GAP: i32 = 8;

/// Draws `text` with its ink left edge on `left` and its ink top on `top`.
fn at_ink<D: CoverageTarget<Color = Color>>(
    style: &FontdueRenderer<'static, Color>,
    text: &str,
    left: i32,
    top: i32,
    target: &mut D,
) -> Result<(), D::Error> {
    let pen = Point::new(text::pen_x_for_ink_left(style, text, left), text::baseline_for_ink_top(style, text, top));
    style.draw_on_baseline(text, pen, target)
}

fn draw_fault<D: CoverageTarget<Color = Color>>(
    frame: u32,
    startup: &Startup,
    version: &str,
    font: &FontdueRenderer<'static, Color>,
    target: &mut D,
) -> Result<(), D::Error> {
    let Some((first, outcome)) = startup.failed().next() else {
        return Ok(());
    };
    let frame = frame as i32;
    // The field, band and strip run to the glass's edge, and the corners past it are never seen.
    // The band paints its own rows, black with the text knocked out of it.
    for rows in [0..BAND_ROWS.start, BAND_ROWS.end..466] {
        let area = Rectangle::new(Point::new(0, rows.start), Size::new(466, rows.len() as u32));
        screens::clear_to(&mut Window::new(&mut *target, Point::zero(), area), chrome::RED)?;
    }
    for y in DASH_ROWS {
        for x in (DASH_FROM..466).step_by(DASH_PITCH as usize) {
            target.fill_solid(&Rectangle::new(Point::new(x, y), DASH), chrome::BLACK)?;
        }
    }
    for centre in PART_CENTRES {
        let half = 5 * PART_MODULE / 2;
        modules(first.glyph(), 5, centre - Point::new_equal(half), PART_MODULE, chrome::BLACK, target)?;
        for top in [centre.y - 38, centre.y + 30] {
            target.fill_solid(&Rectangle::new(Point::new(centre.x - 32, top), Size::new(64, 8)), chrome::BLACK)?;
        }
    }

    {
        let mut band = Knockout::new(&mut *target, BAND_ROWS, STRIP_ROWS, chrome::BLACK);
        // Drawn doubled from half its size, with its ink measured at that size.
        let style = small(font, chrome::RED, NAME_PX / 2, SHAPIRO);
        let name = first.name();
        let ink = style.baseline_bounds(name, Point::zero());
        let x = (CENTER.x + NAME_START - NAME_SPEED * frame) as f32;
        let pen = Point::new(
            libm::roundf(x - ink.size.width as f32) as i32 - 2 * ink.top_left.x,
            libm::roundf(LINE_MIDDLE - ink.size.height as f32) as i32 - 2 * ink.top_left.y,
        );
        style.draw_doubled_on_baseline(name, pen, &mut band)?;
        band.finish();
    }
    {
        let mut strip = Knockout::new(&mut *target, STRIP_ROWS, 0..0, chrome::BLACK);
        let style = small(font, chrome::WHITE, LINE_PX, SHAPIRO);
        let mut line = heapless::String::<64>::new();
        for (part, _) in startup.failed() {
            let _ = write!(line, "{} FAIL_", part.name());
        }
        let ink = style.baseline_bounds(&line, Point::zero());
        let middle = (ink.top_left.y as f32 + ink.size.height as f32 / 2.0) * LINE_SCALE;
        let baseline = libm::roundf(LINE_MIDDLE - middle) as i32;
        let period = style.advance(&line);
        let start = (LINE_START - ink.top_left.x - LINE_SPEED * frame) as f32;
        // The repeat whose pen is at or just left of the panel's edge, then each after it.
        let mut pen = start - libm::ceilf(start / period) * period;
        while pen < 466.0 {
            style.draw_stretched(&line, Point::new(libm::roundf(pen) as i32, baseline), LINE_SCALE, &mut strip)?;
            pen += period;
        }
        strip.finish();
    }

    let field = &mut OnBackground::new(&mut *target, chrome::RED);
    let bold = small(font, chrome::BLACK, 12, FRAKTION_BOLD);
    let regular = small(font, chrome::BLACK, 12, FRAKTION);
    let mut count = heapless::String::<24>::new();
    let _ = write!(count, "SELF TEST {}/6 OK", startup.answered());
    at_ink(&bold, &count, MICRO_LEFT, ABOVE_TOPS[0], field)?;
    let mut failure = heapless::String::<24>::new();
    let _ = write!(failure, "{:02} {} FAIL", first.index() + 1, first.name());
    at_ink(&regular, &failure, MICRO_LEFT, ABOVE_TOPS[1], field)?;
    hatch(FAULT_HATCH, 8, 4, FAULT_HATCH_SPEED * frame, chrome::BLACK, field)?;
    let reason = if startup.demo { "DEMO, NOT A FAULT" } else { outcome.reason() };
    at_ink(&regular, reason, MICRO_LEFT, REASON_TOP, field)?;
    let end = barcode(version, MICRO_LEFT, FAULT_BARCODE_TOP, 12, chrome::BLACK, field)?;
    at_ink(&bold, version, end + FAULT_VERSION_GAP, FAULT_BARCODE_TOP, field)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn passed_at(times: [Micros; 6]) -> Startup {
        let mut startup = Startup::new();
        startup.begin(0);
        for (part, at) in Part::ALL.into_iter().zip(times) {
            startup.report(Report { part, outcome: Outcome::Answered }, at);
        }
        startup
    }

    #[test]
    fn the_identity_starts_once_the_last_glyph_has_landed_and_the_hold_is_over() {
        let startup = passed_at([10_000, 20_000, 30_000, 40_000, 50_000, 1_000_000]);
        let from = 1_000_000 + 4 * ROW + PASS_HOLD;
        assert_eq!(startup.phase(from - 1), Phase::SelfTest);
        assert_eq!(startup.phase(from), Phase::Identity(0));
        assert_eq!(startup.phase(from + 33_334), Phase::Identity(1));
        // 120 identity frames and 19 of the card.
        assert_eq!(startup.phase(from + 4_633_333), Phase::Identity(138));
        assert_eq!(startup.phase(from + 4_633_334), Phase::Done { entry: true, after_card: true });
        assert_eq!(startup.view(from + 4_000_000), Some(View::Card(0)));
    }

    #[test]
    fn a_failure_holds_longer_and_shows_the_fault_screen_for_four_seconds() {
        let mut startup = passed_at([0; 6]);
        startup.outcomes[Part::Magnet.index()] = Some((Outcome::NoReply, 500_000));
        let from = 500_000 + FAIL_HOLD;
        assert_eq!(startup.phase(from - 1), Phase::SelfTest);
        assert_eq!(startup.phase(from), Phase::Fault(0));
        assert_eq!(startup.phase(from + 3_999_999), Phase::Fault(119));
        assert_eq!(startup.phase(from + 4_000_000), Phase::Done { entry: true, after_card: false });
    }

    #[test]
    fn a_touch_skips_the_identity_but_not_the_self_test() {
        let mut startup = passed_at([0; 6]);
        startup.touch(0);
        assert_eq!(startup.phase(ROW), Phase::SelfTest);
        startup.touch(1_000_000);
        assert_eq!(startup.phase(1_000_000), Phase::Done { entry: false, after_card: false });
    }

    #[test]
    fn the_brightness_climbs_then_holds() {
        let startup = passed_at([0; 6]);
        assert_eq!([0, 100_000, RAMP].map(|now| startup.brightness(now, 200)), [0, 100, 200]);
        let from = 4 * ROW + PASS_HOLD;
        assert_eq!(startup.brightness(from + 40_000, 200), 200);
    }

    #[test]
    fn frames_after_the_hold_are_due_at_thirty_a_second() {
        let startup = passed_at([0; 6]);
        let from = 4 * ROW + PASS_HOLD;
        assert_eq!(startup.next_change(from), Some(from + 33_334));
        assert_eq!(startup.next_change(from + 33_334), Some(from + 66_667));
        assert_eq!(startup.next_change(from + 66_667), Some(from + 100_000));
    }

    #[test]
    fn a_passed_cell_builds_its_glyph_a_row_at_a_time() {
        let mut startup = Startup::new();
        startup.begin(0);
        startup.report(Report { part: Part::Touch, outcome: Outcome::Answered }, 300_000);
        let cells = |now| match startup.view(now) {
            Some(View::SelfTest(cells)) => cells[Part::Touch.index()],
            other => panic!("{other:?}"),
        };
        assert!(matches!(startup.view(300_000), Some(View::SelfTest(cells)) if cells[0] == Cell::Waiting));
        assert_eq!([300_000, 329_999, 330_000, 500_000].map(cells), [1, 1, 2, 5].map(Cell::Answered));
        assert_eq!(startup.next_change(310_000), Some(330_000));
        assert_eq!(startup.next_change(420_000), None);
    }

    #[test]
    fn a_demonstration_reports_on_its_schedule_and_ends_on_the_fault_screen() {
        let demo = Startup::demo(Part::Magnet, 1_000_000);
        assert!(!demo.is_boot());
        let cells = |now| match demo.view(now) {
            Some(View::SelfTest(cells)) => cells,
            other => panic!("{other:?}"),
        };
        assert_eq!(cells(1_100_000), [Cell::Waiting; 6]);
        assert_eq!(cells(1_800_000)[Part::Magnet.index()], Cell::Failed);
        assert_eq!(cells(1_800_000)[Part::Gnss.index()], Cell::Waiting);
        assert_eq!(demo.next_change(1_100_000), Some(1_150_000));
        assert_eq!(demo.phase(1_000_000 + 1_300_000 + 4 * ROW + FAIL_HOLD), Phase::Fault(0));
        assert_eq!(demo.brightness(1_000_000, 200), 200);
    }

    /// The fault screen rasterizes its giant name a glyph at a time, at half size, into a raster
    /// of its own with four bytes a pixel on the internal heap. A 140 KB raster for the full
    /// size failed to allocate there.
    #[test]
    fn every_giant_glyph_rasters_within_40_kb() {
        let font = chrome::FONTS[SHAPIRO];
        for c in Part::ALL.into_iter().flat_map(|part| part.name().chars()) {
            let metrics = font.metrics(c, (NAME_PX / 2) as f32);
            assert!(metrics.width * metrics.height * 4 < 40_000, "{c}");
        }
    }

    /// Records which pixels a drawing sets.
    struct Lit(alloc::collections::BTreeSet<(i32, i32)>);

    impl embedded_graphics::geometry::OriginDimensions for Lit {
        fn size(&self) -> Size {
            Size::new(466, 466)
        }
    }

    impl DrawTarget for Lit {
        type Color = Color;
        type Error = core::convert::Infallible;
        fn draw_iter<I: IntoIterator<Item = embedded_graphics::Pixel<Color>>>(&mut self, pixels: I) -> Result<(), Self::Error> {
            self.0.extend(pixels.into_iter().map(|embedded_graphics::Pixel(point, _)| (point.x, point.y)));
            Ok(())
        }
    }

    #[test]
    fn the_hatch_lights_the_pixels_the_stripe_test_does() {
        for mark in [CARD_MARK, CARD_MARK.scaled(CARD_SCALE), Mark { origin: (100.0, 50.0), module: 13.0, stroke: 4.0 }] {
            let mut drawn = Lit(alloc::collections::BTreeSet::new());
            mark.draw(false, true, chrome::LIME, &mut drawn).unwrap();
            let mut tile = Lit(alloc::collections::BTreeSet::new());
            mark.draw(false, false, chrome::LIME, &mut tile).unwrap();
            let scale = mark.module / CARD_MARK.module;
            let inset = mark.module + mark.stroke + HATCH_INSET * scale;
            let (pitch, stripe) = (HATCH_PITCH * scale, HATCH_STRIPE * scale);
            let span = |o: f32| libm::roundf(o + inset) as i32..libm::roundf(o + 5.0 * mark.module - inset) as i32;
            let mut expected = tile.0.clone();
            for y in span(mark.origin.1) {
                for x in span(mark.origin.0) {
                    let across = ((x - CENTER.x) + (y - CENTER.y) + 1) as f32 / core::f32::consts::SQRT_2;
                    if across - libm::floorf(across / pitch) * pitch < stripe {
                        expected.insert((x, y));
                    }
                }
            }
            let differ = drawn.0.symmetric_difference(&expected).count();
            assert!(differ <= expected.len() / 1000, "{differ} of {} differ", expected.len());
        }
    }

    #[test]
    fn the_version_barcode_matches_the_design() {
        struct Columns(alloc::vec::Vec<i32>);
        impl embedded_graphics::geometry::OriginDimensions for Columns {
            fn size(&self) -> Size {
                Size::new(466, 466)
            }
        }
        impl DrawTarget for Columns {
            type Color = Color;
            type Error = core::convert::Infallible;
            fn draw_iter<I: IntoIterator<Item = embedded_graphics::Pixel<Color>>>(&mut self, pixels: I) -> Result<(), Self::Error> {
                for embedded_graphics::Pixel(point, _) in pixels {
                    if point.y == 0 && !self.0.contains(&point.x) {
                        self.0.push(point.x);
                    }
                }
                Ok(())
            }
        }
        let mut columns = Columns(alloc::vec::Vec::new());
        let end = barcode("0.1.0", 34, 0, 1, chrome::LIME, &mut columns).unwrap();
        assert_eq!(end, 99);
        let row: alloc::string::String = (34..99).map(|x| if columns.0.contains(&x) { '#' } else { '.' }).collect();
        // The measured row of the design's identity.
        assert_eq!(row, "#..#..#..#..#..##..##..##..##..#..#..#..#..##..##..##..#..#..#..#");
    }
}

