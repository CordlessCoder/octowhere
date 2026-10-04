//! The group screens, from the 2026-10-02 pairing hand-off: the group and its members, this
//! device's name, leaving, and pairing to add a member or join a group. The 2026-10-03 hand-off
//! adds refreshing devices and the wait of a device that founded a group. Each screen builds a
//! [`List`] from its state and the [`MeshView`] the firmware publishes, and asks the mesh for
//! what it needs through a [`Request`]. Nothing here starts radio traffic without a START, and
//! nothing reports an outcome the mesh has not.

pub mod keyboard;
pub mod layout;
pub mod sim;
pub use octowhere_node::view;
pub mod words;

use embedded_graphics::{prelude::Point, primitives::Rectangle};

use self::{
    keyboard::{Keyboard, Outcome},
    layout::{Face, List, Text, Vertical, format, rect},
    view::{
        Answer, At, Done, End, MemberView, MeshView, PairingView, Phase, Position, Reason,
        RecoveryPhase, RecoveryView, RefreshPhase, RefreshView, Refused, Request, Role,
    },
};
use super::{
    ease::{Ease, SETTLE},
    gesture::{GestureEvent, Micros},
    icon::Glyph,
};
use crate::chrome::{self, Color, FontdueRenderer};

pub const GROUP: Glyph = [0b01110, 0b10101, 0b11111, 0b10101, 0b01110];
pub const NAME: Glyph = [0b01110, 0b10001, 0b11111, 0b10001, 0b10001];
const ADD: Glyph = [0b00100, 0b00100, 0b11111, 0b00100, 0b00100];
const JOIN: Glyph = [0b10000, 0b01000, 0b11111, 0b01000, 0b10000];
const CODE: Glyph = [0b11011, 0b10001, 0b00100, 0b10001, 0b11011];
const WAIT: Glyph = [0b11111, 0b00100, 0b00100, 0b00100, 0b11111];
const TRANSFER: Glyph = [0b00100, 0b01110, 0b10101, 0b00100, 0b00100];
const DONE: Glyph = [0b00001, 0b00010, 0b10100, 0b01000, 0b00000];
const FAULT: Glyph = [0b10001, 0b01010, 0b00100, 0b01010, 0b10001];

const CENTRE: i32 = 233;
/// The top button's outline, and the larger region a tap presses it in.
const NAV: Rectangle = rect(82, 96, 182, 134);
const NAV_HIT: Rectangle = rect(82, 86, 188, 192);
/// The single action at the foot of a screen, and the two side by side.
const ACTION: Rectangle = rect(94, 300, 372, 406);
const ACTIONS: [Rectangle; 2] = [rect(88, 300, 224, 406), rect(242, 300, 378, 406)];
/// The group's own two, whose outer edges meet its card's.
const HUB_ACTIONS: [Rectangle; 2] = [rect(94, 300, 224, 406), rect(242, 300, 372, 406)];
/// This device's card on the group screen, whose whole outline is its tap region.
const OWN_CARD: Rectangle = rect(94, 198, 372, 282);
/// Where the list of devices found scrolls, two rows at a time.
const ROWS: Rectangle = rect(94, 194, 372, 406);
/// On MEMBERS: the refresh strip, BACK's tap region, which stops short of it, and the list
/// under it, which shows a row and part of the next.
const STRIP: Rectangle = rect(94, 148, 372, 208);
const MEMBERS_NAV_HIT: Rectangle = rect(82, 86, 188, 141);
const MEMBER_ROWS: Rectangle = rect(94, 244, 372, 406);
const ROW: i32 = 106;
/// The refresh's time left, and the founding wait's PAIR under the device it waits for.
const LEFT_SLAB: Rectangle = rect(94, 244, 372, 316);
const PENDING_ACTION: Rectangle = rect(94, 348, 372, 406);
const CODE_SLAB: Rectangle = rect(62, 214, 404, 292);
const COUNT_SLAB: Rectangle = rect(94, 259, 372, 335);
const PROGRESS_SLAB: Rectangle = rect(94, 282, 372, 360);
const BAR: Rectangle = rect(106, 345, 360, 353);
const COVER: &str = "COVER RETURNS TO CLOCK";
/// A packet this recent from a member makes it a direct neighbour: seven rounds.
const NEIGHBOUR: i64 = octowhere_mesh::table::NEIGHBOUR_ROUNDS * octowhere_mesh::schedule::ROUND_US;
/// How long a refresh listens: three rounds.
const REFRESH_US: i64 = octowhere_mesh::clock::SWEEP_US;

/// What the group screens keep between visits: the last ended refresh and founding wait the
/// user has been shown, by session.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Memory {
    refresh_seen: u32,
    recovery_seen: u32,
}

/// Why REFRESH DEVICES cannot run.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Unavailable {
    NoRadio,
    NoGroup,
    /// A pairing has the radio.
    Pairing,
}

/// Where a screen that shows this device's membership goes back to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum From {
    Hub,
    Members,
}

/// A list dragged up and down, which settles on a whole row, or at its end, where the last row
/// shows whole.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct Scroll {
    offset: i32,
    /// The offset when the drag started.
    grabbed: Option<i32>,
    /// Settling from one offset to another, since a time.
    settle: Option<(i32, i32, Micros)>,
    /// The row a finger is down on.
    pressed: Option<usize>,
}

impl Scroll {
    /// The farthest the list scrolls, which brings its last row to the bottom of `area`.
    fn max(rows: usize, area: Rectangle) -> i32 {
        (rows as i32 * ROW - area.size.height as i32).max(0)
    }

    fn row_at(&self, point: Point, rows: usize, area: Rectangle) -> Option<usize> {
        if !area.contains(point) {
            return None;
        }
        let row = ((point.y - area.top_left.y + self.offset) / ROW) as usize;
        (row < rows).then_some(row)
    }

    fn top(&self, row: usize, area: Rectangle) -> i32 {
        area.top_left.y + ROW * row as i32 - self.offset
    }

    /// Follows a drag that starts on the list in `area`, and returns the row a tap picked.
    fn handle(
        &mut self,
        event: &GestureEvent,
        rows: usize,
        now: Micros,
        area: Rectangle,
    ) -> Option<usize> {
        let max = Self::max(rows, area);
        match *event {
            GestureEvent::Down(point) => {
                self.finish();
                self.pressed = self.row_at(point, rows, area);
            }
            GestureEvent::DragStart(drag) => {
                self.pressed = None;
                let offset = drag.offset();
                if area.contains(drag.start) && offset.y.abs() > offset.x.abs() {
                    self.grabbed = Some(self.offset);
                    self.offset = (self.offset - offset.y).clamp(0, max);
                }
            }
            GestureEvent::DragMove(drag) => {
                if let Some(from) = self.grabbed {
                    self.offset = (from - drag.offset().y).clamp(0, max);
                }
            }
            GestureEvent::DragEnd(_) => {
                if self.grabbed.take().is_some() {
                    let row = ((self.offset + ROW / 2) / ROW * ROW).min(max);
                    let to = if max - self.offset < (self.offset - row).abs() {
                        max
                    } else {
                        row
                    };
                    self.settle = (to != self.offset).then_some((self.offset, to, now));
                }
            }
            GestureEvent::Tap(point) => {
                self.pressed = None;
                return self.row_at(point, rows, area);
            }
            GestureEvent::None => {}
        }
        None
    }

    fn finish(&mut self) {
        if let Some((_, to, _)) = self.settle.take() {
            self.offset = to;
        }
    }

    /// Settles, and says whether it still moves.
    fn step(&mut self, rows: usize, now: Micros, area: Rectangle) -> bool {
        self.offset = self.offset.min(Self::max(rows, area));
        let Some((from, to, start)) = self.settle else {
            return false;
        };
        let (offset, arrived) = Ease::new(from as f32, to, start).at(now);
        self.offset = libm::roundf(offset) as i32;
        if arrived {
            self.settle = None;
        }
        !arrived
    }

    /// The first and last rows in view in `area`, counted from 1.
    fn shown(&self, rows: usize, area: Rectangle) -> (usize, usize) {
        let first = (self.offset / ROW) as usize;
        let visible = (area.size.height as usize).div_ceil(ROW as usize);
        (first + 1, (first + visible).min(rows))
    }
}

/// The handle a deliberate drag carries to the right to confirm: a code that matches, or
/// leaving the group.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct Slider {
    /// How far right the handle is, while a drag that started on it holds it.
    held: Option<i32>,
    /// Returning to the start from a release short of the end, since a time.
    back: Option<(i32, Micros)>,
    done: bool,
}

/// The handle at rest, and where a drag must start to take it.
const HANDLE: Rectangle = rect(98, 304, 200, 402);
const GRAB: Rectangle = rect(94, 300, 200, 406);
const TRAVEL: i32 = 168;
/// 90 % of the travel, rounded up.
const COMMIT: i32 = 152;

impl Slider {
    /// Follows a horizontal drag that starts on the handle, and returns whether it let go at
    /// the end. A tap, a flick that ends short and a vertical drag confirm nothing.
    fn handle(&mut self, event: &GestureEvent, now: Micros) -> bool {
        if self.done {
            return false;
        }
        match *event {
            GestureEvent::Down(_) => self.back = None,
            GestureEvent::DragStart(drag) => {
                let offset = drag.offset();
                if GRAB.contains(drag.start) && offset.x.abs() >= offset.y.abs() {
                    self.held = Some(offset.x.clamp(0, TRAVEL));
                }
            }
            GestureEvent::DragMove(drag) => {
                if self.held.is_some() {
                    self.held = Some(drag.offset().x.clamp(0, TRAVEL));
                }
            }
            GestureEvent::DragEnd(drag) => {
                if self.held.take().is_some() {
                    let travel = drag.offset().x.clamp(0, TRAVEL);
                    if travel >= COMMIT {
                        self.done = true;
                        return true;
                    }
                    self.back = (travel > 0).then_some((travel, now));
                }
            }
            GestureEvent::Tap(_) | GestureEvent::None => {}
        }
        false
    }

    fn travel(&self, now: Micros) -> i32 {
        if self.done {
            return TRAVEL;
        }
        if let Some(held) = self.held {
            return held;
        }
        match self.back {
            Some((from, start)) => libm::roundf(Ease::new(from as f32, 0, start).at(now).0) as i32,
            None => 0,
        }
    }

    fn step(&mut self, now: Micros) -> bool {
        if self
            .back
            .is_some_and(|(_, start)| now.saturating_sub(start) >= SETTLE)
        {
            self.back = None;
        }
        self.back.is_some()
    }

