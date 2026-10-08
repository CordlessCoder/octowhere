//! `crowded-screens-bench`'s run on the host: the fixture in place of the mesh, and the gestures
//! and heading `tools/crowded-screens-bench.py` gives the board, stepped as the board's frame
//! loop steps the stage, a pass at a time, with touch-inject's strokes sampled at each pass. Each
//! phase is checked to show what the script names it for, at several frame periods, since the
//! board's depend on what it draws.

use embedded_graphics::prelude::Point;
use octowhere_ui::{
    tz::{DATABASE, DateTime},
    ui::{
        clock::{ClockState, ZoneMode, ZoneState},
        drawer::{Child, Root},
        events::HISTORY,
        group::view::{
            GroupView, IDS, MemberView, MeshView, MessagesView, Name, Position, Request, Thread,
        },
        rest::Timeout,
        screens::{Gnss, PeripheralState, Screen},
        stage::{Input, Motion, Sensors, Stage, Touch},
    },
};

use crate::crowded_fixture::{self as fixture, CENTRE, COUNT};

const SECOND: u64 = 1_000_000;
/// The script's turning rate, in degrees a second.
const TURN: i32 = 30;
/// The script's gestures: from the clock face to the member face, the middle that selects the
/// next member, the drawer's upward drag, a list's scroll, the switch to Messages, the first row
/// and the title that goes back or closes.
const TO_MEMBERS: (Point, Point, u64) = (Point::new(80, 233), Point::new(400, 233), 250);
const MIDDLE: Point = Point::new(233, 233);
const OPEN_DRAWER: (Point, Point, u64) = (Point::new(233, 430), Point::new(233, 120), 250);
const SCROLL_DOWN: (Point, Point, u64) = (Point::new(233, 380), Point::new(233, 133), 400);
const SCROLL_UP: (Point, Point, u64) = (Point::new(233, 133), Point::new(233, 380), 400);
const TO_MESSAGES: (Point, Point, u64) = (Point::new(380, 260), Point::new(80, 260), 300);
const FIRST_ROW: Point = Point::new(233, 170);
const TITLE: Point = Point::new(233, 25);
/// The drawer lists' rows, viewport and the first row's gap, as `drawer` lays them out, from
/// which the script works out how many scrolls reach a list's end.
const EVENT_ROW: i32 = 178;
const INBOX_ROW: i32 = 95;
const VIEWPORT: i32 = 287;
const FIRST: i32 = 4;

/// The swipes that take a list of `rows` rows `row` high from its top to its end: each scrolls by
/// its length, and the last stops at the end.
fn scrolls(rows: usize, row: i32) -> usize {
    let travel = SCROLL_DOWN.0.y - SCROLL_DOWN.1.y;
    let end = (FIRST + rows as i32 * row - VIEWPORT) as u32;
    end.div_ceil(travel as u32) as usize
}

/// The board's frame loop with the bench on.
struct Board {
    stage: Stage,
    now: u64,
    period: u64,
    /// touch-inject's stroke under way: from, to, the pass it started at and its length.
    stroke: Option<(Point, Point, u64, u64)>,
    /// The heading turning since, from, at a rate.
    turn: Option<(u64, u16, i32)>,
    heading: u16,
    /// The members' messages read, by bit.
    read: u32,
    own: u8,
}

impl Board {
    fn new(period: u64, mut view: MeshView) -> Self {
        let mut stage = Stage::new(PeripheralState {
            firmware: "0.1.0",
            timeout: Timeout::Never,
            ..PeripheralState::default()
        });
        stage.show(Screen::Clock);
        stage.use_messages(Box::leak(MessagesView::boxed()));
        let now = SECOND;
        fixture::place(&mut view, now as i64);
        let own = view.group.as_ref().expect("placing makes a group").own;
        stage.set_mesh(view);
        stage.update_messages(|messages| {
            fixture::add_messages(messages, own, 0, now as i64);
            true
        });
        let utc = DateTime {
            year: 2026,
            month: 10,
            day: 8,
            hour: 12,
            minute: 0,
            second: 0,
        }
        .to_unix();
        stage.step(Input {
            now,
            sensors: Some(Sensors {
                clock: ClockState {
                    utc: Some(utc),
                    set_from_gnss: true,
                    stopped: false,
                },
                zone: ZoneState {
                    mode: ZoneMode::Automatic,
                    zone: DATABASE.find("Europe/Dublin").map(|zone| zone.id),
                },
                battery: None,
                gnss: Gnss {
                    fix: true,
                    position: Some(CENTRE),
                    hdop_milli: Some(1_000),
                    ..Gnss::default()
                },
            }),
            ..Input::default()
        });
        Self {
            stage,
            now,
            period,
            stroke: None,
            turn: None,
            heading: 0,
            read: 0,
            own,
        }
    }

