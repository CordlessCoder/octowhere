//! The UI between input and drawing: which screen shows, what touch does to it, and when it has
//! to be redrawn. The frame loop feeds it readings and draws it, and acts on the effects it
//! returns. It never touches a device, so the host drives it the same way.

use embedded_graphics::prelude::Point;

use super::{
    always_on,
    clock::{ClockState, ClockView, ZoneMode, ZoneState},
    clock_screen,
    compass::CompassView,
    compass_screen::{self, Accents, DialFootprint, Mode},
    ease::Ease,
    gesture::{Drag, GestureEvent, GestureTracker, Micros},
    pager::Pager,
    panel::{self, Cell},
    picker::Picker,
    rest::{self, Fade, Rest, Timeout},
    screens::{self, Battery, Gnss, PeripheralState, Screen, DEFAULT_BRIGHTNESS},
    second::{self, Effects, Next, Page},
    sheet::Sheet,
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
    /// The controller recognised a hand covering the screen.
    Cover,
}

/// When each of the compass's accents starts after the page settles, and how long each takes.
/// The icon's modules land a row per `ICON_ROW`.
const COMPASS_ENTRY: CompassTimes<Micros> = CompassTimes {
    ring: 0,
    icon: 95_000,
    caption: 120_000,
    dial: 190_000,
};
const RING_FADE: Micros = 110_000;
const ICON_ROW: Micros = 30_000;
const CAPTION_REVEAL: Micros = 120_000;
const DIAL_SWEEP: Micros = 170_000;
/// A heading back from TOP EDGE UP within this shows the dial at once, so tilting through
/// vertical does not replay anything.
const TOP_EDGE_GRACE: Micros = 750_000;

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
    ring: T,
    icon: T,
    caption: T,
    dial: T,
}

/// Dragging the compass or the clock this fraction of the panel's width takes its accents out
/// entirely.
const SWIPE_FADE: f32 = 0.35;
/// When each of the clock face's accents starts after the page settles. The icon's modules land
/// a row per step, and the reveals run over their durations.
const CLOCK_ENTRY: ClockTimes<Micros> = ClockTimes {
    ring: 0,
    icon: 60_000,
    label: 100_000,
    plate: 160_000,
    zone: 240_000,
    mark: 300_000,
};
const CLOCK_LABEL_REVEAL: Micros = 120_000;
const CLOCK_PLATE_REVEAL: Micros = 120_000;
const CLOCK_ZONE_REVEAL: Micros = 160_000;
const CLOCK_MARK_REVEAL: Micros = 160_000;
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
    ring: T,
    icon: T,
    label: T,
    plate: T,
    zone: T,
    mark: T,
}

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
}

/// Where the drag in progress goes, decided when it leaves the tap slop.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Route {
    Pager,
    Sheet,
    Grid,
    Nowhere,
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

pub struct Stage {
    screen: Screen,
    raw_touch: [Option<Point>; 2],
    gesture: GestureTracker,
    pager: Pager,
    peripherals: PeripheralState,
    renderer: FontdueRenderer<'static, Color>,
    /// When the last cover report arrived, while the hand that sent it may still be there.
    covered_at: Option<Micros>,
    /// The compass page, while it has settled into view.
    compass_settled: Option<CompassSettled>,
    /// When the heading gave way to TOP EDGE UP, while nothing else has shown since.
    top_edge_since: Option<Micros>,
    accents: Accents,
    fading: bool,
    /// What the compass page showed after the last step, while it filled the panel.
    drawn_compass: Option<(CompassView, Accents)>,
    /// The pixels the last step changed. Boxed so the frame loop's stack never holds it.
    changed: alloc::boxed::Box<Dirty>,
    dial_footprint: DialFootprint,
    /// When the clock page last settled into view, and what its accents re-reveal on.
    clock_settled: Option<ClockSettled>,
    clock_accents: clock_screen::Accents,
    /// What the clock page showed after the last step, while it filled the panel.
    drawn_clock: Option<(ClockView, clock_screen::Accents)>,
    /// How far the settings panel has come down over the faces.
    sheet: Sheet,
    route: Option<Route>,
    grid: Grid,
    /// When the panel last settled open, while it stays open.
    panel_settled: Option<Micros>,
    panel_accents: panel::Accents,
    /// What the open panel showed after the last step, while nothing covered it.
    drawn_panel: Option<(PeripheralState, i32, panel::Accents)>,
    /// A screen the panel opened, and when.
    page: Option<(Page, Micros)>,
    page_accents: second::Accents,
    drawn_page: Option<(Page, second::Accents, PeripheralState)>,
    /// The start-up sequence, until it hands over to the clock face.
    startup: Option<Startup>,
    /// What the sequence showed after the last step.
    startup_view: Option<startup::View>,
    /// The brightness the sequence last asked for.
    startup_level: Option<u8>,
    /// When the sequence next changes on its own.
    startup_due: Option<Micros>,
    /// A contact that skipped the sequence, which the faces ignore until it lifts.
    swallowed: bool,
    /// The clock face's time and date type in as its next entry starts.
    clock_types_in: bool,
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
}

