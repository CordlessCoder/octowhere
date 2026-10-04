//! The UI between input and drawing: which screen shows, what touch does to it, and when it has
//! to be redrawn. The frame loop feeds it readings and draws it, and acts on the effects it
//! returns. It never touches a device, so the host drives it the same way.

use embedded_graphics::prelude::Point;

use super::{
    always_on,
    charging::{self, Charge},
    clock::{ClockState, ZoneMode, ZoneState},
    clock_screen,
    compass::CompassView,
    compass_screen::{self, Accents, DialFootprint, Mode},
    drawer::{self, Drawer},
    ease::Ease,
    events::{self, Events},
    gesture::{Drag, GestureEvent, GestureTracker, Micros, SILENT_LIFT},
    group::{
        self, Flow,
        layout::{Backdrop, List},
        view::{MeshView, Request},
    },
    identity,
    pager::Pager,
    panel::{self, Cell},
    picker::Picker,
    power_off::{self, Answer, PowerOff},
    rest::{self, AlwaysOn, Fade, Rest, Timeout},
    scatter,
    screens::{self, Battery, DEFAULT_BRIGHTNESS, Gnss, PeripheralState, Screen},
    second::{self, Effects, Next, Page},
    sheet::Sheet,
    shift::Shift,
    startup::{self, Phase, Replay, Report, Startup},
};

pub use super::second::Store;
use crate::{
    board,
    chrome::{self, Color, CoverageTarget, Dirty, FontdueRenderer, FontdueRendererCtx},
};

/// What the motion task publishes each sample.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Motion {
    pub compass: CompassView,
}

/// The parts of the sensor task's snapshot the screens show.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Sensors {
    pub clock: ClockState,
    pub zone: ZoneState,
    pub battery: Option<Battery>,
    pub gnss: Gnss,
}

/// One read of the touch controller.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Touch {
    /// Up to two contacts in panel coordinates; none when nothing touches.
    Contacts([Option<Point>; 2]),
    /// The finger lifted, last found at this point.
    Lifted(Point),
    /// A gesture the controller recognised, while [`Stage::watches_for_wake`] has it report
    /// nothing else.
    Gesture(TouchGesture),
    /// The controller recognised a hand covering the screen.
    Cover,
}

/// The gestures the touch controller recognises itself.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TouchGesture {
    Tap,
    SwipeLeft,
    SwipeRight,
    SwipeUp,
    SwipeDown,
}

/// A short or long press of the power key or the BOOT key.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Key {
    Short,
    Long,
}

/// When each of the compass's accents starts after the page settles, and how long each takes.
/// The icon's modules land a row per `ICON_ROW`.
const COMPASS_ENTRY: CompassTimes<Micros> = CompassTimes {
    icon: 95_000,
    caption: 120_000,
    dial: 190_000,
    texture: 0,
};
const ICON_ROW: Micros = 30_000;
const CAPTION_REVEAL: Micros = 120_000;
const DIAL_SWEEP: Micros = 170_000;
const COMPASS_TEXTURE_SETTLE: Micros = 400_000;
/// A heading back from HOLD LEVEL within this shows the dial at once, so tilting through
/// vertical does not replay anything.
const LEVEL_GRACE: Micros = 750_000;

/// The compass page since it settled.
struct CompassSettled {
    times: CompassTimes,
    /// The state shown, with the value it had when that state began.
    shown: Mode,
    /// When the change of state running started, and the state it left.
    change: Option<(Micros, Mode)>,
}

/// When each of the compass's accents started, or starts. `None` shows it whole at once.
#[derive(Clone, Copy, Debug, Default)]
struct CompassTimes<T = Option<Micros>> {
    icon: T,
    caption: T,
    dial: T,
    texture: T,
}

/// Dragging the compass or the clock this fraction of the panel's width takes its accents out
/// entirely.
const SWIPE_FADE: f32 = 0.35;
/// When each of the clock face's accents starts after the page settles. The icon's modules land
/// a row per step, and the reveals run over their durations.
const CLOCK_ENTRY: ClockTimes<Micros> = ClockTimes {
    icon: 60_000,
    label: 100_000,
    plate: 160_000,
    zone: 240_000,
    scatter: 0,
    battery: 160_000,
};
const CLOCK_ICON_BUILD: Micros = 150_000;
const CLOCK_SCATTER_BLOOM: Micros = 120_000;
const PANEL_SCATTER_BLOOM: Micros = 120_000;
const CLOCK_BATTERY_RISE: Micros = 80_000;
const CLOCK_LABEL_REVEAL: Micros = 120_000;
const CLOCK_PLATE_REVEAL: Micros = 120_000;
const CLOCK_ZONE_REVEAL: Micros = 160_000;
/// The hour rail opens with the zone line, a cell each side at a steady rate.
const CLOCK_RAIL_REVEAL: Micros = 400_000;
/// A time that a fix or a zone change replaces, rather than one that ticks, types in again:
/// the hours, minutes and seconds over the first duration, and the date line after a delay.
const CLOCK_TIME_REVEAL: Micros = 180_000;
const CLOCK_DATE_DELAY: Micros = 120_000;
const CLOCK_DATE_REVEAL: Micros = 160_000;
/// How far, in seconds, the shown time may drift from the time elapsed since it last changed
/// before it counts as replaced. Readings are whole seconds, taken up to about 250 ms late.
const CLOCK_TICK_SLACK: i64 = 2;

/// The clock page since it settled.
struct ClockSettled {
    times: ClockTimes,
    shown: clock_screen::Keys,
    /// The local time last shown, in seconds, and when it changed to that.
    time: Option<(i64, Micros)>,
    /// When the time last began to type in again.
    retyped: Option<Micros>,
}

/// When each of the clock face's accents started, or starts. `None` shows it whole at once.
#[derive(Clone, Copy, Debug, Default)]
struct ClockTimes<T = Option<Micros>> {
    icon: T,
    label: T,
    plate: T,
    zone: T,
    scatter: T,
    battery: T,
}

const MINUTE: Micros = 60_000_000;

/// When each of the panel's accents starts after it settles open. Cells follow each other by
/// `PANEL_STAGGER`.
const PANEL_RULES: Micros = 40_000;
const PANEL_RULES_DRAW: Micros = 160_000;
const PANEL_TITLE_REVEAL: Micros = 120_000;
const PANEL_ICON: Micros = 80_000;
const PANEL_INDEX: Micros = 100_000;
const PANEL_INDEX_REVEAL: Micros = 60_000;
const PANEL_NAME: Micros = 120_000;
const PANEL_NAME_REVEAL: Micros = 90_000;
const PANEL_STAGGER: Micros = 30_000;
const PANEL_MARKERS: Micros = 200_000;
const PANEL_HINT: Micros = 260_000;
const PANEL_HINT_REVEAL: Micros = 160_000;
/// A release faster than this, in pixels per second, scrolls the grid on in its direction.
const SNAP_FLICK: f32 = 600.0;

/// How long a toast shows while nothing touches the screen.
const TOAST_FOR: Micros = 5_000_000;

/// How long without a cover report before another cover is a new hand. The controller does not
/// always report the hand lifting, and a held hand repeats its report up to about 180 ms apart.
/// Covering again after lifting took about 350 ms.
const COVER_REARM: Micros = 260_000;

/// Everything that arrived since the last step.
#[derive(Clone, Copy, Debug, Default)]
pub struct Input {
    pub now: Micros,
    /// A fresh touch reading. `None` when the touch controller was not read this step; the
    /// last reading then stands.
    pub touch: Option<Touch>,
    pub motion: Option<Motion>,
    pub sensors: Option<Sensors>,
    /// A part boot brought up, or gave up on, while the start-up sequence shows.
    pub boot: Option<Report>,
    /// A press of the power key. The start-up sequence ignores it.
    pub key: Option<Key>,
    /// A press of the BOOT key. The start-up sequence ignores it.
    pub boot_key: Option<Key>,
}

/// What the frame loop owes after a step. [`Stage::changed`] holds the pixels it changed.
#[derive(Debug, Default)]
pub struct Update {
    /// The compass's calibration should restart.
    pub recalibrate: bool,
    /// A screen that needs fast motion samples shows, or is sliding in.
    pub samples_fast: bool,
    /// Set the display to this level now, without storing it.
    pub brightness: Option<u8>,
    /// Store this setting. The stage already shows it.
    pub store: Option<Store>,
    /// Switch the display on before this frame goes out, or off, after setting any level, once
    /// it has.
    pub display_on: Option<bool>,
    /// Power the board off. It comes once, with the display dark and switching off.
    pub power_off: bool,
    /// Ask the mesh this.
    pub mesh: Option<Request>,
}

/// Where the drag in progress goes, decided when it leaves the tap slop.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Route {
    Pager,
    Sheet,
    Grid,
    /// The drawer's travel up over the faces.
    Drawer,
    Nowhere,
}

/// A toast telling of an event, and when it showed. A toast that woke the screen keeps how it
/// rested, to rest that way again when it times out untouched.
#[derive(Clone, Copy, Debug)]
struct Toast {
    event: events::Id,
    since: Micros,
    woke: Option<power_off::Prior>,
}

/// The same drag seen upside down, so that the drawer's travel up is the panel's travel down.
fn mirrored(drag: &Drag) -> Drag {
    let flip = |point: Point| Point::new(point.x, board::LCD_HEIGHT as i32 - point.y);
    Drag {
        start: flip(drag.start),
        current: flip(drag.current),
        velocity: (drag.velocity.0, -drag.velocity.1),
    }
}

/// The grid's sideways scroll, in pixels, and its motion.
#[derive(Clone, Copy, Debug, Default)]
struct Grid {
    scroll: i32,
    /// The scroll when the current drag started.
    grabbed: Option<i32>,
    /// Easing from one scroll to another, since a time.
    snap: Option<(i32, i32, Micros)>,
}

impl Grid {
    fn finish(&mut self) {
        if let Some((_, to, _)) = self.snap.take() {
            self.scroll = to;
        }
    }

    fn snap_to(&mut self, to: i32, now: Micros) {
        self.snap = (to != self.scroll).then_some((self.scroll, to, now));
    }

    fn step(&mut self, now: Micros) {
        let Some((from, to, start)) = self.snap else {
            return;
        };
        let (scroll, arrived) = Ease::new(from as f32, to, start).at(now);
        self.scroll = libm::roundf(scroll) as i32;
        if arrived {
            self.snap = None;
        }
    }
}

/// What the settled screen showed after a step, for the next step's damage. The screens settle
/// under exclusive conditions, so at most one has a snapshot.
/// An empty list on the heap, built outside the step so its frame never holds one.
#[inline(never)]
fn new_list() -> alloc::boxed::Box<List> {
    alloc::boxed::Box::default()
}

enum Drawn {
    Compass(CompassView, Accents),
    Clock(clock_screen::Face),
    Panel((PeripheralState, i32, panel::Accents)),
    Page(Page, second::Accents, PeripheralState),
    /// The group screens' drawing, which is compared item by item.
    Group(alloc::boxed::Box<List>),
    /// The drawer's, open and at rest, which compares the drawer's lists of this step and the
    /// last.
    Drawer,
}

