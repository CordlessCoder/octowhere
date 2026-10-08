//! The member face, driven through the stage by the readings, mesh views and gestures that reach
//! it on the device, and drawn as the frame loop draws it.

mod common;

use common::{Buffers, differing};
use embedded_graphics::prelude::Point;
use octowhere_ui::{
    chrome::FB,
    ui::{
        clock::{ClockState, DateTime},
        compass::CompassView,
        group::view::{GroupView, IDS, MemberView, MeshView, Name, Position},
        members,
        rest::Timeout,
        screens::{Gnss, GnssHealth, PeripheralState, Screen},
        second::Page,
        stage::{Input, Motion, Sensors, Stage},
    },
};
use octowhere_ui_script::Driver;

const SECOND: u64 = 1_000_000;
const DUBLIN: (i32, i32) = (533_498_000, -62_603_000);
/// Members 1 to 4: bearing, distance and position age in seconds.
const PLACED: [(f64, f64, u64); 4] = [
    (25.0, 420.0, 11),
    (120.0, 1_200.0, 120),
    (217.0, 780.0, 300),
    (285.0, 2_350.0, 1_080),
];

/// The point `metres` from `from` at `bearing` degrees true.
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

fn member(now: u64) -> MemberView {
    MemberView {
        name: Name::new(b"Ridge").unwrap(),
        mac: [0x48, 0xa1, 0xb2, 0xc3, 0x8c, 0x91],
        device: [0x9c, 0x2a, 0x7f, 0x10, 0, 0, 0, 1],
        joined: Some(0),
        heard: Some(now as i64 - 11 * SECOND as i64),
        position: Position::Never,
        coordinates: None,
    }
}

/// A group of eight, this device first, with `placed` members of [`PLACED`] placed at `now`.
fn group(placed: &[(f64, f64, u64)], now: u64) -> GroupView {
    let mut members = [None; IDS as usize];
    for (id, slot) in members.iter_mut().enumerate().take(8) {
        *slot = Some(MemberView {
            device: [0x9c, 0x2a, 0x7f, 0x10, 0, 0, 0, id as u8],
            ..member(now)
        });
    }
    for (id, &(bearing, metres, age)) in placed.iter().enumerate() {
        members[id + 1] = members[id + 1].map(|member| MemberView {
            position: Position::At(now as i64 - (age * SECOND) as i64),
            coordinates: Some(at(DUBLIN, bearing, metres)),
            ..member
        });
    }
    GroupView { own: 0, members }
}

fn sensors(now: u64, fix: bool) -> Sensors {
    Sensors {
        clock: ClockState {
            utc: Some(
                DateTime {
                    year: 2026,
                    month: 10,
                    day: 4,
                    hour: 12,
                    minute: 0,
                    second: 0,
                }
                .to_unix(),
            ),
            set_from_gnss: true,
            stopped: false,
        },
        gnss: Gnss {
            fix,
            position: Some(DUBLIN),
            hdop_milli: Some(1_100),
            health: GnssHealth {
                last_response: Some(now),
                last_fix: Some(now - 8 * SECOND),
                ..GnssHealth::default()
            },
            ..Gnss::default()
        },
        ..Sensors::default()
    }
}

fn compass(heading: Option<u16>) -> CompassView {
    CompassView {
        live: true,
        calibration_percent: 100,
        heading_decidegrees: heading,
        pitch_deg: 2,
        roll_deg: -3,
        disturbed: false,
    }
}

/// The member face with `group` and a fix, facing 47° magnetic.
fn start(group: impl FnOnce(u64) -> Option<GroupView>) -> Driver<'static> {
    let mut driver = Driver::on(Screen::Members);
    driver.stage = Stage::new(PeripheralState {
        timeout: Timeout::Never,
        ..PeripheralState::default()
    });
    driver.stage.show(Screen::Members);
    driver.wait(30 * SECOND);
    let now = driver.now();
    driver.stage.set_mesh(MeshView {
        group: group(now),
        ..MeshView::default()
    });
    driver.sensors(sensors(now, true));
    driver.motion(Motion {
        compass: compass(Some(470)),
    });
    driver.wait(50_000);
    driver
}

fn shows(driver: &Driver, text: &str) -> bool {
    driver.stage.members_text().any(|shown| shown == text)
}

