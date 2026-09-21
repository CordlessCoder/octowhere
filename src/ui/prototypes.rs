use embedded_graphics::{
    draw_target::DrawTarget,
    prelude::{Drawable, Point, Primitive, Size},
    primitives::{Line, PrimitiveStyle, Rectangle},
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

pub const ACTIVE_ARCHITECTURE: Architecture = Architecture::Immediate;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct State {
    pub selected_node: Option<u8>,
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
    draw_map(state, font, target)?;
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
                Node::Map => draw_map(state, font, target)?,
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
            draw_map
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
            "FIELD MAP",
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
        "01 / 04",
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
        if state.selected_node.is_some() {
            "PIN SELECTED"
        } else {
            "TAP A NODE"
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
    aligned_text(
        "SYNC  12:42",
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