pub struct Stage {
    screen: Screen,
    raw_touch: [Option<Point>; 2],
    /// When a report last found a finger.
    touched_at: Micros,
    gesture: GestureTracker,
    pager: Pager,
    peripherals: PeripheralState,
    renderer: FontdueRenderer<'static, Color>,
    /// When the last cover report arrived, while the hand that sent it may still be there.
    covered_at: Option<Micros>,
    /// The compass page, while it has settled into view.
    compass_settled: Option<CompassSettled>,
    /// When the heading gave way to HOLD LEVEL, while nothing else has shown since.
    level_since: Option<Micros>,
    accents: Accents,
    fading: bool,
    /// The clock's battery gauge is moving for charging, which needs steps that nothing else
    /// asks for, and never ends while charging.
    gauge_moving: bool,
    charge: Charge,
    /// The resting screens' scatter breath, held while the screen is not awake, and whether a
    /// resting screen shows it, which needs steps that nothing else asks for.
    breath: u8,
    breathing: bool,
    drawn: Option<Drawn>,
    /// A group screen's list from before the last, which the next step fills again. A list is
    /// several kilobytes, so the stack never holds one.
    spare_list: Option<alloc::boxed::Box<List>>,
    /// The pixels the last step changed. Boxed so the frame loop's stack never holds it.
    changed: alloc::boxed::Box<Dirty>,
    dial_footprint: DialFootprint,
    /// When the clock page last settled into view, and what its accents re-reveal on.
    clock_settled: Option<ClockSettled>,
    clock_accents: clock_screen::Accents,
    /// How far the settings panel has come down over the faces.
    sheet: Sheet,
    route: Option<Route>,
    grid: Grid,
    /// When the panel last settled open, while it stays open.
    panel_settled: Option<Micros>,
    panel_accents: panel::Accents,
    /// A screen the panel opened, and when.
    page: Option<(Page, Micros)>,
    /// What the firmware last published of the mesh, which the group screens show.
    mesh: alloc::boxed::Box<MeshView>,
    /// When the group screen showing next changes on its own.
    group_due: Option<Micros>,
    page_accents: second::Accents,
    /// The start-up sequence, until it hands over to the clock face.
    startup: Option<Startup>,
    /// What the sequence showed after the last step.
    startup_view: Option<startup::View>,
    /// What the group screens keep between visits.
    group_memory: group::Memory,
    identity_marks: identity::IdentityMarks,
    /// The brightness the sequence last asked for.
    startup_level: Option<u8>,
    /// When the sequence next changes on its own.
    startup_due: Option<Micros>,
    /// A contact that skipped the sequence, which the faces ignore until it lifts.
    swallowed: bool,
    /// When the last tap came while the screen rested, waiting for the second of a double tap.
    rest_tap: Option<Micros>,
    /// The clock face's time and date type in as its next entry starts.
    clock_types_in: bool,
    /// Whether the clock face's next entry is the start-up's handover, which builds the battery
    /// gauge in rather than raising it.
    clock_builds_gauge: bool,
    /// The start-up once it has finished, which a replay counts its parts from.
    last_boot: Option<Startup>,
    rest: Rest,
    /// When the timeout timer last restarted, and the heading then.
    active_since: Micros,
    heading_anchor: Option<u16>,
    /// The level the display shows while awake, which the dim and the always-on face are taken
    /// from.
    level: u8,
    /// The level last sent to the display.
    shown_level: u8,
    /// A change of level under way.
    fade: Option<Fade>,
    /// No entry starts before this, so one does not run on a panel still waking.
    entry_from: Micros,
    /// What the always-on face showed after the last step, while it shows.
    drawn_always_on: Option<always_on::View>,
    /// The battery the always-on face shows, taken again only when it redraws for a minute, and
    /// the stage's minute when it was, for a face whose time stands still.
    shown_battery: (Option<Battery>, Micros),
    /// The power-off confirmation, over whatever showed when the key opened it.
    power_off: Option<PowerOff>,
    /// [`Update::power_off`] has gone out.
    powered_off: bool,
    /// Where the pixel shift has the picture, and the stage's minute at the last step and the
    /// page the pager last settled on, which the shift moves on.
    shift: Shift,
    minute: Micros,
    settled_page: usize,
    /// What happened at run time, which the drawer lists and the unread arc tells of.
    events: Events,
    /// The drawer an upward drag on a face opens, and its travel up over them.
    drawer: Option<Drawer>,
    drawer_sheet: Sheet,
    /// What the drawer drew at the last step, while it shows.
    drawer_list: Option<alloc::boxed::Box<List>>,
    /// A list to build the drawer into next. Lists are kilobytes, so they are swapped rather than
    /// copied.
    drawer_spare: Option<alloc::boxed::Box<List>>,
    toast: Option<Toast>,
    /// An event to tell of once the finger that was down when it came lifts.
    held_toast: Option<events::Id>,
    /// What lies over the screen after the last step: the toast, and whether the unread arc.
    overlay: Option<(alloc::boxed::Box<List>, bool)>,
    overlay_spare: Option<alloc::boxed::Box<List>>,
}

impl Stage {
    /// A stage that opens on the start-up sequence, for boot to report each part to.
    #[must_use]
    pub fn starting(peripherals: PeripheralState) -> Self {
        Self {
            startup: Some(Startup::new()),
            ..Self::new(peripherals)
        }
    }

    #[must_use]
    pub fn new(peripherals: PeripheralState) -> Self {
        Self {
            screen: Screen::ALL[0],
            raw_touch: [None; 2],
            touched_at: 0,
            gesture: GestureTracker::default(),
            pager: Pager::new(0, Screen::ALL.len(), board::LCD_WIDTH as i32),
            renderer: FontdueRenderer::new(
                FontdueRendererCtx::new_rc(),
                20,
                chrome::WHITE,
                chrome::FONTS,
            ),
            covered_at: None,
            compass_settled: None,
            level_since: None,
            accents: Accents::FULL,
            fading: false,
            gauge_moving: false,
            charge: Charge::default(),
            breath: u8::MAX,
            breathing: false,
            drawn: None,
            spare_list: None,
            changed: alloc::boxed::Box::new(Dirty::new()),
            dial_footprint: DialFootprint::default(),
            clock_settled: None,
            clock_accents: clock_screen::Accents::FULL,
            sheet: Sheet::new(board::LCD_HEIGHT as i32),
            route: None,
            grid: Grid::default(),
            panel_settled: None,
            panel_accents: panel::Accents::HIDDEN,
            page: None,
            mesh: alloc::boxed::Box::default(),
            group_due: None,
            page_accents: second::Accents::FULL,
            startup: None,
            startup_view: None,
            group_memory: group::Memory::default(),
            identity_marks: identity::IdentityMarks::default(),
            startup_level: None,
            startup_due: None,
            swallowed: false,
            rest_tap: None,
            clock_types_in: false,
            clock_builds_gauge: false,
            last_boot: None,
            rest: Rest::Awake,
            active_since: 0,
            heading_anchor: None,
            level: peripherals.brightness,
            shown_level: peripherals.brightness,
            fade: None,
            entry_from: 0,
            drawn_always_on: None,
            shown_battery: (None, 0),
            power_off: None,
            powered_off: false,
            shift: Shift::default(),
            minute: 0,
            settled_page: 0,
            events: Events::default(),
            drawer: None,
            drawer_sheet: Sheet::new(board::LCD_HEIGHT as i32),
            drawer_list: None,
            drawer_spare: None,
            toast: None,
            held_toast: None,
            overlay: None,
            overlay_spare: None,
            peripherals,
        }
    }

    /// What happened at run time.
    #[must_use]
    pub fn events(&self) -> &Events {
        &self.events
    }

    /// The drawer, while it shows or moves.
    #[must_use]
    pub fn drawer(&self) -> Option<&Drawer> {
        self.drawer.as_ref()
    }

    /// The event a toast tells of, while one shows.
    #[must_use]
    pub fn toast(&self) -> Option<events::Id> {
        self.toast.map(|toast| toast.event)
    }

    /// How far the picture is moved on the panel against burn-in. The display moves it; touch
    /// arrives where the panel was touched, and the stage takes this off. The start-up, and a
    /// replay of it, show unmoved.
    #[must_use]
    pub fn shift(&self) -> Point {
        if self.startup.is_some() {
            Point::zero()
        } else {
            self.shift.offset()
        }
    }

    /// Where the picture is in pixel shift's round. The start-up shows unmoved whatever it says.
    #[must_use]
    pub fn shift_state(&self) -> Shift {
        self.shift
    }

    /// Holds the picture at `shift::POSITIONS[index]`, or with `None` lets the stage move it
    /// again, for a simulator to look at one position. The display must flush in full after a
    /// change, as it does whenever the offset changes.
    pub fn pin_shift(&mut self, index: Option<usize>, now: Micros) {
        self.shift.pin(index, now);
    }

    /// Takes what the firmware has published of the mesh.
    pub fn set_mesh(&mut self, mesh: MeshView) {
        self.update_mesh(|view| *view = mesh);
    }

    /// Lets `write` change the mesh's view where the stage keeps it, and returns what it
    /// returns.
    pub fn update_mesh<R>(&mut self, write: impl FnOnce(&mut MeshView) -> R) -> R {
        let result = write(&mut self.mesh);
        self.peripherals.members = self.mesh.group.as_ref().map(|group| group.count() as u8);
        self.peripherals.name = self.mesh.name;
        result
    }

    #[must_use]
    pub fn mesh(&self) -> &MeshView {
        &self.mesh
    }

    /// Draws every glyph into `buffer` from now on: one [`chrome::raster_buffer`] made before the
    /// heap fills, so that it never has to grow.
    pub fn use_raster(&mut self, buffer: alloc::vec::Vec<f32>) {
        self.renderer.ctx.borrow_mut().use_raster(buffer);
    }

    /// Whether the start-up sequence still shows.
    #[must_use]
    pub fn starting_up(&self) -> bool {
        self.startup.is_some()
    }

    /// When the stage next changes on its own, if nothing arrives first and it is not
    /// [animating](Self::is_animating), which wants a step as soon as possible.
    #[must_use]
    pub fn next_change(&self) -> Option<Micros> {
        let rest = match self.rest {
            Rest::Awake if self.startup.is_none() => self
                .peripherals
                .timeout
                .duration()
                .map(|timeout| self.active_since + timeout),
            Rest::Dimmed { since } => Some(since + rest::DIM_HOLD),
            Rest::Darkening { since } => Some(since + rest::OFF_FADE),
            _ => None,
        };
        let power_off = self
            .power_off
            .as_ref()
            .and_then(|power_off| match power_off.confirmed() {
                Some(confirmed) => (!self.powered_off).then_some(confirmed + power_off::FADE),
                None => power_off.deadline(),
            });
        let silent_lift = self.raw_touch[0].map(|_| self.touched_at + SILENT_LIFT);
        let page = self.page.as_ref().and_then(|(page, _)| page.next_change());
        let group = self.page.as_ref().and(self.group_due);
        let toast = self.toast.map(|toast| toast.since + TOAST_FOR);
        let drawer = self.drawer_list.as_ref().and_then(|list| list.due());
        let overlay = self.overlay.as_ref().and_then(|(list, _)| list.due());
        [
            self.startup_due,
            rest,
            power_off,
            self.gesture.lift_due(),
            silent_lift,
            page,
            group,
            toast,
            drawer,
            overlay,
        ]
        .into_iter()
        .flatten()
        .min()
    }

