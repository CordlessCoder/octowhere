//! The S1 settings panel: four indexed rows on each of two sideways-scrolling pages.

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
    scatter::{Field, Look, Scatter},
    screens::PeripheralState,
    text::{self, style},
};
use crate::chrome::{
    self, Color, CoverageTarget, FRAKTION, FRAKTION_BOLD, FontdueRenderer, OnBackground,
    RgbColorExt as _, Round, SHAPIRO, Window,
};

/// Distance between the two complete pages during a sideways drag.
pub const PAGE_WIDTH: i32 = 466;
pub const MAX_SCROLL: i32 = PAGE_WIDTH;
const RULE_TOP: [i32; 4] = [83, 160, 227, 294];
const RULE_LEFT: [i32; 4] = [83, 57, 57, 83];
const RULE_RIGHT: [i32; 4] = [383, 409, 409, 383];
const CENTER: Point = Point::new(233, 233);
const CLIP_RADIUS: f32 = 232.0;
const TITLE: &str = "SETTINGS";
const TITLE_TOP: i32 = 29;
const MARKERS_TOP: i32 = 387;
const MARKER: i32 = 8;
const MARKERS_LEFT: i32 = 221;
const MARKER_PITCH: i32 = 16;
pub const HINT: &str = "DRAG UP TO CLOSE";
const HINT_TOP: i32 = 406;
const SCATTER: Scatter = Scatter {
    origin: Point::new(24, 10),
    gap: None,
    color: chrome::PURPLE,
    fields: &[
        Field {
            center: Point::new(35, 180),
            radius: 160.0,
            seed: 0x53_31_4c,
        },
        Field {
            center: Point::new(430, 304),
            radius: 160.0,
            seed: 0x53_31_52,
        },
    ],
};
const SCATTER_LOOKS: [Look; 2] = [
    Look {
        facing: 3.0,
        density: 0.8,
    },
    Look {
        facing: 0.0,
        density: 0.8,
    },
];
const SCATTER_CLEAR: [Rectangle; 3] = [
    Rectangle::new(Point::new(77, 82), Size::new(312, 289)),
    Rectangle::new(Point::new(102, 0), Size::new(262, 83)),
    Rectangle::new(Point::new(112, 371), Size::new(242, 70)),
];

pub const ZONE: Glyph = [0b11011, 0b10001, 0b00100, 0b10001, 0b11011];
pub const BRIGHTNESS: Glyph = [0b00001, 0b00011, 0b00111, 0b01111, 0b11111];
pub const TIMEOUT: Glyph = [0b11111, 0b01110, 0b00100, 0b01110, 0b11111];
const ALWAYS_ON: Glyph = [0b00000, 0b01110, 0b11011, 0b01110, 0b00000];
const CALIBRATING: Glyph = [0b01110, 0b10001, 0b10000, 0b10001, 0b01110];
const GNSS: Glyph = [0b00100, 0b01010, 0b10101, 0b01010, 0b00100];
const BATTERY: Glyph = [0b01110, 0b11111, 0b10001, 0b11111, 0b11111];
pub const DEVICE: Glyph = [0b00100, 0b00000, 0b01100, 0b00100, 0b01110];

/// The cells, in reading order across the two pages.
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
    pub fn page(self) -> i32 {
        self.index() as i32 / 4
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

    /// The row's tap region on the moving page.
    #[must_use]
    pub fn bounds(self, scroll: i32) -> Rectangle {
        let row = self.index() % 4;
        Rectangle::with_corners(
            Point::new(
                page_left(self.page(), scroll) + RULE_LEFT[row],
                RULE_TOP[row],
            ),
            Point::new(
                page_left(self.page(), scroll) + RULE_RIGHT[row],
                if row == 3 { 370 } else { RULE_TOP[row + 1] - 1 },
            ),
        )
    }
}

/// The moving page's horizontal origin.
#[must_use]
pub fn page_left(page: i32, scroll: i32) -> i32 {
    PAGE_WIDTH * page - scroll
}

/// Whether the page is settled and all four rows are touchable.
#[must_use]
pub fn in_view(page: i32, scroll: i32) -> bool {
    page_left(page, scroll) == 0
}

/// The cell under `point`, if any.
#[must_use]
pub fn cell_at(point: Point, scroll: i32) -> Option<Cell> {
    Cell::ALL
        .into_iter()
        .find(|cell| cell.bounds(scroll).contains(point))
}

pub const CELLS: usize = Cell::ALL.len();