    fn draw(&self, prompt: &str, label: &str, now: Micros, list: &mut List) {
        let travel = self.travel(now);
        list.outline(ACTION, chrome::ORANGE);
        if travel * 10 < TRAVEL * 4 {
            list.centred(prompt, 279, 327, Face::Mono, 13, chrome::ORANGE);
            list.centred(label, 279, 351, Face::Mono, 13, chrome::ORANGE);
        }
        let handle = Rectangle::new(HANDLE.top_left + Point::new(travel, 0), HANDLE.size);
        list.fill(handle, chrome::ORANGE);
        list.text(
            Text::new(">", Face::Kh, 44, chrome::BLACK)
                .at(handle.top_left.x + 51, 353)
                .vertical(Vertical::Middle)
                .on(chrome::ORANGE),
        );
    }
}

/// A pairing, from the START that asked for it to the outcome the mesh reports.
#[derive(Clone, Debug, Eq, PartialEq)]
struct Session {
    role: Role,
    /// [`MeshView::sessions`] when it was asked for: the mesh's next session is this one.
    after: u32,
    /// Adding with no group, which founds one.
    founding: bool,
    /// The STOP sheet shows over the code.
    stop: bool,
    slider: Slider,
    scroll: Scroll,
    /// What has been asked of the mesh, shown while it catches up.
    chose: Option<usize>,
    cancelled: bool,
    /// The last parts count seen, which the screens after the transfer keep showing.
    parts: Option<(u8, u8)>,
}

impl Session {
    fn new(role: Role, mesh: &MeshView) -> Self {
        Self {
            role,
            after: mesh.sessions,
            founding: mesh.group.is_none(),
            stop: false,
            slider: Slider::default(),
            scroll: Scroll::default(),
            chose: None,
            cancelled: false,
            parts: None,
        }
    }

    fn view<'m>(&self, mesh: &'m MeshView) -> Option<&'m PairingView> {
        mesh.session_after(self.after)
    }

    /// Whether the pairing has not yet ended, including one the mesh has not taken up yet.
    fn active(&self, mesh: &MeshView) -> bool {
        self.view(mesh)
            .is_none_or(|pairing| !pairing.phase.is_final() && pairing.refused.is_none())
    }

    fn caption(&self) -> &'static str {
        caption(self.role)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Screen {
    /// No group, or the group and this device's card.
    Hub,
    PairOptions,
    Members(Scroll),
    /// A member's details; this device's own offer NAME and LEAVE.
    Member {
        id: u8,
        from: From,
    },
    RemoveUnavailable {
        id: u8,
    },
    Leave {
        from: From,
    },
    /// The second stage of leaving, and the answer count when the leave went out.
    LeaveSlide {
        slider: Slider,
        asked: Option<u32>,
        from: From,
    },
    LeaveDone,
    LeaveFailed,
    /// The keyboard; `back` is where it returns, the panel when `None`.
    Name {
        keyboard: Keyboard,
        asked: Option<u32>,
        back: Option<From>,
    },
    NameFailed {
        keyboard: Keyboard,
        back: Option<From>,
    },
    Entry(Role),
    JoinWarning,
    JoinSlide {
        slider: Slider,
        asked: Option<u32>,
    },
    Full,
    NoRadio(Role),
    Pairing(Session),
    RefreshEntry,
    /// The refresh with this session, running or ended. One asked for and not yet taken up
    /// shows as starting.
    Refresh(u32),
    RefreshUnavailable(Unavailable),
    /// Before a new pairing ends a founding's wait.
    EndWait,
}

/// Where the group screens go after a step.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Exit {
    Stay,
    /// Back to the settings panel.
    Panel,
}

/// The group screens, opened from the panel.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Flow {
    screen: Screen,
}

impl Flow {
    /// The group screen, as the panel's GROUP opens it.
    #[must_use]
    pub fn group() -> Self {
        Self {
            screen: Screen::Hub,
        }
    }

    /// Whether the name's keyboard shows, which a toast keeps clear of.
    #[must_use]
    pub fn typing(&self) -> bool {
        matches!(self.screen, Screen::Name { .. })
    }

    /// The group's members, as an event's VIEW MEMBERS opens them.
    #[must_use]
    pub fn members() -> Self {
        Self {
            screen: Screen::Members(Scroll::default()),
        }
    }

    /// The keyboard for this device's name, as the panel's NAME opens it.
    #[must_use]
    pub fn name(mesh: &MeshView) -> Self {
        Self {
            screen: Screen::Name {
                keyboard: Keyboard::new(mesh.name),
                asked: None,
                back: None,
            },
        }
    }

    /// Whether a pairing under way should hold the screen awake.
    #[must_use]
    pub fn holds_awake(&self, mesh: &MeshView) -> bool {
        matches!(&self.screen, Screen::Pairing(session) if session.active(mesh))
    }

    /// Ends a pairing under way, as a cover or the power key does. Anything else unfinished is
    /// simply dropped with the screen.
    pub fn interrupt(&mut self, mesh: &MeshView, request: &mut Option<Request>) {
        if let Screen::Pairing(session) = &mut self.screen
            && session.active(mesh)
            && !session.cancelled
        {
            session.cancelled = true;
            session.stop = false;
            *request = Some(Request::Cancel);
        }
    }

    pub fn handle(
        &mut self,
        event: &GestureEvent,
        mesh: &MeshView,
        memory: &mut Memory,
        now: Micros,
        font: &FontdueRenderer<'static, Color>,
        request: &mut Option<Request>,
    ) -> Exit {
        let tap = match *event {
            GestureEvent::Tap(point) => Some(point),
            _ => None,
        };
        let tapped = |area: Rectangle| tap.is_some_and(|point| area.contains(point));
        let own = mesh.group.as_ref().map(|group| group.own);
        let next = match &mut self.screen {
            Screen::Hub => {
                if let Some(recovery) = unseen_recovery(mesh, memory) {
                    if tapped(ACTION) {
                        memory.recovery_seen = recovery.session;
                    }
                    None
                } else if pending_recovery(mesh).is_some() {
                    if tapped(NAV_HIT) {
                        return Exit::Panel;
                    }
                    tapped(PENDING_ACTION).then_some(Screen::EndWait)
                } else {
                    match &mesh.group {
                        _ if tapped(NAV_HIT) => return Exit::Panel,
                        None if tapped(ACTIONS[0]) => Some(Screen::Entry(Role::Add)),
                        None if tapped(ACTIONS[1]) => Some(Screen::Entry(Role::Join)),
                        Some(group) if tapped(OWN_CARD) => Some(Screen::Member {
                            id: group.own,
                            from: From::Hub,
                        }),
                        Some(_) if tapped(HUB_ACTIONS[0]) => {
                            Some(Screen::Members(Scroll::default()))
                        }
                        Some(_) if tapped(HUB_ACTIONS[1]) => Some(Screen::PairOptions),
                        _ => None,
                    }
                }
            }
            Screen::PairOptions => {
                if tapped(NAV_HIT) {
                    Some(Screen::Hub)
                } else if tapped(ACTIONS[0]) {
                    Some(
                        if mesh.group.as_ref().is_some_and(|group| group.is_full()) {
                            Screen::Full
                        } else {
                            Screen::Entry(Role::Add)
                        },
                    )
                } else if tapped(ACTIONS[1]) {
                    Some(if mesh.group.is_some() {
                        Screen::JoinWarning
                    } else {
                        Screen::Entry(Role::Join)
                    })
                } else {
                    None
                }
            }
            Screen::Members(scroll) => {
                let ids: heapless::Vec<u8, 32> = mesh
                    .group
                    .iter()
                    .flat_map(|group| group.members().map(|(id, _)| id))
                    .collect();
                if tapped(MEMBERS_NAV_HIT) {
                    Some(Screen::Hub)
                } else if tapped(STRIP) {
                    Some(refresh_screen(mesh, memory))
                } else {
                    scroll
                        .handle(event, ids.len(), now, MEMBER_ROWS)
                        .map(|row| Screen::Member {
                            id: ids[row],
                            from: From::Members,
                        })
                }
            }
            Screen::RefreshEntry => {
                if tapped(NAV_HIT) {
                    Some(Screen::Members(Scroll::default()))
                } else if tapped(ACTION) {
                    *request = Some(Request::Refresh);
                    let last = mesh.refresh.map_or(0, |refresh| refresh.session);
                    Some(Screen::Refresh(last + 1))
                } else {
                    None
                }
            }
            Screen::Refresh(session) => {
                let shown = mesh.refresh.filter(|refresh| refresh.session == *session);
                let running = shown.is_none_or(|refresh| refresh.is_listening());
                if running && tapped(NAV_HIT) {
                    Some(Screen::Members(Scroll::default()))
                } else if !running && tapped(ACTION) {
                    memory.refresh_seen = memory.refresh_seen.max(*session);
                    Some(Screen::Members(Scroll::default()))
                } else {
                    None
                }
            }
            Screen::RefreshUnavailable(why) => tapped(ACTION).then_some(match why {
                Unavailable::NoGroup => Screen::Hub,
                Unavailable::NoRadio | Unavailable::Pairing => Screen::Members(Scroll::default()),
            }),
            Screen::EndWait => {
                if tapped(NAV_HIT) {
                    Some(Screen::Hub)
                } else {
                    tapped(ACTION).then_some(Screen::PairOptions)
                }
            }
            Screen::Member { id, from } => {
                let (id, from) = (*id, *from);
                if Some(id) == own {
                    if tapped(NAV_HIT) {
                        Some(match from {
                            From::Hub => Screen::Hub,
                            From::Members => Screen::Members(Scroll::default()),
                        })
                    } else if tapped(ACTIONS[0]) {
                        Some(Screen::Name {
                            keyboard: Keyboard::new(mesh.name),
                            asked: None,
                            back: Some(from),
                        })
                    } else if tapped(ACTIONS[1]) {
                        Some(Screen::Leave { from })
                    } else {
                        None
                    }
                } else if tapped(NAV_HIT) {
                    Some(Screen::Members(Scroll::default()))
                } else if tapped(ACTION) {
                    Some(Screen::RemoveUnavailable { id })
                } else {
                    None
                }
            }
            Screen::RemoveUnavailable { id } => tapped(NAV_HIT).then_some(Screen::Member {
                id: *id,
                from: From::Members,
            }),
            Screen::Leave { from } => {
                let from = *from;
                if tapped(NAV_HIT) {
                    own.map(|id| Screen::Member { id, from })
                        .or(Some(Screen::Hub))
                } else {
                    tapped(ACTION).then(|| Screen::LeaveSlide {
                        slider: Slider::default(),
                        asked: None,
                        from,
                    })
                }
            }
            Screen::LeaveSlide {
                slider,
                asked,
                from,
            } => {
                if asked.is_some() {
                    None
                } else if slider.handle(event, now) {
                    *request = Some(Request::Leave);
                    *asked = Some(mesh.answered);
                    None
                } else if tapped(NAV_HIT) {
                    Some(Screen::Leave { from: *from })
                } else {
                    None
                }
            }
            Screen::LeaveDone => {
                if tapped(NAV_HIT) {
                    return Exit::Panel;
                }
                if tapped(ACTIONS[0]) {
                    Some(Screen::Entry(Role::Add))
                } else {
                    tapped(ACTIONS[1]).then_some(Screen::Entry(Role::Join))
                }
            }
            Screen::LeaveFailed => (tapped(NAV_HIT) || tapped(ACTION)).then_some(Screen::Hub),
            Screen::Name {
                keyboard,
                asked,
                back,
            } => match keyboard.handle(event, font) {
                Outcome::Stay => None,
                Outcome::Cancel | Outcome::Unchanged => match *back {
                    None => return Exit::Panel,
                    Some(from) => own
                        .map(|id| Screen::Member { id, from })
                        .or(Some(Screen::Hub)),
                },
                Outcome::Save(name) => {
                    *request = Some(Request::Rename(name));
                    *asked = Some(mesh.answered);
                    keyboard.saving();
                    None
                }
            },
            Screen::NameFailed { keyboard, back } => tapped(ACTION).then(|| {
                let mut keyboard = keyboard.clone();
                keyboard.resume();
                Screen::Name {
                    keyboard,
                    asked: None,
                    back: *back,
                }
            }),
            Screen::Entry(role) => {
                let role = *role;
                if tapped(NAV_HIT) {
                    Some(Screen::Hub)
                } else if tapped(ACTION) {
                    Some(self::start(role, mesh, request))
                } else {
                    None
                }
            }
            Screen::JoinWarning => {
                if tapped(NAV_HIT) {
                    Some(Screen::Hub)
                } else {
                    tapped(ACTION).then(|| Screen::JoinSlide {
                        slider: Slider::default(),
                        asked: None,
                    })
                }
            }
            Screen::JoinSlide { slider, asked } => {
                if asked.is_some() {
                    None
                } else if slider.handle(event, now) {
                    // Leaving cannot be undone, so it waits for a radio to join with.
                    if mesh.radio {
                        *request = Some(Request::Leave);
                        *asked = Some(mesh.answered);
                        None
                    } else {
                        Some(Screen::NoRadio(Role::Join))
                    }
                } else if tapped(NAV_HIT) {
                    Some(Screen::JoinWarning)
                } else {
                    None
                }
            }
            Screen::Full | Screen::NoRadio(_) => tapped(ACTION).then_some(Screen::Hub),
            Screen::Pairing(session) => handle_pairing(session, event, mesh, memory, now, request),
        };
        if let Some(next) = next {
            self.screen = next;
        }
        Exit::Stay
    }

