use embedded_graphics::{
    draw_target::DrawTarget,
    prelude::{Drawable, Point, Primitive, Size},
    primitives::{Line, PrimitiveStyle, Rectangle},
    text::Text,
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
const MAP: Rectangle = Rectangle::new(Point::new(48, 112), Size::new(370, 242));
const FOOTER: Rectangle = Rectangle::new(Point::new(92, 368), Size::new(282, 50));

pub fn render<D>(
    architecture: Architecture,
    state: State,
    target: &mut D,
) -> Result<Dirty, D::Error>
where
    D: DrawTarget<Color = Color>,
{
    let mut dirty = Dirty::new();
    let bounds = target.bounding_box();
    target.fill_solid(&bounds, chrome::BLACK)?;

    match architecture {
        Architecture::Immediate => render_immediate(state, target)?,
        Architecture::Retained => render_retained(state, target)?,
        Architecture::Tiled => render_tiled(state, target)?,
    }

    dirty.add(bounds);
    Ok(dirty)
}

fn render_immediate<D>(state: State, target: &mut D) -> Result<(), D::Error>
where
    D: DrawTarget<Color = Color>,
{
    draw_header(state, target)?;
    draw_map(state, target)?;
    draw_footer(state, target)
}

#[derive(Clone, Copy)]
enum Node {
    Header,
    Map,
    Footer,
}

fn render_retained<D>(state: State, target: &mut D) -> Result<(), D::Error>
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
                Node::Header => draw_header(state, target)?,
                Node::Map => draw_map(state, target)?,
                Node::Footer => draw_footer(state, target)?,
            }
        }
    }
    Ok(())
}

fn render_tiled<D>(state: State, target: &mut D) -> Result<(), D::Error>
where
    D: DrawTarget<Color = Color>,
{
    // These regions are the first candidate composition units for partial flushes.
    // Their boundaries match the visual rails, not arbitrary widget rectangles.
    for (bounds, draw) in [
        (
            HEADER,
            draw_header as fn(State, &mut D) -> Result<(), D::Error>,
        ),
        (MAP, draw_map as fn(State, &mut D) -> Result<(), D::Error>),
        (
            FOOTER,
            draw_footer as fn(State, &mut D) -> Result<(), D::Error>,
        ),
    ] {
        if !bounds.intersection(&target.bounding_box()).is_zero_sized() {
            draw(state, target)?;
        }
    }
    Ok(())
}

fn draw_header<D>(state: State, target: &mut D) -> Result<(), D::Error>
where
    D: DrawTarget<Color = Color>,
{
    target.fill_solid(&HEADER, chrome::PURPLE)?;
    target.fill_solid(
        &Rectangle::new(Point::new(92, 48), Size::new(14, 50)),
        chrome::LIME,
    )?;
    text(
        "FIELD MAP",
        Point::new(114, 60),
        chrome::WHITE,
        &chrome::MARATHON_SHAPIRO65_20,
        target,
    )?;
    text(
        if state.selected_node.is_some() {
            "TRACKING"
        } else {
            "STANDBY"
        },
        Point::new(114, 76),
        if state.selected_node.is_some() {
            chrome::LIME
        } else {
            chrome::GRAY
        },
        &chrome::FRAKTION_MONO20,
        target,
    )?;
    text(
        "01 / 04",
        Point::new(286, 64),
        chrome::BLACK,
        &chrome::FRAKTION_MONO20,
        target,
    )
}

fn draw_map<D>(state: State, target: &mut D) -> Result<(), D::Error>
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
    for y in [160, 208, 256, 304, 336] {
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
        text(
            match node {
                1 => "NODE 01",
                2 => "NODE 02",
                _ => "NODE 03",
            },
            Point::new(216, 310),
            chrome::BLACK,
            &chrome::FRAKTION_MONO20,
            target,
        )?;
    }
    text(
        "N 51.898",
        Point::new(64, 134),
        chrome::GRAY,
        &chrome::FRAKTION_MONO20,
        target,
    )?;
    text(
        "E  -8.475",
        Point::new(64, 326),
        chrome::GRAY,
        &chrome::FRAKTION_MONO20,
        target,
    )
}

fn draw_footer<D>(state: State, target: &mut D) -> Result<(), D::Error>
where
    D: DrawTarget<Color = Color>,
{
    target.fill_solid(
        &Rectangle::new(Point::new(92, 368), Size::new(178, 50)),
        if state.selected_node.is_some() {
            chrome::LIME
        } else {
            chrome::ORANGE_RED
        },
    )?;
    text(
        if state.selected_node.is_some() {
            "PIN SELECTED"
        } else {
            "TAP A NODE"
        },
        Point::new(104, 384),
        chrome::BLACK,
        &chrome::FRAKTION_MONO20,
        target,
    )?;
    target.fill_solid(
        &Rectangle::new(Point::new(278, 368), Size::new(96, 50)),
        chrome::BLACK,
    )?;
    text(
        "SYNC  12:42",
        Point::new(274, 384),
        chrome::WHITE,
        &chrome::FRAKTION_MONO20,
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

fn text<D>(
    value: &str,
    position: Point,
    color: Color,
    font: &u8g2_fonts::Font,
    target: &mut D,
) -> Result<(), D::Error>
where
    D: DrawTarget<Color = Color>,
{
    let style = u8g2_fonts::U8g2TextStyle::new(font.clone(), color);
    Text::new(value, position, style).draw(target).map(|_| ())
}