    /// The power-off confirmation, while it shows.
    #[must_use]
    pub fn power_off(&self) -> Option<&PowerOff> {
        self.power_off.as_ref()
    }

    /// Where the screen is on its way to rest.
    #[must_use]
    pub fn rest(&self) -> Rest {
        self.rest
    }

    /// The level last sent to the display.
    #[must_use]
    pub fn shown_level(&self) -> u8 {
        self.shown_level
    }

    /// Jumps to `screen` as though the pager had come to rest on it, with the panel closed.
    pub fn show(&mut self, screen: Screen) {
        self.sheet.set(false);
        self.page = None;
        self.panel_settled = None;
        self.face(screen);
    }

    /// Puts `screen` under the panel, to show when the panel next moves off it.
    fn face(&mut self, screen: Screen) {
        let page = Screen::ALL
            .iter()
            .position(|&each| each == screen)
            .expect("every screen is in the ring");
        self.pager = Pager::new(page, Screen::ALL.len(), board::LCD_WIDTH as i32);
        self.screen = screen;
        self.compass_settled = None;
        self.level_since = None;
        self.clock_settled = None;
        // An open panel or page keeps its snapshot: the face changes under it.
        if matches!(self.drawn, Some(Drawn::Compass(..) | Drawn::Clock(_))) {
            self.drawn = None;
        }
    }

    /// How far the panel has come down over the faces: 0 closed, the panel's height open.
    #[must_use]
    pub fn panel_offset(&self) -> i32 {
        self.sheet.offset()
    }

    /// The grid's sideways scroll: 0 with the first two columns in view.
    #[must_use]
    pub fn panel_scroll(&self) -> i32 {
        self.grid.scroll
    }

    /// How far the panel's accents have come in.
    #[must_use]
    pub fn panel_accents(&self) -> panel::Accents {
        self.panel_accents
    }

    /// The screen the panel opened, if one shows.
    #[must_use]
    pub fn page(&self) -> Option<&Page> {
        self.page.as_ref().map(|(page, _)| page)
    }

    /// The text a group screen showed at the last step, for tests and tools to read.
    pub fn group_text(&self) -> impl Iterator<Item = &str> {
        let items = match &self.drawn {
            Some(Drawn::Group(list)) => list.items(),
            _ => &[],
        };
        items.iter().filter_map(|item| match &item.shape {
            group::layout::Shape::Text(text) => Some(text.text.as_str()),
            _ => None,
        })
    }

    /// How far a face has moved off the panel, sideways or down.
    fn face_offset(&self) -> i32 {
        if self.page.is_some() {
            return self.sheet.height();
        }
        self.pager.view().offset.abs().max(self.sheet.offset())
    }

    /// The pixels the last [`step`](Self::step) changed.
    #[must_use]
    pub fn changed(&self) -> &Dirty {
        &self.changed
    }

    #[must_use]
    pub fn screen(&self) -> Screen {
        self.screen
    }

    /// How far the compass page's accents have come in.
    #[must_use]
    pub fn accents(&self) -> Accents {
        self.accents
    }

    /// How far the clock face's accents have come in.
    #[must_use]
    pub fn clock_accents(&self) -> clock_screen::Accents {
        self.clock_accents
    }

    #[must_use]
    pub fn peripherals(&self) -> &PeripheralState {
        &self.peripherals
    }

    /// Where the last touch reading put the first finger, if one was down.
    #[must_use]
    pub fn contact(&self) -> Option<Point> {
        self.raw_touch[0]
    }

    /// The screen rests on the always-on face or dark, and wakes only on a double tap, so the
    /// touch controller should report only the gestures it recognises. The contacts it would
    /// report otherwise do nothing here.
    #[must_use]
    pub fn watches_for_wake(&self) -> bool {
        matches!(self.rest, Rest::AlwaysOn | Rest::Off)
    }

    /// A finger is down, or the gesture tracker has not yet seen it lift. Touch should be read
    /// again soon even without an interrupt.
    #[must_use]
    pub fn in_contact(&self) -> bool {
        self.raw_touch[0].is_some() || self.gesture.in_contact()
    }

    /// A page slide or a fade is under way, so the next step should come without waiting for
    /// input.
    #[must_use]
    pub fn is_animating(&self) -> bool {
        self.is_changing() || self.gauge_moving || self.breathing
    }

    /// As [`is_animating`](Self::is_animating), but for the charging gauge, which never stops
    /// while charging.
    #[must_use]
    pub fn is_changing(&self) -> bool {
        self.pager.is_moving()
            || self.sheet.is_moving()
            || self.drawer_sheet.is_moving()
            || self.drawer.as_ref().is_some_and(Drawer::is_moving)
            || self.grid.snap.is_some()
            || self.fading
            || self.fade.is_some()
    }

    pub fn step(&mut self, input: Input) -> Update {
        let update = self.advance(input);
        if let Some(level) = update.brightness {
            self.shown_level = level;
        }
        update
    }

    fn advance(&mut self, input: Input) -> Update {
        let Input {
            now,
            touch,
            motion,
            sensors,
            boot,
            key,
            boot_key,
        } = input;
        let offset = self.shift();
        let touch = touch.map(|touch| match touch {
            Touch::Contacts(contacts) => Touch::Contacts(contacts.map(|p| p.map(|p| p - offset))),
            Touch::Lifted(point) => Touch::Lifted(point - offset),
            other => other,
        });
        self.changed.clear();
        self.fading = false;
        self.gauge_moving = false;
        self.breathing = false;
        if self.rest == Rest::Awake {
            self.breath = scatter::breath(now);
        }
        let mut update = Update::default();
        // Set where a change needs the whole panel redrawn. The settled screens work out their
        // own damage instead.
        let mut full = false;

        if let Some(motion) = motion {
            self.peripherals.compass = motion.compass;
            full |= self.screen == Screen::Compass;
        }
        if let Some(sensors) = sensors {
            self.peripherals.clock = self.peripherals.clock.read(sensors.clock, sensors.zone);
            self.peripherals.battery = sensors.battery;
            self.charge.read(sensors.battery, now);
            self.peripherals.gnss = sensors.gnss;
            full |= self.screen == Screen::Clock;
            if let Some(id) = self.events.gnss(&sensors.gnss.health, now) {
                self.announce(id, now, &mut update);
            }
        }
        if let Some(id) = self.events.refresh(self.mesh.refresh.as_ref(), now) {
            self.announce(id, now, &mut update);
        }

        // A finger that has gone quiet stands in for the lift report the controller never sent.
        let touch = touch.or_else(|| {
            (self.raw_touch[0].is_some() && now.saturating_sub(self.touched_at) >= SILENT_LIFT)
                .then_some(Touch::Contacts([None; 2]))
        });
        if let Some(touch) = touch {
            self.raw_touch = match touch {
                Touch::Contacts(contacts) => contacts,
                Touch::Lifted(_) | Touch::Gesture(_) | Touch::Cover => [None; 2],
            };
            if self.raw_touch[0].is_some() {
                self.touched_at = now;
            }
        }
        // The timer does not run during the start-up; it restarts when the clock face takes over.
        if self.startup.is_some() {
            self.restart(now);
            if self.step_startup(now, boot, touch.is_some(), &mut update) {
                return update;
            }
        }
        if let Some(key) = key {
            self.press(key, now, &mut update);
        }
        if let Some(key) = boot_key {
            self.press_boot(key);
        }
        if self.power_off.is_some() {
            self.step_power_off(now, touch, &mut update);
            return update;
        }
        let contact = touch.is_some() && self.raw_touch[0].is_some();
        let mut touch = touch;
        if self.step_rest(now, contact, &mut touch, &mut update) {
            return update;
        }
        self.step_fade(now, &mut update);
        if self.raw_touch[0].is_none() {
            self.swallowed = false;
        }

        let previous = (
            self.pager.view(),
            self.sheet.offset(),
            self.grid.scroll,
            self.page.is_some(),
            self.drawer_sheet.offset(),
        );
        let event = self.gesture(touch, now);
        let mut effects = Effects::default();
        self.step_toast(&event, now, &mut update);
        if self.toast_tapped(&event, now) {
            // The tap opened the drawer at its event, and does nothing else.
        } else if self.drawer.is_some() {
            self.route_drawer(&event, now, &mut effects);
        } else {
            self.route_event(&event, now, &mut effects, &mut update);
        }
        if let Some((Page::Group(flow), _)) = &mut self.page {
            let (exit, moving) = flow.step(&self.mesh, now, &mut effects.mesh);
            self.fading |= moving;
            if exit == group::Exit::Panel {
                self.page = None;
            }
        }

        if let Some(touch) = touch
            && self.new_cover(touch, now)
        {
            self.go_home(now, &mut effects);
        }
        self.apply(effects, &mut update);

        self.pager.step(now);
        self.drawer_sheet.step(now);
        if let Some(drawer) = &mut self.drawer {
            drawer.step(&self.events, now);
            if self.drawer_sheet.is_closed() {
                self.drawer = None;
                self.drawer_list = None;
            }
        }
        let (was_open, was_closed) = (self.sheet.is_open(), self.sheet.is_closed());
        self.sheet.step(now);
        self.grid.step(now);
        if let Some((page, _)) = &mut self.page {
            self.fading |= page.step(now, &self.peripherals, &self.renderer);
        }
        // A panel that springs back without closing keeps its entry: the accents it faded on the
        // way out come back with its offset.
        if self.sheet.is_open() && self.panel_settled.is_none() {
            self.panel_settled = Some(now);
        }
        if self.sheet.is_open() && !was_open {
            // The face under the panel starts its entry again when the panel leaves it.
            self.compass_settled = None;
            self.clock_settled = None;
        }
        if self.sheet.is_closed() {
            self.panel_settled = None;
        }
        // The shift moves where the whole picture changes anyway: a new page settling, the
        // panel settling open or shut, and failing those a minute's change once it is due.
        let page = self.pager.view().page;
        let settled = !self.pager.is_moving() && page != self.settled_page;
        if !self.pager.is_moving() {
            self.settled_page = page;
        }
        let panel = (self.sheet.is_open() && !was_open) || (self.sheet.is_closed() && !was_closed);
        let minute = now / MINUTE;
        if settled || panel || (minute != self.minute && self.shift.is_due(now)) {
            self.shift.advance(now);
        }
        self.minute = minute;

        let view = self.pager.view();
        self.screen = Screen::ALL[view.page];
        let face_shows = self.page.is_none() && !self.sheet.is_open();
        let samples_fast = |screen: Screen| screen == Screen::Compass;
        update.samples_fast = face_shows
            && (samples_fast(self.screen)
                || view
                    .neighbour
                    .is_some_and(|(page, _)| samples_fast(Screen::ALL[page])));

        let accents = self.compass_accents(now);
        if accents != self.accents {
            self.accents = accents;
            full = true;
        }
        let clock_accents = self.advance_clock_accents(now);
        if clock_accents != self.clock_accents {
            self.clock_accents = clock_accents;
            full = true;
        }
        self.panel_accents = self.advance_panel_accents(now);
        self.page_accents = self
            .page
            .as_ref()
            .map_or(second::Accents::FULL, |(_, opened)| {
                second::Accents::at(*opened, now)
            });
        self.fading |= self.page_accents != second::Accents::FULL;

        let current = (
            view,
            self.sheet.offset(),
            self.grid.scroll,
            self.page.is_some(),
            self.drawer_sheet.offset(),
        );
        let grid_only = current.2 != previous.2
            && (current.0, current.1, current.3, current.4)
                == (previous.0, previous.1, previous.3, previous.4);
        full |= current != previous && !grid_only;
        let group_list = match &self.page {
            Some((Page::Group(flow), _)) => {
                let mut list = self.spare_list.take().unwrap_or_else(new_list);
                flow.view(
                    &mut list,
                    &self.mesh,
                    &self.group_memory,
                    now,
                    &self.renderer,
                );
                Some(list)
            }
            _ => None,
        };
        self.group_due = group_list.as_ref().and_then(|list| list.due());
        let drawer_before = self.build_drawer(now);
        self.track_damage(full, group_list, drawer_before);
        self.track_overlay(now);

        let moved = current != previous
            || self.pager.is_moving()
            || self.sheet.is_moving()
            || self.drawer_sheet.is_moving()
            || self.grid.snap.is_some();
        let cover = touch == Some(Touch::Cover);
        // A pairing holds the screen awake until it ends.
        let pairing =
            matches!(&self.page, Some((Page::Group(flow), _)) if flow.holds_awake(&self.mesh));
        if contact || cover || moved || pairing || self.heading_moved(face_shows) {
            self.restart(now);
        }
        if let (Rest::Awake, Some(timeout)) = (self.rest, self.peripherals.timeout.duration())
            && now.saturating_sub(self.active_since) >= timeout
        {
            self.rest = Rest::Dimmed { since: now };
            self.fade_to(rest::dim_level(self.level), rest::DIM_FADE, now);
        }
        update
    }