    /// Takes up what the mesh has answered, and moves what moves on its own. Returns where to go
    /// and whether something still moves.
    pub fn step(
        &mut self,
        mesh: &MeshView,
        now: Micros,
        request: &mut Option<Request>,
    ) -> (Exit, bool) {
        let answered = |asked: Option<u32>| {
            asked
                .filter(|&asked| mesh.answered > asked)
                .and(mesh.answer)
        };
        let next = match &mut self.screen {
            Screen::Members(scroll) => {
                let rows = mesh.group.as_ref().map_or(0, view::GroupView::count);
                return (Exit::Stay, scroll.step(rows, now, MEMBER_ROWS));
            }
            Screen::LeaveSlide { slider, asked, .. } => match answered(*asked) {
                Some(Answer::Left(true)) => Some(Screen::LeaveDone),
                Some(Answer::Left(false)) => Some(Screen::LeaveFailed),
                _ => return (Exit::Stay, slider.step(now)),
            },
            Screen::JoinSlide { slider, asked } => match answered(*asked) {
                Some(Answer::Left(true)) => Some(self::start(Role::Join, mesh, request)),
                Some(Answer::Left(false)) => Some(Screen::LeaveFailed),
                _ => return (Exit::Stay, slider.step(now)),
            },
            Screen::Name {
                keyboard,
                asked,
                back,
            } => match answered(*asked) {
                Some(Answer::Renamed(true)) => Some(match *back {
                    None => return (Exit::Panel, false),
                    Some(from) => mesh
                        .group
                        .as_ref()
                        .map_or(Screen::Hub, |group| Screen::Member {
                            id: group.own,
                            from,
                        }),
                }),
                Some(Answer::Renamed(false)) => Some(Screen::NameFailed {
                    keyboard: keyboard.clone(),
                    back: *back,
                }),
                _ => None,
            },
            Screen::Pairing(session) => {
                if let Some(pairing) = session.view(mesh) {
                    if let Phase::Transfer { done, total } = pairing.phase {
                        session.parts = Some((done, total));
                    }
                    if !matches!(pairing.phase, Phase::Compare { .. }) {
                        session.stop = false;
                    }
                    if !matches!(pairing.phase, Phase::Compare { .. } | Phase::Waiting { .. }) {
                        session.slider = Slider::default();
                    }
                    if pairing.phase != Phase::Found {
                        session.chose = None;
                    }
                }
                let rows = session
                    .view(mesh)
                    .map_or(0, |pairing| pairing.candidates.len());
                return (
                    Exit::Stay,
                    session.slider.step(now) | session.scroll.step(rows, now, ROWS),
                );
            }
            _ => None,
        };
        if let Some(next) = next {
            self.screen = next;
        }
        (Exit::Stay, false)
    }

    /// Makes `list` what the screen shows now.
    pub fn view(
        &self,
        list: &mut List,
        mesh: &MeshView,
        memory: &Memory,
        now: Micros,
        font: &FontdueRenderer<'static, Color>,
    ) {
        list.clear();
        let l = list;
        match &self.screen {
            Screen::Hub
                if unseen_recovery(mesh, memory).is_some() || pending_recovery(mesh).is_some() =>
            {
                match (unseen_recovery(mesh, memory), pending_recovery(mesh)) {
                    (Some(recovery), _) => recovery_screen(l, &recovery, now),
                    (None, Some(recovery)) => pending_hub(l, &recovery, now),
                    (None, None) => {}
                }
            }
            Screen::Hub => match &mesh.group {
                None => {
                    head(
                        l,
                        "GROUP",
                        "GROUP / LOCAL",
                        Some("BACK"),
                        GROUP,
                        chrome::GRAY,
                    );
                    big(l, "NO GROUP", chrome::WHITE);
                    copy(
                        l,
                        &[
                            "NO GROUP DATA IS SENT",
                            &format(format_args!("MY NAME  {}", mesh.name.as_str())),
                        ],
                    );
                    pair(l, ACTIONS, ["ADD", "JOIN"]);
                    footer(l, COVER);
                }
                Some(group) => {
                    head(
                        l,
                        "GROUP",
                        "GROUP / LOCAL",
                        Some("BACK"),
                        GROUP,
                        chrome::GRAY,
                    );
                    l.centred(
                        &format(format_args!("{:02} MEMBERS", group.count())),
                        CENTRE,
                        159,
                        Face::Kh,
                        29,
                        chrome::WHITE,
                    );
                    l.outline(OWN_CARD, chrome::GRAY);
                    l.left(
                        &format(format_args!("{:02}", group.own)),
                        107,
                        216,
                        Face::Mono,
                        16,
                        chrome::GRAY,
                    );
                    name(l, mesh.name.as_str(), 147, 213, false, 22);
                    l.centred("THIS DEVICE  >", CENTRE, 255, Face::Mono, 14, chrome::GRAY);
                    pair(l, HUB_ACTIONS, ["MEMBERS", "PAIR"]);
                    footer(l, COVER);
                }
            },
            Screen::PairOptions => {
                head(
                    l,
                    "PAIRING",
                    "GROUP / CHOOSE ROLE",
                    Some("BACK"),
                    GROUP,
                    chrome::GRAY,
                );
                big(l, "PAIR A DEVICE", chrome::WHITE);
                copy(
                    l,
                    &[
                        "ADD ANOTHER DEVICE TO THIS GROUP",
                        "OR JOIN A DIFFERENT GROUP",
                    ],
                );
                pair(l, ACTIONS, ["ADD", "JOIN"]);
                footer(l, COVER);
            }
            Screen::Members(scroll) => members(l, mesh, scroll, now),
            Screen::Member { id, .. } => match mesh.group.as_ref() {
                Some(group) if group.own == *id => {
                    head(
                        l,
                        "DEVICE",
                        &format(format_args!("GROUP / ID {id:02}")),
                        Some("BACK"),
                        GROUP,
                        chrome::GRAY,
                    );
                    let size = if mesh.name.as_bytes().len() < 12 {
                        35
                    } else {
                        28
                    };
                    name(l, mesh.name.as_str(), CENTRE, 185, true, size);
                    copy(
                        l,
                        &[
                            &format(format_args!("ADDRESS {}", words::mac(&mesh.mac))),
                            &format(format_args!("{:02} MEMBERS / STORED", group.count())),
                        ],
                    );
                    pair(l, ACTIONS, ["NAME", "LEAVE"]);
                    footer(l, COVER);
                }
                Some(group) => match group.member(*id) {
                    Some(member) => member_detail(l, *id, member, now, font),
                    None => gone(l),
                },
                None => gone(l),
            },
            Screen::RemoveUnavailable { .. } => {
                head(
                    l,
                    "REMOVE",
                    "GROUP / LATER FEATURE",
                    Some("BACK"),
                    GROUP,
                    chrome::ORANGE,
                );
                big(l, "UNAVAILABLE", chrome::GRAY);
                copy(
                    l,
                    &[
                        "REMOVAL REQUIRES A GROUP UPDATE",
                        "THIS DEVICE CANNOT REMOVE MEMBERS",
                    ],
                );
                footer(l, COVER);
            }
            Screen::Leave { .. } => {
                head(
                    l,
                    "LEAVE",
                    "GROUP / THIS DEVICE",
                    Some("BACK"),
                    GROUP,
                    chrome::ORANGE,
                );
                big(l, "LEAVE GROUP?", chrome::ORANGE);
                copy(
                    l,
                    &["DELETE MEMBERSHIP HERE", "OTHERS KEEP YOUR MEMBER ENTRY"],
                );
                action(l, "CONTINUE", false, ACTION, chrome::WHITE, None);
                footer(l, COVER);
            }
            Screen::LeaveSlide { slider, asked, .. } => {
                let nav = asked.is_none().then_some("BACK");
                head(l, "LEAVE", "GROUP / CONFIRM", nav, GROUP, chrome::ORANGE);
                l.centred("LEAVE GROUP", CENTRE, 188, Face::Kh, 30, chrome::ORANGE);
                copy_at(
                    l,
                    &["NAME IS KEPT", "YOU WILL NEED TO BE ADDED AGAIN"],
                    239,
                    15,
                );
                slider.draw("SLIDE TO", "LEAVE GROUP", now, l);
                footer(l, COVER);
            }
            Screen::LeaveDone => {
                head(
                    l,
                    "GROUP",
                    "GROUP / LOCAL",
                    Some("BACK"),
                    GROUP,
                    chrome::GRAY,
                );
                big(l, "LEFT GROUP", chrome::WHITE);
                copy(
                    l,
                    &[
                        "THIS DEVICE IS NO LONGER A MEMBER",
                        "OTHERS MAY STILL LIST IT",
                    ],
                );
                pair(l, ACTIONS, ["ADD", "JOIN"]);
                footer(l, COVER);
            }
            Screen::LeaveFailed => {
                head(
                    l,
                    "LEAVE",
                    "GROUP / LOCAL",
                    Some("BACK"),
                    FAULT,
                    chrome::RED,
                );
                big(l, "SAVE FAILED", chrome::RED);
                copy(
                    l,
                    &["GROUP COULD NOT BE CLEARED", "MEMBERSHIP IS STILL STORED"],
                );
                action(l, "BACK TO GROUP", false, ACTION, chrome::WHITE, None);
                footer(l, COVER);
            }
            Screen::Name { keyboard, .. } => keyboard.draw(font, l),
            Screen::NameFailed { .. } => {
                head(l, "MY NAME", "NAME / LOCAL", None, FAULT, chrome::RED);
                big(l, "SAVE FAILED", chrome::RED);
                copy(
                    l,
                    &["LAST SAVED NAME IS KEPT", "YOUR DRAFT IS STILL AVAILABLE"],
                );
                action(l, "RETURN TO EDIT", false, ACTION, chrome::WHITE, None);
                footer(l, COVER);
            }
            Screen::Entry(role) => entry(l, *role, mesh),
            Screen::JoinWarning => {
                head(l, "PAIRING", "JOIN", Some("BACK"), GROUP, chrome::ORANGE);
                big(l, "LEAVE GROUP?", chrome::ORANGE);
                copy(
                    l,
                    &[
                        "JOINING ERASES THIS GROUP HERE",
                        "A FAILED JOIN WILL NOT RESTORE IT",
                    ],
                );
                action(l, "CONTINUE", false, ACTION, chrome::WHITE, None);
                footer(l, COVER);
            }
            Screen::JoinSlide { slider, asked } => {
                let nav = asked.is_none().then_some("BACK");
                head(l, "PAIRING", "JOIN", nav, GROUP, chrome::ORANGE);
                l.centred("LEAVE FIRST", CENTRE, 188, Face::Kh, 30, chrome::ORANGE);
                copy_at(
                    l,
                    &[
                        "ERASE THIS GROUP ON THIS DEVICE",
                        "A FAILED JOIN WILL NOT RESTORE IT",
                    ],
                    239,
                    15,
                );
                slider.draw("SLIDE TO", "LEAVE + JOIN", now, l);
                footer(l, COVER);
            }
            Screen::Full => {
                let count = mesh.group.as_ref().map_or(0, view::GroupView::count);
                outcome(
                    l,
                    "ADD",
                    &Ending {
                        big: "GROUP FULL",
                        color: chrome::ORANGE,
                        glyph: GROUP,
                        copy: ["", "ADDING IS UNAVAILABLE"],
                        action: "BACK TO GROUP",
                    },
                    Some(&format(format_args!("{count:02} / {} MEMBERS", view::IDS))),
                );
            }
            Screen::NoRadio(role) => outcome(
                l,
                caption(*role),
                &Ending {
                    big: "NO RADIO",
                    color: chrome::RED,
                    glyph: FAULT,
                    copy: ["RADIO DID NOT START", "PAIRING UNAVAILABLE"],
                    action: "BACK TO GROUP",
                },
                None,
            ),
            Screen::Pairing(session) => pairing(l, session, mesh, now),
            Screen::RefreshEntry => refresh_entry(l),
            Screen::Refresh(session) => refresh(l, *session, mesh, now),
            Screen::RefreshUnavailable(why) => refresh_unavailable(l, *why),
            Screen::EndWait => status(
                l,
                &Status {
                    caption: "GROUP / RECOVERY",
                    nav: Some("BACK"),
                    glyph: (WAIT, chrome::ORANGE),
                    heading: ("END CURRENT WAIT?", chrome::ORANGE),
                    lines: &[
                        "A NEW PAIRING ENDS RECOVERY",
                        "THE PENDING GROUP IS NOT STORED",
                    ],
                    action: Some("CONTINUE TO PAIR"),
                },
            ),
        }
    }
}

