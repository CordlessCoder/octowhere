//! The drawer an upward drag on a face opens, from the 2026-10-04 hand-off: Events and Messages,
//! two roots side by side, and the screens each opens. Each root keeps its place in its list,
//! and the drawer returns to the face it opened over when it closes. It builds a [`List`] from
//! its state and the events each step; the stage draws it.

pub mod parts;
mod rows;

pub use rows::{TOAST, TOAST_COMPACT, toast};

use embedded_graphics::{prelude::Point, primitives::Rectangle};
use heapless::Vec;

use self::parts::{FOOTER, TOP_HIT};
use super::{
    ease::Ease,
    events::{self, Events, Id},
    gesture::{Drag, GestureEvent, Micros},
    group::{
        layout::{Arc, Backdrop, Face, List, format, rect},
        view::MeshView,
    },
    screens::Gnss,
};
use crate::{
    board,
    chrome::{self, Color, FontdueRenderer},
};

const WIDTH: i32 = board::LCD_WIDTH as i32;
/// Where the roots' lists show their rows, and how far under its top the first row starts.
const VIEWPORT: Rectangle = rect(0, 122, WIDTH, 409);
const VIEWPORT_HEIGHT: i32 = 287;
const FIRST: i32 = 4;
/// A drag past this share of the width, or faster than [`FLICK`] in pixels per second, completes
/// a switch between the roots.
const COMMIT: i32 = WIDTH / 4;
const FLICK: f32 = 600.0;
/// The management page's two choices.
const MARK_ALL_READ: Rectangle = rect(82, 135, 384, 229);
const CLEAR_READ: Rectangle = rect(82, 255, 384, 349);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Root {
    Events,
    Messages,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Child {
    /// An event's detail, by its id, which may go away while it shows.
    Event(Id),
    /// MARK ALL READ and CLEAR READ HISTORY.
    Manage,
}

/// What a gesture asks of the stage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Exit {
    Stay,
    Close,
    /// A downward pull that began at the top of a root's list: the stage takes the drag on, to
    /// close the drawer with it.
    Pull,
    /// VIEW MEMBERS: the group's members, over the faces.
    Members,
}

/// What a drag in progress moves.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Grab {
    None,
    /// The roots, from where they were.
    Slide(i32),
    /// A root's list, from its offset then.
    Scroll(i32),
    Ignore,
}

/// A root's list, scrolled by a pixel offset, and the row it keeps in place as rows change:
/// that row's id and its top below the viewport's.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct Scroll {
    offset: i32,
    anchor: Option<(Id, i32)>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Drawer {
    /// How far the roots have slid left: 0 shows Events, [`WIDTH`] Messages.
    slide: i32,
    settle: Option<Ease>,
    scroll: [Scroll; 2],
    child: Option<Child>,
    grab: Grab,
}

/// What the drawer's screens show.
pub struct Context<'a> {
    pub events: &'a Events,
    pub gnss: &'a Gnss,
    pub mesh: &'a MeshView,
    pub now: Micros,
    pub breath: u8,
    pub font: &'a FontdueRenderer<'static, Color>,
}

/// An event's row in the Events list: its id, its top from the list's, and its height.
type Rows = Vec<(Id, i32, i32), { events::CAPACITY }>;

fn rows(events: &Events) -> Rows {
    let mut top = FIRST;
    events
        .ordered()
        .map(|event| {
            let height = rows::height(event);
            let row = (event.id, top, height);
            top += height;
            row
        })
        .collect()
}

fn content_height(rows: &Rows) -> i32 {
    rows.last().map_or(0, |&(_, top, height)| top + height)
}

impl Default for Drawer {
    fn default() -> Self {
        Self::new()
    }
}

impl Drawer {
    /// Open on Events, at the top of its list.
    #[must_use]
    pub fn new() -> Self {
        Self {
            slide: 0,
            settle: None,
            scroll: [Scroll::default(); 2],
            child: None,
            grab: Grab::None,
        }
    }

    /// Open on an event's detail, as tapping its toast does.
    #[must_use]
    pub fn at_event(id: Id) -> Self {
        Self {
            child: Some(Child::Event(id)),
            ..Self::new()
        }
    }

    /// The root that shows, or that the roots are nearest.
    #[must_use]
    pub fn root(&self) -> Root {
        if self.slide >= WIDTH / 2 {
            Root::Messages
        } else {
            Root::Events
        }
    }

    #[must_use]
    pub fn child(&self) -> Option<Child> {
        self.child
    }

    #[must_use]
    pub fn is_moving(&self) -> bool {
        self.settle.is_some()
    }

    fn root_index(&self) -> usize {
        match self.root() {
            Root::Events => 0,
            Root::Messages => 1,
        }
    }

    /// Acts on a gesture, and returns what the stage should do.
    pub fn handle(&mut self, event: &GestureEvent, events: &mut Events, now: Micros) -> Exit {
        match self.child {
            Some(child) => self.handle_child(child, event, events),
            None => self.handle_root(event, events, now),
        }
    }