    /// Tells the user of event `id`: a toast over whatever shows, which wakes a resting screen.
    /// A finger already down keeps the screen as it is until it lifts. Nothing shows during the
    /// start-up or the power-off, or over the drawer, whose list shows the event itself.
    fn announce(&mut self, id: events::Id, now: Micros, update: &mut Update) {
        if self.startup.is_some() || self.power_off.is_some() || self.drawer.is_some() {
            return;
        }
        if self.raw_touch[0].is_some() || self.gesture.in_contact() {
            self.held_toast = Some(id);
            return;
        }
        let woke = (self.rest != Rest::Awake).then_some(power_off::Prior {
            rest: self.rest,
            level: self.shown_level,
            at: now,
        });
        // A toast already up keeps the rest it woke the screen from.
        let woke = self.toast.and_then(|toast| toast.woke).or(woke);
        self.wake_for_toast(now, update);
        self.toast = Some(Toast {
            event: id,
            since: now,
            woke,
        });
    }

    /// Brings a resting screen up to its level for a toast, over whatever it showed.
    fn wake_for_toast(&mut self, now: Micros, update: &mut Update) {
        match self.rest {
            Rest::Awake => return,
            Rest::Dimmed { .. } | Rest::Darkening { .. } => {}
            Rest::AlwaysOn | Rest::Off => {
                if self.rest == Rest::Off {
                    update.display_on = Some(true);
                }
                self.drawn_always_on = None;
                self.changed.make_full();
            }
        }
        self.rest = Rest::Awake;
        self.restart(now);
        self.level = self.peripherals.brightness;
        self.fade_to(self.level, rest::WAKE_FADE, now);
    }

    /// Shows a toast held for a finger once it lifts, and ends one that has shown long enough,
    /// resting the screen again as it was. Any touch ends it, keeping the screen awake.
    fn step_toast(&mut self, event: &GestureEvent, now: Micros, update: &mut Update) {
        if !self.in_contact()
            && let Some(id) = self.held_toast.take()
        {
            self.announce(id, now, update);
        }
        let Some(toast) = self.toast else {
            return;
        };
        if matches!(event, GestureEvent::Down(_)) {
            if !matches!(event, GestureEvent::Down(point) if self.toast_area().contains(*point)) {
                self.toast = None;
            }
            return;
        }
        if now >= toast.since + TOAST_FOR {
            self.toast = None;
            if let Some(prior) = toast.woke {
                self.rest_again(prior, now, update);
                self.step_always_on(now);
            }
        }
    }

    /// Where the toast shows: lower on the faces, or compact at the top while a keyboard shows.
    fn toast_area(&self) -> embedded_graphics::primitives::Rectangle {
        if self.typing() {
            drawer::TOAST_COMPACT
        } else {
            drawer::TOAST
        }
    }

    fn typing(&self) -> bool {
        matches!(&self.page, Some((Page::Group(flow), _)) if flow.typing())
    }

    /// Opens the drawer at the toast's event when a tap lands on the toast.
    fn toast_tapped(&mut self, event: &GestureEvent, now: Micros) -> bool {
        let (Some(toast), GestureEvent::Tap(point)) = (self.toast, event) else {
            return false;
        };
        if !self.toast_area().contains(*point) {
            return false;
        }
        self.toast = None;
        self.events.read(toast.event);
        self.drawer = Some(Drawer::at_event(toast.event));
        self.drawer_sheet.go(true, now);
        true
    }

    /// Sends a gesture to the drawer, or to its travel while a drag moves it.
    fn route_drawer(&mut self, event: &GestureEvent, now: Micros, effects: &mut Effects) {
        if self.route == Some(Route::Drawer) {
            match *event {
                GestureEvent::DragMove(drag) => self.drawer_sheet.drag(&mirrored(&drag)),
                GestureEvent::DragEnd(drag) => {
                    self.route = None;
                    self.drawer_sheet.release(&mirrored(&drag), now);
                }
                _ => {}
            }
            return;
        }
        if !self.drawer_sheet.is_open() {
            if matches!(event, GestureEvent::Down(_)) {
                self.drawer_sheet.finish();
            }
            return;
        }
        let Some(drawer) = &mut self.drawer else {
            return;
        };
        match drawer.handle(event, &mut self.events, now) {
            drawer::Exit::Stay => {}
            drawer::Exit::Close => self.drawer_sheet.go(false, now),
            drawer::Exit::Pull => {
                if let GestureEvent::DragStart(drag) = event {
                    self.route = Some(Route::Drawer);
                    self.drawer_sheet.grab(&mirrored(drag));
                }
            }
            drawer::Exit::Members => {
                self.close_drawer();
                if let Some((mut page, _)) = self.page.take() {
                    if let Page::Group(flow) = &mut page {
                        flow.interrupt(&self.mesh, &mut effects.mesh);
                    }
                    page.discard(effects);
                }
                self.sheet.set(true);
                self.page = Some((Page::Group(Flow::members()), now));
            }
        }
    }

    /// Shuts the drawer at once.
    fn close_drawer(&mut self) {
        if self.drawer.take().is_some() {
            self.drawer_list = None;
            self.drawer_sheet.set(false);
            self.changed.make_full();
        }
    }

    /// Builds what the drawer shows this step, while it shows or moves, and returns what it
    /// showed the step before.
    fn build_drawer(&mut self, now: Micros) -> Option<alloc::boxed::Box<List>> {
        let Some(drawer) = &self.drawer else {
            return self.drawer_list.take();
        };
        let mut list = self.drawer_spare.take().unwrap_or_else(new_list);
        list.clear();
        drawer.view(
            &mut list,
            &drawer::Context {
                events: &self.events,
                gnss: &self.peripherals.gnss,
                mesh: &self.mesh,
                now,
                breath: self.breath,
                font: &self.renderer,
            },
        );
        self.breathing |= self.rest == Rest::Awake;
        self.drawer_list.replace(list)
    }

    /// Works out what lies over the screen this step, the toast and the unread arc, and damages
    /// where that changed.
    fn track_overlay(&mut self, now: Micros) {
        let covered = self.startup.is_some()
            || self.power_off.is_some()
            || (self.drawer_sheet.is_open() && self.drawer.is_some());
        let (before, arc_before) = match self.overlay.take() {
            Some((list, arc)) => (Some(list), arc),
            None => (None, false),
        };
        let mut list = self.overlay_spare.take().unwrap_or_else(new_list);
        list.clear();
        list.set_backdrop(Backdrop::None);
        let arc = !covered && self.events.unread() > 0;
        if !covered
            && let Some(toast) = self.toast
            && let Some(event) = self.events.get(toast.event)
        {
            drawer::toast(
                &mut list,
                event,
                &drawer::Context {
                    events: &self.events,
                    gnss: &self.peripherals.gnss,
                    mesh: &self.mesh,
                    now,
                    breath: self.breath,
                    font: &self.renderer,
                },
                self.typing(),
            );
        }
        if arc_before != arc {
            self.changed.add(crate::ui::stroke::arc_bounds(
                drawer::UNREAD_ARC.center,
                drawer::UNREAD_ARC.radius,
                drawer::UNREAD_ARC.width,
                drawer::UNREAD_ARC.span,
            ));
        }
        let toast_changed = match &before {
            Some(before) => **before != *list,
            None => !list.items().is_empty(),
        };
        if toast_changed {
            // A toast that comes, goes or changes uncovers whatever lies under it.
            self.changed.add(drawer::TOAST);
            self.changed.add(drawer::TOAST_COMPACT);
        }
        self.overlay = Some((list, arc));
        self.overlay_spare = before;
    }

    /// Whether `touch` is a hand newly covering the screen, rather than one still held there.
    fn new_cover(&mut self, touch: Touch, now: Micros) -> bool {
        let fresh = self
            .covered_at
            .is_none_or(|at| now.saturating_sub(at) >= COVER_REARM);
        match touch {
            Touch::Cover => self.covered_at = Some(now),
            // A finger means the hand has gone. A report without one does not: the controller
            // sends unreadable reports while a hand is held, and those arrive as no contacts.
            Touch::Contacts(contacts) if contacts.iter().any(Option::is_some) => {
                self.covered_at = None;
            }
            Touch::Contacts(_) | Touch::Lifted(_) | Touch::Gesture(_) => {}
        }
        touch == Touch::Cover && fresh
    }