fn caption(role: Role) -> &'static str {
    match role {
        Role::Add => "ADD",
        Role::Join => "JOIN",
    }
}

/// Starts a pairing in `role`, or the screen that says why it cannot.
fn start(role: Role, mesh: &MeshView, request: &mut Option<Request>) -> Screen {
    if !mesh.radio {
        return Screen::NoRadio(role);
    }
    if role == Role::Add && mesh.group.as_ref().is_some_and(view::GroupView::is_full) {
        return Screen::Full;
    }
    *request = Some(match role {
        Role::Add => Request::Add,
        Role::Join => Request::Join,
    });
    Screen::Pairing(Session::new(role, mesh))
}

fn handle_pairing(
    session: &mut Session,
    event: &GestureEvent,
    mesh: &MeshView,
    memory: &mut Memory,
    now: Micros,
    request: &mut Option<Request>,
) -> Option<Screen> {
    let tap = match *event {
        GestureEvent::Tap(point) => Some(point),
        _ => None,
    };
    let tapped = |area: Rectangle| tap.is_some_and(|point| area.contains(point));
    let pairing = session.view(mesh);
    let phase = pairing.map(|pairing| pairing.phase);
    if pairing.is_some_and(|pairing| pairing.refused.is_some())
        || phase.is_some_and(Phase::is_final)
    {
        if tapped(ACTION)
            && let Some(recovery) = pairing.and_then(|pairing| recovery_of(mesh, pairing))
            && recovery.phase.is_final()
        {
            memory.recovery_seen = memory.recovery_seen.max(recovery.session);
        }
        return tapped(ACTION).then_some(Screen::Hub);
    }
    if session.stop {
        if tapped(NAV_HIT) {
            session.stop = false;
        } else if tapped(ACTIONS[0]) || tapped(ACTIONS[1]) {
            *request = Some(if tapped(ACTIONS[0]) {
                Request::Mismatch
            } else {
                Request::Decline
            });
            session.stop = false;
            session.cancelled = true;
        }
        return None;
    }
    match phase {
        // Too late to cancel: the group is being stored.
        Some(Phase::Storing | Phase::Finishing) => {}
        Some(Phase::Compare { .. }) if tapped(NAV_HIT) => session.stop = true,
        _ if tapped(NAV_HIT) => {
            if !session.cancelled {
                session.cancelled = true;
                *request = Some(Request::Cancel);
            }
        }
        Some(Phase::Compare { .. }) => {
            if session.slider.handle(event, now) {
                *request = Some(Request::Accept);
            }
        }
        Some(Phase::Found) => {
            let candidates = pairing.map_or(&[][..], |pairing| &pairing.candidates);
            if session.chose.is_none()
                && let Some(row) = session.scroll.handle(event, candidates.len(), now, ROWS)
            {
                session.chose = Some(row);
                *request = Some(Request::Choose(candidates[row]));
            }
        }
        _ => {}
    }
    None
}

/// The title, the caption under it, the top button when the screen has one, and the status
/// glyph.
fn head(
    list: &mut List,
    title: &str,
    caption: &str,
    nav: Option<&str>,
    glyph: Glyph,
    color: Color,
) {
    list.centred(title, CENTRE, 30, Face::Title, 27, chrome::WHITE);
    list.centred(caption, CENTRE, 70, Face::Mono, 13, chrome::GRAY);
    if let Some(nav) = nav {
        list.outline(NAV, chrome::GRAY);
        list.centred(nav, 132, 109, Face::Mono, 14, chrome::WHITE);
    }
    list.glyph(glyph, color);
}

/// A screen's status heading, smaller when long.
fn big(list: &mut List, text: &str, color: Color) {
    let size = if text.len() < 12 { 35 } else { 28 };
    list.centred(text, CENTRE, 185, Face::Kh, size, color);
}

fn copy(list: &mut List, lines: &[&str]) {
    copy_at(list, lines, 244, 16);
}

fn copy_at(list: &mut List, lines: &[&str], top: i32, size: u8) {
    for (i, line) in lines.iter().enumerate() {
        list.centred(
            line,
            CENTRE,
            top + 23 * i as i32,
            Face::Sans,
            size,
            chrome::WHITE,
        );
    }
}

/// A name, kept in its case, on the baseline a capital at `top` gives.
fn name(list: &mut List, text: &str, x: i32, top: i32, centred: bool, size: u8) {
    let text = Text::new(text, Face::Mono, size, chrome::WHITE)
        .at(x, top)
        .vertical(Vertical::Cap);
    list.text(if centred {
        text
    } else {
        text.align(layout::Align::Left)
    });
}

fn action(
    list: &mut List,
    label: &str,
    primary: bool,
    area: Rectangle,
    color: Color,
    sub: Option<&str>,
) {
    let (ink, on) = if primary {
        list.fill(area, chrome::WHITE);
        (chrome::BLACK, chrome::WHITE)
    } else {
        list.outline(area, chrome::GRAY);
        (color, chrome::BLACK)
    };
    let x = area.top_left.x + area.size.width as i32 / 2;
    match sub {
        // The label and its note sit together as one block.
        Some(sub) => {
            list.text(Text::new(label, Face::Kh, 20, ink).at(x, 333).on(on));
            list.centred(sub, x, 373, Face::Mono, 12, chrome::GRAY);
        }
        None => list.text(
            Text::new(label, Face::Kh, 20, ink)
                .at(x, area.top_left.y + area.size.height as i32 / 2)
                .vertical(Vertical::Middle)
                .on(on),
        ),
    }
}

fn pair(list: &mut List, areas: [Rectangle; 2], labels: [&str; 2]) {
    for (area, label) in areas.into_iter().zip(labels) {
        action(list, label, false, area, chrome::WHITE, None);
    }
}

fn footer(list: &mut List, text: &str) {
    list.centred(text, CENTRE, 420, Face::Mono, 12, chrome::GRAY);
}