    fn handle_root(&mut self, event: &GestureEvent, events: &mut Events, now: Micros) -> Exit {
        let root = self.root_index();
        match *event {
            GestureEvent::Down(_) => {
                if let Some(ease) = self.settle.take() {
                    self.slide = ease.to();
                }
            }
            GestureEvent::DragStart(drag) => {
                let offset = drag.offset();
                self.grab = if offset.x.abs() > offset.y.abs() {
                    Grab::Slide(self.slide)
                } else if offset.y > 0 && self.scroll[root].offset == 0 {
                    self.grab = Grab::Ignore;
                    return Exit::Pull;
                } else if VIEWPORT.contains(drag.start) {
                    Grab::Scroll(self.scroll[root].offset)
                } else {
                    Grab::Ignore
                };
                self.follow(&drag, events);
            }
            GestureEvent::DragMove(drag) => self.follow(&drag, events),
            GestureEvent::DragEnd(drag) => {
                self.follow(&drag, events);
                if let Grab::Slide(from) = self.grab {
                    self.release_slide(from, &drag, now);
                }
                self.grab = Grab::None;
            }
            GestureEvent::Tap(point) => {
                if TOP_HIT.contains(point) {
                    return Exit::Close;
                }
                if self.root() == Root::Events {
                    if FOOTER.contains(point) {
                        self.child = Some(Child::Manage);
                    } else if let Some(id) = self.row_at(point, events) {
                        events.read(id);
                        self.child = Some(Child::Event(id));
                    }
                }
            }
            GestureEvent::None => {}
        }
        Exit::Stay
    }

    fn follow(&mut self, drag: &Drag, events: &Events) {
        let root = self.root_index();
        match self.grab {
            Grab::Slide(from) => self.slide = (from - drag.offset().x).clamp(0, WIDTH),
            Grab::Scroll(from) if root == 0 => {
                let max = (content_height(&rows(events)) - VIEWPORT_HEIGHT).max(0);
                self.scroll[0].offset = (from - drag.offset().y).clamp(0, max);
                self.scroll[0].anchor = None;
            }
            _ => {}
        }
    }

    fn release_slide(&mut self, from: i32, drag: &Drag, now: Micros) {
        let moved = self.slide - from;
        let velocity = drag.velocity.0;
        let target = if from == 0 {
            if moved >= COMMIT || velocity <= -FLICK {
                WIDTH
            } else {
                0
            }
        } else if -moved >= COMMIT || velocity >= FLICK {
            0
        } else {
            WIDTH
        };
        self.settle = (target != self.slide).then(|| Ease::new(self.slide as f32, target, now));
    }

    /// The event whose row `point` falls on, in the Events root.
    fn row_at(&self, point: Point, events: &Events) -> Option<Id> {
        if !VIEWPORT.contains(point) {
            return None;
        }
        let y = point.y - VIEWPORT.top_left.y + self.scroll[0].offset;
        rows(events)
            .iter()
            .find(|&&(_, top, height)| (top..top + height).contains(&y))
            .map(|&(id, ..)| id)
    }

    fn handle_child(&mut self, child: Child, event: &GestureEvent, events: &mut Events) -> Exit {
        let GestureEvent::Tap(point) = *event else {
            return Exit::Stay;
        };
        if TOP_HIT.contains(point) {
            self.child = None;
            return Exit::Stay;
        }
        match child {
            Child::Manage => {
                if MARK_ALL_READ.contains(point) && events.can_mark_all_read() {
                    events.mark_all_read();
                    self.child = None;
                } else if CLEAR_READ.contains(point) && events.can_clear_read() {
                    events.clear_read();
                    self.child = None;
                } else if FOOTER.contains(point) {
                    self.child = None;
                }
            }
            Child::Event(id) => {
                let Some(event) = events.get(id) else {
                    self.child = None;
                    return Exit::Stay;
                };
                let (members, dismiss) = rows::detail_buttons(event);
                if members.is_some_and(|area| area.contains(point)) {
                    return Exit::Members;
                }
                if dismiss.contains(point) && events.dismiss(id).is_ok() {
                    self.child = None;
                }
            }
        }
        Exit::Stay
    }

    /// Settles the roots, follows the events' changes, and says whether anything still moves.
    pub fn step(&mut self, events: &Events, now: Micros) -> bool {
        if let Some(ease) = self.settle {
            let (slide, arrived) = ease.at(now);
            self.slide = libm::roundf(slide) as i32;
            if arrived {
                self.settle = None;
            }
        }
        if let Some(Child::Event(id)) = self.child
            && events.get(id).is_none()
        {
            self.child = None;
        }
        // The row at the top of the list keeps its place as rows above it come and go.
        let rows = rows(events);
        let scroll = &mut self.scroll[0];
        if let Some((id, below)) = scroll.anchor
            && let Some(&(_, top, _)) = rows.iter().find(|&&(row, ..)| row == id)
        {
            scroll.offset = top - below;
        }
        scroll.offset = scroll
            .offset
            .clamp(0, (content_height(&rows) - VIEWPORT_HEIGHT).max(0));
        scroll.anchor = rows
            .iter()
            .find(|&&(_, top, height)| top + height > scroll.offset)
            .map(|&(id, top, _)| (id, top - scroll.offset));
        self.settle.is_some()
    }

