//! The settings panel: eight cells in four columns, two columns in view, that scroll sideways.
//! `context/settings-panel/SETTINGS-PANEL-SPEC.md` is its design, and section 5 of the round 3
//! spec adds the TIMEOUT and ALWAYS ON column.

use core::fmt::Write as _;

use embedded_graphics::{
    prelude::{Point, Size},
    primitives::Rectangle,
};
use heapless::String;

use super::{
    clock_screen,
    icon::{self, Glyph, Tile},
    reveal::{Reveal, draw_revealed, revealed_bounds},
    screens::PeripheralState,
    text::{self, style},
};
use crate::chrome::{
    self, Color, CoverageTarget, FontdueRenderer, OnBackground, RgbColorExt as _, Round, FRAKTION,
    FRAKTION_BOLD, SHAPIRO,
};

/// How far a column's left rule is from the next.
pub const COLUMN: i32 = 146;
/// The scroll at the end, with the last two columns in view. The grid rests at a whole number
/// of columns.
pub const MAX_SCROLL: i32 = 2 * COLUMN;
/// Column 0's left rule at rest.
const GRID: i32 = 87;
const RULES: [i32; 3] = [72, 218, 364];
const COLUMNS: i32 = 4;
const CENTER: Point = Point::new(233, 233);
/// Everything in the grid is cut at the page's circle.
const CLIP_RADIUS: f32 = 232.0;
const INSET: i32 = 10;
const TITLE: &str = "SETTINGS";
const TITLE_TOP: i32 = 38;
const MARKERS_TOP: i32 = 386;
const MARKER: i32 = 8;
const MARKERS_LEFT: i32 = 208;
const MARKER_PITCH: i32 = 14;
pub const HINT: &str = "DRAG UP TO CLOSE";
const HINT_TOP: i32 = 406;

pub const ZONE: Glyph = [0b11011, 0b10001, 0b00100, 0b10001, 0b11011];
pub const BRIGHTNESS: Glyph = [0b00001, 0b00011, 0b00111, 0b01111, 0b11111];
pub const TIMEOUT: Glyph = [0b11111, 0b01110, 0b00100, 0b01110, 0b11111];
const ALWAYS_ON: Glyph = [0b00000, 0b01110, 0b11011, 0b01110, 0b00000];
const CALIBRATING: Glyph = [0b01110, 0b10001, 0b10000, 0b10001, 0b01110];
const GNSS: Glyph = [0b00100, 0b01010, 0b10101, 0b01010, 0b00100];
const BATTERY: Glyph = [0b01110, 0b11111, 0b10001, 0b11111, 0b11111];
pub const DEVICE: Glyph = [0b00100, 0b00000, 0b01100, 0b00100, 0b01110];

/// The cells, in index order: down each column, then across.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Cell {
    Zone,
    Brightness,
    Timeout,
    AlwaysOn,
    Compass,
    Gnss,
    Battery,
    Device,
}

impl Cell {
    pub const ALL: [Self; 8] = [
        Self::Zone,
        Self::Brightness,
        Self::Timeout,
        Self::AlwaysOn,
        Self::Compass,
        Self::Gnss,
        Self::Battery,
        Self::Device,
    ];

    #[must_use]
    pub fn index(self) -> usize {
        Self::ALL.iter().position(|&cell| cell == self).unwrap_or(0)
    }

    #[must_use]
    pub fn column(self) -> i32 {
        self.index() as i32 / 2
    }

    fn name(self) -> &'static str {
        match self {
            Self::Zone => "ZONE",
            Self::Brightness => "BRIGHTNESS",
            Self::Timeout => "TIMEOUT",
            Self::AlwaysOn => "ALWAYS ON",
            Self::Compass => "COMPASS",
            Self::Gnss => "GNSS",
            Self::Battery => "BATTERY",
            Self::Device => "DEVICE",
        }
    }

    /// The cell's inside, between its rules.
    #[must_use]
    pub fn bounds(self, scroll: i32) -> Rectangle {
        Rectangle::new(
            Point::new(left(self.column(), scroll) + 1, RULES[self.index() % 2] + 1),
            Size::new_equal((COLUMN - 1) as u32),
        )
    }
}

/// Column `column`'s left rule.
#[must_use]
pub fn left(column: i32, scroll: i32) -> i32 {
    GRID + COLUMN * column - scroll
}