impl Stage {
    /// A stage that opens on the start-up sequence, for boot to report each part to.
    #[must_use]
    pub fn starting(peripherals: PeripheralState) -> Self {
        Self { startup: Some(Startup::new()), ..Self::new(peripherals) }
    }

    #[must_use]
    pub fn new(peripherals: PeripheralState) -> Self {
        Self {
            screen: Screen::ALL[0],
            raw_touch: [None; 2],
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
            top_edge_since: None,
            accents: Accents::FULL,
            fading: false,
            drawn_compass: None,
            changed: alloc::boxed::Box::new(Dirty::new()),
            dial_footprint: DialFootprint::default(),
            clock_settled: None,
            clock_accents: clock_screen::Accents::FULL,
            drawn_clock: None,
            sheet: Sheet::new(board::LCD_HEIGHT as i32),
            route: None,
            grid: Grid::default(),
            panel_settled: None,
            panel_accents: panel::Accents::HIDDEN,
            drawn_panel: None,
            page: None,
            page_accents: second::Accents::FULL,
            drawn_page: None,
            startup: None,
            startup_view: None,
            startup_level: None,
            startup_due: None,
            swallowed: false,
            clock_types_in: false,
            last_boot: None,
            rest: Rest::Awake,
            active_since: 0,
            heading_anchor: None,
            level: peripherals.brightness,
            shown_level: peripherals.brightness,
            fade: None,
            entry_from: 0,
            drawn_always_on: None,
            peripherals,
        }
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
            Rest::Awake if self.startup.is_none() => self.peripherals.timeout.duration().map(|timeout| self.active_since + timeout),
            Rest::Dimmed { since } => Some(since + rest::DIM_HOLD),
            Rest::Darkening { since } => Some(since + rest::OFF_FADE),
            _ => None,
        };
        [self.startup_due, rest].into_iter().flatten().min()
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
        self.top_edge_since = None;
        self.drawn_compass = None;
        self.clock_settled = None;
        self.drawn_clock = None;
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
        self.pager.is_moving() || self.sheet.is_moving() || self.grid.snap.is_some() || self.fading || self.fade.is_some()
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
        } = input;
        self.changed.clear();
        self.fading = false;
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
            self.peripherals.gnss = sensors.gnss;
            full |= self.screen == Screen::Clock;
        }

        if let Some(touch) = touch {
            self.raw_touch = match touch {
                Touch::Contacts(contacts) => contacts,
                Touch::Cover => [None; 2],
            };
        }
        // The timer does not run during the start-up; it restarts when the clock face takes over.
        if self.startup.is_some() {
            self.restart(now);
            if self.step_startup(now, boot, touch.is_some(), &mut update) {
                return update;
            }
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

        let previous = (self.pager.view(), self.sheet.offset(), self.grid.scroll, self.page.is_some());
        let event = if touch.is_some() && !self.swallowed {
            self.gesture.update(self.raw_touch[0], now)
        } else {
            GestureEvent::None
        };
        let mut effects = Effects::default();
        self.route_event(&event, now, &mut effects, &mut update);

        if let Some(touch) = touch {
            let cover = touch == Touch::Cover;
            let fresh = self
                .covered_at
                .is_none_or(|at| now.saturating_sub(at) >= COVER_REARM);
            if cover && fresh {
                self.go_home(now, &mut effects);
            }
            match touch {
                Touch::Cover => self.covered_at = Some(now),
                // A finger means the hand has gone. A report without one does not: the controller
                // sends unreadable reports while a hand is held, and those arrive as no contacts.
                Touch::Contacts(contacts) if contacts.iter().any(Option::is_some) => {
                    self.covered_at = None;
                }
                Touch::Contacts(_) => {}
            }
        }
        self.apply(effects, &mut update);

        self.pager.step(now);
        let was_open = self.sheet.is_open();
        self.sheet.step(now);
        self.grid.step(now);
        if let Some((page, _)) = &mut self.page {
            self.fading |= page.step(now);
        }
        if self.sheet.is_open() && !was_open || self.sheet.is_open() && self.panel_settled.is_none() {
            self.panel_settled = Some(now);
            // The face under the panel starts its entry again when the panel leaves it.
            self.compass_settled = None;
            self.clock_settled = None;
        }
        if self.sheet.is_closed() {
            self.panel_settled = None;
        }

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
            .map_or(second::Accents::FULL, |(_, opened)| second::Accents::at(*opened, now));
        self.fading |= self.page_accents != second::Accents::FULL;

        let current = (view, self.sheet.offset(), self.grid.scroll, self.page.is_some());
        let grid_only = current.2 != previous.2 && (current.0, current.1, current.3) == (previous.0, previous.1, previous.3);
        full |= current != previous && !grid_only;
        self.track_damage(full);

        let moved = current != previous || self.pager.is_moving() || self.sheet.is_moving() || self.grid.snap.is_some();
        let cover = touch == Some(Touch::Cover);
        if contact || cover || moved || self.heading_moved(face_shows) {
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
            (Some(anchor), Some(heading)) => rest::heading_apart(anchor, heading) > rest::HEADING_RESTART,
            (None, Some(_)) => {
                self.heading_anchor = heading;
                false
            }
            _ => false,
        }
    }

    /// Wakes the screen on a contact, and takes a dimmed screen on to rest. Returns whether the
    /// screen rests, so nothing else steps. A cover while dimmed is dropped from `touch`.
    fn step_rest(&mut self, now: Micros, contact: bool, touch: &mut Option<Touch>, update: &mut Update) -> bool {
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
                if self.peripherals.always_on {
                    self.rest = Rest::AlwaysOn;
                    self.drawn_always_on = None;
                    self.fade = None;
                    update.brightness = Some(rest::always_on_level(self.level));
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
            Rest::AlwaysOn | Rest::Off if contact => {
                self.wake(now, update);
                return false;
            }
            Rest::AlwaysOn | Rest::Off => {}
        }
        if self.rest == Rest::AlwaysOn {
            let view = always_on::View::of(&self.peripherals.clock);
            if self.drawn_always_on.as_ref() != Some(&view) {
                self.drawn_always_on = Some(view);
                self.changed.make_full();
            }
        }
        true
    }

    /// Wakes from the always-on face or from off, onto the face that showed, which runs its
    /// entry, or from the panel onto the clock face. The level fades back up to the stored one,
    /// so an unsaved brightness goes with any other edit.
    fn wake(&mut self, now: Micros, update: &mut Update) {
        self.entry_from = now;
        if self.rest == Rest::Off {
            update.display_on = Some(true);
            self.entry_from += rest::PANEL_WAKE;
        }
        self.rest = Rest::Awake;
        self.swallowed = true;
        self.drawn_always_on = None;
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
        self.fade = Some(Fade { from: self.shown_level, to, start, duration });
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
    fn step_startup(&mut self, now: Micros, boot: Option<Report>, touched: bool, update: &mut Update) -> bool {
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
            (Some(startup::View::SelfTest(before)), Some(startup::View::SelfTest(after))) => {
                if before != after {
                    for (i, _) in before.iter().zip(&after).enumerate().filter(|(_, (a, b))| a != b) {
                        self.changed.add(startup::cell_bounds(i));
                    }
                    self.changed.add(startup::counter_bounds(&self.renderer, &before));
                    self.changed.add(startup::counter_bounds(&self.renderer, &after));
                }
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

    /// Plays a demonstration of `part` failing, for the fault screen bench.
    pub fn bench_fault(&mut self, part: startup::Part, now: Micros) {
        self.replay_startup(Replay::Failing(part), now);
    }

    #[must_use]
    pub fn bench_startup_view(&self) -> Option<startup::View> {
        self.startup_view
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

    /// Works out what the step changed, from what each settled screen showed before.
    fn track_damage(&mut self, full: bool) {
        let view = self.pager.view();
        let faces_settled = self.page.is_none() && self.sheet.is_closed() && view.offset == 0 && view.neighbour.is_none();
        let compass = (faces_settled && self.screen == Screen::Compass).then_some((self.peripherals.compass, self.accents));
        let clock = (faces_settled && self.screen == Screen::Clock).then_some((self.peripherals.clock, self.clock_accents));
        let panel = (self.page.is_none() && self.sheet.is_open())
            .then_some((self.peripherals, self.grid.scroll, self.panel_accents));
        let page = self
            .page
            .as_ref()
            .map(|(page, _)| (page.clone(), self.page_accents, self.peripherals));
        if let (Some(before), Some(after)) = (self.drawn_compass, compass) {
            compass_screen::damage(
                (&before.0, before.1),
                (&after.0, after.1),
                &self.renderer,
                &mut self.dial_footprint,
                &mut self.changed,
            );
        } else if let (Some(before), Some(after)) = (self.drawn_clock, clock) {
            clock_screen::damage((&before.0, before.1), (&after.0, after.1), &self.renderer, &mut self.changed);
        } else if let (Some(before), Some(after)) = (self.drawn_panel, panel) {
            self.panel_damage(&before, &after);
        } else if let (Some(before), Some(after)) = (&self.drawn_page, &page) {
            if before != after {
                self.changed.make_full();
            }
        } else if full || compass.is_some() || clock.is_some() || panel.is_some() || page.is_some() {
            self.changed.make_full();
        }
        self.drawn_compass = compass;
        self.drawn_clock = clock;
        self.drawn_panel = panel;
        self.drawn_page = page;
    }

    /// The open panel's damage: the grid while it scrolls, a cell whose reading changed, or
    /// everything while its accents move.
    fn panel_damage(
        &mut self,
        before: &(PeripheralState, i32, panel::Accents),
        after: &(PeripheralState, i32, panel::Accents),
    ) {
        if before.2 != after.2 {
            self.changed.make_full();
            return;
        }
        if before.1 != after.1 {
            self.changed.add(panel::GRID_ROWS);
            self.changed.add(panel::MARKERS);
            return;
        }
        for cell in Cell::ALL {
            if panel::cell_changed(cell, &before.0, &after.0) {
                self.changed.add(panel::cell_damage(cell, after.1));
            }
        }
    }

    /// Sends a gesture to what it acts on: the pager or the panel's travel on the faces, the
    /// grid on the open panel, or the screen the panel opened.
    fn route_event(&mut self, event: &GestureEvent, now: Micros, effects: &mut Effects, update: &mut Update) {
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
                    Route::Sheet => self.sheet.grab(&drag),
                    Route::Grid => {
                        self.grid.grabbed = Some(self.grid.scroll);
                        self.grid.scroll = (self.grid.scroll - drag.offset().x).clamp(0, panel::MAX_SCROLL);
                    }
                    Route::Nowhere => {}
                }
            }
            GestureEvent::DragMove(drag) => match self.route {
                Some(Route::Pager) => self.pager.handle(event, now),
                Some(Route::Sheet) => self.sheet.drag(&drag),
                Some(Route::Grid) => {
                    if let Some(from) = self.grid.grabbed {
                        self.grid.scroll = (from - drag.offset().x).clamp(0, panel::MAX_SCROLL);
                    }
                }
                _ => {}
            },
            GestureEvent::DragEnd(drag) => {
                match self.route.take() {
                    Some(Route::Pager) => self.pager.handle(event, now),
                    Some(Route::Sheet) => self.sheet.release(&drag, now),
                    Some(Route::Grid) => self.release_grid(&drag, now),
                    _ => {}
                }
            }
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
            // Mostly downward opens the panel; anything else is the pager's.
            if offset.y > 0 && offset.y >= 2 * offset.x.abs() {
                Route::Sheet
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

    /// Snaps the grid to the nearest whole column, or a column on in a flick's direction.
    fn release_grid(&mut self, drag: &Drag, now: Micros) {
        self.grid.grabbed = None;
        let scroll = self.grid.scroll;
        let velocity = drag.velocity.0;
        let column = if velocity <= -SNAP_FLICK {
            scroll.div_euclid(panel::COLUMN) + 1
        } else if velocity >= SNAP_FLICK {
            (scroll + panel::COLUMN - 1).div_euclid(panel::COLUMN) - 1
        } else {
            (scroll + panel::COLUMN / 2).div_euclid(panel::COLUMN)
        };
        self.grid.snap_to((column * panel::COLUMN).clamp(0, panel::MAX_SCROLL), now);
    }

    fn tap_panel(&mut self, point: Point, now: Micros, update: &mut Update) {
        let scroll = self.grid.scroll;
        let Some(cell) = panel::cell_at(point, scroll) else {
            return;
        };
        if !panel::in_view(cell.column(), scroll) {
            self.grid.snap_to(panel::scroll_to(cell.column(), scroll), now);
            return;
        }
        let page = match cell {
            Cell::Zone => Page::Picker(Picker::new(&self.peripherals)),
            Cell::Brightness => Page::Brightness(second::Brightness::new(self.peripherals.brightness)),
            Cell::Timeout => Page::Timeout(second::TimeoutChooser::new(self.peripherals.timeout)),
            Cell::AlwaysOn => {
                let on = !self.peripherals.always_on;
                self.peripherals.always_on = on;
                update.store = Some(Store::AlwaysOn(on));
                return;
            }
            Cell::Gnss | Cell::Battery | Cell::Device => Page::Device(second::Device::default()),
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
            Store::ManualZone(id) => zone = ZoneState { mode: ZoneMode::Manual, zone: Some(id) },
            Store::AutomaticZone => zone.mode = ZoneMode::Automatic,
            Store::Timeout(timeout) => self.peripherals.timeout = timeout,
            Store::AlwaysOn(on) => self.peripherals.always_on = on,
            Store::Clear => {
                zone.mode = ZoneMode::Automatic;
                self.peripherals.timeout = Timeout::default();
                self.peripherals.always_on = false;
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
        if let Some((page, _)) = self.page.take() {
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
                let cell = |start: Micros, i: usize| Some(settled + start + PANEL_STAGGER * i as Micros);
                panel::Accents {
                    ring: progress(now, at(0), RING_FADE),
                    title: progress(now, at(0), PANEL_TITLE_REVEAL),
                    rules: progress(now, at(PANEL_RULES), PANEL_RULES_DRAW),
                    rows: core::array::from_fn(|i| rows_built(now, cell(PANEL_ICON, i))),
                    index: core::array::from_fn(|i| progress(now, cell(PANEL_INDEX, i), PANEL_INDEX_REVEAL)),
                    name: core::array::from_fn(|i| progress(now, cell(PANEL_NAME, i), PANEL_NAME_REVEAL)),
                    markers: now >= settled + PANEL_MARKERS,
                    hint: progress(now, at(PANEL_HINT), PANEL_HINT_REVEAL),
                }
            }
            None => panel::Accents::HIDDEN,
        };
        self.fading |= self.panel_settled.is_some() && entry != panel::Accents::FULL;
        // Going up, the accents follow the panel's offset, so reversing a drag restores them.
        let p = swipe_progress(self.sheet.height() - self.sheet.offset());
        let exit = panel::Accents {
            ring: leaving(p, 0.6, 0.4),
            title: leaving(p, 0.5, 0.3),
            rules: leaving(p, 0.4, 0.4),
            rows: [rows_leaving(p, 0.3, 0.4); panel::CELLS],
            index: [leaving(p, 0.15, 0.25); panel::CELLS],
            name: [leaving(p, 0.1, 0.3); panel::CELLS],
            markers: p <= 0.1,
            hint: leaving(p, 0.0, 0.2),
        };
        entry.min(exit)
    }

    /// Advances the compass page's builds and reveals to `now`, and returns how far each has
    /// come.
    fn compass_accents(&mut self, now: Micros) -> Accents {

        if self.screen != Screen::Compass {
            self.compass_settled = None;
            self.top_edge_since = None;
            return Accents::FULL;
        }
        let offset = self.face_offset();
        let mode = Mode::of(&self.peripherals.compass);
        let CompassSettled { times, shown, change } = match &mut self.compass_settled {
            Some(settled) => settled,
            None if offset == 0 => {
                let from = now.max(self.entry_from);
                let start = |delay: Micros| Some(from + delay);
                let mut times = CompassTimes {
                    ring: start(COMPASS_ENTRY.ring),
                    icon: start(COMPASS_ENTRY.icon),
                    caption: start(COMPASS_ENTRY.caption),
                    dial: start(COMPASS_ENTRY.dial),
                };
                if mode == Mode::NoData {
                    // A fault shows at once.
                    (times.ring, times.icon, times.caption) = (None, None, None);
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
                (times.ring, times.icon, times.caption) = (None, None, None);
                *change = None;
            } else {
                // Back from TOP EDGE UP within the grace, the dial and icon show at once.
                let quick = *shown == Mode::TopEdgeUp
                    && mode.heading().is_some()
                    && self
                        .top_edge_since
                        .is_some_and(|since| now.saturating_sub(since) < TOP_EDGE_GRACE);
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
            if mode == Mode::TopEdgeUp && shown.heading().is_some() {
                self.top_edge_since = Some(now);
            } else if mode != Mode::TopEdgeUp {
                self.top_edge_since = None;
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
            ring: progress(now, times.ring, RING_FADE),
            icon_rows: rows_built(now, times.icon),
            caption: progress(now, times.caption, CAPTION_REVEAL),
            dial: progress(now, times.dial, DIAL_SWEEP),
            change: running,
        };
        self.fading |= entry != Accents::FULL;
        // Going out, the accents follow the page's offset, so reversing a drag restores them.
        let p = swipe_progress(offset);
        let exit = Accents {
            ring: leaving(p, 0.6, 0.4),
            icon_rows: rows_leaving(p, 0.3, 0.5),
            caption: leaving(p, 0.2, 0.3),
            dial: leaving(p, 0.2, 0.4),
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
                    ring: start(CLOCK_ENTRY.ring),
                    icon: start(CLOCK_ENTRY.icon),
                    label: start(CLOCK_ENTRY.label),
                    plate: start(CLOCK_ENTRY.plate),
                    zone: start(CLOCK_ENTRY.zone),
                    mark: start(CLOCK_ENTRY.mark),
                };
                if keys.mode == clock_screen::Mode::NoData {
                    // A fault shows at once.
                    (times.ring, times.icon, times.label) = (None, None, None);
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
                    restart(&mut times.zone, now + (CLOCK_ENTRY.zone - CLOCK_ENTRY.plate));
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
            settled.retyped = if replaced { Some(now) } else { settled.retyped.filter(|_| time.is_some()) };
            settled.time = time.map(|time| (time, now));
        }
        let times = &settled.times;
        let retyped = settled.retyped;
        let entry = Accents {
            ring: progress(now, times.ring, RING_FADE),
            icon_rows: rows_built(now, times.icon),
            label: progress(now, times.label, CLOCK_LABEL_REVEAL),
            plate: progress(now, times.plate, CLOCK_PLATE_REVEAL),
            zone: progress(now, times.zone, CLOCK_ZONE_REVEAL),
            mark: progress(now, times.mark, CLOCK_MARK_REVEAL),
            time: progress(now, retyped, CLOCK_TIME_REVEAL),
            date: progress(now, retyped.map(|start| start + CLOCK_DATE_DELAY), CLOCK_DATE_REVEAL),
        };
        self.fading |= entry != Accents::FULL;
        let p = swipe_progress(offset);
        let exit = Accents {
            ring: leaving(p, 0.6, 0.4),
            icon_rows: rows_leaving(p, 0.3, 0.5),
            label: leaving(p, 0.2, 0.3),
            plate: leaving(p, 0.1, 0.3),
            zone: leaving(p, 0.0, 0.2),
            mark: leaving(p, 0.05, 0.2),
            time: u8::MAX,
            date: u8::MAX,
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
            startup::draw(view, startup, &context, &self.renderer, target).expect("drawing the start-up failed");
            return;
        }
        if let (Rest::AlwaysOn, Some(view)) = (self.rest, &self.drawn_always_on) {
            always_on::draw(view, &self.renderer, target).expect("drawing the always-on face failed");
            return;
        }
        if let Some((page, _)) = &self.page {
            screens::clear(target).expect("clearing the panel failed");
            page.draw(&self.peripherals, self.page_accents, &self.renderer, target)
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
