//! The drawer an upward drag on a face opens, from the 2026-10-04 hand-off: Events and Messages,
//! two roots side by side, and the screens each opens. Each root keeps its place in its list,
//! and the drawer returns to the face it opened over when it closes. It builds a [`List`] from
//! its state and the events each step; the stage draws it.

pub mod messages;
pub mod parts;
pub mod removals;
mod rows;

pub use messages::{Coverage, Mail};
pub use rows::{TOAST, TOAST_COMPACT, age, age_due, toast};

use embedded_graphics::{prelude::Point, primitives::Rectangle};
use heapless::Vec;

use self::{
    parts::{FOOTER, PAIR, TOP_HIT},
    removals::Act,
};
use super::{
    ease::Ease,
    events::{self, Events, Id, Kind},
    gesture::{Drag, GestureEvent, Micros},
    group::{
        keyboard::{Keyboard, Outcome},
        layout::{Arc, Backdrop, Face, List, format, rect},
        view::{MeshView, MessagesView, RemovalStage, RemovalView, Text, Thread},
    },
    screens::Gnss,
    slide::Slide,
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
/// How long an unread message's row must show whole for it to count as read.
const READ_AFTER: Micros = 1_000_000;

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
    /// One conversation.
    Thread(Thread),
    /// SEND TO: the group or one member.
    Recipients,
    /// The draft, on the keyboard.
    Draft,
    /// The draft, read through before it is sent.
    Review,
    /// A removal's request in full, by its event.
    Details(Id),
    /// Declining a removal.
    Decline(Declining),
    /// Two removals that compete, by their events, the winner first, and the one selected.
    Rivals { ids: [Id; 2], selected: usize },
}

/// Declining a removal, by its event: the slide, and whether the removal had switched when the
/// confirmation was last shown, which sets what it says declining costs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Declining {
    pub id: Id,
    slide: Slide,
    switched: bool,
}

impl Declining {
    fn new(id: Id, removal: &RemovalView) -> Self {
        Self {
            id,
            slide: Slide::default(),
            switched: switched(removal),
        }
    }

    /// Follows `removal` as it is now. One that switched under the confirmation costs more to
    /// decline, so the slide starts again with that said.
    fn follow(&mut self, removal: &RemovalView) {
        if self.switched != switched(removal) {
            *self = Self::new(self.id, removal);
        }
    }
}

/// A message being written, and to whom. It outlives the drawer, until it is sent.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Draft {
    pub to: Thread,
    pub keyboard: Keyboard,
}

/// What a gesture asks of the stage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Exit {
    Stay,
    Close,
    /// A downward pull that began at the top of a root's list: the stage takes the drag on, to
    /// close the drawer with it.
    Pull,
    /// VIEW GROUP: the group's members, over the faces.
    Members,
    /// SEND: the draft goes to the mesh, once.
    Send {
        to: Thread,
        text: Text,
    },
    /// A decline confirmed: of the removal with this new key.
    Keep {
        key: [u8; 8],
    },
    /// LEAVE GROUP: the group's own confirmation of leaving.
    Leave,
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
    /// A child's list, scrolled, and in a conversation the message it keeps in place as
    /// messages come: its id and its top below the viewport's.
    child_scroll: i32,
    thread_anchor: Option<(u32, i32)>,
    grab: Grab,
    draft: Option<Draft>,
    /// What REVIEW took from the draft, to send.
    reviewing: Option<Text>,
    /// The unread rows, and lines of rows too tall to show whole, that show whole, and since
    /// when.
    reading: Vec<(messages::Showing, Micros), { messages::SHOWING }>,
}

/// What the drawer's screens show.
pub struct Context<'a> {
    pub events: &'a Events,
    pub gnss: &'a Gnss,
    pub mesh: &'a MeshView,
    pub messages: Option<&'a MessagesView>,
    pub now: Micros,
    pub breath: u8,
    pub font: &'a FontdueRenderer<'static, Color>,
}