#[test]
fn the_member_face_follows_the_compass_in_the_ring() {
    let mut driver = start(|now| Some(group(&PLACED, now)));
    driver.stage.show(Screen::Compass);
    driver.swipe(Point::new(400, 233), Point::new(80, 233), 250_000);
    driver.settle();
    assert_eq!(driver.stage.screen(), Screen::Members);
    driver.swipe(Point::new(400, 233), Point::new(80, 233), 250_000);
    driver.settle();
    assert_eq!(driver.stage.screen(), Screen::Clock);
}

#[test]
fn the_heading_is_turned_to_true_north() {
    let mut driver = start(|now| Some(group(&PLACED, now)));
    let declination = members::declination(
        &sensors(driver.now(), true).gnss,
        &sensors(driver.now(), true).clock,
    )
    .expect("the model holds in Dublin in 2026");
    let heading = (47.0 + declination).round() as i32;
    assert!(
        shows(&driver, &format!("{heading:03}° TRUE / FORWARD")),
        "{:?}",
        driver.stage.members_text().collect::<Vec<_>>()
    );
    for compass in [
        CompassView {
            disturbed: true,
            ..compass(Some(470))
        },
        compass(None),
    ] {
        driver.motion(Motion { compass });
        driver.wait(50_000);
        assert!(shows(&driver, "NORTH UP / NO HEADING"));
    }
}

#[test]
fn a_tap_in_the_middle_steps_through_every_placed_member() {
    let mut driver = start(|now| Some(group(&PLACED, now)));
    assert_eq!(
        driver.stage.member(),
        Some(1),
        "the freshest position first"
    );
    let mut seen = Vec::new();
    for _ in 0..PLACED.len() {
        driver.tap(Point::new(233, 233));
        driver.wait(50_000);
        seen.extend(driver.stage.member());
    }
    assert_eq!(seen, [2, 3, 4, 1]);
    // A tap on the rim does nothing.
    driver.tap(Point::new(233, 30));
    driver.wait(50_000);
    assert_eq!(driver.stage.member(), Some(1));
}

#[test]
fn every_member_of_a_crowded_ring_can_be_selected() {
    let crowded = |now| {
        let mut group = group(&[], now);
        for id in 1..IDS {
            let bearing = if id < 12 {
                40.0 + f64::from(id)
            } else {
                f64::from(id) * 137.5 % 360.0
            };
            group.members[usize::from(id)] = Some(MemberView {
                device: [0x9c, 0x2a, 0x7f, 0x10, 0, 0, 0, id],
                position: Position::At(now as i64 - i64::from(id) * SECOND as i64),
                coordinates: Some(at(DUBLIN, bearing, 100.0 * f64::from(id))),
                ..member(now)
            });
        }
        Some(group)
    };
    let mut driver = start(crowded);
    assert!(shows(&driver, "31 POSITIONS / 32 MEMBERS"));
    let labels = driver
        .stage
        .members_text()
        .filter(|text| text.contains(" / ") && text.len() <= 12)
        .count();
    assert!(labels < 31, "neighbours share a node");
    let mut seen = std::collections::BTreeSet::new();
    for _ in 1..IDS {
        // The selected member is always the one its node is labelled with.
        let selected = driver.stage.member().unwrap();
        assert!(
            driver
                .stage
                .members_text()
                .any(|text| text.starts_with(&format!("{selected:02} "))),
        );
        seen.insert(selected);
        driver.tap(Point::new(233, 233));
        driver.wait(50_000);
    }
    assert_eq!(seen.len(), usize::from(IDS) - 1);
}

#[test]
fn an_expired_position_is_not_placed() {
    let mut driver = start(|now| Some(group(&PLACED, now)));
    assert!(shows(&driver, "04 POSITIONS / 08 MEMBERS"));
    let left = members::EXPIRES - 1_080 * SECOND;
    driver.wait(left + SECOND);
    assert!(shows(&driver, "03 POSITIONS / 08 MEMBERS"));
    assert!(
        !driver
            .stage
            .members_text()
            .any(|text| text.starts_with("04 /")),
        "{:?}",
        driver.stage.members_text().collect::<Vec<_>>()
    );
}

#[test]
fn without_a_fix_the_face_gives_coordinates_instead_of_distances() {
    let mut driver = start(|now| Some(group(&PLACED, now)));
    driver.sensors(sensors(driver.now(), false));
    driver.wait(50_000);
    assert!(shows(&driver, "NO OWN FIX"));
    assert!(!shows(&driver, "METRES"));
    assert!(shows(&driver, "POSITION 11S OLD"));
}

#[test]
fn with_no_positions_view_members_opens_the_list() {
    let mut driver = start(|now| Some(group(&[], now)));
    assert!(shows(&driver, "NO POSITIONS"));
    driver.tap(Point::new(233, 347));
    driver.wait(300_000);
    assert!(matches!(driver.stage.page(), Some(Page::Group(_))));
    assert!(driver.stage.group_text().any(|text| text == "MEMBERS"));
}