/// Whether all of `column` is in view at `scroll`.
#[must_use]
pub fn in_view(column: i32, scroll: i32) -> bool {
    left(column, scroll) >= GRID && left(column, scroll) + COLUMN <= GRID + 2 * COLUMN
}

/// The scroll from `scroll` that brings `column` into view, moving as little as it can.
#[must_use]
pub fn scroll_to(column: i32, scroll: i32) -> i32 {
    if left(column, scroll) < GRID {
        (column * COLUMN).clamp(0, MAX_SCROLL)
    } else {
        ((column - 1) * COLUMN).clamp(scroll, MAX_SCROLL)
    }
}

/// The cell under `point`, if any.
#[must_use]
pub fn cell_at(point: Point, scroll: i32) -> Option<Cell> {
    Cell::ALL.into_iter().find(|cell| cell.bounds(scroll).contains(point))
}

pub const CELLS: usize = Cell::ALL.len();

/// How far each of the panel's accents has come in: the ring's fade, the title's reveal and the
/// rules' draw-out, 0 to 255; per cell, how many rows of its icon's modules show, 0 to 5, and
/// its index's and name's reveals; whether the column markers show; and the hint's reveal. The
/// icons' frames, the values and the tags always show whole.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Accents {
    pub ring: u8,
    pub title: u8,
    pub rules: u8,
    pub rows: [u8; CELLS],
    pub index: [u8; CELLS],
    pub name: [u8; CELLS],
    pub markers: bool,
    pub hint: u8,
}

impl Accents {
    pub const FULL: Self = Self {
        ring: u8::MAX,
        title: u8::MAX,
        rules: u8::MAX,
        rows: [5; CELLS],
        index: [u8::MAX; CELLS],
        name: [u8::MAX; CELLS],
        markers: true,
        hint: u8::MAX,
    };
    pub const HIDDEN: Self = Self {
        ring: 0,
        title: 0,
        rules: 0,
        rows: [0; CELLS],
        index: [0; CELLS],
        name: [0; CELLS],
        markers: false,
        hint: 0,
    };

    /// Each accent at the lesser of the two.
    #[must_use]
    pub fn min(self, other: Self) -> Self {
        let each = |a: [u8; CELLS], b: [u8; CELLS]| core::array::from_fn(|i| a[i].min(b[i]));
        Self {
            ring: self.ring.min(other.ring),
            title: self.title.min(other.title),
            rules: self.rules.min(other.rules),
            rows: each(self.rows, other.rows),
            index: each(self.index, other.index),
            name: each(self.name, other.name),
            markers: self.markers && other.markers,
            hint: self.hint.min(other.hint),
        }
    }
}

/// What a cell shows from the readings.
#[derive(Clone, Debug, Eq, PartialEq)]
struct Content {
    glyph: &'static Glyph,
    icon: Color,
    value: String<16>,
    value_color: Color,
    tag: Option<&'static str>,
}

/// A brightness level as a percentage of full, rounded.
#[must_use]
pub fn percent(level: u8) -> u8 {
    ((u16::from(level) * 100 + 127) / 255) as u8
}