/// A screen whose member went from the group while it showed.
fn gone(list: &mut List) {
    head(list, "MEMBER", "GROUP", Some("BACK"), GROUP, chrome::GRAY);
    big(list, "NOT LISTED", chrome::GRAY);
    copy(
        list,
        &["THIS MEMBER IS NO LONGER", "IN THE GROUP STORED HERE"],
    );
    footer(list, COVER);
}

/// How long ago a member was last heard directly, and whether that makes it a neighbour.
fn direct(member: &MemberView, own: bool, now: Micros, list: &mut List) -> (layout::Line, Color) {
    if own {
        return (format(format_args!("THIS DEVICE")), chrome::GRAY);
    }
    let Some(heard) = member.heard else {
        return (format(format_args!("DIRECT NEVER HEARD")), chrome::GRAY);
    };
    let (age, next) = words::age(heard, now);
    list.changes_at(next);
    let live = now as At - heard <= NEIGHBOUR;
    if live {
        list.changes_at((heard + NEIGHBOUR + 1).max(0) as Micros);
    }
    (
        format(format_args!("DIRECT {age} AGO")),
        if live { chrome::BLUE } else { chrome::GRAY },
    )
}

fn position(member: &MemberView, now: Micros, list: &mut List) -> layout::Line {
    match member.position {
        Position::Never => format(format_args!("POSITION NEVER RECEIVED")),
        Position::Unknown => format(format_args!("POSITION AGE UNKNOWN")),
        Position::At(at) => {
            let (age, next) = words::age(at, now);
            list.changes_at(next);
            format(format_args!("POSITION {age} OLD"))
        }
    }
}

fn members(list: &mut List, mesh: &MeshView, scroll: &Scroll, now: Micros) {
    let Some(group) = &mesh.group else {
        return gone(list);
    };
    let count = group.count();
    head(
        list,
        "MEMBERS",
        &format(format_args!("GROUP / {count:02}")),
        Some("BACK"),
        GROUP,
        chrome::GRAY,
    );
    list.outline(STRIP, chrome::GRAY);
    let (label, color) = match mesh.refresh {
        _ if !mesh.radio => (format(format_args!("NO RADIO")), chrome::RED),
        Some(RefreshView {
            phase: RefreshPhase::Listening { until },
            ..
        }) => (
            format(format_args!("LISTENING  {}", time_left(until, now, list))),
            chrome::WHITE,
        ),
        _ => (format(format_args!("REFRESH DEVICES")), chrome::WHITE),
    };
    list.text(
        Text::new(&label, Face::Kh, 20, color)
            .at(CENTRE, STRIP.top_left.y + STRIP.size.height as i32 / 2)
            .vertical(Vertical::Middle),
    );
    list.centred(
        &format(format_args!("{count:02} MEMBERS / DRAG LIST")),
        CENTRE,
        222,
        Face::Mono,
        13,
        chrome::GRAY,
    );
    list.clip(Some(MEMBER_ROWS));
    for (row, (id, member)) in group.members().enumerate() {
        let top = scroll.top(row, MEMBER_ROWS);
        if top >= MEMBER_ROWS.top_left.y + MEMBER_ROWS.size.height as i32
            || top + ROW <= MEMBER_ROWS.top_left.y
        {
            continue;
        }
        list.fill(rect(94, top, 372, top + 1), chrome::GRAY);
        list.left(
            &format(format_args!("{id:02}")),
            102,
            top + 17,
            Face::Mono,
            16,
            chrome::GRAY,
        );
        list.text(
            Text::new(member.name.as_str(), Face::Mono, 18, chrome::WHITE)
                .at(138, top + 16)
                .align(layout::Align::Left)
                .vertical(Vertical::Cap),
        );
        let (contact, color) = direct(member, id == group.own, now, list);
        list.left(&contact, 102, top + 51, Face::Mono, 14, color);
        let seen = position(member, now, list);
        list.left(&seen, 102, top + 77, Face::Sans, 14, chrome::GRAY);
    }
    list.clip(None);
    let (first, last) = scroll.shown(count, MEMBER_ROWS);
    footer(
        list,
        &format(format_args!(
            "{first:02}-{last:02} / {count:02} / TAP FOR DETAILS"
        )),
    );
}

fn member_detail(
    list: &mut List,
    id: u8,
    member: &MemberView,
    now: Micros,
    font: &FontdueRenderer<'static, Color>,
) {
    head(
        list,
        "MEMBER",
        &format(format_args!("GROUP / ID {id:02}")),
        Some("BACK"),
        GROUP,
        chrome::GRAY,
    );
    list.centred(
        &words::mac(&member.mac),
        CENTRE,
        158,
        Face::Mono,
        13,
        chrome::GRAY,
    );
    // The longest names take the smaller size to fit between the screen's margins.
    let wide = crate::ui::text::style(font, chrome::WHITE, 30, Face::Mono.index())
        .advance(member.name.as_str())
        > 270.0;
    name(
        list,
        member.name.as_str(),
        CENTRE,
        189,
        true,
        if wide { 26 } else { 30 },
    );
    let (contact, color) = direct(member, false, now, list);
    list.centred(&contact, CENTRE, 232, Face::Mono, 16, color);
    let seen = position(member, now, list);
    list.centred(&seen, CENTRE, 256, Face::Mono, 16, chrome::GRAY);
    let joined = match member.joined {
        Some(joined) => {
            let (age, next) = words::age(joined, now);
            list.changes_at(next);
            format(format_args!("JOINED {age} AGO"))
        }
        None => format(format_args!("JOINED TIME UNKNOWN")),
    };
    list.centred(&joined, CENTRE, 280, Face::Mono, 13, chrome::GRAY);
    action(
        list,
        "REMOVE",
        false,
        ACTION,
        chrome::GRAY,
        Some("UNAVAILABLE"),
    );
    footer(list, COVER);
}

fn entry(list: &mut List, role: Role, mesh: &MeshView) {
    match (role, &mesh.group) {
        (Role::Add, Some(group)) => {
            head(
                list,
                "PAIRING",
                "ADD / NOT ACTIVE",
                Some("BACK"),
                ADD,
                chrome::GRAY,
            );
            list.centred(
                &format(format_args!(
                    "THIS {} / ID {:02}",
                    mesh.name.as_str(),
                    group.own
                )),
                CENTRE,
                164,
                Face::Mono,
                13,
                chrome::GRAY,
            );
            big(list, "ADD MEMBER", chrome::WHITE);
            copy(list, &["ON THE OTHER DEVICE", "CHOOSE JOIN GROUP"]);
            action(list, "START SEARCH", true, ACTION, chrome::WHITE, None);
        }
        (Role::Add, None) => {
            head(
                list,
                "PAIRING",
                "ADD / NOT ACTIVE",
                Some("BACK"),
                ADD,
                chrome::GRAY,
            );
            big(list, "NO GROUP", chrome::WHITE);
            copy(
                list,
                &["SUCCESS CREATES A GROUP OF TWO", "YOU BECOME MEMBER 00"],
            );
            action(list, "START SEARCH", true, ACTION, chrome::WHITE, None);
        }
        (Role::Join, _) => {
            head(
                list,
                "PAIRING",
                "JOIN / NOT ACTIVE",
                Some("BACK"),
                JOIN,
                chrome::GRAY,
            );
            list.centred(
                &format(format_args!("THIS {} / NO GROUP", mesh.name.as_str())),
                CENTRE,
                164,
                Face::Mono,
                13,
                chrome::GRAY,
            );
            big(list, "JOIN GROUP", chrome::WHITE);
            copy(list, &["ON THE OTHER DEVICE", "CHOOSE ADD MEMBER"]);
            action(list, "START JOIN", true, ACTION, chrome::WHITE, None);
        }
    }
    footer(list, COVER);
}