    /// Acts on the power key. A short press rests the screen, or wakes it, and cancels the
    /// power-off confirmation; a long one wakes the screen onto the confirmation.
    fn press(&mut self, key: Key, now: Micros, update: &mut Update) {
        if self
            .power_off
            .as_ref()
            .is_some_and(|power_off| power_off.confirmed().is_some())
        {
            return;
        }
        // Either press ends a pairing before it rests the screen or asks to power off.
        if let Some((Page::Group(flow), _)) = &mut self.page {
            flow.interrupt(&self.mesh, &mut update.mesh);
        }
        let (prior, prior_level) = (self.rest, self.shown_level);
        let resting = prior != Rest::Awake;
        if resting {
            self.wake_by_key(now, update);
        }
        match key {
            Key::Short => {
                if self.power_off.take().is_some() {
                    self.forget_drawn();
                }
                if !resting {
                    self.sleep(update);
                }
            }
            Key::Long if self.power_off.is_none() => {
                self.route = None;
                let prior = power_off::Prior {
                    rest: prior,
                    level: prior_level,
                    at: now,
                };
                let shown = match prior.rest {
                    Rest::AlwaysOn | Rest::Off => self.entry_from + rest::WAKE_FADE,
                    Rest::Dimmed { .. } | Rest::Darkening { .. } => now + rest::WAKE_FADE,
                    Rest::Awake => now,
                };
                self.power_off = Some(PowerOff::new(shown, prior));
                self.changed.make_full();
            }
            Key::Long => {}
        }
    }

    /// Acts on the BOOT key, which has no behaviour designed yet.
    #[expect(clippy::unused_self, reason = "the BOOT key's behaviour goes here")]
    fn press_boot(&self, _key: Key) {}

    /// Wakes a screen on its way to rest, or resting, as a contact would.
    fn wake_by_key(&mut self, now: Micros, update: &mut Update) {
        match self.rest {
            Rest::Awake => {}
            Rest::Dimmed { .. } | Rest::Darkening { .. } => {
                self.rest = Rest::Awake;
                self.fade_to(self.level, rest::WAKE_FADE, now);
            }
            Rest::AlwaysOn | Rest::Off => self.wake(now, update),
        }
    }

    /// Rests the screen at once, on the always-on face or off, without the timeout's dim.
    fn sleep(&mut self, update: &mut Update) {
        self.route = None;
        self.close_drawer();
        self.toast = None;
        self.fade = None;
        self.level = self.peripherals.brightness;
        if self.peripherals.always_on.is_on() {
            self.rest = Rest::AlwaysOn;
            self.drawn_always_on = None;
            update.brightness = Some(self.peripherals.always_on.level(self.level));
        } else {
            self.rest = Rest::Off;
            update.brightness = Some(0);
            update.display_on = Some(false);
        }
    }

    /// Steps the power-off confirmation, which takes every touch while it shows and holds the
    /// timeout.
    fn step_power_off(&mut self, now: Micros, touch: Option<Touch>, update: &mut Update) {
        self.restart(now);
        // After the answer, so that a cancel turns a fade back before it moves on.
        self.answer_power_off(now, touch, update);
        self.step_fade(now, update);
    }

    fn answer_power_off(&mut self, now: Micros, touch: Option<Touch>, update: &mut Update) {
        if self.raw_touch[0].is_none() {
            self.swallowed = false;
        }
        let event = self.gesture(touch, now);
        let cover = touch.is_some_and(|touch| self.new_cover(touch, now));
        let powered_off = self.powered_off;
        let Some(power_off) = &mut self.power_off else {
            return;
        };
        if let Some(confirmed) = power_off.confirmed() {
            if !powered_off && now >= confirmed + power_off::FADE {
                self.powered_off = true;
                update.power_off = true;
                update.display_on = Some(false);
            }
            return;
        }
        if self.raw_touch[0].is_some() {
            power_off.touched(now);
        }
        let before = power_off.clone();
        let answer = if cover || power_off.deadline().is_some_and(|deadline| now >= deadline) {
            Answer::Cancel
        } else {
            power_off.handle(&event, now)
        };
        let changed = *power_off != before;
        let prior = power_off.prior();
        match answer {
            Answer::Stay if changed => self.changed.add(power_off::SLIDER),
            Answer::Stay => {}
            Answer::Cancel => {
                self.power_off = None;
                self.forget_drawn();
                self.rest_again(prior, now, update);
                self.step_always_on(now);
            }
            Answer::Confirm => {
                self.fade_to(0, power_off::FADE, now);
                self.changed.make_full();
            }
        }
    }

    /// Puts the screen back the way it showed before the power key woke it. A dim or darkening
    /// comes back at the level it had reached and carries on with the time it had left.
    fn rest_again(&mut self, prior: power_off::Prior, now: Micros, update: &mut Update) {
        let (to, fade, elapsed) = match prior.rest {
            Rest::Awake => return,
            Rest::AlwaysOn | Rest::Off => return self.sleep(update),
            Rest::Dimmed { since } => (
                rest::dim_level(self.level),
                rest::DIM_FADE,
                prior.at - since,
            ),
            Rest::Darkening { since } => (0, rest::OFF_FADE, prior.at - since),
        };
        self.rest = match prior.rest {
            Rest::Dimmed { .. } => Rest::Dimmed {
                since: now - elapsed,
            },
            _ => Rest::Darkening {
                since: now - elapsed,
            },
        };
        self.fade = None;
        self.shown_level = prior.level;
        update.brightness = Some(prior.level);
        if elapsed < fade {
            self.fade_to(to, fade - elapsed, now);
        }
    }

    /// Forgets what each screen showed, so the next step redraws in full.
    fn forget_drawn(&mut self) {
        self.drawn = None;
        self.changed.make_full();
    }

    /// Restarts the timeout timer.
    fn restart(&mut self, now: Micros) {
        self.active_since = now;
        self.heading_anchor = self.peripherals.compass.heading_decidegrees;
    }

    /// Whether the compass shows and its heading has turned far enough to count as use.
    fn heading_moved(&mut self, face_shows: bool) -> bool {
        let heading = self.peripherals.compass.heading_decidegrees;
        if !face_shows || self.screen != Screen::Compass {
            return false;
        }
        match (self.heading_anchor, heading) {
            (Some(anchor), Some(heading)) => {
                rest::heading_apart(anchor, heading) > rest::HEADING_RESTART
            }
            (None, Some(_)) => {
                self.heading_anchor = heading;
                false
            }
            _ => false,
        }
    }

    /// Wakes the screen on a contact, and takes a dimmed screen on to rest. Returns whether the
    /// screen rests, so nothing else steps. A cover while dimmed is dropped from `touch`.
    fn step_rest(
        &mut self,
        now: Micros,
        contact: bool,
        touch: &mut Option<Touch>,
        update: &mut Update,
    ) -> bool {
        let double_tapped = self.watches_for_wake() && self.double_tapped(*touch, now);
        match self.rest {
            Rest::Awake => return false,
            // The contact that lifts the dim does nothing else.
            Rest::Dimmed { .. } | Rest::Darkening { .. } if contact => {
                self.rest = Rest::Awake;
                self.swallowed = true;
                self.fade_to(self.level, rest::WAKE_FADE, now);
                return false;
            }
            Rest::Dimmed { since } if now.saturating_sub(since) >= rest::DIM_HOLD => {
                if self.peripherals.always_on.is_on() {
                    self.rest = Rest::AlwaysOn;
                    self.drawn_always_on = None;
                    self.fade = None;
                    update.brightness = Some(self.peripherals.always_on.level(self.level));
                } else {
                    self.rest = Rest::Darkening { since: now };
                    self.fade_to(0, rest::OFF_FADE, now);
                    return false;
                }
            }
            Rest::Darkening { since } if now.saturating_sub(since) >= rest::OFF_FADE => {
                self.rest = Rest::Off;
                self.fade = None;
                update.brightness = Some(0);
                update.display_on = Some(false);
            }
            Rest::Dimmed { .. } | Rest::Darkening { .. } => {
                if *touch == Some(Touch::Cover) {
                    *touch = None;
                }
                return false;
            }
            Rest::AlwaysOn | Rest::Off if double_tapped => {
                self.wake(now, update);
                return false;
            }
            Rest::AlwaysOn | Rest::Off => {}
        }
        self.step_always_on(now);
        true
    }

    /// Whether `touch` is the second tap of a double tap on the resting screen.
    fn double_tapped(&mut self, touch: Option<Touch>, now: Micros) -> bool {
        if touch != Some(Touch::Gesture(TouchGesture::Tap)) {
            return false;
        }
        match self.rest_tap.take() {
            Some(first) if now.saturating_sub(first) <= rest::DOUBLE_TAP => true,
            _ => {
                self.rest_tap = Some(now);
                false
            }
        }
    }

    /// Works out what the always-on face shows, while the screen rests on it. It must run in
    /// every step that leaves the screen there, or that step draws the face under it.
    fn step_always_on(&mut self, now: Micros) {
        if self.rest != Rest::AlwaysOn {
            return;
        }
        // The battery moves with the minute's redraw rather than waking the panel on its own.
        // A stopped or unreadable clock has no minute to redraw on, so the stage's clock marks
        // its minutes instead.
        let clock = &self.peripherals.clock;
        let (battery, taken) = self.shown_battery;
        let minute = now / MINUTE;
        let held = always_on::View::of(clock, battery);
        let redraw = self.drawn_always_on.as_ref() != Some(&held);
        if redraw || (held.is_still() && taken / MINUTE != minute) {
            self.shown_battery = (self.peripherals.battery, now);
            let view = always_on::View::of(clock, self.peripherals.battery);
            if self.drawn_always_on.as_ref() != Some(&view) {
                self.drawn_always_on = Some(view);
                self.changed.make_full();
                self.shift.advance(now);
            }
        }
    }

    /// Wakes from the always-on face or from off, onto the face that showed, which runs its
    /// entry, or from the panel onto the clock face. The level fades back up to the stored one,
    /// so an unsaved brightness goes with any other edit.
    fn wake(&mut self, now: Micros, update: &mut Update) {
        self.restart(now);
        self.entry_from = now;
        if self.rest == Rest::Off {
            update.display_on = Some(true);
            self.entry_from += rest::PANEL_WAKE;
        }
        self.rest = Rest::Awake;
        self.swallowed = true;
        self.drawn_always_on = None;
        self.shift.advance(now);
        if self.page.is_some() || !self.sheet.is_closed() {
            self.route = None;
            self.show(Screen::Clock);
        } else {
            self.face(self.screen);
        }
        self.level = self.peripherals.brightness;
        self.fade_to(self.level, rest::WAKE_FADE, self.entry_from);
        self.changed.make_full();
    }

    /// Fades from the level that shows to `to`, starting at `start`.
    fn fade_to(&mut self, to: u8, duration: Micros, start: Micros) {
        self.fade = Some(Fade {
            from: self.shown_level,
            to,
            start,
            duration,
        });
    }

    fn step_fade(&mut self, now: Micros, update: &mut Update) {
        let Some(fade) = self.fade else {
            return;
        };
        let level = fade.level(now);
        if level != self.shown_level {
            update.brightness = Some(level);
        }
        if fade.done(now) {
            self.fade = None;
        }
    }