/// How far each of the panel's accents has come in: the ring's fade, the title's reveal and the
/// rules' draw-out, 0 to 255; per cell, how many rows of its icon's modules show, 0 to 5, and
/// its index's and name's reveals; whether the page markers show; and the hint's reveal. The
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
    Content {
        glyph,
        icon,
        value,
        value_color,
        tag,
    }
}

fn index_style(font: &FontdueRenderer<'static, Color>) -> FontdueRenderer<'static, Color> {
    style(font, chrome::GRAY, 12, FRAKTION)
}

fn name_style(font: &FontdueRenderer<'static, Color>) -> FontdueRenderer<'static, Color> {
    style(font, chrome::WHITE, 19, FRAKTION_BOLD)
}

fn value_style(
    font: &FontdueRenderer<'static, Color>,
    color: Color,
) -> FontdueRenderer<'static, Color> {
    style(font, color, 20, FRAKTION_BOLD)
}

fn title_style(font: &FontdueRenderer<'static, Color>) -> FontdueRenderer<'static, Color> {
    style(font, chrome::WHITE, 27, SHAPIRO)
}

pub fn hint_style(font: &FontdueRenderer<'static, Color>) -> FontdueRenderer<'static, Color> {
    style(font, chrome::GRAY, 12, FRAKTION)
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

fn icon_tile(cell: Cell, scroll: i32) -> Tile {
    let row = cell.index() % 4;
    Tile {
        corner: Point::new(
            page_left(cell.page(), scroll) + RULE_RIGHT[row] - 47,
            RULE_TOP[row] + 13,
        ),
        module: 7,
        padding: 4,
    }
}

fn draw_cell<D: CoverageTarget<Color = Color>>(
    cell: Cell,
    peripherals: &PeripheralState,
    scroll: i32,
    accents: &Accents,
    font: &FontdueRenderer<'static, Color>,
    target: &mut D,
) -> Result<(), D::Error> {
    let bounds = cell.bounds(scroll);
    if !target.visible(&bounds) {
        return Ok(());
    }
    let i = cell.index();
    let row = i % 4;
    let x = page_left(cell.page(), scroll) + RULE_LEFT[row];
    let top = RULE_TOP[row];
    let content = content(cell, peripherals);
    icon_tile(cell, scroll).draw(content.glyph, content.icon, accents.rows[i], target)?;
    let mut index = String::<2>::new();
    _ = write!(index, "{:02}", i + 1);
    let style = index_style(font);
    let pen = Point::new(x + 13, text::baseline_for_ink_top(&style, &index, top + 10));
    draw_revealed(
        &style,
        &index,
        pen,
        Reveal::of(accents.index[i], 2),
        &mut OnBackground::new(&mut *target, chrome::BLACK),
    )?;
    let style = name_style(font);
    let name = cell.name();
    let pen = Point::new(x + 45, text::baseline_for_ink_top(&style, name, top + 9));
    draw_revealed(
        &style,
        name,
        pen,
        Reveal::of(accents.name[i], name.len()),
        &mut OnBackground::new(&mut *target, chrome::BLACK),
    )?;
    let mut value = String::<24>::new();
    if let Some(tag) = content.tag {
        _ = write!(value, "{tag}  ");
    }
    _ = value.push_str(&content.value);
    let style = value_style(font, content.value_color);
    let pen = Point::new(x + 45, text::baseline_for_ink_top(&style, &value, top + 38));
    style.draw_on_baseline(
        &value,
        pen,
        &mut OnBackground::new(&mut *target, chrome::BLACK),
    )?;
    let marker = Rectangle::new(Point::new(x, top + 12), Size::new(3, 16));
    target.fill_solid(
        &marker,
        if cell == Cell::Compass && content.icon == chrome::ORANGE {
            chrome::ORANGE
        } else {
            chrome::shade(chrome::BLUE, 30)
        },
    )
}

/// The rules, drawn out from the middle to `progress` of their length.
fn draw_rules<D: CoverageTarget<Color = Color>>(
    scroll: i32,
    progress: u8,
    target: &mut D,
) -> Result<(), D::Error> {
    if progress == 0 {
        return Ok(());
    }
    for page in 0..=1 {
        let shift = page_left(page, scroll);
        for row in 0..4 {
            let start = shift + RULE_LEFT[row];
            let end = shift + RULE_RIGHT[row];
            let width = ((end - start) as i64 * i64::from(progress) / 255) as i32;
            target.fill_solid(
                &Rectangle::new(Point::new(start, RULE_TOP[row]), Size::new(width as u32, 1)),
                chrome::shade(chrome::GRAY, 145),
            )?;
        }
        target.fill_solid(
            &Rectangle::new(
                Point::new(shift + 83, 370),
                Size::new((300 * i32::from(progress) / 255) as u32, 1),
            ),
            chrome::shade(chrome::GRAY, 145),
        )?;
    }
    Ok(())
}

pub fn draw_scatter<D: CoverageTarget<Color = Color>>(target: &mut D) -> Result<(), D::Error> {
    let mut scatter = SCATTER.clone();
    scatter.color = chrome::shade(chrome::PURPLE, 100);
    scatter.draw_clear_of(&SCATTER_LOOKS, &SCATTER_CLEAR, target)
}

pub fn draw<D: CoverageTarget<Color = Color>>(
    peripherals: &PeripheralState,
    scroll: i32,
    accents: Accents,
    font: &FontdueRenderer<'static, Color>,
    target: &mut D,
) -> Result<(), D::Error> {
    draw_scatter(target)?;
    {
        let grid = &mut Round::new(&mut *target, CENTER, CLIP_RADIUS);
        let rows = &mut Window::new(
            grid,
            Point::zero(),
            Rectangle::new(Point::new(0, 83), Size::new(466, 288)),
        );
        draw_rules(scroll, accents.rules, rows)?;
        for cell in Cell::ALL {
            draw_cell(cell, peripherals, scroll, &accents, font, rows)?;
        }
    }
    if accents.ring > 0 {
        let ring = chrome::BLACK.lerp(&chrome::GRAY, accents.ring);
        super::smooth::perimeter().draw(&mut OnBackground::new(&mut *target, chrome::BLACK), ring);
    }
    let field = &mut OnBackground::new(&mut *target, chrome::BLACK);
    draw_revealed(
        &title_style(font),
        TITLE,
        title_pen(font),
        Reveal::of(accents.title, TITLE.len()),
        field,
    )?;
    let page = ((scroll + PAGE_WIDTH / 2) / PAGE_WIDTH).clamp(0, 1);
    let subtitle = if page == 0 {
        "DISPLAY / 01"
    } else {
        "DEVICE / 02"
    };
    let style = hint_style(font);
    let pen = Point::new(
        text::pen_x_for_ink_centre(&style, subtitle, CENTER.x as f32),
        text::baseline_for_ink_top(&style, subtitle, 64),
    );
    style.draw_on_baseline(
        subtitle,
        pen,
        &mut OnBackground::new(&mut *target, chrome::BLACK),
    )?;
    if accents.markers {
        for index in 0..2 {
            let square = marker(index);
            if page == index {
                target.fill_solid(&square, chrome::WHITE)?;
            } else {
                target.fill_solid(&square, chrome::GRAY)?;
                target.fill_solid(&square.offset(-2), chrome::BLACK)?;
            }
        }
    }
    let field = &mut OnBackground::new(&mut *target, chrome::BLACK);
    draw_revealed(
        &hint_style(font),
        HINT,
        hint_pen(font),
        Reveal::of(accents.hint, HINT.len()),
        field,
    )
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
    fn one_complete_page_is_active_at_each_end() {
        assert!(in_view(0, 0));
        assert!(!in_view(1, 0));
        assert!(!in_view(0, MAX_SCROLL));
        assert!(in_view(1, MAX_SCROLL));
    }

    #[test]
    fn each_page_has_four_rows() {
        assert_eq!(Cell::Zone.page(), 0);
        assert_eq!(Cell::AlwaysOn.page(), 0);
        assert_eq!(Cell::Compass.page(), 1);
        assert_eq!(Cell::Device.page(), 1);
    }

    #[test]
    fn cells_are_hit_between_their_rules() {
        assert_eq!(cell_at(Point::new(150, 115), 0), Some(Cell::Zone));
        assert_eq!(cell_at(Point::new(150, 190), 0), Some(Cell::Brightness));
        assert_eq!(cell_at(Point::new(300, 260), 0), Some(Cell::Timeout));
        assert_eq!(cell_at(Point::new(300, 330), 0), Some(Cell::AlwaysOn));
        assert_eq!(
            cell_at(Point::new(300, 115), MAX_SCROLL),
            Some(Cell::Compass)
        );
        assert_eq!(
            cell_at(Point::new(300, 260), MAX_SCROLL),
            Some(Cell::Battery)
        );
        assert_eq!(cell_at(Point::new(420, 100), 0), None);
        assert_eq!(cell_at(Point::new(150, 30), 0), None);
    }

    #[test]
    fn a_level_shows_as_a_rounded_percentage() {
        assert_eq!([120, 26, 255, 128].map(percent), [47, 10, 100, 50]);
    }
}