    /// The report touch-inject's stroke gives at this pass, as `Injector::next` does.
    fn injected(&mut self) -> Option<Touch> {
        let (from, to, start, duration) = self.stroke?;
        let elapsed = self.now - start;
        if elapsed > duration && elapsed >= 1_000 {
            self.stroke = None;
            return Some(Touch::Lifted(to));
        }
        let (done, whole) = (elapsed.min(duration) as i64, duration.max(1) as i64);
        let along = |from: i32, to: i32| from + (i64::from(to - from) * done / whole) as i32;
        Some(Touch::Contacts([
            Some(Point::new(along(from.x, to.x), along(from.y, to.y))),
            None,
        ]))
    }

    /// One pass of the frame loop. Returns whether it redrew the whole panel.
    fn pass(&mut self) -> bool {
        self.now += self.period;
        let touch = self.injected();
        let motion = self.turn.map(|(since, from, rate)| {
            let compass = fixture::turned(from, rate, self.now - since);
            self.heading = compass.heading_decidegrees.unwrap_or(0);
            Motion { compass }
        });
        let update = self.stage.step(Input {
            now: self.now,
            touch,
            motion,
            ..Input::default()
        });
        if let Some(Request::Read(id)) = update.mesh
            && let Some(index) = fixture::sender(id)
        {
            self.read |= 1 << index;
        }
        self.stage.changed().is_full()
    }

    fn wait(&mut self, duration: u64) -> Vec<bool> {
        let end = self.now + duration;
        let mut full = Vec::new();
        while self.now < end {
            full.push(self.pass());
        }
        full
    }

    /// A stroke as touch-inject gives it, starting at the next pass, then a second's rest.
    fn stroke(&mut self, (from, to, ms): (Point, Point, u64)) {
        self.stroke = Some((from, to, self.now + self.period, ms * 1_000));
        while self.stroke.is_some() {
            self.pass();
        }
        self.wait(SECOND);
    }

    fn tap(&mut self, point: Point) {
        self.stroke((point, point, 0));
    }

    fn turning(&mut self, rate: Option<i32>) {
        self.turn = rate.map(|rate| (self.now, self.heading, rate));
    }

    /// The member face's selection, as the fixture's member it is.
    fn selected(&self) -> Option<usize> {
        let id = self.stage.member()?;
        fixture::ids(self.own).position(|each| each == id)
    }

    fn members_text(&self) -> Vec<&str> {
        self.stage.members_text().collect()
    }

    fn drawer(&self) -> (Root, Option<Child>) {
        let drawer = self.stage.drawer().expect("the drawer is open");
        (drawer.root(), drawer.child())
    }

    /// How far the drawer's root lists are scrolled.
    fn scrolled(&self) -> [i32; 2] {
        let drawer = format!("{:?}", self.stage.drawer().expect("the drawer is open"));
        let mut offsets = drawer.split("offset: ").skip(1).map(|rest| {
            rest.split(|c: char| !c.is_ascii_digit())
                .next()
                .and_then(|digits| digits.parse().ok())
                .expect("a scroll's offset")
        });
        [offsets.next().unwrap(), offsets.next().unwrap()]
    }
}

fn device(name: &str, last: u8) -> MemberView {
    MemberView {
        name: Name::new(name.as_bytes()).unwrap(),
        mac: [0x48, 0xa1, 0xb2, 0xc3, 0x8c, last],
        device: [last; 8],
        joined: None,
        heard: None,
        position: Position::Never,
        coordinates: None,
    }
}

/// The group the node published: this device at `own`, and the other board at 0.
fn grouped(own: u8) -> MeshView {
    let mut members = [None; IDS as usize];
    members[usize::from(own)] = Some(device("Octowhere", 1));
    members[0] = Some(device("Other", 2));
    MeshView {
        group: Some(GroupView { own, members }),
        ..MeshView::default()
    }
}