    /// Builds what shows into `list`.
    pub fn view(&self, list: &mut List, context: &Context) {
        list.set_backdrop(Backdrop::Breathing(context.breath));
        match self.child {
            Some(Child::Event(id)) => {
                if let Some(event) = context.events.get(id) {
                    rows::detail(list, event, context);
                }
            }
            Some(Child::Manage) => manage(list, context),
            None => {
                if self.slide < WIDTH {
                    let start = list.items().len();
                    self.events_root(list, context);
                    list.shift_from(start, -self.slide);
                }
                if self.slide > 0 {
                    let start = list.items().len();
                    messages_root(list, context);
                    list.shift_from(start, WIDTH - self.slide);
                }
            }
        }
    }

    fn events_root(&self, list: &mut List, context: &Context) {
        let events = context.events;
        parts::root_top(list, 0);
        parts::title(list, "EVENTS", context.font);
        let ongoing = events.ongoing();
        let counts = if ongoing > 0 {
            format(format_args!(
                "{ongoing:02} ONGOING / {:02} UNREAD / {:02} EVENTS",
                events.unread(),
                events.len()
            ))
        } else {
            format(format_args!(
                "{:02} UNREAD / {:02} EVENTS",
                events.unread(),
                events.len()
            ))
        };
        parts::meta(list, &counts);
        if events.is_empty() {
            empty(list, "NO EVENTS", "New events will appear here.");
        } else {
            let rows = rows(events);
            let offset = self.scroll[0].offset;
            list.clip(Some(VIEWPORT));
            for (event, &(_, top, height)) in events.ordered().zip(&rows) {
                let top = VIEWPORT.top_left.y + top - offset;
                if top < VIEWPORT.top_left.y + VIEWPORT_HEIGHT && top + height > VIEWPORT.top_left.y
                {
                    rows::row(list, event, top, height, context);
                }
            }
            list.clip(None);
            parts::scroll_arc(list, content_height(&rows), VIEWPORT_HEIGHT, offset);
        }
        parts::button(list, FOOTER, "OPTIONS", true);
    }
}

/// The arc across the bottom of the faces while anything is unread.
pub const UNREAD_ARC: Arc = Arc {
    center: Point::new(parts::CENTRE, parts::CENTRE),
    radius: 230 * 4,
    width: 12,
    span: (800, 1000),
    color: chrome::WHITE,
};

/// The Messages root, until messages have their screens: no thread to show.
fn messages_root(list: &mut List, context: &Context) {
    parts::root_top(list, 1);
    parts::title(list, "MESSAGES", context.font);
    parts::meta(list, "00 UNREAD / 00 THREADS");
    empty(list, "NO MESSAGES", "Messages will appear here.");
}

/// A root with nothing in its list: a hollow square, what is missing, and what comes next.
fn empty(list: &mut List, title: &str, line: &str) {
    list.outline(rect(219, 178, 247, 206), chrome::GRAY);
    list.fill(rect(230, 189, 236, 195), chrome::GRAY);
    list.text(parts::centred(
        title,
        parts::CENTRE,
        231,
        Face::Kh,
        29,
        chrome::GRAY,
    ));
    list.text(parts::centred(
        line,
        parts::CENTRE,
        280,
        Face::Sans,
        18,
        chrome::GRAY,
    ));
}

/// MARK ALL READ and CLEAR READ HISTORY, each dimmed while it would change nothing.
fn manage(list: &mut List, context: &Context) {
    let events = context.events;
    parts::back(list);
    parts::title(list, "EVENTS", context.font);
    parts::meta(list, "MANAGE HISTORY");
    let choices = [
        (
            MARK_ALL_READ,
            "MARK ALL READ",
            "Keep all events in the drawer.",
            events.can_mark_all_read(),
        ),
        (
            CLEAR_READ,
            "CLEAR READ HISTORY",
            "Keep active faults and unread events.",
            events.can_clear_read(),
        ),
    ];
    for (area, title, line, enabled) in choices {
        let (outline, ink, note) = if enabled {
            (chrome::GRAY, chrome::WHITE, chrome::GRAY)
        } else {
            (chrome::DISABLED, chrome::DISABLED, chrome::DISABLED)
        };
        list.outline(area, outline);
        let top = area.top_left.y;
        list.text(parts::text(title, 105, top + 21, Face::Kh, 21, ink));
        list.text(parts::text(line, 105, top + 62, Face::Sans, 15, note));
    }
    parts::button(list, FOOTER, "BACK TO EVENTS", true);
}