    /// Steps the start-up sequence, with `touched` set when a fresh touch reading came in.
    /// Returns whether it still shows; once it has handed over, the rest of the step shows the
    /// clock face.
    fn step_startup(
        &mut self,
        now: Micros,
        boot: Option<Report>,
        touched: bool,
        update: &mut Update,
    ) -> bool {
        let Some(startup) = &mut self.startup else {
            return false;
        };
        startup.begin(now);
        if let Some(report) = boot {
            startup.report(report, now);
        }
        let contact = touched && self.raw_touch[0].is_some();
        if contact {
            startup.touch(now);
        }
        let level = self.peripherals.brightness;
        let brightness = match startup.phase(now) {
            Phase::Done { entry, after_card } => {
                // A replay or a demonstration leaves the boot's own record.
                if let Some(finished) = self.startup.take().filter(Startup::is_boot) {
                    self.last_boot = Some(finished);
                }
                self.startup_view = None;
                self.startup_due = None;
                // A finger still down came during the sequence, so it is not a gesture.
                self.swallowed = self.raw_touch[0].is_some();
                self.face(Screen::Clock);
                if !entry {
                    self.settle_clock(now);
                }
                self.clock_types_in = after_card;
                self.clock_builds_gauge = entry;
                self.level = level;
                level
            }
            _ => startup.brightness(now, level),
        };
        if self.startup_level != Some(brightness) {
            self.startup_level = Some(brightness);
            update.brightness = Some(brightness);
        }
        let Some(startup) = &self.startup else {
            return false;
        };
        self.startup_due = startup.next_change(now);
        let view = startup.view(now);
        match (self.startup_view, view) {
            (
                Some(startup::View::SelfTest {
                    cells: before,
                    scroll: was,
                }),
                Some(startup::View::SelfTest {
                    cells: after,
                    scroll: is,
                }),
            ) => {
                if was != is {
                    self.changed.add(startup::LIST);
                } else {
                    for (i, _) in before
                        .iter()
                        .zip(&after)
                        .enumerate()
                        .filter(|(_, (a, b))| a != b)
                    {
                        self.changed.add(startup::cell_bounds(i, is));
                    }
                }
                if before != after {
                    self.changed
                        .add(startup::counter_bounds(&self.renderer, &before));
                    self.changed
                        .add(startup::counter_bounds(&self.renderer, &after));
                }
            }
            (Some(startup::View::Identity(before)), Some(startup::View::Identity(after))) => {
                self.identity_marks.changes(
                    before,
                    after,
                    &self.peripherals.clock,
                    &mut self.changed,
                );
            }
            (before, after) if before != after => self.changed.make_full(),
            _ => {}
        }
        self.startup_view = view;
        true
    }

    /// Closes the panel and plays the start-up again: the identity and the card, or a
    /// demonstration of a part failing.
    fn replay_startup(&mut self, replay: Replay, now: Micros) {
        let startup = match replay {
            Replay::Good => self.last_boot.clone().unwrap_or_default().replay(now),
            Replay::Failing(part) => Startup::demo(part, now),
        };
        self.show(Screen::Clock);
        self.startup_due = startup.next_change(now);
        self.startup_view = startup.view(now);
        self.startup = Some(startup);
        self.changed.make_full();
    }

    /// Shows the clock face as though its entry had finished.
    fn settle_clock(&mut self, now: Micros) {
        let time = clock_screen::shown_time(&self.peripherals.clock);
        self.clock_settled = Some(ClockSettled {
            times: ClockTimes::default(),
            shown: clock_screen::Keys::of(&self.peripherals.clock),
            time: time.map(|time| (time, now)),
            retyped: None,
        });
    }

    /// Works out what the step changed, from what each settled screen showed before. A group
    /// screen passes the list it draws this step, and the drawer the list it drew the step
    /// before.
    fn track_damage(
        &mut self,
        full: bool,
        group_list: Option<alloc::boxed::Box<List>>,
        drawer_before: Option<alloc::boxed::Box<List>>,
    ) {
        let view = self.pager.view();
        let faces_settled = self.page.is_none()
            && self.sheet.is_closed()
            && self.drawer.is_none()
            && view.offset == 0
            && view.neighbour.is_none();
        let drawer_open = self.drawer_sheet.is_open()
            && self
                .drawer
                .as_ref()
                .is_some_and(|drawer| !drawer.is_moving());
        let drawn = if drawer_open && self.drawer_list.is_some() {
            Some(Drawn::Drawer)
        } else if self.drawer.is_some() {
            None
        } else if faces_settled && self.screen == Screen::Compass {
            Some(Drawn::Compass(self.peripherals.compass, self.accents))
        } else if faces_settled && self.screen == Screen::Clock {
            Some(Drawn::Clock((
                self.peripherals.clock,
                self.peripherals.battery,
                self.clock_accents,
            )))
        } else if let Some(list) = group_list {
            Some(Drawn::Group(list))
        } else if let Some((page, _)) = &self.page {
            Some(Drawn::Page(
                page.clone(),
                self.page_accents,
                self.peripherals,
            ))
        } else if self.sheet.is_open() {
            Some(Drawn::Panel((
                self.peripherals,
                self.grid.scroll,
                self.panel_accents,
            )))
        } else {
            None
        };
        match (self.drawn.take(), &drawn) {
            (Some(Drawn::Compass(view, accents)), Some(Drawn::Compass(now, now_accents))) => {
                compass_screen::damage(
                    (&view, accents),
                    (now, *now_accents),
                    &self.renderer,
                    &mut self.dial_footprint,
                    &mut self.changed,
                );
            }
            (Some(Drawn::Clock(before)), Some(Drawn::Clock(after))) => {
                clock_screen::damage(&before, after, &self.renderer, &mut self.changed);
            }
            (Some(Drawn::Panel(before)), Some(Drawn::Panel(after))) => {
                self.panel_damage(&before, after);
            }
            (Some(Drawn::Group(before)), Some(Drawn::Group(after))) => {
                after.damage(&before, &self.renderer, &mut self.changed);
                self.spare_list = Some(before);
            }
            (Some(Drawn::Drawer), Some(Drawn::Drawer)) => {
                match (&drawer_before, &self.drawer_list) {
                    (Some(before), Some(after)) => {
                        after.damage(before, &self.renderer, &mut self.changed);
                    }
                    _ => self.changed.make_full(),
                }
            }
            (
                Some(Drawn::Page(page, accents, state)),
                Some(Drawn::Page(now, now_accents, now_state)),
            ) => {
                if (&page, accents, state) != (now, *now_accents, *now_state) {
                    match now
                        .scroll_damage(&page)
                        .filter(|_| (accents, state) == (*now_accents, *now_state))
                    {
                        Some(lines) => lines.iter().for_each(|&line| self.changed.add(line)),
                        None => self.changed.make_full(),
                    }
                }
            }
            (_, Some(_)) => self.changed.make_full(),
            (_, None) if full || self.drawer.is_some() => self.changed.make_full(),
            (_, None) => {}
        }
        self.drawn = drawn;
        self.drawer_spare = drawer_before;
    }

    /// The open panel's damage: the grid while it scrolls, a cell whose reading changed, or
    /// everything while its accents move.
    fn panel_damage(
        &mut self,
        before: &(PeripheralState, i32, panel::Accents),
        after: &(PeripheralState, i32, panel::Accents),
    ) {
        if (panel::Accents {
            breath: after.2.breath,
            ..before.2
        }) != after.2
        {
            self.changed.make_full();
            return;
        }
        if before.2.breath != after.2.breath {
            panel::scatter_damage(&before.2, &after.2, &mut self.changed);
        }
        if before.1 != after.1 {
            self.changed.make_full();
            return;
        }
        for cell in Cell::ALL {
            if panel::in_view(cell.page(), after.1)
                && panel::cell_changed(cell, &before.0, &after.0)
            {
                self.changed.add(panel::cell_damage(cell, after.1));
            }
        }
    }

    /// What this step's report, or its absence, makes of the contact.
    fn gesture(&mut self, touch: Option<Touch>, now: Micros) -> GestureEvent {
        match touch {
            Some(_) if self.swallowed => GestureEvent::None,
            Some(Touch::Gesture(_)) => self.gesture.expire(now),
            Some(Touch::Lifted(last)) => self.gesture.lift(Some(last), now),
            Some(_) => self.gesture.update(self.raw_touch[0], now),
            None => self.gesture.expire(now),
        }
    }

    /// Sends a gesture to what it acts on: the pager or the panel's travel on the faces, the
    /// grid on the open panel, or the screen the panel opened.
    fn route_event(
        &mut self,
        event: &GestureEvent,
        now: Micros,
        effects: &mut Effects,
        update: &mut Update,
    ) {
        if let Some((Page::Group(flow), _)) = &mut self.page {
            let exit = flow.handle(
                event,
                &self.mesh,
                &mut self.group_memory,
                now,
                &self.renderer,
                &mut effects.mesh,
            );
            if exit == group::Exit::Panel {
                self.page = None;
            }
            return;
        }
        if let Some((page, _)) = &mut self.page {
            let next = page.handle(event, &self.peripherals, effects);
            match next {
                Next::Stay => {}
                Next::Panel => self.page = None,
                Next::Open(page) => self.page = Some((page, now)),
                Next::ReplayStartUp(replay) => self.replay_startup(replay, now),
            }
            return;
        }
        match *event {
            GestureEvent::Down(_) => {
                self.route = None;
                if self.sheet.is_closed() {
                    self.pager.handle(event, now);
                }
                self.sheet.finish();
                self.grid.finish();
            }
            GestureEvent::DragStart(drag) => {
                let route = self.route_for(&drag);
                self.route = Some(route);
                match route {
                    Route::Pager => self.pager.handle(event, now),
                    Route::Drawer => {
                        self.drawer = Some(Drawer::new());
                        self.drawer_sheet.grab(&mirrored(&drag));
                    }
                    Route::Sheet => self.sheet.grab(&drag),
                    Route::Grid => {
                        self.grid.grabbed = Some(self.grid.scroll);
                        self.grid.scroll =
                            (self.grid.scroll - drag.offset().x).clamp(0, panel::MAX_SCROLL);
                    }
                    Route::Nowhere => {}
                }
            }
            GestureEvent::DragMove(drag) => match self.route {
                Some(Route::Pager) => self.pager.handle(event, now),
                Some(Route::Sheet) => self.sheet.drag(&drag),
                Some(Route::Drawer) => self.drawer_sheet.drag(&mirrored(&drag)),
                Some(Route::Grid) => {
                    if let Some(from) = self.grid.grabbed {
                        self.grid.scroll = (from - drag.offset().x).clamp(0, panel::MAX_SCROLL);
                    }
                }
                _ => {}
            },
            GestureEvent::DragEnd(drag) => match self.route.take() {
                Some(Route::Pager) => self.pager.handle(event, now),
                Some(Route::Sheet) => self.sheet.release(&drag, now),
                Some(Route::Drawer) => self.drawer_sheet.release(&mirrored(&drag), now),
                Some(Route::Grid) => self.release_grid(&drag, now),
                _ => {}
            },
            GestureEvent::Tap(point) => {
                if self.sheet.is_open() {
                    self.tap_panel(point, now, update);
                }
            }
            GestureEvent::None => {}
        }
    }

