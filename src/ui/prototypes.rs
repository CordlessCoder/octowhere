use embedded_graphics::{
    Pixel,
    draw_target::DrawTarget,
    prelude::{Dimensions, Drawable, Point, Primitive, Size},
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

const DISPLAY_CENTER: Point = Point::new(233, 233);
const DISPLAY_RADIUS: i32 = 233;

struct CircularTarget<'a, D> {
    inner: &'a mut D,
    bounds: Rectangle,
}

impl<D: DrawTarget> Dimensions for CircularTarget<'_, D> {
    fn bounding_box(&self) -> Rectangle {
        self.bounds
    }
}

impl<D: DrawTarget> DrawTarget for CircularTarget<'_, D> {
    type Color = D::Color;
    type Error = D::Error;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        self.inner
            .draw_iter(pixels.into_iter().filter(|Pixel(point, _)| {
                let dx = point.x - DISPLAY_CENTER.x;
                let dy = point.y - DISPLAY_CENTER.y;
                dx * dx + dy * dy <= DISPLAY_RADIUS * DISPLAY_RADIUS
            }))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct State {
    pub touch_active: bool,
}

const HEADER: Rectangle = Rectangle::new(Point::new(28, 24), Size::new(410, 64));
const MAP: Rectangle = Rectangle::new(Point::new(28, 104), Size::new(410, 260));
const FOOTER: Rectangle = Rectangle::new(Point::new(28, 384), Size::new(410, 58));

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
    let mut target = CircularTarget {
        inner: target,
        bounds,
    };
    target.fill_solid(&bounds, chrome::BLACK)?;

    match architecture {
        Architecture::Immediate => render_immediate(state, &mut target)?,
        Architecture::Retained => render_retained(state, &mut target)?,
        Architecture::Tiled => render_tiled(state, &mut target)?,
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
        &Rectangle::new(Point::new(28, 24), Size::new(14, 64)),
        chrome::LIME,
    )?;
    text(
        "FIELD MAP",
        Point::new(112, 50),
        chrome::WHITE,
        &chrome::MARATHON_SHAPIRO65_20,
        target,
    )?;
    text(
        if state.touch_active {
            "TRACKING"
        } else {
            "STANDBY"
        },
        Point::new(112, 76),
        if state.touch_active {
            chrome::LIME
        } else {
            chrome::GRAY
        },
        &chrome::FRAKTION_MONO20,
        target,
    )?;
    text(
        "01 / 04",
        Point::new(318, 56),
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
    for x in [70, 145, 220, 295, 370] {
        Line::new(Point::new(x, 104), Point::new(x, 364))
            .into_styled(frame)
            .draw(target)?;
    }
    for y in [144, 204, 264, 324] {
        Line::new(Point::new(28, y), Point::new(438, y))
            .into_styled(frame)
            .draw(target)?;
    }

    // Block-built symbols keep the map readable and avoid decorative geometry.
    marker(Point::new(118, 178), chrome::LIME, target)?;
    marker(Point::new(300, 238), chrome::ORANGE_RED, target)?;
    marker(Point::new(365, 318), chrome::WHITE, target)?;
    if state.touch_active {
        target.fill_solid(
            &Rectangle::new(Point::new(218, 300), Size::new(92, 34)),
            chrome::LIME,
        )?;
        text(
            "LOCKED",
            Point::new(229, 308),
            chrome::BLACK,
            &chrome::FRAKTION_MONO20,
            target,
        )?;
    }
    text(
        "N 51.898",
        Point::new(42, 120),
        chrome::GRAY,
        &chrome::FRAKTION_MONO20,
        target,
    )?;
    text(
        "E  -8.475",
        Point::new(42, 340),
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
        &Rectangle::new(Point::new(28, 384), Size::new(250, 58)),
        if state.touch_active {
            chrome::LIME
        } else {
            chrome::ORANGE_RED
        },
    )?;
    text(
        if state.touch_active {
            "HOLD TO PIN"
        } else {
            "TAP A NODE"
        },
        Point::new(96, 383),
        chrome::BLACK,
        &chrome::FRAKTION_MONO20,
        target,
    )?;
    target.fill_solid(
        &Rectangle::new(Point::new(294, 384), Size::new(144, 58)),
        chrome::BLACK,
    )?;
    text(
        "SYNC  12:42",
        Point::new(286, 383),
        chrome::WHITE,
        &chrome::FRAKTION_MONO20,
        target,
    )
}

fn marker<D>(center: Point, color: Color, target: &mut D) -> Result<(), D::Error>
where
    D: DrawTarget<Color = Color>,
{
    target.fill_solid(
        &Rectangle::new(center - Point::new(12, 12), Size::new(24, 24)),
        color,
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