#[test]
fn this_device_keeps_its_place_and_the_others_give_way() {
    let mut view = grouped(5);
    fixture::place(&mut view, 0);
    let group = view.group.unwrap();
    assert_eq!(group.own, 5);
    assert_eq!(group.member(5), Some(&device("Octowhere", 1)));
    assert_eq!(group.count(), usize::from(IDS));
    assert_eq!(group.member(0).unwrap().name.as_str(), "Kestrel");
    assert_eq!(group.member(4).unwrap().name.as_str(), "Lough");
    assert_eq!(group.member(6).unwrap().name.as_str(), "Basecamp");
}

fn run(period: u64, view: MeshView) {
    let mut board = Board::new(period, view);
    let events = board.stage.events();
    assert_eq!(events.len(), COUNT, "a conversation event from each member");
    assert!(events.len() > HISTORY);
    assert_eq!(events.unread(), COUNT);
    let messages = board.stage.messages().unwrap();
    assert_eq!(fixture::conversations(messages, board.own), COUNT);
    assert_eq!(messages.unread(), COUNT);

    // clock
    board.wait(3 * SECOND);
    assert_eq!(board.stage.screen(), Screen::Clock);

    // members
    board.stroke(TO_MEMBERS);
    board.wait(SECOND);
    assert_eq!(board.stage.screen(), Screen::Members);
    let text = board.members_text();
    assert!(text.contains(&"31 POSITIONS / 32 MEMBERS"), "{text:?}");
    assert!(text.contains(&"NORTH UP / NO HEADING"), "{text:?}");
    // The freshest, first of the 035-061° run, is selected until a tap says otherwise.
    assert_eq!(board.selected(), Some(1));

    // turn
    board.turning(Some(TURN));
    let full = board.wait(30 * SECOND);
    let text = board.members_text();
    assert!(
        text.iter().any(|line| line.ends_with("° TRUE / FORWARD")),
        "{text:?}"
    );
    if period >= 50_000 {
        assert!(
            full.iter().skip(1).all(|&full| full),
            "the face redraws whole every pass while it turns: {} of {}",
            full.iter().filter(|&&full| full).count(),
            full.len()
        );
    }

    // select: thirty taps step from the freshest through every member to Kestrel, at 043°
    // inside the 035-061° run.
    board.turning(None);
    for tap in 1..=COUNT - 1 {
        board.tap(MIDDLE);
        assert_eq!(board.selected(), Some((1 + tap) % COUNT));
    }
    assert_eq!(board.selected(), Some(0));
    let text = board.members_text();
    assert!(
        text.iter().any(|line| line.starts_with("SECTOR +07 / ")),
        "Kestrel's sector holds the run's seven: {text:?}"
    );
    let kestrel = format!("{:02} / ", fixture::ids(board.own).next().unwrap());
    assert!(
        text.iter().any(|line| line.starts_with(&kestrel)),
        "{text:?}"
    );

    // turn-selected
    board.turning(Some(TURN));
    let full = board.wait(20 * SECOND);
    if period >= 50_000 {
        assert!(full.iter().skip(1).all(|&full| full));
    }
    assert_eq!(board.selected(), Some(0));

    // deselect: the next tap is the freshest again, the face's own choice.
    board.turning(None);
    board.tap(MIDDLE);
    assert_eq!(board.selected(), Some(1));

    // drawer-open
    board.stroke(OPEN_DRAWER);
    board.wait(2 * SECOND);
    assert_eq!(board.drawer(), (Root::Events, None));
    assert_eq!(board.scrolled(), [0, 0]);

    // events-scroll: down to the end and back, which a pull at the top would close.
    let events = board.stage.events().len();
    let swipes = scrolls(events, EVENT_ROW);
    let end = FIRST + events as i32 * EVENT_ROW - VIEWPORT;
    for swipe in 1..=swipes {
        board.stroke(SCROLL_DOWN);
        let travelled = (swipe as i32 * (SCROLL_DOWN.0.y - SCROLL_DOWN.1.y)).min(end);
        assert_eq!(board.scrolled()[0], travelled);
    }
    assert_eq!(board.scrolled()[0], end);
    for _ in 0..swipes {
        board.stroke(SCROLL_UP);
    }
    assert_eq!(board.scrolled()[0], 0);
    assert_eq!(board.drawer(), (Root::Events, None));

    // messages-switch
    board.stroke(TO_MESSAGES);
    board.wait(SECOND);
    assert_eq!(board.drawer(), (Root::Messages, None));

    // messages-scroll
    let conversations = fixture::conversations(board.stage.messages().unwrap(), board.own);
    let swipes = scrolls(conversations, INBOX_ROW);
    for _ in 0..swipes {
        board.stroke(SCROLL_DOWN);
    }
    assert_eq!(
        board.scrolled()[1],
        FIRST + conversations as i32 * INBOX_ROW - VIEWPORT
    );
    for _ in 0..swipes {
        board.stroke(SCROLL_UP);
    }
    assert_eq!(board.scrolled()[1], 0);
    assert_eq!(board.drawer(), (Root::Messages, None));

    // conversation: the first row is the newest, the last member's, and its message counts as
    // read once it has shown whole for a second.
    let last = fixture::ids(board.own).last().unwrap();
    board.tap(FIRST_ROW);
    board.wait(3 * SECOND);
    match board.drawer() {
        (Root::Messages, Some(Child::Thread(Thread::Member(id, _)))) => assert_eq!(id, last),
        other => panic!("{other:?}"),
    }
    assert_eq!(board.read, 1 << (COUNT - 1));
    assert_eq!(board.stage.messages().unwrap().unread(), COUNT - 1);

    // back
    board.tap(TITLE);
    assert_eq!(board.drawer(), (Root::Messages, None));

    // close
    board.tap(TITLE);
    board.wait(SECOND);
    assert!(board.stage.drawer().is_none());
    assert_eq!(board.stage.screen(), Screen::Members);
}