    fn route_for(&self, drag: &Drag) -> Route {
        let offset = drag.offset();
        if self.sheet.is_closed() {
            // Mostly downward opens the panel, mostly upward the drawer; anything else is the
            // pager's.
            if offset.y > 0 && offset.y >= 2 * offset.x.abs() {
                Route::Sheet
            } else if offset.y < 0 && -offset.y >= 2 * offset.x.abs() {
                Route::Drawer
            } else {
                Route::Pager
            }
        } else if self.sheet.is_open() {
            if offset.x.abs() >= offset.y.abs() {
                Route::Grid
            } else if offset.y < 0 {
                Route::Sheet
            } else {
                Route::Nowhere
            }
        } else {
            Route::Sheet
        }
    }

    /// Snaps the grid to a complete page, or follows a flick to the next one.
    fn release_grid(&mut self, drag: &Drag, now: Micros) {
        self.grid.grabbed = None;
        let scroll = self.grid.scroll;
        let velocity = drag.velocity.0;
        let page = if velocity <= -SNAP_FLICK {
            scroll.div_euclid(panel::PAGE_WIDTH) + 1
        } else if velocity >= SNAP_FLICK {
            (scroll + panel::PAGE_WIDTH - 1).div_euclid(panel::PAGE_WIDTH) - 1
        } else {
            (scroll + panel::PAGE_WIDTH / 2).div_euclid(panel::PAGE_WIDTH)
        };
        self.grid
            .snap_to((page * panel::PAGE_WIDTH).clamp(0, panel::MAX_SCROLL), now);
    }

    fn tap_panel(&mut self, point: Point, now: Micros, update: &mut Update) {
        let scroll = self.grid.scroll;
        let Some(cell) = panel::cell_at(point, scroll) else {
            return;
        };
        if !panel::in_view(cell.page(), scroll) {
            return;
        }
        let page = match cell {
            Cell::Zone => Page::Picker(Picker::new(&self.peripherals)),
            Cell::Brightness => {
                Page::Brightness(second::Brightness::new(self.peripherals.brightness))
            }
            Cell::Timeout => Page::Timeout(second::TimeoutChooser::new(self.peripherals.timeout)),
            Cell::AlwaysOn => {
                Page::AlwaysOn(second::AlwaysOnChooser::new(self.peripherals.always_on))
            }
            Cell::Group => Page::Group(Flow::group()),
            Cell::Name => Page::Group(Flow::name(&self.mesh)),
            Cell::Device => Page::Device(second::Device::default()),
            Cell::Compass => {
                update.recalibrate = true;
                // The motion task resets the calibration on its next sample; the panel and the
                // compass show it from now.
                let compass = &mut self.peripherals.compass;
                compass.calibration_percent = 0;
                compass.heading_decidegrees = None;
                self.face(Screen::Compass);
                self.sheet.go(false, now);
                return;
            }
        };
        self.page = Some((page, now));
    }

    /// Takes what a screen asked for into the update, and shows a stored setting at once.
    fn apply(&mut self, effects: Effects, update: &mut Update) {
        if effects.mesh.is_some() {
            update.mesh = effects.mesh;
        }
        if let Some(level) = effects.brightness {
            self.level = level;
            update.brightness = Some(level);
        }
        let Some(store) = effects.store else {
            return;
        };
        update.store = Some(store);
        let mut zone = self.peripherals.clock.zone();
        match store {
            Store::Brightness(level) => {
                self.peripherals.brightness = level;
                self.level = level;
                update.brightness = Some(level);
            }
            Store::ManualZone(id) => {
                zone = ZoneState {
                    mode: ZoneMode::Manual,
                    zone: Some(id),
                }
            }
            Store::AutomaticZone => zone.mode = ZoneMode::Automatic,
            Store::Timeout(timeout) => self.peripherals.timeout = timeout,
            Store::AlwaysOn(choice) => self.peripherals.always_on = choice,
            Store::Clear => {
                zone.mode = ZoneMode::Automatic;
                self.peripherals.timeout = Timeout::default();
                self.peripherals.always_on = AlwaysOn::Off;
                self.peripherals.brightness = DEFAULT_BRIGHTNESS;
                self.level = DEFAULT_BRIGHTNESS;
                update.brightness = Some(DEFAULT_BRIGHTNESS);
            }
        }
        let clock = self.peripherals.clock;
        self.peripherals.clock = clock.read(clock.clock(), zone);
    }

    /// Returns to the clock face from anywhere, discarding any edit in progress.
    fn go_home(&mut self, now: Micros, effects: &mut Effects) {
        self.close_drawer();
        if let Some((mut page, _)) = self.page.take() {
            if let Page::Group(flow) = &mut page {
                flow.interrupt(&self.mesh, &mut effects.mesh);
            }
            page.discard(effects);
            self.sheet.set(true);
        }
        self.route = None;
        if !self.sheet.is_closed() {
            self.face(Screen::Clock);
            self.sheet.go(false, now);
        } else if Screen::ALL[self.pager.page()] != Screen::Clock {
            self.pager.advance(false, now);
        }
    }

    /// Advances the panel's builds and reveals to `now`, and returns how far each has come.
    fn advance_panel_accents(&mut self, now: Micros) -> panel::Accents {
        let entry = match self.panel_settled {
            Some(settled) => {
                let at = |delay: Micros| Some(settled + delay);
                let cell =
                    |start: Micros, i: usize| Some(settled + start + PANEL_STAGGER * i as Micros);
                panel::Accents {
                    title: progress(now, at(0), PANEL_TITLE_REVEAL),
                    rules: progress(now, at(PANEL_RULES), PANEL_RULES_DRAW),
                    rows: core::array::from_fn(|i| rows_built(now, cell(PANEL_ICON, i))),
                    index: core::array::from_fn(|i| {
                        progress(now, cell(PANEL_INDEX, i), PANEL_INDEX_REVEAL)
                    }),
                    name: core::array::from_fn(|i| {
                        progress(now, cell(PANEL_NAME, i), PANEL_NAME_REVEAL)
                    }),
                    markers: now >= settled + PANEL_MARKERS,
                    hint: progress(now, at(PANEL_HINT), PANEL_HINT_REVEAL),
                    scatter: progress(now, at(0), PANEL_SCATTER_BLOOM),
                    breath: self.breath,
                }
            }
            None => panel::Accents::HIDDEN,
        };
        self.fading |= self.panel_settled.is_some()
            && entry
                != panel::Accents {
                    breath: entry.breath,
                    ..panel::Accents::FULL
                };
        self.breathing |= self.panel_settled.is_some() && self.rest == Rest::Awake;
        // Going up, the accents follow the panel's offset, so reversing a drag restores them.
        let p = swipe_progress(self.sheet.height() - self.sheet.offset());
        let exit = panel::Accents {
            title: leaving(p, 0.5, 0.3),
            rules: leaving(p, 0.4, 0.4),
            rows: [rows_leaving(p, 0.3, 0.4); panel::CELLS],
            index: [leaving(p, 0.15, 0.25); panel::CELLS],
            name: [leaving(p, 0.1, 0.3); panel::CELLS],
            markers: p <= 0.1,
            hint: leaving(p, 0.0, 0.2),
            scatter: leaving(p, 0.0, 0.5),
            breath: u8::MAX,
        };
        entry.min(exit)
    }

    /// Advances the compass page's builds and reveals to `now`, and returns how far each has
    /// come.
    fn compass_accents(&mut self, now: Micros) -> Accents {
        if self.screen != Screen::Compass {
            self.compass_settled = None;
            self.level_since = None;
            return Accents::FULL;
        }
        let offset = self.face_offset();
        let mode = Mode::of(&self.peripherals.compass);
        let CompassSettled {
            times,
            shown,
            change,
        } = match &mut self.compass_settled {
            Some(settled) => settled,
            None if offset == 0 => {
                let from = now.max(self.entry_from);
                let start = |delay: Micros| Some(from + delay);
                let mut times = CompassTimes {
                    icon: start(COMPASS_ENTRY.icon),
                    caption: start(COMPASS_ENTRY.caption),
                    dial: start(COMPASS_ENTRY.dial),
                    texture: start(COMPASS_ENTRY.texture),
                };
                if mode == Mode::NoData {
                    // A fault shows at once.
                    (times.icon, times.caption) = (None, None);
                }
                self.compass_settled.insert(CompassSettled {
                    times,
                    shown: mode,
                    change: None,
                })
            }
            None => return Accents::HIDDEN,
        };
        if !shown.same_state(mode) {
            let from_now = |start: Option<Micros>| Some(start.map_or(now, |start| start.max(now)));
            if mode == Mode::NoData {
                (times.icon, times.caption) = (None, None);
                *change = None;
            } else {
                if *shown == Mode::NoData {
                    times.texture = Some(now);
                }
                // Back from HOLD LEVEL within the grace, the dial and icon show at once.
                let quick = *shown == Mode::HoldLevel
                    && mode.heading().is_some()
                    && self
                        .level_since
                        .is_some_and(|since| now.saturating_sub(since) < LEVEL_GRACE);
                if !quick {
                    times.icon = from_now(times.icon);
                    if mode.heading().is_some() && shown.heading().is_none() {
                        times.dial = from_now(times.dial);
                    }
                }
                if compass_screen::caption(*shown).0 != compass_screen::caption(mode).0 {
                    times.caption = from_now(times.caption);
                }
                // A change still running gives way: the new one starts from the state shown.
                *change = Some((now, *shown));
            }
            if mode == Mode::HoldLevel && shown.heading().is_some() {
                self.level_since = Some(now);
            } else if mode != Mode::HoldLevel {
                self.level_since = None;
            }
            *shown = mode;
        }
        let running = change.map_or(compass_screen::Change::NONE, |(start, from)| {
            compass_screen::Change::at(from, mode, now.saturating_sub(start))
        });
        if running.done() {
            *change = None;
        }
        let entry = Accents {
            icon_rows: rows_built(now, times.icon),
            caption: progress(now, times.caption, CAPTION_REVEAL),
            dial: progress(now, times.dial, DIAL_SWEEP),
            texture: texture_step(now, times.texture),
            field: u8::MAX,
            change: running,
        };
        self.fading |= entry != Accents::FULL;
        // Going out, the accents follow the page's offset, so reversing a drag restores them.
        let p = swipe_progress(offset);
        let exit = Accents {
            icon_rows: rows_leaving(p, 0.3, 0.5),
            caption: leaving(p, 0.2, 0.3),
            dial: leaving(p, 0.2, 0.4),
            texture: 5,
            // The field recedes to 40 % as the page leaves, rather than going.
            field: (255 - (255 - u16::from(leaving(p, 0.0, 0.5))) * 3 / 5) as u8,
            change: compass_screen::Change::NONE,
        };
        entry.min(exit)
    }