#[test]
fn without_a_group_view_group_opens_the_group_screen() {
    let mut driver = start(|_| None);
    assert!(shows(&driver, "NO GROUP"));
    driver.tap(Point::new(233, 347));
    driver.wait(300_000);
    assert!(matches!(driver.stage.page(), Some(Page::Group(_))));
}

#[test]
fn the_member_face_samples_motion_fast() {
    let mut driver = start(|now| Some(group(&PLACED, now)));
    assert!(driver.wait(50_000).samples_fast);
    driver.stage.show(Screen::Clock);
    assert!(!driver.wait(50_000).samples_fast);
}

/// Drawing only each step's damage, in two buffers as the frame loop does, must leave the panel
/// as drawing whole would: entering the face, turning, ages ticking, a selection, a new
/// position, losing and regaining the fix, and losing the heading.
#[test]
fn the_member_faces_damage_redraws_what_changed() {
    let mut driver = start(|now| Some(group(&PLACED[..3], now)));
    driver.stage.show(Screen::Compass);
    driver.wait(SECOND);
    let mut buffers = Buffers::new();
    let mut steps = 0;
    driver.observe(move |stage, _| {
        let mut whole = FB::boxed();
        stage.draw(&mut *whole);
        let partial = buffers.draw(stage, stage.changed());
        let wrong = differing(partial, &whole);
        assert_eq!(wrong, 0, "step {steps}: {wrong} pixels differ");
        steps += 1;
    });
    driver.swipe(Point::new(400, 233), Point::new(80, 233), 250_000);
    driver.settle();
    driver.wait(SECOND);
    driver.motion_over(2 * SECOND, |t| Motion {
        compass: compass(Some((470.0 + 900.0 * t) as u16)),
    });
    driver.wait(3 * SECOND);
    driver.tap(Point::new(233, 233));
    driver.wait(SECOND);
    let now = driver.now();
    driver.stage.set_mesh(MeshView {
        group: Some(group(&PLACED, now)),
        ..MeshView::default()
    });
    driver.wait(SECOND);
    driver.sensors(sensors(driver.now(), false));
    driver.wait(SECOND);
    driver.tap(Point::new(233, 300));
    driver.wait(SECOND);
    driver.sensors(sensors(driver.now(), true));
    driver.wait(SECOND);
    driver.motion(Motion {
        compass: CompassView {
            disturbed: true,
            ..compass(Some(470))
        },
    });
    driver.wait(SECOND);
    driver.swipe(Point::new(233, 430), Point::new(233, 120), 250_000);
    driver.settle();
    driver.wait(SECOND);
    driver.motion(Motion {
        compass: compass(Some(1470)),
    });
    driver.wait(SECOND);
    driver.swipe(Point::new(233, 200), Point::new(233, 420), 250_000);
    driver.settle();
    driver.wait(SECOND);
}

/// The open drawer covers the face, which is not built under it, and is built at the heading it
/// turned to meanwhile as the drawer closes.
#[test]
fn the_open_drawer_leaves_the_face_unbuilt_until_it_closes() {
    let mut driver = start(|now| Some(group(&PLACED, now)));
    driver.swipe(Point::new(233, 430), Point::new(233, 120), 250_000);
    driver.settle();
    driver.wait(300_000);
    assert!(driver.stage.drawer().is_some());
    assert_eq!(driver.stage.members_text().count(), 0);
    driver.motion(Motion {
        compass: compass(Some(1470)),
    });
    driver.wait(SECOND);
    assert_eq!(driver.stage.members_text().count(), 0);
    driver.swipe(Point::new(233, 200), Point::new(233, 420), 250_000);
    driver.settle();
    driver.wait(300_000);
    assert!(driver.stage.drawer().is_none());
    let declination = members::declination(
        &sensors(driver.now(), true).gnss,
        &sensors(driver.now(), true).clock,
    )
    .expect("the model holds in Dublin in 2026");
    let heading = (147.0 + declination).round() as i32;
    assert!(
        shows(&driver, &format!("{heading:03}° TRUE / FORWARD")),
        "{:?}",
        driver.stage.members_text().collect::<Vec<_>>()
    );
}