/// A pairing's end, as its screen says it.
struct Ending {
    big: &'static str,
    color: Color,
    glyph: Glyph,
    copy: [&'static str; 2],
    action: &'static str,
}

/// An end screen; `first` stands in for the first line of copy where it is worked out.
fn outcome(list: &mut List, caption: &str, ending: &Ending, first: Option<&str>) {
    let glyph_color = if ending.color == chrome::ORANGE || ending.color == chrome::RED {
        ending.color
    } else {
        chrome::GRAY
    };
    head(list, "PAIRING", caption, None, ending.glyph, glyph_color);
    big(list, ending.big, ending.color);
    copy(list, &[first.unwrap_or(ending.copy[0]), ending.copy[1]]);
    action(list, ending.action, false, ACTION, chrome::WHITE, None);
    footer(list, COVER);
}

fn ending(end: End, role: Role, transferred: bool) -> Ending {
    let back = "BACK TO GROUP";
    let fault = |big, color, copy| Ending {
        big,
        color,
        glyph: FAULT,
        copy,
        action: back,
    };
    let declined = || {
        fault(
            "PEER DECLINED",
            chrome::ORANGE,
            ["OTHER USER REJECTED", "NOT PAIRED"],
        )
    };
    let timed_out = || {
        fault(
            "TIME EXPIRED",
            chrome::ORANGE,
            ["CODE NOT CONFIRMED", "NO MEMBER ADDED"],
        )
    };
    let store_failed = || match role {
        Role::Add => fault(
            "SAVE FAILED",
            chrome::RED,
            ["MEMBERSHIP COULD NOT BE STORED", "PAIRING NOT COMPLETE"],
        ),
        Role::Join => fault(
            "SAVE FAILED",
            chrome::RED,
            ["GROUP COULD NOT BE STORED", "NOT JOINED"],
        ),
    };
    match end {
        End::NotFound => fault(
            "NOT FOUND",
            chrome::ORANGE,
            ["NO DEVICE FOUND IN TIME", "TRY WITH BOTH SCREENS OPEN"],
        ),
        End::TimedOut | End::Peer(Reason::TimedOut) => timed_out(),
        End::Declined => fault(
            "CODE REJECTED",
            chrome::ORANGE,
            ["YOU DECLINED THE CODE", "NOT PAIRED"],
        ),
        // The other user's report of a mismatch is a rejection here: this device saw no
        // difference itself.
        End::Peer(Reason::Declined | Reason::Mismatch) => declined(),
        End::Mismatch => fault(
            "CODES DIFFER",
            chrome::RED,
            ["YOU REPORTED A MISMATCH", "DO NOT PAIR THESE CODES"],
        ),
        End::Cancelled => Ending {
            big: "CANCELLED",
            color: chrome::WHITE,
            glyph: WAIT,
            copy: ["PAIRING STOPPED HERE", "NO MEMBER ADDED"],
            action: back,
        },
        End::Peer(Reason::Cancelled) => fault(
            "PEER CANCELLED",
            chrome::ORANGE,
            ["OTHER DEVICE LEFT PAIRING", "NO MEMBER ADDED"],
        ),
        End::Lost if transferred => fault(
            "CONTACT LOST",
            chrome::ORANGE,
            ["TRANSFER DID NOT RESUME", "PAIRING NOT COMPLETE"],
        ),
        End::Lost => fault(
            "CONTACT LOST",
            chrome::ORANGE,
            ["KEY EXCHANGE DID NOT FINISH", "PAIRING NOT COMPLETE"],
        ),
        End::Inauthentic => fault(
            "KEY CHECK FAILED",
            chrome::RED,
            ["OTHER DEVICE FAILED A KEY CHECK", "NOT PAIRED"],
        ),
        End::Malformed => fault(
            "TRANSFER FAILED",
            chrome::RED,
            ["THE GROUP RECEIVED WAS UNUSABLE", "NOT JOINED"],
        ),
        End::Full => Ending {
            big: "GROUP FULL",
            color: chrome::ORANGE,
            glyph: GROUP,
            copy: ["ALL 32 IDS ARE TAKEN", "ADDING IS UNAVAILABLE"],
            action: back,
        },
        End::StoreFailed | End::Peer(Reason::StoreFailed) => store_failed(),
        End::Unconfirmed => Ending {
            big: "CHECK MEMBER",
            color: chrome::ORANGE,
            glyph: WAIT,
            copy: ["NO FINAL REPLY RECEIVED", "CHECK THE OTHER DEVICE'S GROUP"],
            action: "VIEW GROUP",
        },
    }
}

/// The other device by its name once it has sent one, and by its address until then.
fn other(pairing: &PairingView) -> layout::Line {
    match (pairing.peer_name, pairing.peer) {
        (Some(name), _) => format(format_args!("{}", name.as_str())),
        (None, Some(mac)) => format(format_args!("{}", words::mac(&mac))),
        (None, None) => layout::Line::new(),
    }
}

/// The other device as the code screens name it: its name once it has sent one, and its
/// address.
fn peer(pairing: &PairingView) -> layout::Line {
    let mac = pairing.peer.map(|mac| words::mac(&mac)).unwrap_or_default();
    match pairing.peer_name {
        Some(name) => format(format_args!("{} / {mac}", name.as_str())),
        None => format(format_args!("{mac}")),
    }
}

fn pairing(list: &mut List, session: &Session, mesh: &MeshView, now: Micros) {
    let caption = session.caption();
    let Some(pairing) = session.view(mesh) else {
        // Asked for, and not yet taken up: the role's first screen, with no time to count.
        return discovery(list, session, mesh, None, now);
    };
    if let Some(refused) = pairing.refused {
        let ending = match refused {
            Refused::InGroup => Ending {
                big: "IN A GROUP",
                color: chrome::ORANGE,
                glyph: GROUP,
                copy: ["LEAVE THIS GROUP TO JOIN ANOTHER", "NOT PAIRED"],
                action: "BACK TO GROUP",
            },
            Refused::NoRandom => Ending {
                big: "UNAVAILABLE",
                color: chrome::RED,
                glyph: FAULT,
                copy: ["NO RANDOM SOURCE FOR KEYS", "PAIRING UNAVAILABLE"],
                action: "BACK TO GROUP",
            },
            Refused::Removing => Ending {
                big: "REMOVING",
                color: chrome::ORANGE,
                glyph: GROUP,
                copy: ["A MEMBER IS BEING REMOVED", "ADD ONCE THE KEY HAS CHANGED"],
                action: "BACK TO GROUP",
            },
        };
        return outcome(list, caption, &ending, None);
    }
    let cancel = (!session.cancelled).then_some("CANCEL");
    match pairing.phase {
        Phase::Found if session.role == Role::Add => {
            head(list, "PAIRING", caption, cancel, JOIN, chrome::GRAY);
            list.centred(
                "SELECT DEVICE / UNVERIFIED",
                CENTRE,
                168,
                Face::Mono,
                13,
                chrome::GRAY,
            );
            list.clip(Some(ROWS));
            for (row, mac) in pairing.candidates.iter().enumerate() {
                let top = session.scroll.top(row, ROWS);
                let held = session.chose.or(session.scroll.pressed) == Some(row);
                list.outline(
                    rect(94, top, 372, top + ROW),
                    if held { chrome::VIOLET } else { chrome::GRAY },
                );
                list.left(
                    &words::mac(mac),
                    107,
                    top + 21,
                    Face::Mono,
                    22,
                    chrome::WHITE,
                );
                list.left(
                    "NAME NOT SENT YET",
                    107,
                    top + 64,
                    Face::Mono,
                    15,
                    chrome::GRAY,
                );
            }
            list.clip(None);
            if pairing.candidates.len() == 1 {
                for (line, top) in [("CHECK ITS ADDRESS ON", 334), ("THE OTHER DEVICE", 360)] {
                    list.centred(line, CENTRE, top, Face::Sans, 16, chrome::WHITE);
                }
            }
            footer(list, COVER);
        }
        Phase::Searching | Phase::Found => discovery(list, session, mesh, pairing.deadline, now),
        Phase::Connecting => {
            head(list, "PAIRING", caption, cancel, WAIT, chrome::GRAY);
            big(list, "PREPARING", chrome::WHITE);
            copy(list, &["WORKING OUT THE CODE", "NOT PAIRED YET"]);
            footer(list, COVER);
        }
        Phase::Compare { .. } if session.stop => {
            head(list, "PAIRING", caption, Some("BACK"), CODE, chrome::ORANGE);
            big(list, "STOP PAIRING", chrome::ORANGE);
            copy(
                list,
                &[
                    "REPORT A MISMATCH IF CODES DIFFER",
                    "OR DECLINE THIS PAIRING",
                ],
            );
            pair(list, ACTIONS, ["MISMATCH", "DECLINE"]);
            footer(list, COVER);
        }
        Phase::Compare { code } | Phase::Waiting { code } => {
            let waiting = matches!(pairing.phase, Phase::Waiting { .. });
            if waiting {
                head(list, "PAIRING", caption, cancel, WAIT, chrome::GRAY);
            } else {
                head(list, "PAIRING", caption, Some("STOP"), CODE, chrome::ORANGE);
            }
            list.centred(&peer(pairing), CENTRE, 165, Face::Mono, 14, chrome::GRAY);
            if waiting {
                list.centred(
                    "CONFIRMED ON THIS DEVICE",
                    CENTRE,
                    188,
                    Face::Mono,
                    14,
                    chrome::GRAY,
                );
            } else {
                list.centred(
                    "COMPARE BOTH SCREENS",
                    CENTRE,
                    188,
                    Face::Mono,
                    14,
                    chrome::WHITE,
                );
            }
            list.fill(CODE_SLAB, chrome::WHITE);
            list.text(
                Text::new(&words::code(code), Face::Kh, 69, chrome::BLACK)
                    .at(CENTRE, 229)
                    .on(chrome::WHITE),
            );
            if waiting {
                list.outline(ACTION, chrome::GRAY);
                list.centred("WAITING FOR", CENTRE, 325, Face::Sans, 18, chrome::WHITE);
                list.centred("OTHER USER", CENTRE, 350, Face::Sans, 18, chrome::WHITE);
            } else {
                session.slider.draw("SLIDE IF", "CODES MATCH", now, list);
            }
            let left = pairing.deadline.map_or_else(
                || format(format_args!("--:--")),
                |deadline| {
                    let (left, next) = words::countdown(deadline, now);
                    if let Some(next) = next {
                        list.changes_at(next);
                    }
                    format(format_args!("{left}"))
                },
            );
            footer(list, &format(format_args!("{left} LEFT / COVER CANCELS")));
        }
        Phase::Transfer { done, total } => progress(
            list,
            session,
            pairing,
            Progress::Moving(done, total),
            cancel,
        ),
        Phase::Storing => progress(list, session, pairing, Progress::Storing, None),
        Phase::Finishing => progress(list, session, pairing, Progress::Finishing, None),
        Phase::Done(done) => finished(list, session, mesh, pairing, done),
        Phase::Ended(End::Unconfirmed) if session.founding => match recovery_of(mesh, pairing) {
            Some(recovery) => recovery_screen(list, &recovery, now),
            None => outcome(
                list,
                caption,
                &ending(End::Unconfirmed, session.role, true),
                None,
            ),
        },
        Phase::Ended(End::Unconfirmed) => status(
            list,
            &Status {
                caption: "GROUP / RECOVERY",
                nav: None,
                glyph: (WAIT, chrome::ORANGE),
                heading: ("CHECK MEMBER", chrome::ORANGE),
                lines: &[
                    "YOUR GROUP REMAINS STORED",
                    "MEMBER NOT CONFIRMED YET",
                    "IT APPEARS WHEN ITS RECORD ARRIVES",
                ],
                action: Some("VIEW GROUP"),
            },
        ),
        Phase::Ended(end) => {
            let ending = ending(end, session.role, session.parts.is_some());
            if end == End::Full {
                let count = pairing.group.map_or(0, |(_, count)| count);
                outcome(
                    list,
                    caption,
                    &ending,
                    Some(&format(format_args!("{count:02} / {} MEMBERS", view::IDS))),
                );
            } else {
                outcome(list, caption, &ending, None);
            }
        }
    }
}

/// Searching to add, or announcing to join, with the time left when the mesh has given it.
fn discovery(
    list: &mut List,
    session: &Session,
    mesh: &MeshView,
    deadline: Option<At>,
    now: Micros,
) {
    let caption = session.caption();
    let cancel = (!session.cancelled).then_some("CANCEL");
    let left = match deadline {
        Some(deadline) => {
            let (left, next) = words::countdown(deadline, now);
            if let Some(next) = next {
                list.changes_at(next);
            }
            format(format_args!("{left}"))
        }
        None => format(format_args!("--:--")),
    };
    head(list, "PAIRING", caption, cancel, WAIT, chrome::GRAY);
    match session.role {
        Role::Add => {
            list.centred("SEARCHING", CENTRE, 177, Face::Kh, 30, chrome::WHITE);
            list.centred(
                "OTHER DEVICE: CHOOSE JOIN",
                CENTRE,
                223,
                Face::Sans,
                15,
                chrome::WHITE,
            );
            list.fill(COUNT_SLAB, chrome::WHITE);
            list.text(
                Text::new(&left, Face::Kh, 43, chrome::BLACK)
                    .at(CENTRE, 276)
                    .on(chrome::WHITE),
            );
            list.centred(
                "TIME LEFT TO FIND A DEVICE",
                CENTRE,
                355,
                Face::Mono,
                13,
                chrome::GRAY,
            );
        }
        Role::Join => {
            list.centred("MY DEVICE", CENTRE, 161, Face::Mono, 13, chrome::GRAY);
            name(list, mesh.name.as_str(), CENTRE, 186, true, 28);
            list.centred(
                &words::mac(&mesh.mac),
                CENTRE,
                226,
                Face::Mono,
                16,
                chrome::GRAY,
            );
            list.centred(
                "WAITING TO BE SELECTED",
                CENTRE,
                274,
                Face::Sans,
                17,
                chrome::WHITE,
            );
            list.centred(
                &format(format_args!("{left} LEFT")),
                CENTRE,
                303,
                Face::Mono,
                16,
                chrome::GRAY,
            );
            list.centred(
                "KEEP BOTH DEVICES NEAR",
                CENTRE,
                354,
                Face::Sans,
                15,
                chrome::WHITE,
            );
        }
    }
    footer(list, COVER);
}

enum Progress {
    Moving(u8, u8),
    Storing,
    Finishing,
}

fn progress(
    list: &mut List,
    session: &Session,
    pairing: &PairingView,
    progress: Progress,
    nav: Option<&str>,
) {
    let add = session.role == Role::Add;
    head(
        list,
        "PAIRING",
        session.caption(),
        nav,
        TRANSFER,
        chrome::GRAY,
    );
    let (heading, label, (done, total)) = match progress {
        Progress::Moving(done, total) => (
            if add { "SENDING" } else { "RECEIVING" },
            if add {
                "PACKETS ACKNOWLEDGED"
            } else {
                "PACKETS RECEIVED"
            },
            (done, total),
        ),
        Progress::Storing => (
            "STORING",
            if add {
                "WRITING MEMBER"
            } else {
                "WRITING MEMBERSHIP"
            },
            session.parts.map_or((0, 0), |(_, total)| (total, total)),
        ),
        Progress::Finishing => (
            "WAITING",
            "FINAL REPLY PENDING",
            session.parts.map_or((0, 0), |(_, total)| (total, total)),
        ),
    };
    list.centred(heading, CENTRE, 174, Face::Kh, 32, chrome::WHITE);
    name(list, &other(pairing), CENTRE, 220, true, 20);
    list.centred(label, CENTRE, 256, Face::Mono, 13, chrome::GRAY);
    list.fill(PROGRESS_SLAB, chrome::WHITE);
    list.text(
        Text::new(
            &format(format_args!("{done:02} / {total:02}")),
            Face::Kh,
            36,
            chrome::BLACK,
        )
        .at(CENTRE, 299)
        .on(chrome::WHITE),
    );
    list.fill(BAR, chrome::BLACK);
    if total > 0 {
        let width = BAR.size.width as i32 * i32::from(done) / i32::from(total);
        list.fill(rect(106, 345, 106 + width, 353), chrome::GRAY);
    }
    let note = if matches!(progress, Progress::Moving(..)) {
        "KEEP BOTH DEVICES NEAR"
    } else {
        "NOT COMPLETE YET"
    };
    list.centred(note, CENTRE, 382, Face::Mono, 14, chrome::GRAY);
    footer(list, COVER);
}

fn finished(
    list: &mut List,
    session: &Session,
    mesh: &MeshView,
    pairing: &PairingView,
    done: Done,
) {
    let caption = session.caption();
    let count = pairing.group.map_or(0, |(_, count)| count);
    let me = mesh.name.as_str();
    match done {
        Done::Joined {
            confirmed: false, ..
        } => outcome(
            list,
            caption,
            &Ending {
                big: "GROUP STORED",
                color: chrome::ORANGE,
                glyph: WAIT,
                copy: ["LOCAL MEMBERSHIP IS STORED", "PEER RECEIPT NOT CONFIRMED"],
                action: "VIEW GROUP",
            },
            None,
        ),
        Done::Added { id, .. } if session.founding => {
            let own = pairing.group.map_or(0, |(own, _)| own);
            let other = other(pairing);
            head(list, "PAIRING", caption, None, DONE, chrome::GRAY);
            big(list, "GROUP CREATED", chrome::WHITE);
            copy(
                list,
                &[
                    &format(format_args!("{own:02} {me} / THIS DEVICE")),
                    &format(format_args!("{id:02} {other} / GROUP {count:02}")),
                ],
            );
            action(list, "VIEW GROUP", false, ACTION, chrome::WHITE, None);
            footer(list, COVER);
        }
        Done::Joined { id, .. } if count == 2 => {
            head(list, "PAIRING", caption, None, DONE, chrome::GRAY);
            big(list, "JOINED", chrome::WHITE);
            copy(
                list,
                &[
                    &format(format_args!("{id:02} {me} / THIS DEVICE")),
                    &format(format_args!("GROUP {count:02} MEMBERS")),
                ],
            );
            action(list, "VIEW GROUP", false, ACTION, chrome::WHITE, None);
            footer(list, COVER);
        }
        Done::Added { id, returning } => {
            let word = if returning { "RESTORED" } else { "ADDED" };
            member_done(list, caption, word, &other(pairing), id, count);
        }
        Done::Joined { id, .. } => member_done(list, caption, "JOINED", me, id, count),
    }
}

/// The member a pairing added or restored, or this device as it joined, with its id.
fn member_done(list: &mut List, caption: &str, word: &str, who: &str, id: u8, count: u8) {
    head(list, "PAIRING", caption, None, DONE, chrome::GRAY);
    list.centred(word, CENTRE, 164, Face::Mono, 16, chrome::GRAY);
    name(list, who, CENTRE, 196, true, 31);
    list.centred(
        &format(format_args!("MEMBER {id:02}")),
        CENTRE,
        243,
        Face::Kh,
        25,
        chrome::WHITE,
    );
    list.centred(
        &format(format_args!("GROUP {count:02} MEMBERS")),
        CENTRE,
        279,
        Face::Mono,
        14,
        chrome::GRAY,
    );
    action(list, "VIEW GROUP", false, ACTION, chrome::WHITE, None);
    footer(list, COVER);
}

/// The time left before `until`, as minutes and seconds, changing as it ticks.
fn time_left(until: At, now: Micros, list: &mut List) -> layout::Line {
    let (left, next) = words::countdown(until, now);
    if let Some(next) = next {
        list.changes_at(next);
    }
    format(format_args!("{left}"))
}

/// A status screen as the 2026-10-03 hand-off lays them out: a heading, up to three lines under
/// it, the first white, and one action.
struct Status<'a> {
    caption: &'a str,
    nav: Option<&'a str>,
    glyph: (Glyph, Color),
    heading: (&'a str, Color),
    lines: &'a [&'a str],
    action: Option<&'a str>,
}