fn content(cell: Cell, peripherals: &PeripheralState) -> Content {
    let mut value = String::new();
    let (glyph, mut icon, mut value_color, mut tag) = (&ZONE, chrome::WHITE, chrome::WHITE, None);
    let glyph = match cell {
        Cell::Zone => {
            let view = &peripherals.clock;
            tag = Some(match view.zone().mode {
                super::clock::ZoneMode::Automatic => "AUTO",
                super::clock::ZoneMode::Manual => "MANUAL",
            });
            match clock_screen::abbreviation(view) {
                Some(abbreviation) => _ = value.push_str(abbreviation),
                None if view.zone().zone.is_none() => {
                    _ = value.push_str("NO ZONE");
                    value_color = chrome::GRAY;
                }
                None => {
                    _ = value.push_str("--");
                    value_color = chrome::GRAY;
                }
            }
            glyph
        }
        Cell::Brightness => {
            _ = write!(value, "{}%", percent(peripherals.brightness));
            &BRIGHTNESS
        }
        Cell::Timeout => {
            _ = value.push_str(peripherals.timeout.label());
            &TIMEOUT
        }
        Cell::AlwaysOn => {
            if peripherals.always_on {
                tag = Some("ON");
            } else {
                _ = value.push_str("OFF");
                value_color = chrome::GRAY;
            }
            &ALWAYS_ON
        }
        Cell::Compass => {
            let compass = &peripherals.compass;
            if compass.live {
                let percent = compass.calibration_percent.min(100);
                _ = write!(value, "CAL {percent}%");
                if percent < 100 {
                    (icon, value_color) = (chrome::ORANGE, chrome::ORANGE);
                }
                &CALIBRATING
            } else {
                _ = value.push_str("NO DATA");
                (icon, value_color) = (chrome::RED, chrome::RED);
                &icon::NO_DATA
            }
        }
        Cell::Gnss => {
            let gnss = &peripherals.gnss;
            if gnss.fix {
                _ = write!(value, "FIX  {}", gnss.in_use);
                icon = chrome::BLUE;
            } else {
                _ = value.push_str("NO FIX");
                value_color = chrome::GRAY;
            }
            &GNSS
        }
        Cell::Battery => {
            match peripherals.battery {
                Some(battery) => {
                    tag = battery.usb.then_some("USB");
                    if battery.present {
                        _ = write!(value, "{}%", battery.percent.min(100));
                    } else {
                        _ = value.push_str("NONE");
                        value_color = chrome::GRAY;
                    }
                }
                None => {
                    _ = value.push_str("--");
                    value_color = chrome::GRAY;
                }
            }
            &BATTERY
        }
        Cell::Device => {
            _ = value.push_str(peripherals.firmware);
            &DEVICE
        }
    };
    Content { glyph, icon, value, value_color, tag }
}