impl Context<'_> {
    /// This device's id in its group.
    #[must_use]
    pub fn own(&self) -> u8 {
        self.mesh.group.as_ref().map_or(0, |group| group.own)
    }

    #[must_use]
    pub fn show(&self) -> removals::Show<'_> {
        removals::Show {
            own: self.own(),
            now: self.now,
            font: self.font,
        }
    }

    #[must_use]
    pub fn mail(&self) -> Mail<'_> {
        Mail {
            messages: self.messages,
            group: self.mesh.group.as_ref(),
            font: self.font,
        }
    }
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
            child_scroll: 0,
            thread_anchor: None,
            grab: Grab::None,
            draft: None,
            reviewing: None,
            reading: Vec::new(),
        }
    }

    /// Open on what an event shows, as tapping its toast does: its detail, for messages their
    /// conversation, on the Messages root, and for one of two competing removals both.
    #[must_use]
    pub fn at_event(id: Id, events: &Events) -> Self {
        match events.get(id).map(|event| event.kind) {
            Some(Kind::Messages { thread, .. }) => Self::at_thread(thread),
            _ => Self {
                child: Some(match events.rivals_of(id) {
                    Some((ids, selected)) => Child::Rivals { ids, selected },
                    None => Child::Event(id),
                }),
                ..Self::new()
            },
        }
    }

    /// Open on a conversation, over the Messages root.
    #[must_use]
    fn at_thread(thread: Thread) -> Self {
        Self {
            slide: WIDTH,
            child: Some(Child::Thread(thread)),
            ..Self::new()
        }
    }

    /// Takes up a draft kept from before the drawer last closed.
    pub fn keep_draft(&mut self, draft: Option<Draft>) {
        self.draft = draft;
    }

    /// The draft, to keep while the drawer is closed.
    pub fn take_draft(&mut self) -> Option<Draft> {
        self.draft.take()
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
    pub fn handle(
        &mut self,
        event: &GestureEvent,
        events: &mut Events,
        mail: &Mail,
        now: Micros,
    ) -> Exit {
        match self.child {
            Some(Child::Decline(declining)) => {
                self.handle_decline(declining, event, events, mail, now)
            }
            Some(child) => self.handle_child(child, event, events, mail, now),
            None => self.handle_root(event, events, mail, now),
        }
    }

    fn handle_root(
        &mut self,
        event: &GestureEvent,
        events: &mut Events,
        mail: &Mail,
        now: Micros,
    ) -> Exit {
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
                self.follow(&drag, events, mail);
            }
            GestureEvent::DragMove(drag) => self.follow(&drag, events, mail),
            GestureEvent::DragEnd(drag) => {
                self.follow(&drag, events, mail);
                if let Grab::Slide(from) = self.grab {
                    self.release_slide(from, &drag, now);
                }
                self.grab = Grab::None;
            }
            GestureEvent::Tap(point) => {
                if TOP_HIT.contains(point) {
                    return Exit::Close;
                }
                match self.root() {
                    Root::Events => {
                        if parts::pressed(FOOTER, point) {
                            self.child = Some(Child::Manage);
                        } else if let Some(id) = self.row_at(point, events) {
                            events.read(id);
                            self.open(&Self::at_event(id, events));
                        }
                    }
                    Root::Messages => {
                        if parts::pressed(FOOTER, point) && mail.group.is_some() {
                            self.open_child(Child::Recipients);
                        } else if let Some(thread) =
                            messages::inbox_row_at(point, self.scroll[1].offset, mail)
                        {
                            self.open_child(Child::Thread(thread));
                        }
                    }
                }
            }
            GestureEvent::None => {}
        }
        Exit::Stay
    }

    fn follow(&mut self, drag: &Drag, events: &Events, mail: &Mail) {
        let root = self.root_index();
        match self.grab {
            Grab::Slide(from) => self.slide = (from - drag.offset().x).clamp(0, WIDTH),
            Grab::Scroll(from) => {
                let content = if root == 0 {
                    content_height(&rows(events))
                } else {
                    messages::inbox_height(&messages::conversations(mail))
                };
                let max = (content - VIEWPORT_HEIGHT).max(0);
                self.scroll[root].offset = (from - drag.offset().y).clamp(0, max);
                self.scroll[root].anchor = None;
            }
            _ => {}
        }
    }

    /// Shows `child`, at the top of its list.
    fn open_child(&mut self, child: Child) {
        self.child = Some(child);
        self.child_scroll = 0;
        self.thread_anchor = None;
        self.reading.clear();
    }

    /// Shows what `drawer`, a drawer opened at an event, shows.
    fn open(&mut self, drawer: &Self) {
        self.slide = drawer.slide;
        if let Some(child) = drawer.child {
            self.open_child(child);
        }
    }

    /// The draft to `to`: the one kept, if it goes there, or a new one.
    fn write(&mut self, to: Thread) {
        if self.draft.as_ref().is_none_or(|draft| draft.to != to) {
            self.draft = Some(Draft {
                to,
                keyboard: Keyboard::message(),
            });
        }
        self.open_child(Child::Draft);
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

    fn handle_child(
        &mut self,
        child: Child,
        event: &GestureEvent,
        events: &mut Events,
        mail: &Mail,
        now: Micros,
    ) -> Exit {
        if child == Child::Draft {
            return self.handle_draft(event, mail);
        }
        let content = match child {
            Child::Thread(thread) => Some((
                messages::thread_height(thread, mail),
                messages::viewport(thread, mail).size.height as i32,
            )),
            Child::Recipients => Some((messages::picker_height(mail), VIEWPORT_HEIGHT)),
            Child::Review => self
                .reviewing
                .map(|text| (messages::review_scroll(text.as_str(), mail), 0)),
            _ => None,
        };
        if let Some((content, height)) = content {
            let max = (content - height).max(0);
            match *event {
                GestureEvent::DragStart(drag) => {
                    self.grab = if drag.offset().y.abs() >= drag.offset().x.abs() {
                        Grab::Scroll(self.child_scroll)
                    } else {
                        Grab::Ignore
                    };
                }
                GestureEvent::DragMove(drag) | GestureEvent::DragEnd(drag) => {
                    if let Grab::Scroll(from) = self.grab {
                        self.child_scroll = (from - drag.offset().y).clamp(0, max);
                        self.thread_anchor = None;
                    }
                    if matches!(event, GestureEvent::DragEnd(_)) {
                        self.grab = Grab::None;
                    }
                }
                _ => {}
            }
        }
        let GestureEvent::Tap(point) = *event else {
            return Exit::Stay;
        };
        if TOP_HIT.contains(point) {
            match child {
                Child::Review => self.open_child(Child::Draft),
                Child::Details(id) => self.open_child(Child::Event(id)),
                _ => self.child = None,
            }
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
                } else if parts::pressed(FOOTER, point) {
                    self.child = None;
                }
            }
            Child::Event(id) => {
                let Some(event) = events.get(id) else {
                    self.child = None;
                    return Exit::Stay;
                };
                match event.kind {
                    Kind::Removal(removal) => {
                        let own = mail.group.map_or(0, |group| group.own);
                        let act = removals::buttons(&removal, own, now)
                            .into_iter()
                            .find(|(area, _)| parts::pressed(*area, point));
                        match act.map(|(_, act)| act) {
                            Some(Act::Details) => self.open_child(Child::Details(id)),
                            Some(Act::Decline) => {
                                self.open_child(Child::Decline(Declining::new(id, &removal)));
                            }
                            Some(Act::ViewGroup) => return Exit::Members,
                            None => {}
                        }
                    }
                    Kind::Removed { .. } => {
                        if parts::pressed(PAIR[0], point) {
                            self.child = None;
                        } else if parts::pressed(PAIR[1], point) {
                            return Exit::Leave;
                        }
                    }
                    _ => {
                        if rows::dismiss_button(event)
                            .is_some_and(|area| parts::pressed(area, point))
                            && events.dismiss(id).is_ok()
                        {
                            self.child = None;
                        }
                    }
                }
            }
            Child::Rivals { ids, selected } => {
                if parts::pressed(FOOTER, point) {
                    self.open_child(Child::Event(ids[selected]));
                } else if let Some(row) = removals::rival_at(point.y) {
                    self.child = Some(Child::Rivals { ids, selected: row });
                }
            }
            Child::Details(_) | Child::Decline(_) => {}
            Child::Thread(thread) => {
                if parts::pressed(FOOTER, point) && mail.writable(thread) {
                    self.write(thread);
                }
            }
            Child::Recipients => {
                if parts::pressed(FOOTER, point) {
                    self.child = None;
                } else if let Some(thread) = messages::picker_row_at(point, self.child_scroll, mail)
                {
                    self.write(thread);
                }
            }
            Child::Review => {
                if parts::pressed(PAIR[0], point) {
                    self.open_child(Child::Draft);
                } else if parts::pressed(PAIR[1], point)
                    && let (Some(text), Some(draft)) = (self.reviewing, &self.draft)
                    && mail.writable(draft.to)
                {
                    let to = draft.to;
                    // Sent once: the draft and the review go with it.
                    self.draft = None;
                    self.reviewing = None;
                    self.open_child(Child::Thread(to));
                    return Exit::Send { to, text };
                }
            }
            Child::Draft => {}
        }
        Exit::Stay
    }

    /// The decline's slide, revalidated against the removal as it is now, so that a slide begun
    /// before the switch cannot finish one after it.
    fn handle_decline(
        &mut self,
        mut declining: Declining,
        event: &GestureEvent,
        events: &Events,
        mail: &Mail,
        now: Micros,
    ) -> Exit {
        let id = declining.id;
        let own = mail.group.map_or(0, |group| group.own);
        let Some(removal) = self::declining(events, id, own, now) else {
            self.open_child(Child::Event(id));
            return Exit::Stay;
        };
        declining.follow(&removal);
        let confirmed = declining.slide.handle(event, now, &removals::SLIDE);
        self.child = Some(Child::Decline(declining));
        if confirmed {
            self.open_child(Child::Event(id));
            return Exit::Keep { key: removal.key };
        }
        if let GestureEvent::Tap(point) = *event
            && (TOP_HIT.contains(point) || parts::pressed(FOOTER, point))
        {
            self.open_child(Child::Event(id));
        }
        Exit::Stay
    }

    fn handle_draft(&mut self, event: &GestureEvent, mail: &Mail) -> Exit {
        let Some(draft) = &mut self.draft else {
            self.child = None;
            return Exit::Stay;
        };
        match draft.keyboard.handle(event, mail.font) {
            Outcome::Cancel => {
                let to = draft.to;
                let messages = mail.messages.is_some_and(|messages| {
                    messages
                        .thread(to, mail.group.map_or(0, |group| group.own))
                        .next()
                        .is_some()
                });
                if messages {
                    self.open_child(Child::Thread(to));
                } else {
                    self.child = None;
                }
            }
            Outcome::Review(text) => {
                self.reviewing = Some(text);
                self.open_child(Child::Review);
            }
            Outcome::Stay | Outcome::Save(_) | Outcome::Unchanged => {}
        }
        Exit::Stay
    }

    /// Settles the roots, follows the events' and messages' changes, and says whether anything
    /// still moves.
    pub fn step(&mut self, events: &Events, mail: &Mail, now: Micros) -> bool {
        // A conversation keeps the message at the top of its list in place as newer ones come.
        if let Some(Child::Thread(thread)) = self.child {
            if let Some((id, below)) = self.thread_anchor
                && let Some(top) = messages::message_top(thread, id, mail)
            {
                self.child_scroll = top - below;
            }
            let height = messages::viewport(thread, mail).size.height as i32;
            let max = (messages::thread_height(thread, mail) - height).max(0);
            self.child_scroll = self.child_scroll.clamp(0, max);
            self.thread_anchor = (self.child_scroll > 0)
                .then(|| messages::thread_anchor(thread, self.child_scroll, mail))
                .flatten();
        }
        let inbox = messages::inbox_height(&messages::conversations(mail));
        self.scroll[1].offset = self.scroll[1]
            .offset
            .clamp(0, (inbox - VIEWPORT_HEIGHT).max(0));
        if let Some(ease) = self.settle {
            let (slide, arrived) = ease.at(now);
            self.slide = libm::roundf(slide) as i32;
            if arrived {
                self.settle = None;
            }
        }
        let own = mail.group.map_or(0, |group| group.own);
        let mut slides = false;
        match self.child {
            Some(Child::Event(id) | Child::Details(id)) if events.get(id).is_none() => {
                self.child = None;
            }
            Some(Child::Rivals { ids, .. }) if ids.iter().any(|&id| events.get(id).is_none()) => {
                self.child = None;
            }
            Some(Child::Decline(mut declining)) => {
                match self::declining(events, declining.id, own, now) {
                    Some(removal) => {
                        declining.follow(&removal);
                        slides = declining.slide.step(now);
                        self.child = Some(Child::Decline(declining));
                    }
                    None => self.open_child(Child::Event(declining.id)),
                }
            }
            _ => {}
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
        self.settle.is_some() || slides
    }

    /// The unread message of the conversation showing that has now been read, if one has: its
    /// whole row shown for a second, or for a row too tall to show whole, every line of its body
    /// shown whole for a second, at any time this session, which `coverage` keeps. Only while
    /// the conversation shows in the `foreground`, with nothing over it and the screen awake; a
    /// row or line that stops showing starts its second again.
    pub fn read(
        &mut self,
        mail: &Mail,
        coverage: &mut Coverage,
        foreground: bool,
        now: Micros,
    ) -> Option<u32> {
        let thread = match self.child {
            Some(Child::Thread(thread)) if foreground => thread,
            _ => {
                self.reading.clear();
                return None;
            }
        };
        let shown = messages::showing(thread, self.child_scroll, mail);
        self.reading.retain(|(held, _)| shown.contains(held));
        for showing in shown {
            let covered = match showing {
                messages::Showing::Line { id, bytes } => coverage.covers(id, bytes),
                messages::Showing::Row(_) => false,
            };
            if !covered && !self.reading.iter().any(|&(held, _)| held == showing) {
                // Full, it is not timed, and stays unread.
                _ = self.reading.push((showing, now));
            }
        }
        let mut i = 0;
        while let Some(&(showing, since)) = self.reading.get(i) {
            if now.saturating_sub(since) < READ_AFTER {
                i += 1;
                continue;
            }
            self.reading.remove(i);
            match showing {
                messages::Showing::Row(id) => return Some(id),
                messages::Showing::Line { id, bytes } => {
                    let whole = coverage.cover(id, bytes)
                        && mail
                            .messages
                            .and_then(|messages| messages.get(id))
                            .is_some_and(|message| coverage.whole(id, message.text()));
                    if whole {
                        return Some(id);
                    }
                }
            }
        }
        None
    }

    /// Starts every unread row's and line's second again, as anything over the drawer or the
    /// screen resting does.
    pub fn pause_reading(&mut self) {
        self.reading.clear();
    }

    /// When an unread message showing will count as read, if one is showing.
    #[must_use]
    pub fn read_due(&self) -> Option<Micros> {
        self.reading
            .iter()
            .map(|&(_, since)| since + READ_AFTER)
            .min()
    }

    /// Builds what shows into `list`.
    pub fn view(&self, list: &mut List, context: &Context) {
        list.set_backdrop(Backdrop::Breathing(context.breath));
        let mail = context.mail();
        match self.child {
            Some(Child::Event(id)) => {
                if let Some(event) = context.events.get(id) {
                    rows::detail(list, event, context);
                }
            }
            Some(Child::Manage) => manage(list, context),
            Some(Child::Details(id)) => {
                if let Some(removal) = removal(context.events, id) {
                    removals::details(list, &removal, &context.show());
                }
            }
            Some(Child::Decline(declining)) => {
                if let Some(removal) = removal(context.events, declining.id) {
                    removals::confirm(list, &removal, &declining.slide, &context.show());
                }
            }
            Some(Child::Rivals { ids, selected }) => {
                if let [Some(winner), Some(rival)] = ids.map(|id| removal(context.events, id)) {
                    removals::rivals(list, [&winner, &rival], selected, &context.show());
                }
            }
            Some(Child::Thread(thread)) => {
                messages::conversation(list, thread, self.child_scroll, &mail, context.now);
            }
            Some(Child::Recipients) => messages::picker(list, self.child_scroll, &mail),
            Some(Child::Draft) => {
                if let Some(draft) = &self.draft {
                    messages::draft_header(list, draft.to, &mail);
                    draft.keyboard.draw(context.font, list);
                }
            }
            Some(Child::Review) => {
                if let (Some(text), Some(draft)) = (self.reviewing, &self.draft) {
                    messages::review(list, draft.to, text.as_str(), self.child_scroll, &mail);
                }
            }
            None => {
                if self.slide < WIDTH {
                    let start = list.items().len();
                    self.events_root(list, context);
                    list.shift_from(start, -self.slide);
                }
                if self.slide > 0 {
                    let start = list.items().len();
                    messages::inbox(list, self.scroll[1].offset, &mail, context.now);
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

/// The removal event `id` is of.
fn removal(events: &Events, id: Id) -> Option<RemovalView> {
    match events.get(id)?.kind {
        Kind::Removal(removal) => Some(removal),
        _ => None,
    }
}

/// The removal of event `id`, while member `own`'s device can decline it.
fn declining(events: &Events, id: Id, own: u8, now: Micros) -> Option<RemovalView> {
    removal(events, id).filter(|removal| removals::declinable(removal, own, now))
}

fn switched(removal: &RemovalView) -> bool {
    matches!(removal.stage, RemovalStage::Switched { .. })
}

/// The arc across the bottom of the faces while anything is unread.
pub const UNREAD_ARC: Arc = Arc {
    center: Point::new(parts::CENTRE, parts::CENTRE),
    radius: 230 * 4,
    width: 12,
    span: (800, 1000),
    color: chrome::WHITE,
};

/// A list with nothing in it: a hollow square, what is missing, and what comes next.
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
            "Events only. Messages stay unread.",
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