fn status(list: &mut List, status: &Status) {
    head(
        list,
        "GROUP",
        status.caption,
        status.nav,
        status.glyph.0,
        status.glyph.1,
    );
    list.centred(
        status.heading.0,
        CENTRE,
        166,
        Face::Kh,
        28,
        status.heading.1,
    );
    for (i, line) in status.lines.iter().enumerate() {
        let color = if i == 0 { chrome::WHITE } else { chrome::GRAY };
        list.centred(line, CENTRE, 218 + 23 * i as i32, Face::Sans, 15, color);
    }
    if let Some(label) = status.action {
        action(list, label, false, ACTION, chrome::WHITE, None);
    }
    footer(list, COVER);
}

/// The screen the refresh strip opens: why it cannot run, the refresh running, an ended one
/// once, or the start of a new one.
fn refresh_screen(mesh: &MeshView, memory: &mut Memory) -> Screen {
    if !mesh.radio {
        return Screen::RefreshUnavailable(Unavailable::NoRadio);
    }
    if mesh.group.is_none() {
        return Screen::RefreshUnavailable(Unavailable::NoGroup);
    }
    if mesh.pairing_active() {
        return Screen::RefreshUnavailable(Unavailable::Pairing);
    }
    match mesh.refresh {
        Some(refresh) if refresh.is_listening() => Screen::Refresh(refresh.session),
        Some(refresh) if refresh.session > memory.refresh_seen => {
            memory.refresh_seen = refresh.session;
            Screen::Refresh(refresh.session)
        }
        _ => Screen::RefreshEntry,
    }
}

fn refresh_entry(list: &mut List) {
    let seconds = REFRESH_US / 1_000_000;
    let lasts = format(format_args!(
        "LISTENS FOR {} MIN {} SEC",
        seconds / 60,
        seconds % 60
    ));
    status(
        list,
        &Status {
            caption: "GROUP / REFRESH",
            nav: Some("BACK"),
            glyph: (GROUP, chrome::GRAY),
            heading: ("REFRESH DEVICES", chrome::WHITE),
            lines: &[
                "LISTEN FOR GROUP MEMBERS NOW",
                &lasts,
                "YOU CAN LEAVE THIS SCREEN",
            ],
            action: Some("START REFRESH"),
        },
    );
}

fn heard_line(heard: u32) -> layout::Line {
    match heard.count_ones() {
        1 => format(format_args!("01 DEVICE HEARD")),
        n => format(format_args!("{n:02} DEVICES HEARD")),
    }
}

fn learned_line(learned: u32) -> layout::Line {
    match learned.count_ones() {
        0 => format(format_args!("NO NEW MEMBERS LEARNED")),
        1 => format(format_args!("01 NEW MEMBER LEARNED")),
        n => format(format_args!("{n:02} NEW MEMBERS LEARNED")),
    }
}

/// The refresh `session`: its time left and what it has heard while it runs, or what it found
/// once it ended.
fn refresh(list: &mut List, session: u32, mesh: &MeshView, now: Micros) {
    let shown = mesh.refresh.filter(|refresh| refresh.session == session);
    let (refresh, until) = match shown {
        // Asked for, and not yet taken up: all its time is still to come.
        None => (None, now as At + REFRESH_US),
        Some(refresh) => match refresh.phase {
            RefreshPhase::Listening { until } => (Some(refresh), until),
            _ => return refresh_result(list, &refresh, now),
        },
    };
    let (heard, learned) = refresh.map_or((0, 0), |refresh| (refresh.heard, refresh.learned));
    head(
        list,
        "GROUP",
        "GROUP / REFRESH",
        Some("BACK"),
        WAIT,
        chrome::GRAY,
    );
    list.centred("LISTENING", CENTRE, 167, Face::Kh, 30, chrome::WHITE);
    list.centred(
        "FOR GROUP DEVICES",
        CENTRE,
        210,
        Face::Sans,
        15,
        chrome::WHITE,
    );
    let left = if refresh.is_some() {
        time_left(until, now, list)
    } else {
        time_left(until, until as Micros, list)
    };
    list.fill(LEFT_SLAB, chrome::WHITE);
    list.text(
        Text::new(&left, Face::Kh, 44, chrome::BLACK)
            .at(CENTRE, 258)
            .on(chrome::WHITE),
    );
    list.centred(
        &format(format_args!("TIME LEFT / {}", heard_line(heard))),
        CENTRE,
        337,
        Face::Mono,
        13,
        chrome::GRAY,
    );
    list.centred(
        &learned_line(learned),
        CENTRE,
        362,
        Face::Mono,
        13,
        chrome::GRAY,
    );
    list.centred(
        "CONTINUES WHEN YOU LEAVE",
        CENTRE,
        390,
        Face::Mono,
        13,
        chrome::GRAY,
    );
    footer(list, COVER);
}