fn index_style(font: &FontdueRenderer<'static, Color>) -> FontdueRenderer<'static, Color> {
    style(font, chrome::GRAY, 14, FRAKTION)
}

fn name_style(font: &FontdueRenderer<'static, Color>) -> FontdueRenderer<'static, Color> {
    style(font, chrome::GRAY, 14, FRAKTION_BOLD)
}

fn value_style(font: &FontdueRenderer<'static, Color>, color: Color) -> FontdueRenderer<'static, Color> {
    style(font, color, 16, FRAKTION)
}

fn tag_style(font: &FontdueRenderer<'static, Color>) -> FontdueRenderer<'static, Color> {
    style(font, chrome::BLACK, 14, FRAKTION_BOLD)
}

fn title_style(font: &FontdueRenderer<'static, Color>) -> FontdueRenderer<'static, Color> {
    style(font, chrome::WHITE, 16, SHAPIRO)
}

pub fn hint_style(font: &FontdueRenderer<'static, Color>) -> FontdueRenderer<'static, Color> {
    style(font, chrome::GRAY, 14, FRAKTION)
}

fn title_pen(font: &FontdueRenderer<'static, Color>) -> Point {
    let style = title_style(font);
    Point::new(
        text::pen_x_for_ink_centre(&style, TITLE, CENTER.x as f32),
        text::baseline_for_ink_top(&style, TITLE, TITLE_TOP),
    )
}

pub fn hint_pen(font: &FontdueRenderer<'static, Color>) -> Point {
    let style = hint_style(font);
    Point::new(
        text::pen_x_for_ink_centre(&style, HINT, CENTER.x as f32),
        text::baseline_for_ink_top(&style, HINT, HINT_TOP),
    )
}

fn marker(index: i32) -> Rectangle {
    Rectangle::new(
        Point::new(MARKERS_LEFT + index * MARKER_PITCH, MARKERS_TOP),
        Size::new_equal(MARKER as u32),
    )
}

/// All the column markers.
pub const MARKERS: Rectangle = Rectangle::new(
    Point::new(MARKERS_LEFT, MARKERS_TOP),
    Size::new(((COLUMNS - 1) * MARKER_PITCH + MARKER) as u32, MARKER as u32),
);

/// The rows the grid, its rules and everything in its cells occupy.
pub const GRID_ROWS: Rectangle = Rectangle::new(
    Point::new(0, RULES[0]),
    Size::new(466, (RULES[2] - RULES[0] + 1) as u32),
);

fn icon_tile(cell: Cell, scroll: i32) -> Tile {
    let inside = cell.bounds(scroll);
    Tile {
        corner: Point::new(left(cell.column(), scroll) + 70, inside.top_left.y + 9),
        module: 10,
        padding: 8,
    }
}

/// Where a cell's value line starts and the tag box before it, if any.
struct ValueLine {
    tag: Option<(Rectangle, Point)>,
    pen: Point,
}

fn value_line(cell: Cell, content: &Content, scroll: i32, font: &FontdueRenderer<'static, Color>) -> ValueLine {
    let middle = (cell.bounds(scroll).top_left.y + 128) as f32;
    let mut x = left(cell.column(), scroll) + INSET;
    let tag = content.tag.map(|tag| {
        let style = tag_style(font);
        let width = libm::roundf(style.advance("0") * tag.len() as f32) as i32 + 10;
        let bounds = Rectangle::new(Point::new(x, middle as i32 - 9), Size::new(width as u32, 18));
        let pen = Point::new(
            text::pen_x_for_ink_centre(&style, tag, x as f32 + width as f32 / 2.0),
            text::baseline_for_ink_middle(&style, tag, middle),
        );
        x += width + 6;
        (bounds, pen)
    });
    let style = value_style(font, content.value_color);
    let pen = Point::new(
        text::pen_x_for_ink_left(&style, &content.value, x),
        text::baseline_for_ink_middle(&style, &content.value, middle),
    );
    ValueLine { tag, pen }
}

fn draw_cell<D: CoverageTarget<Color = Color>>(
    cell: Cell,
    peripherals: &PeripheralState,
    scroll: i32,
    accents: &Accents,
    font: &FontdueRenderer<'static, Color>,
    target: &mut D,
) -> Result<(), D::Error> {
    let inside = cell.bounds(scroll);
    if !target.visible(&inside) {
        return Ok(());
    }
    let i = cell.index();
    let content = content(cell, peripherals);
    icon_tile(cell, scroll).draw(content.glyph, content.icon, accents.rows[i], target)?;
    let field = &mut OnBackground::new(&mut *target, chrome::BLACK);
    let x = left(cell.column(), scroll) + INSET;
    let mut index = String::<2>::new();
    _ = write!(index, "{:02}", i + 1);
    let style = index_style(font);
    let pen = Point::new(x, inside.top_left.y + 9 + text::cap(&style));
    draw_revealed(&style, &index, pen, Reveal::of(accents.index[i], 2), field)?;
    let style = name_style(font);
    let name = cell.name();
    let pen = Point::new(x, inside.top_left.y + 100 + text::cap(&style));
    draw_revealed(&style, name, pen, Reveal::of(accents.name[i], name.len()), field)?;

    let line = value_line(cell, &content, scroll, font);
    if let (Some((bounds, pen)), Some(tag)) = (line.tag, content.tag) {
        target.fill_solid(&bounds, chrome::GRAY)?;
        tag_style(font).draw_on_baseline(tag, pen, &mut OnBackground::new(&mut *target, chrome::GRAY))?;
    }
    let field = &mut OnBackground::new(&mut *target, chrome::BLACK);
    value_style(font, content.value_color).draw_on_baseline(&content.value, line.pen, field)
}

/// The rules, drawn out from the middle to `progress` of their length.
fn draw_rules<D: CoverageTarget<Color = Color>>(scroll: i32, progress: u8, target: &mut D) -> Result<(), D::Error> {
    if progress == 0 {
        return Ok(());
    }
    let k = f32::from(progress) / 255.0;
    let (start, end) = (left(0, scroll), left(COLUMNS, scroll));
    let from = libm::roundf(CENTER.x as f32 - (CENTER.x - start) as f32 * k) as i32;
    let to = libm::roundf(CENTER.x as f32 + (end - CENTER.x) as f32 * k) as i32;
    for row in RULES {
        target.fill_solid(&Rectangle::with_corners(Point::new(from, row), Point::new(to, row)), chrome::GRAY)?;
    }
    let half = (RULES[2] - RULES[0]) as f32 * k / 2.0;
    let (top, bottom) = (
        libm::roundf(RULES[1] as f32 - half) as i32,
        libm::roundf(RULES[1] as f32 + half) as i32,
    );
    for column in 0..=COLUMNS {
        let x = left(column, scroll);
        target.fill_solid(&Rectangle::with_corners(Point::new(x, top), Point::new(x, bottom)), chrome::GRAY)?;
    }
    Ok(())
}

pub fn draw<D: CoverageTarget<Color = Color>>(
    peripherals: &PeripheralState,
    scroll: i32,
    accents: Accents,
    font: &FontdueRenderer<'static, Color>,
    target: &mut D,
) -> Result<(), D::Error> {
    {
        let grid = &mut Round::new(&mut *target, CENTER, CLIP_RADIUS);
        draw_rules(scroll, accents.rules, grid)?;
        for cell in Cell::ALL {
            draw_cell(cell, peripherals, scroll, &accents, font, grid)?;
        }
    }
    if accents.ring > 0 {
        let ring = chrome::BLACK.lerp(&chrome::GRAY, accents.ring);
        super::smooth::perimeter().draw(&mut OnBackground::new(&mut *target, chrome::BLACK), ring);
    }
    let field = &mut OnBackground::new(&mut *target, chrome::BLACK);
    draw_revealed(&title_style(font), TITLE, title_pen(font), Reveal::of(accents.title, TITLE.len()), field)?;
    if accents.markers {
        let first = (scroll + COLUMN / 2) / COLUMN;
        for index in 0..COLUMNS {
            let square = marker(index);
            if (first..first + 2).contains(&index) {
                target.fill_solid(&square, chrome::WHITE)?;
            } else {
                target.fill_solid(&square, chrome::GRAY)?;
                target.fill_solid(&square.offset(-2), chrome::BLACK)?;
            }
        }
    }
    let field = &mut OnBackground::new(&mut *target, chrome::BLACK);
    draw_revealed(&hint_style(font), HINT, hint_pen(font), Reveal::of(accents.hint, HINT.len()), field)
}

/// What a cell covers, for damage: its inside, which holds everything it draws.
#[must_use]
pub fn cell_damage(cell: Cell, scroll: i32) -> Rectangle {
    cell.bounds(scroll)
}

/// Whether a cell's contents differ between two readings.
#[must_use]
pub fn cell_changed(cell: Cell, before: &PeripheralState, after: &PeripheralState) -> bool {
    content(cell, before) != content(cell, after)
}

/// Everything the title and hint can cover, at any stage of their reveals.
#[must_use]
pub fn text_damage(font: &FontdueRenderer<'static, Color>) -> [Rectangle; 2] {
    [
        revealed_bounds(&title_style(font), TITLE, title_pen(font)),
        revealed_bounds(&hint_style(font), HINT, hint_pen(font)),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_two_columns_show_at_rest_and_the_last_two_at_the_end() {
        let shown = |scroll| (0..4).map(|c| in_view(c, scroll)).collect::<heapless::Vec<_, 4>>();
        assert_eq!(shown(0), [true, true, false, false]);
        assert_eq!(shown(COLUMN), [false, true, true, false]);
        assert_eq!(shown(MAX_SCROLL), [false, false, true, true]);
    }

    #[test]
    fn a_cropped_column_scrolls_into_view_the_short_way() {
        assert_eq!(scroll_to(2, 0), COLUMN);
        assert_eq!(scroll_to(3, COLUMN), MAX_SCROLL);
        assert_eq!(scroll_to(1, MAX_SCROLL), COLUMN);
        assert_eq!(scroll_to(0, COLUMN), 0);
    }

    #[test]
    fn cells_are_hit_between_their_rules() {
        assert_eq!(cell_at(Point::new(150, 150), 0), Some(Cell::Zone));
        assert_eq!(cell_at(Point::new(150, 300), 0), Some(Cell::Brightness));
        assert_eq!(cell_at(Point::new(300, 100), 0), Some(Cell::Timeout));
        assert_eq!(cell_at(Point::new(300, 300), 0), Some(Cell::AlwaysOn));
        assert_eq!(cell_at(Point::new(400, 100), 0), Some(Cell::Compass));
        assert_eq!(cell_at(Point::new(300, 100), MAX_SCROLL), Some(Cell::Battery));
        assert_eq!(cell_at(Point::new(150, 218), 0), None);
        assert_eq!(cell_at(Point::new(150, 30), 0), None);
    }

    #[test]
    fn a_level_shows_as_a_rounded_percentage() {
        assert_eq!([120, 26, 255, 128].map(percent), [47, 10, 100, 50]);
    }
}
