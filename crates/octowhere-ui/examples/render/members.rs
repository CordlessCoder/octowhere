//! The member face in each of its states, with the hand-off's fixture: this device in Dublin
//! facing 062° true, and members at 025°, 120°, 217° and 285°, 420 m and further, their
//! positions 11 s to 18 min old. Names match the hand-off's targets where one exists.

use embedded_graphics::prelude::Point;
use octowhere_ui::{
    chrome::FB,
    ui::{
        compass::CompassView,
        group::view::{GroupView, IDS, MemberView, MeshView, Name, Position},
        members,
        rest::Timeout,
        screens::{Gnss, GnssHealth, PeripheralState, Screen},
        script::Driver,
        stage::{Motion, Sensors, Stage},
    },
};

const SECOND: u64 = 1_000_000;
const DUBLIN: (i32, i32) = (533_498_000, -62_603_000);
/// The fixture's true heading.
const HEADING: f64 = 62.0;

/// Each member's name, bearing, distance, position age and when it was last heard directly,
/// in seconds; members past these have no position.
const PLACED: [(&str, f64, f64, u64, Option<u64>); 4] = [
    ("Ridge", 25.0, 420.0, 11, Some(11)),
    ("NORTH-1", 120.0, 1_200.0, 120, Some(40)),
    ("Ana's Watch", 217.0, 780.0, 300, None),
    ("Harbour", 285.0, 2_350.0, 1_080, Some(600)),
];
const UNPLACED: [&str; 3] = ["Basecamp", "Kestrel", "Lough"];

/// The point `metres` from `from` at `bearing` degrees true, on a sphere.
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

fn member(name: &str, now: u64) -> MemberView {
    MemberView {
        name: Name::new(name.as_bytes()).expect("a fixture name is printable"),
        mac: [0x48, 0xa1, 0xb2, 0xc3, 0x8c, name.len() as u8],
        joined: Some(now as i64 - 86_400 * SECOND as i64),
        heard: None,
        position: Position::Never,
        coordinates: None,
    }
}

/// A group of `count` members, this device first, with the first `placed` of [`PLACED`]
/// placed, aged at `now`.
fn group(count: usize, placed: usize, now: u64) -> GroupView {
    let mut members = [None; IDS as usize];
    members[0] = Some(MemberView {
        position: Position::At(now as i64),
        coordinates: Some(DUBLIN),
        ..member("Octowhere", now)
    });
    let ago = |seconds: u64| now as i64 - (seconds * SECOND) as i64;
    for (id, &(name, bearing, metres, age, heard)) in PLACED.iter().enumerate().take(placed) {
        members[id + 1] = Some(MemberView {
            heard: heard.map(ago),
            position: Position::At(ago(age)),
            coordinates: Some(at(DUBLIN, bearing, metres)),
            ..member(name, now)
        });
    }
    for id in placed + 1..count {
        let name = UNPLACED[(id - placed - 1) % UNPLACED.len()];
        members[id] = Some(member(name, now));
    }
    GroupView { own: 0, members }
}

/// `count` members all placed round this device, for a crowded ring: some in a tight cluster,
/// two at one bearing, the rest spread.
fn crowded(count: usize, now: u64) -> GroupView {
    let mut members = [None; IDS as usize];
    members[0] = Some(member("Octowhere", now));
    for id in 1..count {
        let bearing = match id {
            1..=5 => 40.0 + id as f64 * 3.0,
            6 | 7 => 200.0,
            _ => (id as f64 * 137.5) % 360.0,
        };
        members[id] = Some(MemberView {
            heard: Some(now as i64 - (id as u64 * 7 * SECOND) as i64),
            position: Position::At(now as i64 - (id as u64 * 23 * SECOND) as i64),
            coordinates: Some(at(DUBLIN, bearing, 150.0 * id as f64)),
            ..member(UNPLACED[id % UNPLACED.len()], now)
        });
    }
    GroupView { own: 0, members }
}

/// The readings: a fix in Dublin taken 8 s ago with HDOP 1.1, or none.
fn sensors(now: u64, fix: bool) -> Sensors {
    let base = super::sensors();
    Sensors {
        gnss: Gnss {
            fix,
            hdop_milli: fix.then_some(1_100),
            health: GnssHealth {
                last_response: Some(now),
                last_fix: Some(now - 8 * SECOND),
                ..GnssHealth::default()
            },
            ..base.gnss
        },
        ..base
    }
}

/// The compass reading whose true heading, with Dublin's declination, is [`HEADING`].
fn compass(heading: bool) -> CompassView {
    let base = super::sensors();
    let declination = members::declination(&base.gnss, &base.clock).expect("Dublin has one");
    let magnetic = (HEADING - f64::from(declination)).rem_euclid(360.0);
    CompassView {
        live: true,
        calibration_percent: 100,
        heading_decidegrees: heading.then_some((magnetic * 10.0).round() as u16),
        pitch_deg: 3,
        roll_deg: -4,
        disturbed: false,
    }
}

/// The member face with `mesh` built at the stage's time, its readings fresh.
fn face(mesh: impl Fn(u64) -> Option<GroupView>, fix: bool, heading: bool) -> Driver<'static> {
    let mut driver = Driver::on(Screen::Members);
    driver.stage = Stage::new(PeripheralState {
        firmware: "0.1.0",
        timeout: Timeout::Never,
        ..PeripheralState::default()
    });
    driver.stage.show(Screen::Members);
    // Long enough for the fix to have been taken 8 s before.
    driver.wait(30 * SECOND);
    let now = driver.now();
    driver.stage.set_mesh(MeshView {
        group: mesh(now),
        ..MeshView::default()
    });
    driver.sensors(sensors(now, fix));
    driver.motion(Motion {
        compass: compass(heading),
    });
    driver.wait(20_000);
    driver
}

fn snap(frames: &mut Vec<(String, Box<FB>)>, name: &str, driver: &Driver) {
    let mut fb = FB::boxed();
    driver.stage.draw(&mut *fb);
    frames.push((name.into(), fb));
}

pub fn frames() -> Vec<(String, Box<FB>)> {
    let mut frames = Vec::new();
    let four = |now| Some(group(8, 4, now));
    snap(
        &mut frames,
        "members-bearing-circle-four",
        &face(four, true, true),
    );
    snap(
        &mut frames,
        "members-bearing-circle-one",
        &face(|now| Some(group(2, 1, now)), true, true),
    );
    snap(
        &mut frames,
        "members-heading-unavailable",
        &face(four, true, false),
    );
    snap(
        &mut frames,
        "members-no-own-fix",
        &face(|now| Some(group(8, 3, now)), false, true),
    );
    snap(
        &mut frames,
        "members-none",
        &face(|now| Some(group(8, 0, now)), true, true),
    );
    snap(&mut frames, "members-no-group", &face(|_| None, true, true));
    let mut next = face(four, true, true);
    next.tap(Point::new(233, 233));
    next.wait(20_000);
    snap(&mut frames, "members-selected-next", &next);
    snap(
        &mut frames,
        "members-crowded",
        &face(|now| Some(crowded(32, now)), true, true),
    );
    // A third of the way across from the compass.
    let mut swiping = face(four, true, true);
    swiping.stage.show(Screen::Compass);
    swiping.wait(20_000);
    for (step, x) in [400, 340, 280, 245].into_iter().enumerate() {
        swiping.touch(Some(Point::new(x, 233)));
        swiping.wait(16_667 * (step as u64 + 1) - 16_667 * step as u64);
    }
    snap(&mut frames, "members-swiping", &swiping);
    frames
}
