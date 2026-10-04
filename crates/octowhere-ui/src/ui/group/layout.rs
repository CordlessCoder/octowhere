//! The group screens as lists of what they draw: text placed by its ink, filled and outlined
//! boxes and the status glyph. A screen builds its list from its state and the mesh each step;
//! the stage draws the list, and damages only what differs from the list drawn before.

use core::fmt::Write as _;

use embedded_graphics::{
    prelude::{Point, Size, Transform},
    primitives::Rectangle,
};
use heapless::{String, Vec};

use super::super::{
    gesture::Micros,
    icon::Glyph,
    members, panel, stroke,
    text::{self, style},
};
use crate::chrome::{
    self, Color, CoverageTarget, Dirty, FRAKTION, FRAKTION_SANS_LIGHT, FontdueRenderer,
    INTERFERENCE_BOLD, OnBackground, SHAPIRO, Window,
};

pub type Line = String<40>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Face {
    /// Marathon Shapiro: titles.
    Title,
    /// PP Fraktion Mono Regular: names, addresses, keys and small captions.
    Mono,
    /// KH Interference Bold: status headings, ids and numbers.
    Kh,
    /// PP Fraktion Sans Light: explanations.
    Sans,
}

impl Face {
    #[must_use]
    pub fn index(self) -> usize {
        match self {
            Self::Title => SHAPIRO,
            Self::Mono => FRAKTION,
            Self::Kh => INTERFERENCE_BOLD,
            Self::Sans => FRAKTION_SANS_LIGHT,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Align {
    /// Ink starts on `x`.
    Left,
    /// Ink centred on `x`.
    Centre,
    /// Ink ends on `x`.
    Right,
    /// The pen starts on `x`, so that a caret can be placed between characters.
    Pen,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Vertical {
    /// Ink starts on `y`.
    Top,
    /// A capital's ink would start on `y`, so a name sits on one baseline whatever its letters.
    Cap,
    /// Ink centred on `y`.
    Middle,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Text {
    pub text: Line,
    pub face: Face,
    pub size: u8,
    pub color: Color,
    pub x: i16,
    pub y: i16,
    pub align: Align,
    pub vertical: Vertical,
    /// The colour under the text, which its edges blend with.
    pub on: Color,
}

impl Text {
    /// Centred by its ink, over black, until told otherwise.
    #[must_use]
    pub fn new(text: &str, face: Face, size: u8, color: Color) -> Self {
        Self {
            text: line(text),
            face,
            size,
            color,
            x: 0,
            y: 0,
            align: Align::Centre,
            vertical: Vertical::Top,
            on: chrome::BLACK,
        }
    }

    #[must_use]
    pub fn at(mut self, x: i32, y: i32) -> Self {
        (self.x, self.y) = (x as i16, y as i16);
        self
    }

    #[must_use]
    pub fn align(mut self, align: Align) -> Self {
        self.align = align;
        self
    }

    #[must_use]
    pub fn vertical(mut self, vertical: Vertical) -> Self {
        self.vertical = vertical;
        self
    }

    #[must_use]
    pub fn on(mut self, on: Color) -> Self {
        self.on = on;
        self
    }

    pub fn style(&self, font: &FontdueRenderer<'static, Color>) -> FontdueRenderer<'static, Color> {
        style(font, self.color, u32::from(self.size), self.face.index())
    }

    pub fn pen(&self, style: &FontdueRenderer<'static, Color>) -> Point {
        let (x, y) = (i32::from(self.x), i32::from(self.y));
        let x = match self.align {
            Align::Left => text::pen_x_for_ink_left(style, &self.text, x),
            Align::Centre => text::pen_x_for_ink_centre(style, &self.text, x as f32),
            Align::Right => text::pen_x_for_ink_right(style, &self.text, x),
            Align::Pen => x,
        };
        let baseline = match self.vertical {
            Vertical::Top => text::baseline_for_ink_top(style, &self.text, y),
            Vertical::Cap => y + text::cap(style),
            Vertical::Middle => text::baseline_for_ink_middle(style, &self.text, y as f32),
        };
        Point::new(x, baseline)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Shape {
    Text(Text),
    Fill(Rectangle, Color),
    /// A one-pixel outline in the first colour around a fill in the second.
    Boxed(Rectangle, Color, Color),
    /// The status glyph in its frame, in a colour.
    Glyph(Glyph, Color),
    Path(Path),
    /// The polygon through a path's points, filled; its width is unused.
    Polygon(Path),
    Arc(Arc),
    Turned(Turned),
}

/// Text centred on `center` and turned clockwise by `angle` tenths of a degree, as the member
/// face's labels are turned along its ring.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Turned {
    pub text: Line,
    pub face: Face,
    pub size: u8,
    pub color: Color,
    pub center: Point,
    pub angle: i16,
}

impl Turned {
    fn style(&self, font: &FontdueRenderer<'static, Color>) -> FontdueRenderer<'static, Color> {
        style(font, self.color, u32::from(self.size), self.face.index())
    }
}

/// Straight segments through `points`, in quarter pixels, `width` quarter pixels wide.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Path {
    pub points: Vec<(i16, i16), 6>,
    pub width: u8,
    pub color: Color,
}

/// An arc about `center`, `radius` quarter pixels to the middle of its `width`, over `span`
/// tenths of a degree clockwise from the right.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Arc {
    pub center: Point,
    pub radius: u16,
    pub width: u8,
    pub span: (i16, i16),
    pub color: Color,
}

/// What a list draws under its items.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Backdrop {
    /// The settings panel's scatter, clear of the screen's middle.
    #[default]
    Panel,
    /// The same scatter breathing at a level, clear of every text's ink by 2 px, as the
    /// 2026-10-04 screens have it.
    Breathing(u8),
    /// The member face's grid, turned this many degrees anticlockwise so that it keeps to
    /// true north.
    Grid(u16),
    /// Nothing: the items lie over whatever was drawn before them.
    None,
}

/// How far the breathing scatter keeps from a text's ink.
const INK_CLEAR: u32 = 2;
/// The most texts a breathing scatter keeps clear of.
const CLEARS: usize = 48;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Item {
    pub shape: Shape,
    /// Drawn only inside this.
    pub clip: Option<Rectangle>,
}

/// The status glyph's frame, and where its modules start.
pub const GLYPH: Rectangle = Rectangle::new(Point::new(342, 96), Size::new_equal(42));
const GLYPH_MODULE: i32 = 6;

const ITEMS: usize = 128;

/// One screen's drawing.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct List {
    items: Vec<Item, ITEMS>,
    clip: Option<Rectangle>,
    backdrop: Backdrop,
    /// When the screen next changes on its own: a countdown or an age reaching its next value.
    due: Option<Micros>,
}

impl List {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        self.items.clear();
        self.clip = None;
        self.due = None;
        self.backdrop = Backdrop::Panel;
    }

    pub fn set_backdrop(&mut self, backdrop: Backdrop) {
        self.backdrop = backdrop;
    }

    /// The boxes the breathing scatter keeps clear of: every text's ink, grown by 2 px.
    fn clears(&self, font: &FontdueRenderer<'static, Color>) -> Vec<Rectangle, CLEARS> {
        self.items
            .iter()
            .filter(|item| matches!(item.shape, Shape::Text(_)))
            .map(|item| bounds(item, font).offset(INK_CLEAR as i32))
            .take(CLEARS)
            .collect()
    }

    #[must_use]
    pub fn items(&self) -> &[Item] {
        &self.items
    }

    #[must_use]
    pub fn due(&self) -> Option<Micros> {
        self.due
    }

    /// Notes that the screen changes on its own at `at`.
    pub fn changes_at(&mut self, at: Micros) {
        self.due = Some(self.due.map_or(at, |due| due.min(at)));
    }

    /// Clips what is added from here to `clip`, or with `None` stops clipping.
    pub fn clip(&mut self, clip: Option<Rectangle>) {
        self.clip = clip;
    }

    pub fn push(&mut self, shape: Shape) {
        let pushed = self.items.push(Item {
            shape,
            clip: self.clip,
        });
        debug_assert!(pushed.is_ok(), "a screen drew more than {ITEMS} items");
    }

    pub fn fill(&mut self, rect: Rectangle, color: Color) {
        self.push(Shape::Fill(rect, color));
    }

    /// An outlined box over black.
    pub fn outline(&mut self, rect: Rectangle, color: Color) {
        self.push(Shape::Boxed(rect, color, chrome::BLACK));
    }

    pub fn boxed(&mut self, rect: Rectangle, outline: Color, fill: Color) {
        self.push(Shape::Boxed(rect, outline, fill));
    }

    pub fn glyph(&mut self, glyph: Glyph, color: Color) {
        self.push(Shape::Glyph(glyph, color));
    }

    /// Straight segments through `points`, in quarter pixels, `width` quarter pixels wide.
    pub fn path(&mut self, points: &[(i16, i16)], width: u8, color: Color) {
        self.push(Shape::Path(Path {
            points: points.iter().copied().take(6).collect(),
            width,
            color,
        }));
    }

    /// The polygon through `points`, in quarter pixels, filled.
    pub fn polygon(&mut self, points: &[(i16, i16)], color: Color) {
        self.push(Shape::Polygon(Path {
            points: points.iter().copied().take(6).collect(),
            width: 0,
            color,
        }));
    }

    pub fn arc(&mut self, arc: Arc) {
        self.push(Shape::Arc(arc));
    }

    /// Text whose ink is centred on column `x` and starts on row `top`, over black.
    pub fn centred(&mut self, text: &str, x: i32, top: i32, face: Face, size: u8, color: Color) {
        self.push(Shape::Text(Text::new(text, face, size, color).at(x, top)));
    }

    /// Text whose ink starts at column `x` and row `top`, over black.
    pub fn left(&mut self, text: &str, x: i32, top: i32, face: Face, size: u8, color: Color) {
        self.push(Shape::Text(
            Text::new(text, face, size, color)
                .at(x, top)
                .align(Align::Left),
        ));
    }

    pub fn text(&mut self, text: Text) {
        self.push(Shape::Text(text));
    }

    /// Moves every item from the `start`th on, and its clip, `dx` pixels across, as a page that
    /// slides sideways.
    pub fn shift_from(&mut self, start: usize, dx: i32) {
        if dx == 0 {
            return;
        }
        let quarter = (dx * 4) as i16;
        for item in self.items.iter_mut().skip(start) {
            item.clip = item.clip.map(|clip| clip.translate(Point::new(dx, 0)));
            match &mut item.shape {
                Shape::Text(text) => text.x += dx as i16,
                Shape::Fill(rect, _) | Shape::Boxed(rect, ..) => {
                    *rect = rect.translate(Point::new(dx, 0));
                }
                Shape::Glyph(..) => {}
                Shape::Path(path) | Shape::Polygon(path) => {
                    path.points.iter_mut().for_each(|(x, _)| *x += quarter);
                }
                Shape::Arc(arc) => arc.center.x += dx,
                Shape::Turned(turned) => turned.center.x += dx,
            }
        }
    }

    pub fn draw<D: CoverageTarget<Color = Color>>(
        &self,
        font: &FontdueRenderer<'static, Color>,
        target: &mut D,
    ) -> Result<(), D::Error> {
        match self.backdrop {
            Backdrop::Panel => panel::draw_scatter(&panel::Accents::FULL, target)?,
            Backdrop::Breathing(breath) => {
                panel::draw_breathing_scatter(breath, &self.clears(font), target)?;
            }
            Backdrop::Grid(turn) => members::draw_grid(turn, target)?,
            Backdrop::None => {}
        }
        for item in &self.items {
            match item.clip {
                Some(clip) => draw_shape(
                    &item.shape,
                    font,
                    &mut Window::new(&mut *target, Point::zero(), clip),
                )?,
                None => draw_shape(&item.shape, font, target)?,
            }
        }
        Ok(())
    }

    /// Marks in `damage` what differs between `before` and this list: everything either list
    /// draws that the other does not.
    pub fn damage(
        &self,
        before: &Self,
        font: &FontdueRenderer<'static, Color>,
        damage: &mut Dirty,
    ) {
        match (before.backdrop, self.backdrop) {
            (Backdrop::Panel, Backdrop::Panel) | (Backdrop::None, Backdrop::None) => {}
            (Backdrop::Grid(was), Backdrop::Grid(is)) if was == is => {}
            (Backdrop::Breathing(was), Backdrop::Breathing(is)) => {
                let (old, new) = (before.clears(font), self.clears(font));
                if (was, &old) != (is, &new) {
                    panel::breathing_scatter_damage((was, &old), (is, &new), damage);
                }
            }
            _ => damage.make_full(),
        }
        for item in &before.items {
            if !self.items.contains(item) {
                damage.add(bounds(item, font));
            }
        }
        for item in &self.items {
            if !before.items.contains(item) {
                damage.add(bounds(item, font));
            }
        }
    }
}

fn line(text: &str) -> Line {
    let mut line = Line::new();
    for c in text.chars() {
        if line.push(c).is_err() {
            break;
        }
    }
    line
}

/// Formats into a [`Line`], cut off at its length.
#[must_use]
pub fn format(args: core::fmt::Arguments<'_>) -> Line {
    let mut line = Line::new();
    _ = line.write_fmt(args);
    line
}

fn bounds(item: &Item, font: &FontdueRenderer<'static, Color>) -> Rectangle {
    let area = match &item.shape {
        Shape::Text(text) => {
            let style = text.style(font);
            style
                .baseline_bounds(&text.text, text.pen(&style))
                .offset(1)
        }
        Shape::Fill(rect, _) | Shape::Boxed(rect, ..) => *rect,
        Shape::Glyph(..) => GLYPH,
        Shape::Path(path) | Shape::Polygon(path) => {
            stroke::path_bounds(&path.points, path.width).unwrap_or(Rectangle::zero())
        }
        Shape::Arc(arc) => stroke::arc_bounds(arc.center, arc.radius, arc.width, arc.span),
        Shape::Turned(turned) => {
            let reach = libm::ceilf(turned.style(font).rotated_reach(&turned.text)) as u32;
            Rectangle::with_center(turned.center, Size::new_equal(2 * reach + 3))
        }
    };
    match item.clip {
        Some(clip) => area.intersection(&clip),
        None => area,
    }
}

fn draw_shape<D: CoverageTarget<Color = Color>>(
    shape: &Shape,
    font: &FontdueRenderer<'static, Color>,
    target: &mut D,
) -> Result<(), D::Error> {
    match shape {
        Shape::Text(text) => {
            let style = text.style(font);
            let pen = text.pen(&style);
            style.draw_on_baseline(&text.text, pen, &mut OnBackground::new(target, text.on))
        }
        Shape::Fill(rect, color) => target.fill_solid(rect, *color),
        Shape::Boxed(rect, outline, fill) => draw_box(*rect, *outline, *fill, target),
        Shape::Glyph(glyph, color) => {
            draw_box(GLYPH, *color, chrome::BLACK, target)?;
            let corner = GLYPH.top_left + Point::new_equal(GLYPH_MODULE);
            for (row, bits) in glyph.iter().enumerate() {
                for column in (0..5).filter(|column| bits & (0b10000 >> column) != 0) {
                    let module = Rectangle::new(
                        corner + Point::new(column * GLYPH_MODULE, row as i32 * GLYPH_MODULE),
                        Size::new_equal(GLYPH_MODULE as u32),
                    );
                    target.fill_solid(&module, *color)?;
                }
            }
            Ok(())
        }
        Shape::Path(path) => {
            stroke::draw_path(&path.points, path.width, path.color, target);
            Ok(())
        }
        Shape::Polygon(path) => {
            stroke::fill_polygon(&path.points, path.color, target);
            Ok(())
        }
        Shape::Arc(arc) => {
            stroke::draw_arc(
                arc.center, arc.radius, arc.width, arc.span, arc.color, target,
            );
            Ok(())
        }
        Shape::Turned(turned) => {
            let (sin, cos) = libm::sincosf((f32::from(turned.angle) / 10.0).to_radians());
            turned
                .style(font)
                .draw_rotated(&turned.text, turned.center, cos, sin, target)
        }
    }
}

fn draw_box<D: CoverageTarget<Color = Color>>(
    rect: Rectangle,
    outline: Color,
    fill: Color,
    target: &mut D,
) -> Result<(), D::Error> {
    if outline == fill {
        return target.fill_solid(&rect, fill);
    }
    target.fill_solid(&rect, outline)?;
    target.fill_solid(&rect.offset(-1), fill)
}

/// A rectangle from its corners, the far one excluded, as the design gives them.
#[must_use]
pub const fn rect(x0: i32, y0: i32, x1: i32, y1: i32) -> Rectangle {
    Rectangle::new(
        Point::new(x0, y0),
        Size::new((x1 - x0) as u32, (y1 - y0) as u32),
    )
}
