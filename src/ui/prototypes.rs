use core::fmt::Write as _;

use embedded_graphics::{
    draw_target::DrawTarget,
    prelude::{Drawable, Point, Primitive, Size},
    primitives::{Circle, Line, PrimitiveStyle, Rectangle},
    text::Text,
};
use embedded_layout::{
    align::{Align, horizontal, vertical},
    layout::linear::{LinearLayout, spacing::FixedMargin},
    prelude::Views,
};

use crate::chrome::{self, Color, Dirty};

/// Rendering boundaries under consideration for the first product screens.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Architecture {
    /// One ordered function owns the composition and redraws its visible scene.
    Immediate,
    /// A fixed scene tree owns layout and each node can be invalidated separately.
    Retained,
    /// Fixed regions are composed independently so the flush grid can skip them.
    Tiled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Screen {
    Map,
    Motion,
    Clock,
    Touch,
    Power,
    Navigation,
    Compass,
}

impl Screen {
    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::Map => Self::Motion,
            Self::Motion => Self::Clock,
            Self::Clock => Self::Touch,
            Self::Touch => Self::Power,
            Self::Power => Self::Navigation,
            Self::Navigation => Self::Compass,
            Self::Compass => Self::Map,
        }
    }

    #[must_use]
    const fn title(self) -> &'static str {
        match self {
            Self::Map => "FIELD MAP",
            Self::Motion => "MOTION",
            Self::Clock => "CLOCK",
            Self::Touch => "TOUCH",
            Self::Power => "POWER / IO",
            Self::Navigation => "NAV / RADIO",
            Self::Compass => "COMPASS",
        }
    }

    #[must_use]
    const fn page(self) -> &'static str {
        match self {
            Self::Map => "01 / 07",
            Self::Motion => "02 / 07",
            Self::Clock => "03 / 07",
            Self::Touch => "04 / 07",
            Self::Power => "05 / 07",
            Self::Navigation => "06 / 07",
            Self::Compass => "07 / 07",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ClockState {
    pub hours: u8,
    pub minutes: u8,
    pub seconds: u8,
    pub day: u8,
    pub month: u8,
    pub year: u8,
    pub valid: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PeripheralState {
    pub accel_micro_ms2: [i32; 3],
    pub gyro_micro_rad_s: [i32; 3],
    pub imu_valid: bool,
    pub clock: ClockState,
    pub touch_points: u8,
    pub touch_position: Option<Point>,
    pub touch_positions: [Option<Point>; 2],
    pub pmic_valid: bool,
    pub tca_valid: bool,
    pub gnss_valid: bool,
    pub lora_valid: bool,
    pub compass_valid: bool,
    pub battery_mv: Option<u16>,
    pub vbus_mv: Option<u16>,
    pub vsys_mv: Option<u16>,
    pub gnss_bytes: u16,
    pub lora_irq: u8,
    pub magnetic_microtesla: [i32; 3],
}

pub const ACTIVE_ARCHITECTURE: Architecture = Architecture::Immediate;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct State {
    pub screen: Screen,
    pub selected_node: Option<u8>,
    pub peripherals: PeripheralState,
}

const HEADER: Rectangle = Rectangle::new(Point::new(92, 48), Size::new(282, 50));
const HEADER_CONTENT: Rectangle = Rectangle::new(Point::new(108, 48), Size::new(258, 50));
const MAP: Rectangle = Rectangle::new(Point::new(48, 112), Size::new(370, 242));
const FOOTER: Rectangle = Rectangle::new(Point::new(92, 368), Size::new(282, 50));
const FOOTER_PIN: Rectangle = Rectangle::new(Point::new(92, 368), Size::new(178, 50));
const FOOTER_PIN_CONTENT: Rectangle = Rectangle::new(Point::new(100, 368), Size::new(162, 50));
const FOOTER_SYNC: Rectangle = Rectangle::new(Point::new(278, 368), Size::new(96, 50));
const FOOTER_SYNC_CONTENT: Rectangle = Rectangle::new(Point::new(282, 368), Size::new(88, 50));

pub fn render<D>(
    architecture: Architecture,
    state: State,
    font: &chrome::FontdueRenderer<'static, Color>,
    target: &mut D,
) -> Result<Dirty, D::Error>
where
    D: DrawTarget<Color = Color>,
{
    let mut dirty = Dirty::new();
    let bounds = target.bounding_box();
    target.fill_solid(&bounds, chrome::BLACK)?;

    match architecture {
        Architecture::Immediate => render_immediate(state, font, target)?,
        Architecture::Retained => render_retained(state, font, target)?,
        Architecture::Tiled => render_tiled(state, font, target)?,
    }

    dirty.add(bounds);
    Ok(dirty)
}

fn render_immediate<D>(
    state: State,
    font: &chrome::FontdueRenderer<'static, Color>,
    target: &mut D,
) -> Result<(), D::Error>
where
    D: DrawTarget<Color = Color>,
{
    draw_header(state, font, target)?;
    draw_screen(state, font, target)?;
    draw_footer(state, font, target)
}

#[derive(Clone, Copy)]
enum Node {
    Header,
    Map,
    Footer,
}

fn render_retained<D>(
    state: State,
    font: &chrome::FontdueRenderer<'static, Color>,
    target: &mut D,
) -> Result<(), D::Error>
where
    D: DrawTarget<Color = Color>,
{
    // This is deliberately a fixed scene. It gives us retained invalidation without
    // introducing a heap-backed widget tree before the interaction model is known.
    let nodes = [Node::Header, Node::Map, Node::Footer];
    for node in nodes {
        let bounds = match node {
            Node::Header => HEADER,
            Node::Map => MAP,
            Node::Footer => FOOTER,
        };
        if !bounds.intersection(&target.bounding_box()).is_zero_sized() {
            match node {
                Node::Header => draw_header(state, font, target)?,
                Node::Map => draw_screen(state, font, target)?,
                Node::Footer => draw_footer(state, font, target)?,
            }
        }
    }
    Ok(())
}

fn render_tiled<D>(
    state: State,
    font: &chrome::FontdueRenderer<'static, Color>,
    target: &mut D,
) -> Result<(), D::Error>
where
    D: DrawTarget<Color = Color>,
{
    // These regions are the first candidate composition units for partial flushes.
    // Their boundaries match the visual rails, not arbitrary widget rectangles.
    for (bounds, draw) in [
        (
            HEADER,
            draw_header
                as fn(
                    State,
                    &chrome::FontdueRenderer<'static, Color>,
                    &mut D,
                ) -> Result<(), D::Error>,
        ),
        (
            MAP,
            draw_screen
                as fn(
                    State,
                    &chrome::FontdueRenderer<'static, Color>,
                    &mut D,
                ) -> Result<(), D::Error>,
        ),
        (
            FOOTER,
            draw_footer
                as fn(
                    State,
                    &chrome::FontdueRenderer<'static, Color>,
                    &mut D,
                ) -> Result<(), D::Error>,
        ),
    ] {
        if !bounds.intersection(&target.bounding_box()).is_zero_sized() {
            draw(state, font, target)?;
        }
    }
    Ok(())
}

fn draw_header<D>(
    state: State,
    font: &chrome::FontdueRenderer<'static, Color>,
    target: &mut D,
) -> Result<(), D::Error>
where
    D: DrawTarget<Color = Color>,
{
    target.fill_solid(&HEADER, chrome::PURPLE)?;
    target.fill_solid(
        &Rectangle::new(Point::new(92, 48), Size::new(14, 50)),
        chrome::LIME,
    )?;
    let mut labels = [
        Text::new(
            state.screen.title(),
            Point::zero(),
            font_style(font, chrome::WHITE, chrome::PURPLE, 20, 0),
        ),
        Text::new(
            if state.selected_node.is_some() {
                "TRACKING"
            } else {
                "STANDBY"
            },
            Point::zero(),
            font_style(
                font,
                if state.selected_node.is_some() {
                    chrome::LIME
                } else {
                    chrome::GRAY
                },
                chrome::PURPLE,
                20,
                1,
            ),
        ),
    ];
    LinearLayout::vertical(Views::new(&mut labels))
        .with_spacing(FixedMargin(1))
        .arrange()
        .align_to(&HEADER_CONTENT, horizontal::Left, vertical::Center)
        .draw(target)?;
    aligned_text(
        state.screen.page(),
        &HEADER_CONTENT,
        font,
        chrome::BLACK,
        chrome::PURPLE,
        16,
        1,
        horizontal::Right,
        vertical::Center,
        target,
    )
}

fn draw_screen<D>(
    state: State,
    font: &chrome::FontdueRenderer<'static, Color>,
    target: &mut D,
) -> Result<(), D::Error>
where
    D: DrawTarget<Color = Color>,
{
    match state.screen {
        Screen::Map => draw_map(state, font, target),
        Screen::Motion => draw_motion(state, font, target),
        Screen::Clock => draw_clock(state, font, target),
        Screen::Touch => draw_touch(state, font, target),
        Screen::Power => draw_power(state, font, target),
        Screen::Navigation => draw_navigation(state, font, target),
        Screen::Compass => draw_compass(state, font, target),
    }
}

fn draw_map<D>(
    state: State,
    font: &chrome::FontdueRenderer<'static, Color>,
    target: &mut D,
) -> Result<(), D::Error>
where
    D: DrawTarget<Color = Color>,
{
    let frame = PrimitiveStyle::with_stroke(chrome::GRAY, 2);
    MAP.into_styled(frame).draw(target)?;
    for x in [122, 196, 270, 344] {
        Line::new(Point::new(x, 112), Point::new(x, 354))
            .into_styled(frame)
            .draw(target)?;
    }
    for y in [160, 208, 256, 304] {
        Line::new(Point::new(48, y), Point::new(418, y))
            .into_styled(frame)
            .draw(target)?;
    }

    // Block-built symbols keep the map readable and avoid decorative geometry.
    marker(
        Point::new(132, 178),
        chrome::LIME,
        state.selected_node == Some(1),
        target,
    )?;
    marker(
        Point::new(294, 238),
        chrome::ORANGE_RED,
        state.selected_node == Some(2),
        target,
    )?;
    marker(
        Point::new(354, 316),
        chrome::WHITE,
        state.selected_node == Some(3),
        target,
    )?;
    if let Some(node) = state.selected_node {
        target.fill_solid(
            &Rectangle::new(Point::new(214, 294), Size::new(92, 34)),
            chrome::LIME,
        )?;
        aligned_text(
            match node {
                1 => "NODE 01",
                2 => "NODE 02",
                _ => "NODE 03",
            },
            &Rectangle::new(Point::new(214, 294), Size::new(92, 34)),
            font,
            chrome::BLACK,
            chrome::LIME,
            16,
            1,
            horizontal::Center,
            vertical::Center,
            target,
        )?;
    }
    aligned_text(
        "N 51.898",
        &Rectangle::new(Point::new(64, 124), Size::new(140, 30)),
        font,
        chrome::GRAY,
        chrome::BLACK,
        16,
        1,
        horizontal::Left,
        vertical::Center,
        target,
    )?;
    aligned_text(
        "E  -8.475",
        &Rectangle::new(Point::new(64, 316), Size::new(140, 30)),
        font,
        chrome::GRAY,
        chrome::BLACK,
        16,
        1,
        horizontal::Left,
        vertical::Center,
        target,
    )
}

fn draw_motion<D>(
    state: State,
    font: &chrome::FontdueRenderer<'static, Color>,
    target: &mut D,
) -> Result<(), D::Error>
where
    D: DrawTarget<Color = Color>,
{
    let frame = PrimitiveStyle::with_stroke(chrome::GRAY, 2);
    let accel = Rectangle::new(Point::new(64, 136), Size::new(162, 178));
    let gyro = Rectangle::new(Point::new(240, 136), Size::new(162, 178));
    for panel in [accel, gyro] {
        panel.into_styled(frame).draw(target)?;
    }
    aligned_text(
        "ACCEL / M/S2",
        &Rectangle::new(Point::new(76, 148), Size::new(138, 28)),
        font,
        chrome::LIME,
        chrome::BLACK,
        16,
        1,
        horizontal::Left,
        vertical::Center,
        target,
    )?;
    aligned_text(
        "GYRO / RAD/S",
        &Rectangle::new(Point::new(252, 148), Size::new(138, 28)),
        font,
        chrome::ORANGE_RED,
        chrome::BLACK,
        16,
        1,
        horizontal::Left,
        vertical::Center,
        target,
    )?;
    draw_axis_values(
        state.peripherals.accel_micro_ms2,
        Point::new(76, 180),
        chrome::WHITE,
        font,
        target,
    )?;
    draw_axis_values(
        state.peripherals.gyro_micro_rad_s,
        Point::new(252, 180),
        chrome::WHITE,
        font,
        target,
    )?;
    aligned_text(
        if state.peripherals.imu_valid {
            "LIVE / SI"
        } else {
            "WAITING"
        },
        &Rectangle::new(Point::new(76, 274), Size::new(314, 26)),
        font,
        chrome::GRAY,
        chrome::BLACK,
        16,
        1,
        horizontal::Left,
        vertical::Center,
        target,
    )
}

fn draw_axis_values<D>(
    values: [i32; 3],
    origin: Point,
    color: Color,
    font: &chrome::FontdueRenderer<'static, Color>,
    target: &mut D,
) -> Result<(), D::Error>
where
    D: DrawTarget<Color = Color>,
{
    for (index, (axis, value)) in ["X", "Y", "Z"].into_iter().zip(values).enumerate() {
        let mut text = heapless::String::<16>::new();
        let value = format_fixed3(value);
        _ = write!(text, "{axis} {value}");
        aligned_text(
            text.as_str(),
            &Rectangle::new(
                origin + Point::new(0, index as i32 * 28),
                Size::new(138, 26),
            ),
            font,
            color,
            chrome::BLACK,
            16,
            1,
            horizontal::Left,
            vertical::Center,
            target,
        )?;
    }
    Ok(())
}

fn format_fixed3(value: i32) -> heapless::String<16> {
    let mut text = heapless::String::new();
    let sign = if value < 0 { '-' } else { '+' };
    let value = value.unsigned_abs();
    _ = write!(
        text,
        "{sign}{}.{:03}",
        value / 1_000_000,
        value % 1_000_000 / 1_000
    );
    text
}

fn draw_clock<D>(
    state: State,
    font: &chrome::FontdueRenderer<'static, Color>,
    target: &mut D,
) -> Result<(), D::Error>
where
    D: DrawTarget<Color = Color>,
{
    let frame = PrimitiveStyle::with_stroke(chrome::GRAY, 2);
    let panel = Rectangle::new(Point::new(64, 136), Size::new(338, 178));
    panel.into_styled(frame).draw(target)?;
    let clock = state.peripherals.clock;
    let mut time = heapless::String::<16>::new();
    let mut date = heapless::String::<24>::new();
    if clock.valid {
        _ = write!(
            time,
            "{:02}:{:02}:{:02}",
            clock.hours, clock.minutes, clock.seconds
        );
        _ = write!(
            date,
            "20{:02}-{:02}-{:02}",
            clock.year, clock.month, clock.day
        );
    } else {
        time.push_str("--:--:--").unwrap();
        date.push_str("RTC UNAVAILABLE").unwrap();
    }
    aligned_text(
        time.as_str(),
        &Rectangle::new(Point::new(76, 158), Size::new(314, 62)),
        font,
        chrome::WHITE,
        chrome::BLACK,
        28,
        1,
        horizontal::Center,
        vertical::Center,
        target,
    )?;
    aligned_text(
        date.as_str(),
        &Rectangle::new(Point::new(76, 232), Size::new(314, 34)),
        font,
        chrome::LIME,
        chrome::BLACK,
        16,
        1,
        horizontal::Center,
        vertical::Center,
        target,
    )?;
    aligned_text(
        if clock.valid {
            "RTC / LIVE"
        } else {
            "RTC / ERROR"
        },
        &Rectangle::new(Point::new(76, 278), Size::new(314, 24)),
        font,
        if clock.valid {
            chrome::GRAY
        } else {
            chrome::ORANGE_RED
        },
        chrome::BLACK,
        16,
        1,
        horizontal::Center,
        vertical::Center,
        target,
    )
}

fn draw_touch<D>(
    state: State,
    font: &chrome::FontdueRenderer<'static, Color>,
    target: &mut D,
) -> Result<(), D::Error>
where
    D: DrawTarget<Color = Color>,
{
    let frame = PrimitiveStyle::with_stroke(chrome::GRAY, 2);
    let panel = Rectangle::new(Point::new(64, 136), Size::new(338, 178));
    panel.into_styled(frame).draw(target)?;
    for position in state.peripherals.touch_positions.into_iter().flatten() {
        Circle::new(position - Point::new(14, 14), 28)
            .into_styled(PrimitiveStyle::with_stroke(chrome::LIME, 2))
            .draw(target)?;
        target.fill_solid(
            &Rectangle::new(position - Point::new(3, 3), Size::new(6, 6)),
            chrome::WHITE,
        )?;
    }
    let mut points = heapless::String::<16>::new();
    let mut position = heapless::String::<24>::new();
    _ = write!(points, "POINTS  {:02}", state.peripherals.touch_points);
    if let Some(point) = state.peripherals.touch_position {
        _ = write!(position, "X {:03}   Y {:03}", point.x, point.y);
    } else {
        position.push_str("NO ACTIVE TOUCH").unwrap();
    }
    aligned_text(
        points.as_str(),
        &Rectangle::new(Point::new(76, 152), Size::new(314, 28)),
        font,
        chrome::WHITE,
        chrome::BLACK,
        16,
        1,
        horizontal::Left,
        vertical::Center,
        target,
    )?;
    aligned_text(
        position.as_str(),
        &Rectangle::new(Point::new(76, 274), Size::new(314, 26)),
        font,
        chrome::LIME,
        chrome::BLACK,
        16,
        1,
        horizontal::Center,
        vertical::Center,
        target,
    )
}

fn status_panel<D>(
    title: &str,
    status: bool,
    region: Rectangle,
    font: &chrome::FontdueRenderer<'static, Color>,
    target: &mut D,
) -> Result<(), D::Error>
where
    D: DrawTarget<Color = Color>,
{
    region
        .into_styled(PrimitiveStyle::with_stroke(chrome::GRAY, 2))
        .draw(target)?;
    aligned_text(
        title,
        &Rectangle::new(
            region.top_left + Point::new(12, 10),
            Size::new(region.size.width - 24, 28),
        ),
        font,
        chrome::WHITE,
        chrome::BLACK,
        16,
        1,
        horizontal::Left,
        vertical::Center,
        target,
    )?;
    aligned_text(
        if status { "ONLINE" } else { "NO DATA" },
        &Rectangle::new(
            region.top_left + Point::new(12, 54),
            Size::new(region.size.width - 24, 30),
        ),
        font,
        if status {
            chrome::LIME
        } else {
            chrome::ORANGE_RED
        },
        chrome::BLACK,
        20,
        0,
        horizontal::Left,
        vertical::Center,
        target,
    )
}

fn draw_power<D>(
    state: State,
    font: &chrome::FontdueRenderer<'static, Color>,
    target: &mut D,
) -> Result<(), D::Error>
where
    D: DrawTarget<Color = Color>,
{
    let battery = format_optional_mv("VBAT", state.peripherals.battery_mv);
    let vbus = format_optional_mv("VBUS", state.peripherals.vbus_mv);
    let vsys = format_optional_mv("VSYS", state.peripherals.vsys_mv);
    aligned_text(
        &battery,
        &Rectangle::new(Point::new(76, 138), Size::new(314, 28)),
        font,
        chrome::WHITE,
        chrome::BLACK,
        16,
        1,
        horizontal::Center,
        vertical::Center,
        target,
    )?;
    aligned_text(
        &vbus,
        &Rectangle::new(Point::new(76, 174), Size::new(314, 28)),
        font,
        chrome::LIME,
        chrome::BLACK,
        16,
        1,
        horizontal::Center,
        vertical::Center,
        target,
    )?;
    aligned_text(
        &vsys,
        &Rectangle::new(Point::new(76, 210), Size::new(314, 28)),
        font,
        chrome::WHITE,
        chrome::BLACK,
        16,
        1,
        horizontal::Center,
        vertical::Center,
        target,
    )?;
    aligned_text(
        if state.peripherals.tca_valid {
            "TCA9554 / RESET OK"
        } else {
            "TCA9554 / ERROR"
        },
        &Rectangle::new(Point::new(76, 270), Size::new(314, 28)),
        font,
        chrome::GRAY,
        chrome::BLACK,
        16,
        1,
        horizontal::Center,
        vertical::Center,
        target,
    )
}

fn draw_navigation<D>(
    state: State,
    font: &chrome::FontdueRenderer<'static, Color>,
    target: &mut D,
) -> Result<(), D::Error>
where
    D: DrawTarget<Color = Color>,
{
    aligned_text(
        "GNSS NMEA",
        &Rectangle::new(Point::new(76, 142), Size::new(314, 28)),
        font,
        chrome::WHITE,
        chrome::BLACK,
        16,
        1,
        horizontal::Center,
        vertical::Center,
        target,
    )?;
    let mut nmea = heapless::String::<24>::new();
    _ = write!(
        nmea,
        "{} BYTES / {}",
        state.peripherals.gnss_bytes,
        if state.peripherals.gnss_valid {
            "VALID"
        } else {
            "WAITING"
        }
    );
    aligned_text(
        &nmea,
        &Rectangle::new(Point::new(76, 174), Size::new(314, 28)),
        font,
        chrome::LIME,
        chrome::BLACK,
        16,
        1,
        horizontal::Center,
        vertical::Center,
        target,
    )?;
    aligned_text(
        "LORA IRQ",
        &Rectangle::new(Point::new(76, 222), Size::new(314, 28)),
        font,
        chrome::WHITE,
        chrome::BLACK,
        16,
        1,
        horizontal::Center,
        vertical::Center,
        target,
    )?;
    let mut irq = heapless::String::<16>::new();
    _ = write!(
        irq,
        "0x{:02X} / {}",
        state.peripherals.lora_irq,
        if state.peripherals.lora_valid {
            "READY"
        } else {
            "ERROR"
        }
    );
    aligned_text(
        &irq,
        &Rectangle::new(Point::new(76, 254), Size::new(314, 28)),
        font,
        chrome::LIME,
        chrome::BLACK,
        16,
        1,
        horizontal::Center,
        vertical::Center,
        target,
    )?;
    aligned_text(
        "LIVE PERIPHERAL DATA",
        &Rectangle::new(Point::new(76, 270), Size::new(314, 28)),
        font,
        chrome::GRAY,
        chrome::BLACK,
        16,
        1,
        horizontal::Center,
        vertical::Center,
        target,
    )
}

fn draw_compass<D>(
    state: State,
    font: &chrome::FontdueRenderer<'static, Color>,
    target: &mut D,
) -> Result<(), D::Error>
where
    D: DrawTarget<Color = Color>,
{
    let panel = Rectangle::new(Point::new(64, 136), Size::new(338, 178));
    panel
        .into_styled(PrimitiveStyle::with_stroke(chrome::GRAY, 2))
        .draw(target)?;
    aligned_text(
        "BMM350 MAGNETOMETER",
        &Rectangle::new(Point::new(76, 150), Size::new(314, 30)),
        font,
        chrome::WHITE,
        chrome::BLACK,
        16,
        1,
        horizontal::Center,
        vertical::Center,
        target,
    )?;
    draw_axis_values(
        state.peripherals.magnetic_microtesla,
        Point::new(92, 196),
        chrome::WHITE,
        font,
        target,
    )?;
    aligned_text(
        if state.peripherals.compass_valid {
            "LIVE / uT"
        } else {
            "NO DATA"
        },
        &Rectangle::new(Point::new(76, 278), Size::new(314, 24)),
        font,
        if state.peripherals.compass_valid {
            chrome::LIME
        } else {
            chrome::ORANGE_RED
        },
        chrome::BLACK,
        16,
        1,
        horizontal::Center,
        vertical::Center,
        target,
    )
}

fn format_mv(label: &str, value: u16) -> heapless::String<16> {
    let mut text = heapless::String::new();
    _ = write!(text, "{label} {value}mV");
    text
}

fn format_optional_mv(label: &str, value: Option<u16>) -> heapless::String<16> {
    value.map_or_else(
        || {
            let mut text = heapless::String::new();
            _ = write!(text, "{label} --");
            text
        },
        |value| format_mv(label, value),
    )
}

fn draw_footer<D>(
    state: State,
    font: &chrome::FontdueRenderer<'static, Color>,
    target: &mut D,
) -> Result<(), D::Error>
where
    D: DrawTarget<Color = Color>,
{
    target.fill_solid(
        &FOOTER_PIN,
        if state.selected_node.is_some() {
            chrome::LIME
        } else {
            chrome::ORANGE_RED
        },
    )?;
    aligned_text(
        match state.screen {
            Screen::Map if state.selected_node.is_some() => "PIN SELECTED",
            Screen::Map => "TAP A NODE",
            _ => "TAP HEADER",
        },
        &FOOTER_PIN_CONTENT,
        font,
        chrome::BLACK,
        if state.selected_node.is_some() {
            chrome::LIME
        } else {
            chrome::ORANGE_RED
        },
        16,
        1,
        horizontal::Center,
        vertical::Center,
        target,
    )?;
    target.fill_solid(&FOOTER_SYNC, chrome::BLACK)?;
    let mut sync = heapless::String::<16>::new();
    if state.peripherals.clock.valid {
        _ = write!(
            sync,
            "SYNC {:02}:{:02}",
            state.peripherals.clock.hours, state.peripherals.clock.minutes
        );
    } else {
        sync.push_str("SYNC --:--").unwrap();
    }
    aligned_text(
        sync.as_str(),
        &FOOTER_SYNC_CONTENT,
        font,
        chrome::WHITE,
        chrome::BLACK,
        16,
        1,
        horizontal::Center,
        vertical::Center,
        target,
    )
}

fn marker<D>(center: Point, color: Color, selected: bool, target: &mut D) -> Result<(), D::Error>
where
    D: DrawTarget<Color = Color>,
{
    if selected {
        target.fill_solid(
            &Rectangle::new(center - Point::new(16, 16), Size::new(32, 32)),
            chrome::WHITE,
        )?;
    }
    target.fill_solid(
        &Rectangle::new(center - Point::new(12, 12), Size::new(24, 24)),
        if selected { chrome::LIME } else { color },
    )?;
    target.fill_solid(
        &Rectangle::new(center - Point::new(4, 4), Size::new(8, 8)),
        chrome::BLACK,
    )
}

#[allow(clippy::too_many_arguments)]
fn aligned_text<D, H, V>(
    value: &str,
    region: &Rectangle,
    font: &chrome::FontdueRenderer<'static, Color>,
    color: Color,
    background: Color,
    size: u32,
    font_index: usize,
    horizontal: H,
    vertical: V,
    target: &mut D,
) -> Result<(), D::Error>
where
    D: DrawTarget<Color = Color>,
    H: embedded_layout::align::HorizontalAlignment,
    V: embedded_layout::align::VerticalAlignment,
{
    Text::new(
        value,
        Point::zero(),
        font_style(font, color, background, size, font_index),
    )
    .align_to(region, horizontal, vertical)
    .draw(target)
    .map(|_| ())
}

fn font_style(
    font: &chrome::FontdueRenderer<'static, Color>,
    color: Color,
    background: Color,
    size: u32,
    font_index: usize,
) -> chrome::FontdueRenderer<'static, Color> {
    let mut style = font.clone();
    style.text_color = color;
    style.background_color = background;
    style.font_size = size;
    style.font_index = font_index;
    style
}