#[test]
fn an_age_ticking_redraws_only_what_shows_it() {
    let mut driver = start(|now| Some(group(&PLACED, now)));
    driver.wait(SECOND);
    let mut partial = 0;
    for _ in 0..90 {
        driver.wait(16_667);
        let changed = driver.stage.changed();
        if !changed.is_empty() {
            assert!(!changed.is_full(), "an age's tick damages its text alone");
            assert!(changed.pixels() < 466 * 466 / 10, "{}", changed.pixels());
            partial += 1;
        }
    }
    assert!(partial > 0, "the ages ticked");
}

/// A sample that leaves the held heading where it is changes nothing the member face shows. A
/// heading at rest lies within half a degree of the whole degree shown, so a quarter of a degree
/// either way always stays held, and a degree and a quarter always moves it.
#[test]
fn the_member_face_asks_only_for_motion_that_moves_its_heading() {
    let mut driver = start(|now| Some(group(&PLACED, now)));
    let needs =
        |driver: &Driver, compass: CompassView| driver.stage.needs_motion(&Motion { compass });
    assert!(!needs(&driver, compass(Some(470))));
    assert!(
        !needs(&driver, compass(Some(472))),
        "a wobble within the hold"
    );
    assert!(
        !needs(
            &driver,
            CompassView {
                pitch_deg: 9,
                roll_deg: 4,
                ..compass(Some(468))
            }
        ),
        "a tilt the face does not show"
    );
    assert!(needs(&driver, compass(Some(490))), "a turn of two degrees");
    assert!(
        needs(
            &driver,
            CompassView {
                disturbed: true,
                ..compass(Some(470))
            }
        ),
        "a heading it stops trusting"
    );

    // The compass face and the panel show every change.
    driver.stage.show(Screen::Compass);
    driver.wait(SECOND);
    assert!(needs(&driver, compass(Some(471))));
    driver.stage.show(Screen::Clock);
    driver.wait(SECOND);
    assert!(!needs(&driver, compass(Some(472))));
    driver.swipe(Point::new(233, 80), Point::new(233, 420), 250_000);
    driver.settle();
    assert!(needs(&driver, compass(Some(476))));
}

/// Where the heading is withheld, a turn far enough to count as use is still asked for, so the
/// screen's timeout restarts as it would.
#[test]
fn a_turn_counts_as_use_while_the_heading_is_withheld() {
    let mut driver = start(|now| Some(group(&PLACED, now)));
    let disturbed = |heading| Motion {
        compass: CompassView {
            disturbed: true,
            ..compass(Some(heading))
        },
    };
    driver.motion(disturbed(470));
    driver.wait(SECOND);
    assert!(!driver.stage.needs_motion(&disturbed(520)));
    assert!(driver.stage.needs_motion(&disturbed(580)));
}

/// Stepping only for the samples the stage asks for, and handing it the rest with the sensor
/// task's next wake, shows what stepping for every sample shows each time it steps.
#[test]
fn the_member_face_shows_the_same_stepped_only_for_the_motion_it_asks_for() {
    let mut every = start(|now| Some(group(&PLACED, now))).stage;
    let mut asked = start(|now| Some(group(&PLACED, now))).stage;
    let from = start(|now| Some(group(&PLACED, now))).now();
    // Still, wobbling within the hold, then turning 10° over a second, then still again.
    let heading = |sample: u64| -> u16 {
        let wobble = [0, 3, -4, 6, -2, 5][sample as usize % 6];
        let turned = sample.saturating_sub(50).min(50) * 2;
        (470 + turned as i32 + wobble) as u16
    };
    let mut stepped = 0;
    let (mut whole_every, mut whole_asked) = (FB::boxed(), FB::boxed());
    for sample in 1..=150 {
        let now = from + sample * 20_000;
        let motion = Motion {
            compass: CompassView {
                pitch_deg: (sample % 7) as i8,
                ..compass(Some(heading(sample)))
            },
        };
        let wake = |stage: &mut Stage, motion| {
            stage.step(Input {
                now,
                motion,
                ..Input::default()
            });
        };
        wake(&mut every, Some(motion));
        // The sensor task wakes the frame loop every 250 ms, and that step takes the sample.
        let tick = now % 250_000 < 20_000;
        if !tick && !asked.needs_motion(&motion) {
            continue;
        }
        wake(&mut asked, Some(motion));
        stepped += 1;
        every.draw(&mut *whole_every);
        asked.draw(&mut *whole_asked);
        let wrong = differing(&whole_every, &whole_asked);
        assert_eq!(wrong, 0, "sample {sample}: {wrong} pixels differ");
    }
    assert!(
        (10..60).contains(&stepped),
        "stepped for {stepped} of 150 samples"
    );
}