fn refresh_result(list: &mut List, refresh: &RefreshView, now: Micros) {
    let (at, interrupted) = match refresh.phase {
        RefreshPhase::Ended { at } => (at, false),
        RefreshPhase::Interrupted { at } => (at, true),
        RefreshPhase::Listening { .. } => return,
    };
    // The caption says how long ago, so an old result does not read as current.
    let (age, next) = words::age(at, now);
    let caption = if now as At - at < 60 * 1_000_000 {
        list.changes_at((at + 60 * 1_000_000).max(0) as Micros);
        format(format_args!("REFRESH / JUST ENDED"))
    } else {
        list.changes_at(next);
        format(format_args!("REFRESH / {age} AGO"))
    };
    let (heard, learned) = (heard_line(refresh.heard), learned_line(refresh.learned));
    let nothing = refresh.heard == 0 && refresh.learned == 0;
    let (heading, lines): (&str, [&str; 3]) = if interrupted {
        (
            "REFRESH STOPPED",
            ["PAIRING TOOK THE RADIO", &heard, &learned],
        )
    } else if nothing {
        (
            "NOTHING NEW",
            [&heard, &learned, "MEMBER AGES ARE UNCHANGED"],
        )
    } else if refresh.learned == 0 {
        ("REFRESH ENDED", [&heard, &learned, "DIRECT AGES UPDATED"])
    } else {
        (
            "REFRESH ENDED",
            [&heard, &learned, "AGES REMAIN IN THE MEMBER LIST"],
        )
    };
    status(
        list,
        &Status {
            caption: &caption,
            nav: None,
            glyph: (GROUP, chrome::GRAY),
            heading: (heading, chrome::WHITE),
            lines: &lines,
            action: Some("VIEW MEMBERS"),
        },
    );
}

fn refresh_unavailable(list: &mut List, why: Unavailable) {
    let (heading, color, glyph, first, action) = match why {
        Unavailable::NoRadio => (
            "NO RADIO",
            chrome::RED,
            FAULT,
            "RADIO DID NOT START",
            "VIEW MEMBERS",
        ),
        Unavailable::NoGroup => (
            "NO GROUP",
            chrome::GRAY,
            GROUP,
            "JOIN OR CREATE A GROUP FIRST",
            "VIEW GROUP",
        ),
        Unavailable::Pairing => (
            "PAIRING ACTIVE",
            chrome::GRAY,
            GROUP,
            "PAIRING IS USING THE RADIO",
            "VIEW MEMBERS",
        ),
    };
    let glyph_color = if color == chrome::RED {
        chrome::RED
    } else {
        chrome::GRAY
    };
    status(
        list,
        &Status {
            caption: "GROUP / REFRESH",
            nav: None,
            glyph: (glyph, glyph_color),
            heading: (heading, color),
            lines: &[first, "REFRESH IS UNAVAILABLE"],
            action: Some(action),
        },
    );
}

/// A founding's wait under way, which leaves this device in no group until it ends.
fn pending_recovery(mesh: &MeshView) -> Option<RecoveryView> {
    mesh.recovery
        .filter(|recovery| mesh.group.is_none() && !recovery.phase.is_final())
}

/// A founding's wait that ended since the user was last shown one.
fn unseen_recovery(mesh: &MeshView, memory: &Memory) -> Option<RecoveryView> {
    mesh.recovery
        .filter(|recovery| recovery.phase.is_final() && recovery.session > memory.recovery_seen)
}

/// The wait the founding `pairing` left, while it is the latest.
fn recovery_of(mesh: &MeshView, pairing: &PairingView) -> Option<RecoveryView> {
    mesh.recovery
        .filter(|recovery| recovery.session == pairing.session)
}

/// The joining device of a founding's wait, by its name once it has sent one.
fn recovery_peer(recovery: &RecoveryView) -> layout::Line {
    match recovery.peer_name {
        Some(name) => format(format_args!("{}", name.as_str())),
        None => format(format_args!("{}", words::mac(&recovery.peer))),
    }
}

/// The group screen while a founding waits: the device it waits for, and the time left.
fn pending_hub(list: &mut List, recovery: &RecoveryView, now: Micros) {
    head(
        list,
        "GROUP",
        "GROUP / LOCAL",
        Some("BACK"),
        WAIT,
        chrome::ORANGE,
    );
    list.centred("GROUP PENDING", CENTRE, 163, Face::Kh, 28, chrome::ORANGE);
    list.centred(
        "NOT STORED ON THIS DEVICE",
        CENTRE,
        207,
        Face::Mono,
        13,
        chrome::GRAY,
    );
    match recovery.peer_name {
        Some(peer) => name(list, peer.as_str(), CENTRE, 237, true, 25),
        None => list.centred(
            "NAME NOT RECEIVED",
            CENTRE,
            240,
            Face::Mono,
            16,
            chrome::GRAY,
        ),
    }
    list.centred(
        &words::mac(&recovery.peer),
        CENTRE,
        274,
        Face::Mono,
        14,
        chrome::GRAY,
    );
    let (line, color) = match recovery.phase {
        RecoveryPhase::Listening { until } => (
            format(format_args!(
                "LISTENING / {} LEFT",
                time_left(until, now, list)
            )),
            chrome::WHITE,
        ),
        RecoveryPhase::SaveFailed { until } => (
            format(format_args!(
                "SAVE FAILED / {} LEFT",
                time_left(until, now, list)
            )),
            chrome::RED,
        ),
        _ => (
            format(format_args!("HEARD / STORING THE GROUP")),
            chrome::WHITE,
        ),
    };
    list.centred(&line, CENTRE, 310, Face::Mono, 16, color);
    action(list, "PAIR", false, PENDING_ACTION, chrome::WHITE, None);
    footer(list, COVER);
}

/// Where a founding's wait is, after the pairing that left it, and once it has ended.
fn recovery_screen(list: &mut List, recovery: &RecoveryView, now: Micros) {
    let who = recovery_peer(recovery);
    let heard = format(format_args!("{who} HEARD ON THE GROUP"));
    let listening = format(format_args!("LISTENING FOR {who}"));
    let count = format(format_args!("{:02} MEMBERS", recovery.count));
    let left = |until: At, list: &mut List| time_left(until, now, list);
    let (heading, color, glyph, lines): (&str, Color, Glyph, [layout::Line; 3]) =
        match recovery.phase {
            RecoveryPhase::Listening { until } => (
                "CHECK MEMBER",
                chrome::ORANGE,
                WAIT,
                [
                    format(format_args!("NO FINAL REPLY RECEIVED")),
                    listening,
                    format(format_args!(
                        "{} LEFT / GROUP NOT STORED",
                        left(until, list)
                    )),
                ],
            ),
            RecoveryPhase::Storing => (
                "CHECK MEMBER",
                chrome::ORANGE,
                WAIT,
                [
                    heard,
                    format(format_args!("STORING THE GROUP")),
                    format(format_args!("GROUP NOT STORED YET")),
                ],
            ),
            RecoveryPhase::SaveFailed { until } => (
                "SAVE FAILED",
                chrome::RED,
                FAULT,
                [
                    heard,
                    format(format_args!("GROUP NOT STORED")),
                    format(format_args!("TRYING AGAIN / {} LEFT", left(until, list))),
                ],
            ),
            RecoveryPhase::Stored => (
                "GROUP STORED",
                chrome::WHITE,
                DONE,
                [
                    heard,
                    count,
                    format(format_args!("MEMBERSHIP SAVED ON THIS DEVICE")),
                ],
            ),
            RecoveryPhase::Expired => (
                "NO GROUP",
                chrome::GRAY,
                GROUP,
                [
                    format(format_args!("WAIT ENDED / NOTHING HEARD")),
                    format(format_args!("NO GROUP WAS STORED")),
                    format(format_args!("CHECK THE OTHER DEVICE")),
                ],
            ),
            RecoveryPhase::NotStored => (
                "SAVE FAILED",
                chrome::RED,
                FAULT,
                [
                    heard,
                    format(format_args!("GROUP NOT STORED")),
                    format(format_args!("THE WAIT HAS ENDED")),
                ],
            ),
        };
    let glyph_color = if color == chrome::WHITE {
        chrome::GRAY
    } else {
        color
    };
    status(
        list,
        &Status {
            caption: "GROUP / RECOVERY",
            nav: None,
            glyph: (glyph, glyph_color),
            heading: (heading, color),
            lines: &[&lines[0], &lines[1], &lines[2]],
            action: Some("VIEW GROUP"),
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::gesture::Drag;

    /// Drags a list of `rows` on MEMBERS up by `by` and lets it settle.
    fn dragged(rows: usize, by: i32) -> i32 {
        let mut scroll = Scroll::default();
        let start = Point::new(233, 380);
        let drag = |up: i32| Drag {
            start,
            current: start - Point::new(0, up),
            velocity: (0.0, 0.0),
        };
        scroll.handle(&GestureEvent::Down(start), rows, 0, MEMBER_ROWS);
        scroll.handle(&GestureEvent::DragStart(drag(10)), rows, 0, MEMBER_ROWS);
        scroll.handle(&GestureEvent::DragMove(drag(by)), rows, 0, MEMBER_ROWS);
        scroll.handle(&GestureEvent::DragEnd(drag(by)), rows, 0, MEMBER_ROWS);
        let mut now = 0;
        while scroll.step(rows, now, MEMBER_ROWS) {
            now += 16_667;
        }
        scroll.offset
    }

    #[test]
    fn a_list_dragged_to_its_end_rests_there() {
        for rows in [2, 8] {
            assert_eq!(
                dragged(rows, 1_000),
                Scroll::max(rows, MEMBER_ROWS),
                "{rows} rows"
            );
        }
        assert_eq!(dragged(2, 10), 0, "a short drag settles back");
        assert_eq!(
            dragged(8, 120),
            ROW,
            "between rows it settles on the nearer"
        );
    }
}