/// Frame periods from the clock face at rest to the member face's full draws.
const PERIODS: [u64; 4] = [16_667, 50_000, 100_000, 150_000];

#[test]
fn the_run_reaches_each_phase_in_no_group() {
    for period in PERIODS {
        run(period, MeshView::default());
    }
}

#[test]
fn the_run_reaches_each_phase_in_a_group() {
    for period in PERIODS {
        run(period, grouped(5));
    }
}

/// The render's `at` and the hand-off's runs, which [`fixture`]'s offsets are taken from.
#[test]
fn the_members_stand_where_the_render_puts_them() {
    fn at(from: (i32, i32), bearing: f64, metres: f64) -> (i32, i32) {
        let (phi1, lambda1) = (
            (f64::from(from.0) * 1e-7).to_radians(),
            (f64::from(from.1) * 1e-7).to_radians(),
        );
        let (d, theta) = (metres / 6_371_008.8, bearing.to_radians());
        let phi2 = (phi1.sin() * d.cos() + phi1.cos() * d.sin() * theta.cos()).asin();
        let lambda2 =
            lambda1 + (theta.sin() * d.sin() * phi1.cos()).atan2(d.cos() - phi1.sin() * phi2.sin());
        (
            (phi2.to_degrees() * 1e7).round() as i32,
            (lambda2.to_degrees() * 1e7).round() as i32,
        )
    }
    const RUNS: [(f64, f64, usize, u64, u64); 4] = [
        (35.0, 61.0, 7, 11, 1_080),
        (118.0, 154.0, 8, 23, 420),
        (218.0, 246.0, 8, 120, 1_080),
        (305.0, 335.0, 7, 11, 180),
    ];
    let mut expected = vec![(at(CENTRE, 43.0, 150.0), 23, 7)];
    for (first, last, count, youngest, oldest) in RUNS {
        for i in 0..count {
            let share = i as f64 / (count - 1) as f64;
            let age = youngest + ((oldest - youngest) as f64 * share) as u64;
            let bearing = (first + (last - first) * share) % 360.0;
            expected.push((at(CENTRE, bearing, 400.0), age, 60));
        }
    }
    let now = 10_000 * SECOND as i64;
    let mut view = MeshView::default();
    fixture::place(&mut view, now);
    let group = view.group.unwrap();
    for (index, id) in fixture::ids(group.own).enumerate() {
        let member = group.member(id).unwrap();
        let (coordinates, age, heard) = expected[index];
        let ago = |seconds: u64| now - (seconds * SECOND) as i64;
        assert_eq!(member.coordinates, Some(coordinates), "member {index}");
        assert_eq!(member.position, Position::At(ago(age)));
        assert_eq!(member.heard, Some(ago(heard)));
    }
}