    /// Advances the clock face's builds and reveals to `now`, and returns how far each has come.
    fn advance_clock_accents(&mut self, now: Micros) -> clock_screen::Accents {
        use clock_screen::Accents;
        if self.screen != Screen::Clock {
            self.clock_settled = None;
            return Accents::FULL;
        }
        let offset = self.face_offset();
        let keys = clock_screen::Keys::of(&self.peripherals.clock);
        let time = clock_screen::shown_time(&self.peripherals.clock);
        let types_in = self.clock_types_in && offset == 0;
        let settled = match &mut self.clock_settled {
            Some(settled) => settled,
            None if offset == 0 => {
                let from = now.max(self.entry_from);
                let start = |delay: Micros| Some(from + delay);
                let mut times = ClockTimes {
                    icon: start(CLOCK_ENTRY.icon),
                    label: start(CLOCK_ENTRY.label),
                    plate: start(CLOCK_ENTRY.plate),
                    zone: start(CLOCK_ENTRY.zone),
                    scatter: start(CLOCK_ENTRY.scatter),
                    battery: start(CLOCK_ENTRY.battery),
                };
                if keys.mode == clock_screen::Mode::NoData {
                    // A fault shows at once.
                    (times.icon, times.label) = (None, None);
                }
                if core::mem::take(&mut self.clock_builds_gauge) {
                    self.charge.enter(from + CLOCK_ENTRY.battery);
                    times.battery = None;
                }
                self.clock_settled.insert(ClockSettled {
                    times,
                    shown: keys.clone(),
                    time: time.map(|time| (time, now)),
                    retyped: (types_in && time.is_some()).then_some(now),
                })
            }
            None => return Accents::HIDDEN,
        };
        if types_in {
            self.clock_types_in = false;
        }
        let ClockSettled { times, shown, .. } = settled;
        if *shown != keys {
            if keys.mode == clock_screen::Mode::NoData {
                // A fault shows at once.
                *times = ClockTimes::default();
            } else {
                // The icon rebuilds on any change of state, the label on a change of text, and
                // the plate and the zone name together on any change of zone. One the entry has
                // not reached yet shows the new text when it gets there.
                let restart = |start: &mut Option<Micros>, at: Micros| {
                    if start.is_none_or(|start| start <= now) {
                        *start = Some(at);
                    }
                };
                if shown.mode != keys.mode {
                    restart(&mut times.icon, now);
                }
                if shown.mode.label() != keys.mode.label() {
                    restart(&mut times.label, now);
                }
                if (&shown.plate, shown.zone) != (&keys.plate, keys.zone)
                    && times.plate.is_none_or(|start| start <= now)
                {
                    times.plate = Some(now);
                    restart(
                        &mut times.zone,
                        now + (CLOCK_ENTRY.zone - CLOCK_ENTRY.plate),
                    );
                }
            }
            *shown = keys;
        }
        if time != settled.time.map(|(time, _)| time) {
            let replaced = match (time, settled.time) {
                (None, _) => false,
                (Some(_), None) => true,
                (Some(time), Some((was, since))) => {
                    let elapsed = ((now - since) / 1_000_000) as i64;
                    (time - was - elapsed).abs() > CLOCK_TICK_SLACK
                }
            };
            // Dashes and faults replace the time at once.
            settled.retyped = if replaced {
                Some(now)
            } else {
                settled.retyped.filter(|_| time.is_some())
            };
            settled.time = time.map(|time| (time, now));
        }
        let times = &settled.times;
        let retyped = settled.retyped;
        let icon = 1.0
            - libm::powf(
                1.0 - f32::from(progress(now, times.icon, CLOCK_ICON_BUILD)) / 255.0,
                3.0,
            );
        let exposed = libm::roundf(self.charge.exposed(now) * 255.0) as u8;
        let lit = matches!(
            self.rest,
            Rest::Awake | Rest::Dimmed { .. } | Rest::Darkening { .. }
        );
        let entry = Accents {
            icon_rows: libm::roundf(5.0 * icon) as u8,
            label: progress(now, times.label, CLOCK_LABEL_REVEAL),
            plate: progress(now, times.plate, CLOCK_PLATE_REVEAL),
            zone: progress(now, times.zone, CLOCK_ZONE_REVEAL),
            rail: progress(now, times.zone, CLOCK_RAIL_REVEAL),
            scatter: progress(now, times.scatter, CLOCK_SCATTER_BLOOM),
            battery: progress(now, times.battery, CLOCK_BATTERY_RISE),
            time: progress(now, retyped, CLOCK_TIME_REVEAL),
            date: progress(
                now,
                retyped.map(|start| start + CLOCK_DATE_DELAY),
                CLOCK_DATE_REVEAL,
            ),
            bands: if exposed > 0 {
                self.charge.phase(now)
            } else {
                charging::OPEN
            },
            exposed,
            breath: self.breath,
        };
        self.fading |= entry
            != Accents {
                bands: entry.bands,
                exposed: entry.exposed,
                breath: entry.breath,
                ..Accents::FULL
            };
        self.breathing |= self.rest == Rest::Awake;
        self.gauge_moving |= lit && self.charge.is_moving(now);
        let p = swipe_progress(offset);
        let exit = Accents {
            icon_rows: rows_leaving(p, 0.3, 0.5),
            label: leaving(p, 0.2, 0.3),
            plate: leaving(p, 0.1, 0.3),
            zone: leaving(p, 0.0, 0.2),
            rail: leaving(p, 0.0, 0.2),
            // The scatter thins as it moves with the page, the entry's bloom reversed.
            scatter: leaving(p, 0.0, 0.5),
            battery: leaving(p, 0.1, 0.3),
            time: u8::MAX,
            date: u8::MAX,
            bands: u8::MAX,
            exposed: u8::MAX,
            breath: u8::MAX,
        };
        entry.min(exit)
    }

    /// Draws the current state into `target`.
    pub fn draw<D>(&self, target: &mut D)
    where
        D: CoverageTarget<Color = Color>,
        D::Error: core::fmt::Debug,
    {
        if let (Some(startup), Some(view)) = (&self.startup, self.startup_view) {
            let context = startup::Context {
                clock: &self.peripherals.clock,
                firmware: self.peripherals.firmware,
            };
            startup::draw(view, startup, &context, &self.renderer, target)
                .expect("drawing the start-up failed");
            return;
        }
        if let Some(power_off) = &self.power_off {
            screens::clear(target).expect("clearing the panel failed");
            power_off
                .draw(&self.renderer, target)
                .expect("drawing the power-off confirmation failed");
            return;
        }
        if self.drawer.is_some()
            && self.drawer_sheet.is_open()
            && let Some(list) = &self.drawer_list
        {
            screens::clear(target).expect("clearing the panel failed");
            list.draw(&self.renderer, target)
                .expect("drawing the drawer failed");
            return;
        }
        self.draw_under(target);
        if let (Some(_), Some(list)) = (&self.drawer, &self.drawer_list) {
            // The drawer's surface, part way up over what it covers.
            let height = board::LCD_HEIGHT as i32;
            let top = height - self.drawer_sheet.offset();
            let surface = embedded_graphics::primitives::Rectangle::new(
                Point::new(0, top),
                embedded_graphics::prelude::Size::new(
                    u32::from(board::LCD_WIDTH),
                    (height - top) as u32,
                ),
            );
            target
                .fill_solid(&surface, chrome::BLACK)
                .expect("clearing under the drawer failed");
            list.draw(
                &self.renderer,
                &mut chrome::Window::new(&mut *target, Point::new(0, top), surface),
            )
            .expect("drawing the drawer failed");
        }
        if let Some((list, arc)) = &self.overlay {
            list.draw(&self.renderer, target)
                .expect("drawing the toast failed");
            if *arc {
                let arc = drawer::UNREAD_ARC;
                crate::ui::stroke::draw_arc(
                    arc.center, arc.radius, arc.width, arc.span, arc.color, target,
                );
            }
        }
    }

    /// Draws what the drawer and the toast lie over: the always-on face, a screen the panel
    /// opened, or the faces and the panel.
    fn draw_under<D>(&self, target: &mut D)
    where
        D: CoverageTarget<Color = Color>,
        D::Error: core::fmt::Debug,
    {
        if let (Rest::AlwaysOn, Some(view)) = (self.rest, &self.drawn_always_on) {
            always_on::draw(view, &self.renderer, target)
                .expect("drawing the always-on face failed");
            return;
        }
        if let Some((page, _)) = &self.page {
            screens::clear(target).expect("clearing the panel failed");
            match (page, &self.drawn) {
                (Page::Group(_), Some(Drawn::Group(list))) => list.draw(&self.renderer, target),
                (Page::Group(_), _) => Ok(()),
                _ => page.draw(&self.peripherals, self.page_accents, &self.renderer, target),
            }
            .expect("drawing a screen failed");
            return;
        }
        let view = self.pager.view();
        screens::render(
            screens::State {
                screen: self.screen,
                peripherals: self.peripherals,
                offset: view.offset,
                neighbour: view
                    .neighbour
                    .map(|(page, offset)| (Screen::ALL[page], offset)),
                compass_accents: self.accents,
                clock_accents: self.clock_accents,
                sheet: self.sheet.offset(),
                panel_scroll: self.grid.scroll,
                panel_accents: self.panel_accents,
            },
            &self.renderer,
            target,
        )
        .expect("drawing a screen failed")
    }
}

fn texture_step(now: Micros, start: Option<Micros>) -> u8 {
    let Some(start) = start else {
        return 5;
    };
    match now.saturating_sub(start) {
        0 => 0,
        1..80_000 => 1,
        80_000..120_000 => 2,
        120_000..220_000 => 3,
        220_000..COMPASS_TEXTURE_SETTLE => 4,
        _ => 5,
    }
}

/// How far an accent that starts at `start` and takes `duration` has come, 0 to 255.
fn progress(now: Micros, start: Option<Micros>, duration: Micros) -> u8 {
    match start {
        None => u8::MAX,
        Some(start) if now < start => 0,
        Some(start) => ((now - start).min(duration) * 255 / duration) as u8,
    }
}

/// How many rows of an icon's modules have landed, a row every `ICON_ROW` from `start`.
fn rows_built(now: Micros, start: Option<Micros>) -> u8 {
    match start {
        None => 5,
        Some(start) if now < start => 0,
        Some(start) => ((now - start) / ICON_ROW + 1).min(5) as u8,
    }
}

/// How far a page at `offset` has gone towards hiding its accents: 0 at rest, 1 at
/// `SWIPE_FADE` of the panel's width.
fn swipe_progress(offset: i32) -> f32 {
    offset.unsigned_abs() as f32 / board::LCD_WIDTH as f32 / SWIPE_FADE
}

/// What is left of an accent that leaves over `from..from + over` of the swipe, 0 to 255.
fn leaving(p: f32, from: f32, over: f32) -> u8 {
    libm::roundf((1.0 - ((p - from) / over).clamp(0.0, 1.0)) * 255.0) as u8
}

fn rows_leaving(p: f32, from: f32, over: f32) -> u8 {
    libm::roundf(5.0 * f32::from(leaving(p, from, over)) / 255.0) as u8
}
