mod common;

use common::{Buffers, differing, differing_as_shown};
use embedded_graphics::prelude::{Point, Size};
use embedded_graphics::primitives::Rectangle;
use octowhere_ui::{
    chrome::{self, Color, Dirty, FB, Window},
    tz::DATABASE,
    ui::{
        clock::{ClockState, DateTime, ZoneMode, ZoneState},
        clock_screen::Accents as ClockAccents,
        compass::CompassView,
        compass_screen::{Accents, CENTER as COMPASS_CENTER},
        gesture::{LIFT_GRACE, Micros, SILENT_LIFT},
        screens::Screen,
        script::{self, Driver},
        stage::{Input, Motion, Sensors, Stage, Touch, TouchGesture},
        startup::{Outcome, Part},
    },
};

/// Steps until the compass page's entry has faded in.
fn settled_on_compass() -> Driver<'static> {
    let mut driver = Driver::on(Screen::Compass);
    driver.motion(heading(470));
    driver.wait(500_000);
    driver
}

fn heading(decidegrees: u16) -> Motion {
    Motion {
        compass: CompassView {
            live: true,
            calibration_percent: 100,
            heading_decidegrees: Some(decidegrees),
            ..CompassView::default()
        },
    }
}

#[test]
fn a_swipe_left_moves_to_the_next_screen_and_right_moves_back() {
    let mut driver = Driver::new();
    let left: Vec<_> = (0..8)
        .map(|step| Point::new(400 - step * 40, 233))
        .collect();
    driver.stroke(&left);
    driver.settle();
    assert_eq!(driver.stage.screen(), Screen::Compass);

    let right: Vec<_> = left.iter().rev().copied().collect();
    driver.stroke(&right);
    driver.settle();
    assert_eq!(driver.stage.screen(), Screen::Clock);
}

#[test]
fn a_cover_on_the_compass_goes_to_the_clock_and_leaves_the_calibration() {
    let mut driver = settled_on_compass();
    assert!(!driver.cover().recalibrate);
    driver.settle();
    assert_eq!(driver.stage.screen(), Screen::Clock);
    assert_eq!(
        driver.stage.peripherals().compass.heading_decidegrees,
        Some(470)
    );
}

#[test]
fn a_cover_on_the_clock_changes_nothing() {
    let mut driver = Driver::on(Screen::Clock);
    driver.wait(700_000);
    driver.cover();
    assert!(!driver.stage.is_changing());
    assert_eq!(driver.stage.screen(), Screen::Clock);
}

#[test]
fn a_tap_on_the_compass_does_nothing() {
    let mut driver = settled_on_compass();
    let updates = driver.stroke(&[COMPASS_CENTER + Point::new(30, -40)]);
    assert!(updates.iter().all(|update| !update.recalibrate));
    driver.tap(Point::new(233, 73));
    assert!(!driver.stage.is_changing());
    assert_eq!(driver.stage.screen(), Screen::Compass);
}

fn accents(driver: &Driver) -> Accents {
    let mut fb = FB::boxed();
    driver.stage.draw(&mut *fb);
    driver.stage.accents()
}

#[test]
fn the_compass_accents_build_after_the_page_settles() {
    let mut driver = Driver::on(Screen::Compass);
    driver.motion(heading(470));
    assert_eq!(accents(&driver).icon_rows, 0);
    assert_eq!(accents(&driver).texture, 0);
    assert!(driver.stage.is_animating());
    driver.wait(40_000);
    assert_eq!(accents(&driver).texture, 1);
    driver.wait(60_000);
    assert_eq!(accents(&driver).texture, 2);
    driver.wait(50_000);
    assert_eq!(accents(&driver).texture, 3);
    let early = accents(&driver);
    assert!(early.icon_rows > 0 && early.icon_rows < 5, "{early:?}");
    assert_eq!(early.dial, 0);
    driver.wait(300_000);
    assert_eq!(accents(&driver), Accents::FULL);
    assert!(!driver.stage.is_changing());
}

#[test]
fn a_swipe_off_the_compass_takes_its_accents_reversibly() {
    let mut driver = settled_on_compass();
    driver.touch(Some(Point::new(400, 233)));
    driver.touch(Some(Point::new(360, 233)));
    driver.touch(Some(Point::new(340, 233)));
    let part = accents(&driver);
    assert!(part.caption < 255 && part.icon_rows > 0, "{part:?}");
    driver.touch(Some(Point::new(200, 233)));
    assert_eq!(
        accents(&driver),
        Accents {
            texture: 5,
            field: 102,
            ..Accents::HIDDEN
        }
    );
    driver.touch(Some(Point::new(399, 233)));
    driver.touch(None);
    driver.touch(None);
    driver.touch(None);
    driver.settle();
    assert_eq!(driver.stage.screen(), Screen::Compass);
    assert_eq!(accents(&driver), Accents::FULL, "the entry replayed");
}

#[test]
fn the_compass_accents_stay_hidden_while_it_slides_in() {
    let mut driver = Driver::on(Screen::Clock);
    driver.motion(heading(470));
    driver.wait(500_000);
    driver.touch(Some(Point::new(400, 233)));
    driver.touch(Some(Point::new(200, 233)));
    driver.touch(Some(Point::new(10, 233)));
    driver.lift();
    for _ in 0..120 {
        if driver.stage.screen() == Screen::Compass {
            break;
        }
        assert_eq!(
            driver.stage.accents().caption,
            255,
            "an unsettled compass changed its accents"
        );
        driver.step(Input::default());
    }
    assert_eq!(driver.stage.screen(), Screen::Compass);
    assert_eq!(
        accents(&driver).caption,
        0,
        "the entry began before the page settled"
    );
}

#[test]
fn a_heading_after_calibration_sweeps_the_dial_and_rebuilds_the_icon() {
    let mut driver = Driver::on(Screen::Compass);
    let mut calibrating = heading(470);
    calibrating.compass.heading_decidegrees = None;
    calibrating.compass.calibration_percent = 99;
    driver.motion(calibrating);
    driver.wait(500_000);
    driver.motion(heading(470));
    driver.wait(60_000);
    let early = accents(&driver);
    assert!(early.dial > 0 && early.dial < 255, "{early:?}");
    assert!(early.icon_rows < 5 && early.caption < 255, "{early:?}");
    driver.wait(200_000);
    assert_eq!(accents(&driver), Accents::FULL);
}

#[test]
fn motion_redraws_only_the_screens_that_show_it() {
    let mut driver = Driver::on(Screen::Compass);
    driver.motion(heading(900));
    assert!(driver.stage.changed().is_full());

    let mut driver = Driver::on(Screen::Clock);
    driver.wait(500_000);
    driver.motion(heading(900));
    assert!(driver.stage.changed().is_empty());
}

#[test]
fn the_compass_asks_for_fast_samples_while_it_shows_or_slides_in() {
    let mut driver = Driver::on(Screen::Compass);
    assert!(driver.step(Input::default()).samples_fast);

    let mut driver = Driver::on(Screen::Clock);
    assert!(!driver.step(Input::default()).samples_fast);
    // Dragging left uncovers the compass, which follows the clock.
    driver.touch(Some(Point::new(400, 233)));
    driver.touch(Some(Point::new(340, 233)));
    assert!(driver.touch(Some(Point::new(300, 233))).samples_fast);
}

#[test]
fn a_contact_keeps_the_stage_reading_touch_until_it_lifts() {
    let mut driver = Driver::on(Screen::Clock);
    driver.touch(Some(Point::new(233, 233)));
    assert!(driver.stage.in_contact());
    driver.lift();
    assert!(!driver.stage.in_contact());
}

#[test]
fn a_lift_counts_once_its_grace_runs_out() {
    let mut driver = Driver::on(Screen::Clock);
    driver.touch(Some(Point::new(233, 233)));
    driver.touch(None);
    let reported = driver.now();
    assert!(driver.stage.in_contact());
    assert_eq!(driver.stage.next_change(), Some(reported + LIFT_GRACE));
    driver.wait(LIFT_GRACE);
    assert!(!driver.stage.in_contact());
}

#[test]
fn a_lift_carries_a_swipe_the_last_reports_missed() {
    let mut driver = Driver::new();
    driver.touch(Some(Point::new(400, 233)));
    driver.touch(Some(Point::new(396, 233)));
    driver.read(Touch::Lifted(Point::new(60, 233)));
    driver.wait(LIFT_GRACE);
    driver.settle();
    assert_eq!(driver.stage.screen(), Screen::Compass);
}

#[test]
fn a_finger_gone_quiet_counts_as_lifted() {
    let mut driver = Driver::on(Screen::Clock);
    driver.touch(Some(Point::new(233, 233)));
    let touched = driver.now();
    assert_eq!(driver.stage.next_change(), Some(touched + SILENT_LIFT));
    driver.wait(SILENT_LIFT);
    assert_eq!(driver.stage.contact(), None);
    assert!(
        driver.stage.in_contact(),
        "the grace follows the missing report"
    );
    driver.wait(LIFT_GRACE);
    assert!(!driver.stage.in_contact());
}

fn render(stage: &Stage) -> Box<FB> {
    let mut fb = FB::boxed();
    stage.draw(&mut *fb);
    fb
}

/// Every screen, in states that exercise the most drawing.
fn stages() -> Vec<(String, Stage)> {
    let mut stages = Vec::new();
    for screen in Screen::ALL {
        let mut driver = Driver::on(screen);
        driver.motion(heading(3599));
        driver.wait(500_000);
        stages.push((format!("{screen:?}"), driver.stage));
    }
    for (name, compass) in [
        ("no data", CompassView::default()),
        (
            "calibrating",
            CompassView {
                live: true,
                calibration_percent: 40,
                ..CompassView::default()
            },
        ),
        (
            "tilted and disturbed",
            CompassView {
                live: true,
                calibration_percent: 100,
                heading_decidegrees: Some(1234),
                pitch_deg: -90,
                roll_deg: i8::MIN,
                disturbed: true,
            },
        ),
    ] {
        let mut driver = Driver::on(Screen::Compass);
        driver.motion(Motion { compass });
        driver.wait(500_000);
        stages.push((format!("compass {name}"), driver.stage));
    }
    for (name, input) in clock_walk() {
        let mut driver = Driver::on(Screen::Clock);
        driver.step(input);
        driver.wait(500_000);
        stages.push((format!("clock {name}"), driver.stage));
    }
    stages
}

/// A redraw clipped to regions must leave the same pixels as a full one, or partial flushing
/// shows seams.
#[test]
fn drawing_in_tiles_matches_drawing_whole() {
    for (name, stage) in stages() {
        let differing = tiled_differs(&stage);
        assert_eq!(differing, 0, "{name}: {differing} pixels differ");
    }
}

/// How many pixels differ between drawing `stage` whole and drawing it in tiles.
fn tiled_differs(stage: &Stage) -> usize {
    const TILE: u32 = 70;
    let whole = render(stage);
    let mut tiled = FB::boxed();
    for y in (0..466).step_by(TILE as usize) {
        for x in (0..466).step_by(TILE as usize) {
            let area = Rectangle::new(Point::new(x, y), Size::new_equal(TILE));
            stage.draw(&mut Window::new(&mut *tiled, Point::zero(), area));
        }
    }
    differing(&whole, &tiled)
}

#[test]
fn the_compass_withholds_its_heading_until_calibrated() {
    let calibrated = {
        let mut driver = Driver::on(Screen::Compass);
        driver.motion(heading(450));
        render(&driver.stage)
    };
    let calibrating = {
        let mut driver = Driver::on(Screen::Compass);
        let mut motion = heading(450);
        motion.compass.calibration_percent = 99;
        motion.compass.heading_decidegrees = None;
        driver.motion(motion);
        render(&driver.stage)
    };
    assert_ne!(
        calibrated.buffer(),
        calibrating.buffer(),
        "the two states drew the same frame"
    );
}

fn hold_level() -> Motion {
    let mut motion = heading(470);
    motion.compass.heading_decidegrees = None;
    motion
}

/// A settled compass that loses its heading to `absence` for `gap`, then gets it back.
fn heading_back_after(absence: Motion, gap: Micros) -> Accents {
    let mut driver = settled_on_compass();
    driver.motion(absence);
    driver.wait(gap);
    driver.motion(heading(470));
    accents(&driver)
}

#[test]
fn a_heading_back_quickly_from_hold_level_skips_the_reveal() {
    let back = heading_back_after(hold_level(), 300_000);
    assert_eq!(
        (back.dial, back.icon_rows, back.caption),
        (255, 5, 255),
        "{back:?}"
    );
}

#[test]
fn a_heading_back_slowly_from_hold_level_reveals_again() {
    let accents = heading_back_after(hold_level(), 1_000_000);
    assert!(accents.dial < 255 && accents.icon_rows < 5, "{accents:?}");
}

#[test]
fn a_heading_back_from_no_data_reveals_again() {
    let accents = heading_back_after(Motion::default(), 300_000);
    assert!(accents.dial < 255 && accents.icon_rows < 5, "{accents:?}");
}

/// Readings that walk the compass through every state and turn the dial both ways.
fn compass_walk() -> Vec<(String, Motion)> {
    let mut walk = Vec::new();
    for step in 0..40 {
        // Uneven steps, across north, both ways.
        let decidegrees = (3400 + step * 37) % 3600;
        walk.push((
            format!("heading {decidegrees}"),
            heading(decidegrees as u16),
        ));
    }
    for step in 0..12 {
        let mut motion = heading(1234 - step * 13);
        motion.compass.pitch_deg = (step as i8 - 6) * 17;
        motion.compass.roll_deg = (step as i8).wrapping_mul(23).wrapping_sub(90);
        motion.compass.disturbed = step % 3 == 0;
        walk.push((format!("tilted {step}"), motion));
    }
    // A degree at a time, through zero and across a change in the line's length.
    for (pitch, roll) in [
        (5, -12),
        (6, -12),
        (6, -11),
        (1, -1),
        (0, 0),
        (-1, 1),
        (9, 99),
        (10, 100),
        (-10, -100),
    ] {
        let mut motion = heading(1800);
        motion.compass.pitch_deg = pitch;
        motion.compass.roll_deg = roll;
        walk.push((format!("tilt {pitch} {roll}"), motion));
    }
    walk.push(("hold level".into(), hold_level()));
    walk.push(("back from hold level".into(), heading(900)));
    for percent in [0, 9, 10, 54, 99] {
        let mut motion = heading(900);
        motion.compass.heading_decidegrees = None;
        motion.compass.calibration_percent = percent;
        walk.push((format!("calibrating {percent}"), motion));
    }
    walk.push(("no data".into(), Motion::default()));
    walk.push(("heading after no data".into(), heading(2700)));
    walk
}

/// Redrawing only what each step marked leaves both buffers as a full redraw would, through
/// fades, turns and every state change.
#[test]
fn compass_damage_redraws_what_changed() {
    let mut driver = settled_on_compass();
    let mut buffers = Buffers::new();
    for _ in 0..2 {
        buffers.draw(&driver.stage, &Dirty::new_full());
    }
    for (name, motion) in compass_walk() {
        // Each reading, then a few quiet steps for any fade it starts.
        driver.motion(motion);
        for frame in 0..12 {
            let partial = buffers.draw(&driver.stage, driver.stage.changed());
            let whole = render(&driver.stage);
            let wrong = differing(partial, &whole);
            assert_eq!(wrong, 0, "{name}, frame {frame}: {wrong} pixels differ");
            let wrong = differing(&buffers.panel, &whole);
            assert_eq!(
                wrong, 0,
                "{name}, frame {frame}: {wrong} pixels differ on the panel"
            );
            driver.step(Input::default());
        }
    }
}

/// How many pixels a turning dial repaints per step, against the full panel.
#[test]
fn a_turning_dial_repaints_a_fraction_of_the_panel() {
    let mut driver = settled_on_compass();
    let mut buffers = Buffers::new();
    for _ in 0..2 {
        buffers.draw(&driver.stage, &Dirty::new_full());
    }
    buffers.pixels = 0;
    const STEPS: u64 = 90;
    for step in 0..STEPS {
        driver.motion(heading((470 + step * 10) as u16));
        buffers.draw(&driver.stage, driver.stage.changed());
    }
    let share = buffers.pixels as f64 / (STEPS * 466 * 466) as f64;
    println!(
        "a degree a step repaints {:.1}% of the panel",
        share * 100.0
    );
    // What the flush would send for the last step, at a few region costs, and what that takes
    // at the cost per pixel and per region measured on the target.
    let mut repaint = buffers.previous.clone();
    driver.motion(heading(470 + STEPS as u16 * 10 + 10));
    repaint.extend(driver.stage.changed());
    for overhead in [0, 64, 128, 256, 512, 1024] {
        let (regions, pixels) = repaint
            .rectangles(overhead)
            .fold((0, 0), |(regions, pixels), rect| {
                (regions + 1, pixels + rect.size.width * rect.size.height)
            });
        println!(
            "overhead {overhead}: {regions} regions, {pixels} px, modelled {:.2} ms",
            f64::from(pixels) * 59e-6 + f64::from(regions) * 0.056
        );
    }
    assert!(share < 0.5, "{:.1}%", share * 100.0);
}

fn clock_at(hour: u8, minute: u8, second: u8) -> ClockState {
    ClockState {
        utc: Some(
            DateTime {
                year: 2026,
                month: 10,
                day: 24,
                hour,
                minute,
                second,
            }
            .to_unix(),
        ),
        set_from_gnss: true,
        stopped: false,
    }
}

fn zone(name: &str, mode: ZoneMode) -> ZoneState {
    ZoneState {
        mode,
        zone: DATABASE.find(name).map(|zone| zone.id),
    }
}

fn sensors(clock: ClockState, zone: ZoneState) -> Input {
    Input {
        sensors: Some(Sensors {
            clock,
            zone,
            ..Sensors::default()
        }),
        ..Input::default()
    }
}

/// The clock's accents, with the scatter's breath, which never settles, taken as full.
fn clock_accents(driver: &Driver) -> ClockAccents {
    let mut fb = FB::boxed();
    driver.stage.draw(&mut *fb);
    ClockAccents {
        breath: u8::MAX,
        ..driver.stage.clock_accents()
    }
}

/// The panel's accents, with the scatter's breath taken as full.
fn panel_accents(driver: &Driver) -> PanelAccents {
    PanelAccents {
        breath: u8::MAX,
        ..driver.stage.panel_accents()
    }
}

#[test]
fn the_resting_scatter_breathes_only_while_awake_and_redraws_only_its_marks() {
    let dublin = zone("Europe/Dublin", ZoneMode::Automatic);
    let mut driver = Driver::on(Screen::Clock);
    driver.step(sensors(clock_at(12, 7, 42), dublin));
    driver.wait(1_000_000);
    assert!(driver.stage.is_animating() && !driver.stage.is_changing());
    let mut breaths = std::collections::BTreeSet::new();
    for _ in 0..300 {
        driver.step(Input::default());
        breaths.insert(driver.stage.clock_accents().breath);
        let changed = driver.stage.changed();
        assert!(!changed.is_full(), "a breath redrew the whole face");
    }
    assert!(breaths.len() > 10, "{breaths:?}");
}

/// Readings that walk the clock through ticks, rollovers and every state.
fn clock_walk() -> Vec<(String, Input)> {
    let dublin = zone("Europe/Dublin", ZoneMode::Automatic);
    let mut walk = Vec::new();
    for second in 55..60 {
        walk.push((
            format!("second {second}"),
            sensors(clock_at(22, 59, second), dublin),
        ));
    }
    // Into the next hour, then across local midnight.
    walk.push(("hour".into(), sensors(clock_at(23, 0, 0), dublin)));
    walk.push((
        "midnight".into(),
        sensors(
            clock_at(23, 0, 1),
            zone("Europe/Berlin", ZoneMode::Automatic),
        ),
    ));
    walk.push((
        "manual".into(),
        sensors(clock_at(23, 0, 2), zone("Europe/Berlin", ZoneMode::Manual)),
    ));
    walk.push((
        "rtc".into(),
        sensors(
            ClockState {
                set_from_gnss: false,
                ..clock_at(23, 0, 3)
            },
            dublin,
        ),
    ));
    walk.push(("gnss".into(), sensors(clock_at(23, 0, 4), dublin)));
    walk.push((
        "stopped".into(),
        sensors(
            ClockState {
                stopped: true,
                ..clock_at(0, 0, 0)
            },
            dublin,
        ),
    ));
    walk.push(("set again".into(), sensors(clock_at(23, 0, 5), dublin)));
    walk.push((
        "no zone".into(),
        sensors(clock_at(23, 0, 6), ZoneState::default()),
    ));
    walk.push((
        "no zone tick".into(),
        sensors(clock_at(23, 1, 6), ZoneState::default()),
    ));
    walk.push((
        "found".into(),
        sensors(
            clock_at(23, 1, 7),
            zone("America/Argentina/Buenos_Aires", ZoneMode::Automatic),
        ),
    ));
    walk.push((
        "no data".into(),
        sensors(
            ClockState {
                utc: None,
                ..clock_at(0, 0, 0)
            },
            dublin,
        ),
    ));
    walk.push(("back".into(), sensors(clock_at(23, 1, 8), dublin)));
    walk
}

#[test]
fn clock_damage_redraws_what_changed() {
    let mut driver = Driver::on(Screen::Clock);
    let mut buffers = Buffers::new();
    for (name, input) in clock_walk() {
        // Each reading, then quiet steps through the builds and reveals it starts.
        driver.step(input);
        for frame in 0..30 {
            let partial = buffers.draw(&driver.stage, driver.stage.changed());
            let whole = render(&driver.stage);
            let wrong = differing(partial, &whole);
            assert_eq!(wrong, 0, "{name}, frame {frame}: {wrong} pixels differ");
            let wrong = differing(&buffers.panel, &whole);
            assert_eq!(
                wrong, 0,
                "{name}, frame {frame}: {wrong} pixels differ on the panel"
            );
            driver.step(Input::default());
        }
    }
}

#[test]
fn a_second_repaints_one_small_region() {
    let dublin = zone("Europe/Dublin", ZoneMode::Automatic);
    let mut driver = Driver::on(Screen::Clock);
    driver.step(sensors(clock_at(12, 7, 41), dublin));
    driver.wait(700_000);
    driver.step(sensors(clock_at(12, 7, 42), dublin));
    let pixels = driver.stage.changed().pixels();
    assert!(pixels > 0 && pixels < 1_500, "{pixels} px");
    driver.step(sensors(clock_at(12, 7, 42), dublin));
    assert!(
        driver.stage.changed().is_empty(),
        "an unchanged reading repainted"
    );
}

#[test]
fn the_clock_accents_build_after_the_page_settles_and_leave_with_the_offset() {
    let dublin = zone("Europe/Dublin", ZoneMode::Automatic);
    let mut driver = Driver::on(Screen::Clock);
    driver.step(sensors(clock_at(12, 7, 42), dublin));
    driver.wait(100_000);
    let early = clock_accents(&driver);
    assert!(early.icon_rows > 0 && early.icon_rows < 5, "{early:?}");
    assert_eq!(early.zone, 0);
    driver.wait(600_000);
    assert_eq!(clock_accents(&driver), ClockAccents::FULL);
    assert!(!driver.stage.is_changing());

    driver.touch(Some(Point::new(400, 233)));
    driver.touch(Some(Point::new(380, 233)));
    let part = clock_accents(&driver);
    assert!(part.zone < 255 && part.icon_rows == 5, "{part:?}");
    driver.touch(Some(Point::new(200, 233)));
    assert_eq!(clock_accents(&driver).icon_rows, 0);
    driver.touch(Some(Point::new(399, 233)));
    driver.lift();
    driver.settle();
    assert_eq!(driver.stage.screen(), Screen::Clock);
    assert_eq!(
        clock_accents(&driver),
        ClockAccents::FULL,
        "the entry replayed"
    );
}

#[test]
fn the_clock_rebuilds_on_a_change_and_shows_a_fault_at_once() {
    let dublin = zone("Europe/Dublin", ZoneMode::Automatic);
    let mut driver = Driver::on(Screen::Clock);
    driver.step(sensors(
        ClockState {
            set_from_gnss: false,
            ..clock_at(12, 7, 42)
        },
        dublin,
    ));
    driver.wait(500_000);
    driver.step(sensors(clock_at(12, 7, 43), dublin));
    let resync = clock_accents(&driver);
    assert_eq!(
        (resync.icon_rows, resync.label, resync.plate),
        (0, 255, 255),
        "{resync:?}"
    );
    driver.wait(500_000);
    driver.step(sensors(
        ClockState {
            utc: None,
            ..clock_at(12, 7, 43)
        },
        dublin,
    ));
    assert_eq!(clock_accents(&driver), ClockAccents::FULL);
    driver.step(sensors(clock_at(12, 7, 44), dublin));
    // The plate kept the zone's offset through the fault, so only the icon and label rebuild.
    let back = clock_accents(&driver);
    assert!(
        back.icon_rows < 5 && back.label < 255 && back.plate == 255,
        "{back:?}"
    );
}

#[test]
fn a_tap_on_the_clock_does_nothing() {
    let mut driver = Driver::on(Screen::Clock);
    driver.wait(500_000);
    driver.tap(Point::new(233, 73));
    driver.wait(500_000);
    assert_eq!(driver.stage.screen(), Screen::Clock);
}

#[test]
fn interference_rebuilds_the_icon_and_leaves_the_dial() {
    let mut driver = settled_on_compass();
    let mut disturbed = heading(470);
    disturbed.compass.disturbed = true;
    for motion in [disturbed, heading(470)] {
        driver.motion(motion);
        let changing = accents(&driver);
        assert_eq!(
            (changing.dial, changing.caption),
            (255, 255),
            "{changing:?}"
        );
        assert!(
            changing.icon_rows < 5 && !changing.change.done(),
            "{changing:?}"
        );
        assert!(driver.stage.is_animating());
        driver.wait(200_000);
        assert_eq!(accents(&driver), Accents::FULL);
        assert!(!driver.stage.is_changing());
    }
}

/// The slab's colour at its top row and its bottom row.
fn slab_ends(stage: &Stage) -> (Option<Color>, Option<Color>) {
    let fb = render(stage);
    (
        fb.pixel(Point::new(140, 205)),
        fb.pixel(Point::new(140, 293)),
    )
}

#[test]
fn the_slab_wipes_to_its_new_colour_from_the_top() {
    let mut driver = settled_on_compass();
    let white = slab_ends(&driver.stage);
    let mut disturbed = heading(470);
    disturbed.compass.disturbed = true;
    driver.motion(disturbed);
    let (top, bottom) = slab_ends(&driver.stage);
    assert_ne!(top, white.0, "the wipe's first step did not land");
    assert_eq!(bottom, white.1, "the wipe reached the bottom at once");
    driver.wait(100_000);
    assert_eq!(slab_ends(&driver.stage), (top, top));
}

#[test]
fn a_fault_on_the_compass_shows_at_once() {
    let mut driver = Driver::on(Screen::Compass);
    driver.motion(Motion::default());
    let entering = accents(&driver);
    assert_eq!((entering.icon_rows, entering.caption), (5, 255));
    let mut driver = settled_on_compass();
    driver.motion(Motion::default());
    let fault = accents(&driver);
    assert_eq!((fault.icon_rows, fault.caption), (5, 255));
}

#[test]
fn a_running_clock_ticks_each_second_unless_stopped() {
    let dublin = zone("Europe/Dublin", ZoneMode::Automatic);
    let mut driver = Driver::on(Screen::Clock);
    driver.run_clock();
    driver.step(sensors(clock_at(12, 7, 41), dublin));
    driver.wait(500_000);
    assert!(driver.stage.changed().is_empty());
    driver.wait(600_000);
    assert_eq!(
        driver.stage.peripherals().clock.clock().utc,
        clock_at(12, 7, 42).utc
    );

    driver.step(sensors(
        ClockState {
            stopped: true,
            ..clock_at(12, 7, 50)
        },
        dublin,
    ));
    driver.wait(1_500_000);
    assert_eq!(
        driver.stage.peripherals().clock.clock().utc,
        clock_at(12, 7, 50).utc
    );
}

#[test]
fn a_swipe_takes_its_duration_and_turns_the_page() {
    let mut driver = Driver::new();
    let updates = driver.swipe(Point::new(400, 233), Point::new(60, 233), 300_000);
    assert_eq!(
        updates.len(),
        18 + 1 + 1 + LIFT_GRACE.div_ceil(script::FRAME) as usize
    );
    driver.settle();
    assert_eq!(driver.stage.screen(), Screen::Compass);
}

/// Whether the time is typing in again after a step with `input`, then whether it finished.
fn retypes(driver: &mut Driver, input: Input) -> bool {
    driver.step(input);
    let typing = clock_accents(driver).time < 255;
    if typing {
        driver.wait(600_000);
        assert_eq!(
            clock_accents(driver),
            ClockAccents::FULL,
            "the reveal never finished"
        );
    }
    typing
}

#[test]
fn the_time_types_in_again_when_a_fix_or_zone_replaces_it_and_not_on_a_tick() {
    let dublin = zone("Europe/Dublin", ZoneMode::Automatic);
    let unset = |clock: ClockState| ClockState {
        set_from_gnss: false,
        ..clock
    };
    let mut driver = Driver::on(Screen::Clock);
    driver.step(sensors(unset(clock_at(12, 7, 40)), ZoneState::default()));
    driver.wait(500_000);

    assert!(
        retypes(&mut driver, sensors(clock_at(12, 7, 41), dublin)),
        "a first fix"
    );
    // The reveal took 400 ms and the clock ticks a second later, which is no jump.
    driver.wait(100_000);
    assert!(
        !retypes(&mut driver, sensors(clock_at(12, 7, 42), dublin)),
        "a tick"
    );
    driver.wait(500_000);
    assert!(
        !retypes(&mut driver, sensors(unset(clock_at(12, 7, 43)), dublin)),
        "no time change"
    );
    assert!(
        retypes(&mut driver, sensors(clock_at(12, 12, 0), dublin)),
        "a correction"
    );
    // Both are an hour ahead of UTC in October.
    let london = zone("Europe/London", ZoneMode::Automatic);
    assert!(
        !retypes(&mut driver, sensors(clock_at(12, 12, 0), london)),
        "the same offset"
    );
    let berlin = zone("Europe/Berlin", ZoneMode::Automatic);
    assert!(
        retypes(&mut driver, sensors(clock_at(12, 12, 0), berlin)),
        "a new offset"
    );
    let stopped = ClockState {
        stopped: true,
        ..clock_at(12, 12, 0)
    };
    assert!(!retypes(&mut driver, sensors(stopped, berlin)), "dashes");
    assert!(
        retypes(&mut driver, sensors(clock_at(12, 12, 1), berlin)),
        "leaving STOPPED"
    );
}

// The settings panel.

use octowhere_ui::ui::{
    panel::{self, Accents as PanelAccents},
    screens::{Battery, Gnss, PeripheralState},
    second::Page,
    stage::{Store, Update},
};

const HEIGHT: i32 = 466;

/// Settled on `screen`, with readings in every cell.
fn driver_on(screen: Screen) -> Driver<'static> {
    let mut driver = Driver::on(screen);
    driver.stage = Stage::new(PeripheralState {
        firmware: "0.1.0",
        ..PeripheralState::default()
    });
    driver.stage.show(screen);
    driver.step(Input {
        sensors: Some(Sensors {
            clock: clock_at(12, 7, 42),
            zone: zone("Europe/Dublin", ZoneMode::Automatic),
            battery: Some(Battery {
                present: true,
                percent: 87,
                millivolts: 4020,
                charging: true,
                usb: true,
            }),
            gnss: Gnss {
                fix: true,
                in_use: 9,
                in_view: 14,
                position: None,
                ..Gnss::default()
            },
        }),
        motion: Some(heading(470)),
        ..Input::default()
    });
    driver.wait(600_000);
    driver
}

fn pull_down(driver: &mut Driver) {
    driver.swipe(Point::new(233, 80), Point::new(233, 420), 250_000);
    driver.settle();
}

fn open_panel(screen: Screen) -> Driver<'static> {
    let mut driver = driver_on(screen);
    pull_down(&mut driver);
    driver.wait(500_000);
    driver
}

fn tap(driver: &mut Driver, x: i32, y: i32) -> Vec<Update> {
    let updates = driver.tap(Point::new(x, y));
    driver.wait(300_000);
    updates
}

/// Taps a row on its page, swiping to that page first.
fn tap_cell(driver: &mut Driver, cell: panel::Cell) -> Vec<Update> {
    if !panel::in_view(cell.page(), driver.stage.panel_scroll()) {
        if cell.page() == 1 {
            driver.swipe(Point::new(380, 250), Point::new(80, 250), 300_000);
        } else {
            driver.swipe(Point::new(80, 250), Point::new(380, 250), 300_000);
        }
        driver.settle();
    }
    let middle = cell.bounds(driver.stage.panel_scroll()).center();
    tap(driver, middle.x, middle.y)
}

fn stored(updates: &[Update]) -> Option<Store> {
    updates.iter().find_map(|update| update.store)
}

#[test]
fn a_downward_drag_on_either_face_opens_the_panel() {
    for screen in Screen::ALL {
        let driver = open_panel(screen);
        assert_eq!(driver.stage.panel_offset(), HEIGHT, "from {screen:?}");
        assert_eq!(panel_accents(&driver), PanelAccents::FULL);
    }
}

#[test]
fn a_short_or_mostly_sideways_drag_leaves_the_panel_shut() {
    let mut driver = driver_on(Screen::Clock);
    driver.swipe(Point::new(233, 80), Point::new(233, 150), 300_000);
    driver.settle();
    assert_eq!(driver.stage.panel_offset(), 0);
    // Down, but more sideways than down, is the pager's.
    driver.swipe(Point::new(400, 150), Point::new(160, 260), 200_000);
    driver.settle();
    assert_eq!(driver.stage.panel_offset(), 0);
    assert_eq!(driver.stage.screen(), Screen::Compass);
}

#[test]
fn the_panel_arrives_with_its_accents_hidden_and_builds_them_once_open() {
    let mut driver = driver_on(Screen::Clock);
    for y in [80, 150, 250] {
        driver.touch(Some(Point::new(233, y)));
    }
    assert_eq!(panel_accents(&driver), PanelAccents::HIDDEN);
    for _ in 0..3 {
        driver.touch(None);
    }
    while driver.stage.panel_offset() < HEIGHT {
        driver.wait(0);
    }
    driver.wait(100_000);
    let early = panel_accents(&driver);
    assert!(
        early.title > 0 && early.hint == 0 && early.rows[5] == 0,
        "{early:?}"
    );
    driver.wait(400_000);
    assert_eq!(panel_accents(&driver), PanelAccents::FULL);
}

#[test]
fn a_drag_that_springs_back_open_does_not_replay_the_panel_entry() {
    let mut driver = open_panel(Screen::Clock);
    driver.swipe(Point::new(233, 420), Point::new(233, 360), 150_000);
    assert!(driver.stage.panel_offset() < HEIGHT);
    while driver.stage.panel_offset() < HEIGHT {
        driver.wait(0);
    }
    // The offset rounds to the top before the settle ends, so watch past that.
    for _ in 0..30 {
        assert_eq!(
            panel_accents(&driver),
            PanelAccents::FULL,
            "the entry replayed"
        );
        driver.wait(0);
    }
}

#[test]
fn an_upward_drag_closes_the_panel_to_the_face_it_came_from() {
    let mut driver = open_panel(Screen::Compass);
    driver.swipe(Point::new(233, 420), Point::new(233, 100), 250_000);
    driver.settle();
    assert_eq!(driver.stage.panel_offset(), 0);
    assert_eq!(driver.stage.screen(), Screen::Compass);
    assert_eq!(
        accents(&driver),
        Accents::FULL,
        "the compass entry did not run again"
    );
}

#[test]
fn a_cover_on_the_panel_closes_it_to_the_clock() {
    let mut driver = open_panel(Screen::Compass);
    driver.cover();
    driver.settle();
    assert_eq!(driver.stage.panel_offset(), 0);
    assert_eq!(driver.stage.screen(), Screen::Clock);
}

#[test]
fn a_sideways_drag_switches_between_complete_pages() {
    let mut driver = open_panel(Screen::Clock);
    driver.swipe(Point::new(380, 250), Point::new(80, 250), 400_000);
    driver.settle();
    assert_eq!(driver.stage.panel_scroll(), panel::PAGE_WIDTH);
    assert_eq!(driver.stage.panel_offset(), HEIGHT);
    driver.swipe(Point::new(200, 250), Point::new(260, 250), 400_000);
    driver.settle();
    assert_eq!(
        driver.stage.panel_scroll(),
        panel::PAGE_WIDTH,
        "a short drag moved it"
    );
    driver.swipe(Point::new(260, 250), Point::new(200, 250), 50_000);
    driver.settle();
    assert_eq!(
        driver.stage.panel_scroll(),
        panel::MAX_SCROLL,
        "it went past the end"
    );
    driver.swipe(Point::new(200, 250), Point::new(260, 250), 50_000);
    driver.settle();
    assert_eq!(
        driver.stage.panel_scroll(),
        0,
        "a flick back did not return to page one"
    );
    driver.swipe(Point::new(100, 250), Point::new(400, 250), 400_000);
    driver.settle();
    assert_eq!(driver.stage.panel_scroll(), 0);
}

#[test]
fn the_second_page_rows_open_only_after_the_page_settles() {
    let mut driver = open_panel(Screen::Clock);
    tap(&mut driver, 300, 115);
    assert!(driver.stage.page().is_some());
    driver.cover();
    driver.settle();
    let mut driver = open_panel(Screen::Clock);
    driver.swipe(Point::new(380, 250), Point::new(80, 250), 300_000);
    driver.settle();
    assert_eq!(driver.stage.panel_scroll(), panel::MAX_SCROLL);
    assert!(driver.stage.page().is_none());
    tap(&mut driver, 300, 330);
    assert!(matches!(driver.stage.page(), Some(Page::Device(_))));
}

#[test]
fn the_compass_cell_restarts_calibration_and_closes_to_the_compass() {
    let mut driver = open_panel(Screen::Clock);
    let updates = tap_cell(&mut driver, panel::Cell::Compass);
    assert!(updates.iter().any(|update| update.recalibrate));
    driver.settle();
    assert_eq!(driver.stage.panel_offset(), 0);
    assert_eq!(driver.stage.screen(), Screen::Compass);
    assert_eq!(driver.stage.peripherals().compass.heading_decidegrees, None);
}

#[test]
fn the_brightness_editor_shows_each_step_live_and_stores_on_a_tap_on_its_hint() {
    let mut driver = open_panel(Screen::Clock);
    tap(&mut driver, 150, 190);
    assert!(matches!(driver.stage.page(), Some(Page::Brightness(_))));
    let updates = driver.swipe(Point::new(150, 280), Point::new(420, 280), 300_000);
    let levels: Vec<_> = updates
        .iter()
        .filter_map(|update| update.brightness)
        .collect();
    assert!(
        levels.len() > 3 && levels.last() == Some(&255),
        "{levels:?}"
    );
    assert_eq!(
        driver.stage.peripherals().brightness,
        120,
        "the level was stored before a tap"
    );
    let updates = tap(&mut driver, 233, 378);
    assert_eq!(stored(&updates), Some(Store::Brightness(255)));
    assert!(driver.stage.page().is_none());
    assert_eq!(driver.stage.peripherals().brightness, 255);
}

#[test]
fn cancel_or_a_cover_puts_the_brightness_back() {
    let mut driver = open_panel(Screen::Clock);
    tap(&mut driver, 150, 190);
    driver.swipe(Point::new(150, 280), Point::new(80, 280), 200_000);
    let updates = tap(&mut driver, 120, 120);
    assert_eq!(
        updates.iter().rev().find_map(|update| update.brightness),
        Some(120)
    );
    assert!(driver.stage.page().is_none());
    assert_eq!(driver.stage.panel_offset(), HEIGHT);

    tap(&mut driver, 150, 190);
    driver.swipe(Point::new(150, 280), Point::new(80, 280), 200_000);
    let update = driver.cover();
    assert_eq!(update.brightness, Some(120));
    assert_eq!(update.store, None);
    driver.settle();
    assert_eq!(driver.stage.screen(), Screen::Clock);
    assert_eq!(driver.stage.panel_offset(), 0);
}

#[test]
fn clearing_takes_a_drag_all_the_way_to_the_target() {
    let mut driver = open_panel(Screen::Clock);
    tap(&mut driver, 150, 190);
    driver.swipe(Point::new(150, 280), Point::new(420, 280), 300_000);
    tap(&mut driver, 233, 250);
    tap_cell(&mut driver, panel::Cell::Device);
    scroll_device_to_end(&mut driver);
    tap(&mut driver, 233, 342);
    assert!(matches!(driver.stage.page(), Some(Page::Clear(_))));
    // Short of the target, or not starting on the handle, erases nothing.
    let updates = driver.swipe(Point::new(120, 258), Point::new(250, 258), 300_000);
    assert_eq!(stored(&updates), None);
    let updates = driver.swipe(Point::new(200, 258), Point::new(420, 258), 300_000);
    assert_eq!(stored(&updates), None);
    let updates = driver.swipe(Point::new(120, 258), Point::new(420, 258), 300_000);
    assert_eq!(stored(&updates), Some(Store::Clear));
    assert!(driver.stage.page().is_none());
    assert_eq!(driver.stage.peripherals().brightness, 120);
}

#[test]
fn the_picker_stores_a_zone_by_hand_and_automatic_from_its_first_step() {
    let mut driver = open_panel(Screen::Clock);
    tap(&mut driver, 150, 115);
    assert!(matches!(driver.stage.page(), Some(Page::Picker(_))));
    // One row up is the next offset, +02:00, and its zones start at the top of the list.
    driver.swipe(Point::new(233, 300), Point::new(233, 255), 300_000);
    tap(&mut driver, 233, 250);
    let updates = tap(&mut driver, 233, 250);
    let Some(Store::ManualZone(zone)) = stored(&updates) else {
        panic!("no zone was stored: {:?}", stored(&updates));
    };
    assert_eq!(
        DATABASE
            .zone(zone)
            .at(clock_at(12, 7, 42).utc.unwrap())
            .utc_offset,
        7200
    );
    assert_eq!(
        driver.stage.peripherals().clock.zone(),
        octowhere_ui::ui::clock::ZoneState {
            mode: ZoneMode::Manual,
            zone: Some(zone),
        }
    );

    tap(&mut driver, 150, 115);
    let updates = tap(&mut driver, 233, 342);
    assert_eq!(stored(&updates), Some(Store::AutomaticZone));
    assert_eq!(
        driver.stage.peripherals().clock.zone().mode,
        ZoneMode::Automatic
    );
}

#[test]
fn a_fling_in_the_picker_keeps_stepping_and_stops() {
    let mut driver = open_panel(Screen::Clock);
    tap(&mut driver, 150, 115);
    let Some(Page::Picker(before)) = driver.stage.page().cloned() else {
        panic!()
    };
    driver.swipe(Point::new(233, 300), Point::new(233, 200), 60_000);
    driver.settle();
    let Some(Page::Picker(after)) = driver.stage.page().cloned() else {
        panic!()
    };
    assert_ne!(before, after);
    assert!(!driver.stage.is_changing());
}

#[test]
fn a_long_zone_name_scrolls_and_redraws_only_its_lines() {
    let mut driver = open_panel(Screen::Clock);
    driver.step(sensors(
        clock_at(12, 7, 42),
        zone("America/Port-au-Prince", ZoneMode::Manual),
    ));
    tap(&mut driver, 150, 115);
    tap(&mut driver, 233, 250);
    assert!(matches!(driver.stage.page(), Some(Page::Picker(_))));
    // Holding at its start, the name leaves the stage at rest until its run is due.
    driver.settle();
    let start = render(&driver.stage);
    let mut buffers = Buffers::new();
    buffers.draw(&driver.stage, &Dirty::new_full());
    let (mut moved, mut limited) = (false, true);
    for step in 0..6_000_000 / script::FRAME {
        driver.step(Input::default());
        let changed = driver.stage.changed();
        // The two lines, 274 px wide and 89 rows, grown to the panel's 2 × 2 grain.
        limited &= changed.is_empty() || changed.pixels() <= 276 * 90;
        let partial = buffers.draw(&driver.stage, changed);
        let whole = render(&driver.stage);
        let wrong = differing(partial, &whole);
        assert_eq!(wrong, 0, "step {step}: {wrong} pixels differ");
        moved |= differing(&whole, &start) > 0;
    }
    assert!(moved, "the name never scrolled");
    assert!(limited, "a scroll step redrew more than the zone's lines");
}

/// The panel and every screen it opens, drawn clipped to tiles, must match drawing whole.
#[test]
fn the_panel_and_its_screens_draw_the_same_in_tiles() {
    let mut stages = vec![("panel", open_panel(Screen::Clock).stage)];
    let mut driver = open_panel(Screen::Clock);
    for x in [380, 340, 320] {
        driver.touch(Some(Point::new(x, 250)));
    }
    stages.push(("scrolling", driver.stage));
    let mut driver = driver_on(Screen::Compass);
    for y in [80, 150, 250] {
        driver.touch(Some(Point::new(233, y)));
    }
    stages.push(("pulling", driver.stage));
    use panel::Cell;
    for (name, cell) in [
        ("brightness", Cell::Brightness),
        ("timeout", Cell::Timeout),
        ("always on", Cell::AlwaysOn),
        ("device", Cell::Device),
        ("picker", Cell::Zone),
    ] {
        let mut driver = open_panel(Screen::Clock);
        tap_cell(&mut driver, cell);
        stages.push((name, driver.stage));
    }
    let mut driver = open_panel(Screen::Clock);
    tap(&mut driver, 150, 115);
    tap(&mut driver, 233, 250);
    stages.push(("zones", driver.stage));
    for (name, stage) in stages {
        let differing = tiled_differs(&stage);
        assert_eq!(differing, 0, "{name}: {differing} pixels differ");
    }
}

/// Steps through opening, scrolling and value changes, checking each partial redraw against a
/// full one.
#[test]
fn panel_damage_redraws_what_changed() {
    let mut driver = driver_on(Screen::Clock);
    let mut buffers = Buffers::new();
    let mut check = |driver: &Driver, what: &str| {
        let partial = buffers.draw(&driver.stage, driver.stage.changed());
        let whole = render(&driver.stage);
        let wrong = differing(partial, &whole);
        assert_eq!(wrong, 0, "{what}: {wrong} pixels differ");
        let wrong = differing(&buffers.panel, &whole);
        assert_eq!(wrong, 0, "{what}: {wrong} pixels differ on the panel");
    };
    for (index, y) in [80, 160, 260, 360, 420].into_iter().enumerate() {
        driver.touch(Some(Point::new(233, y)));
        check(&driver, &format!("pull {index}"));
    }
    for step in 0..40 {
        driver.touch(None);
        check(&driver, &format!("settle {step}"));
    }
    for (index, x) in [380, 350, 300, 260].into_iter().enumerate() {
        driver.touch(Some(Point::new(x, 250)));
        check(&driver, &format!("scroll {index}"));
    }
    for step in 0..20 {
        driver.touch(None);
        check(&driver, &format!("snap {step}"));
    }
    let mut calibrating = heading(470);
    calibrating.compass.heading_decidegrees = None;
    for percent in [10, 60, 100] {
        calibrating.compass.calibration_percent = percent;
        driver.motion(calibrating);
        check(&driver, &format!("calibration {percent}"));
    }
}

/// The clear leaves the parts a screen paints solid, so drawing over an old frame must still
/// replace every pixel, settled, mid-swipe and under the moving panel.
#[test]
fn a_frame_replaces_everything_under_it() {
    use embedded_graphics::draw_target::DrawTarget as _;
    let mut cases = stages();
    for (name, path) in [
        (
            "swiping left",
            [
                Point::new(400, 233),
                Point::new(380, 233),
                Point::new(250, 236),
            ],
        ),
        (
            "swiping right",
            [
                Point::new(60, 233),
                Point::new(80, 233),
                Point::new(300, 230),
            ],
        ),
        (
            "pulling the panel",
            [
                Point::new(233, 40),
                Point::new(233, 60),
                Point::new(236, 200),
            ],
        ),
    ] {
        for screen in Screen::ALL {
            let mut driver = Driver::on(screen);
            driver.step(sensors(
                clock_at(12, 7, 42),
                zone("Europe/Dublin", ZoneMode::Automatic),
            ));
            driver.wait(500_000);
            for point in path {
                driver.touch(Some(point));
            }
            cases.push((format!("{screen:?} {name}"), driver.stage));
        }
    }
    for (name, stage) in cases {
        let mut over = FB::boxed();
        over.fill_solid(
            &Rectangle::new(Point::zero(), Size::new(466, 466)),
            octowhere_ui::chrome::PURPLE,
        )
        .unwrap();
        stage.draw(&mut *over);
        let wrong = differing(&over, &render(&stage));
        assert_eq!(wrong, 0, "{name}: {wrong} pixels kept the old frame");
    }
}

/// Reports every part a tenth of a second apart, `failing` with no reply.
fn boot_all(driver: &mut Driver, failing: Option<Part>) {
    for part in Part::ALL {
        driver.wait(100_000);
        let outcome = if failing == Some(part) {
            Outcome::NoReply
        } else {
            Outcome::Answered
        };
        driver.boot(part, outcome);
    }
}

/// Redrawing only what each step marked leaves both buffers and the panel as a full redraw
/// would, from the first self-test frame through the identity, the card and the clock's entry,
/// and through a failure's fault screen and its exit.
#[test]
fn start_up_damage_redraws_what_changed() {
    for failing in [None, Some(Part::Magnet)] {
        let mut driver = Driver::starting();
        driver.sensors(
            sensors(
                clock_at(12, 7, 42),
                zone("Europe/Dublin", ZoneMode::Automatic),
            )
            .sensors
            .unwrap(),
        );
        let mut buffers = Buffers::new();
        let mut check = |driver: &Driver, when: &str| {
            let partial = buffers.draw(&driver.stage, driver.stage.changed());
            let whole = render(&driver.stage);
            assert_eq!(
                differing_as_shown(&driver.stage, partial, &whole),
                0,
                "{failing:?} {when}"
            );
            assert_eq!(
                differing_as_shown(&driver.stage, &buffers.panel, &whole),
                0,
                "{failing:?} {when}, on the panel"
            );
        };
        for part in Part::ALL {
            // The radio's check starts a while before it ends, so the list scrolls while the
            // parts wait.
            if part == Part::Radio {
                driver.boot_started(part);
                check(&driver, "radio started");
            }
            for _ in 0..6 {
                driver.step(Input::default());
                check(&driver, "waiting");
            }
            let outcome = if failing == Some(part) {
                Outcome::NoReply
            } else {
                Outcome::Answered
            };
            driver.boot(part, outcome);
            check(&driver, part.name());
        }
        for step in 0..340 {
            // A new minute every few steps, some between the identity's frames.
            if step % 7 == 3 {
                let clock = clock_at(12, (8 + step / 7) as u8, 0);
                driver.sensors(
                    sensors(clock, zone("Europe/Dublin", ZoneMode::Automatic))
                        .sensors
                        .unwrap(),
                );
            } else {
                driver.step(Input::default());
            }
            check(&driver, &format!("step {step}"));
        }
        assert!(!driver.stage.starting_up());
    }
}

#[test]
fn the_brightness_climbs_from_dark_on_the_first_frame() {
    let mut driver = Driver::starting();
    assert_eq!(driver.step(Input::default()).brightness, Some(0));
    let levels: Vec<_> = (0..14)
        .filter_map(|_| driver.step(Input::default()).brightness)
        .collect();
    assert!(
        levels.windows(2).all(|pair| pair[0] < pair[1]),
        "{levels:?}"
    );
    assert_eq!(levels.last(), Some(&120));
}

#[test]
fn a_touch_during_the_self_test_does_nothing() {
    let mut driver = Driver::starting();
    driver.boot(Part::Power, Outcome::Answered);
    driver.stroke(&[Point::new(400, 233), Point::new(200, 233)]);
    assert!(driver.stage.starting_up());
}

#[test]
fn a_touch_during_the_identity_goes_to_the_settled_clock_and_is_not_a_swipe() {
    let mut driver = Driver::starting();
    boot_all(&mut driver, None);
    driver.wait(600_000);
    assert!(driver.stage.starting_up());
    driver.touch(Some(Point::new(400, 233)));
    assert!(!driver.stage.starting_up());
    assert_eq!(clock_accents(&driver), ClockAccents::FULL);
    for x in [340, 280, 220, 160] {
        driver.touch(Some(Point::new(x, 233)));
    }
    driver.stroke(&[]);
    driver.settle();
    assert_eq!(driver.stage.screen(), Screen::Clock);
}

#[test]
fn after_the_card_the_clock_runs_its_entry_and_types_its_time_in() {
    let mut driver = Driver::starting();
    driver.sensors(
        sensors(
            clock_at(12, 7, 42),
            zone("Europe/Dublin", ZoneMode::Automatic),
        )
        .sensors
        .unwrap(),
    );
    boot_all(&mut driver, None);
    while driver.stage.starting_up() {
        driver.step(Input::default());
    }
    let accents = driver.stage.clock_accents();
    assert!(
        accents.time < u8::MAX && accents.icon_rows < 5,
        "{accents:?}"
    );
}

#[test]
fn a_failure_shows_the_fault_screen_then_the_clock() {
    let mut driver = Driver::starting();
    boot_all(&mut driver, Some(Part::Gnss));
    // The list's scroll to the radio, the hold, then 120 frames at 30 fps and the 18 of the
    // exit.
    driver.wait(4_900_000);
    assert!(driver.stage.starting_up());
    driver.wait(200_000);
    assert!(!driver.stage.starting_up());
    assert_eq!(driver.stage.screen(), Screen::Clock);
}

/// Scrolls the device page to its action boxes.
fn scroll_device_to_end(driver: &mut Driver) {
    driver.swipe(Point::new(233, 440), Point::new(233, 80), 300_000);
}

fn open_device_page() -> Driver<'static> {
    let mut driver = open_panel(Screen::Clock);
    tap_cell(&mut driver, panel::Cell::Device);
    assert!(matches!(driver.stage.page(), Some(Page::Device(_))));
    driver
}

#[test]
fn replay_on_the_device_page_plays_the_identity_and_the_card_then_the_clock() {
    let mut driver = open_device_page();
    scroll_device_to_end(&mut driver);
    tap(&mut driver, 233, 298);
    tap(&mut driver, 233, 258);
    assert!(driver.stage.starting_up());
    assert!(driver.stage.page().is_none());
    assert_eq!(driver.stage.panel_offset(), 0);
    // 139 frames at 30 fps, 300 ms of them waited out after the tap.
    driver.wait(4_300_000);
    assert!(driver.stage.starting_up());
    driver.wait(100_000);
    assert!(!driver.stage.starting_up());
    assert_eq!(driver.stage.screen(), Screen::Clock);
}

#[test]
fn a_replay_after_a_failed_boot_still_shows_the_identity() {
    let mut driver = Driver::starting();
    boot_all(&mut driver, Some(Part::Magnet));
    driver.wait(5_400_000);
    assert!(!driver.stage.starting_up());
    driver.swipe(Point::new(233, 80), Point::new(233, 420), 250_000);
    driver.settle();
    driver.wait(600_000);
    tap_cell(&mut driver, panel::Cell::Device);
    scroll_device_to_end(&mut driver);
    tap(&mut driver, 233, 298);
    tap(&mut driver, 233, 258);
    assert!(driver.stage.starting_up());
    driver.wait(1_000_000);
    assert!(
        driver.stage.starting_up(),
        "the fault screen would have ended by now"
    );
}

#[test]
fn replay_damage_redraws_what_changed() {
    let mut driver = open_device_page();
    scroll_device_to_end(&mut driver);
    let mut buffers = Buffers::new();
    for _ in 0..2 {
        buffers.draw(&driver.stage, &Dirty::new_full());
    }
    tap(&mut driver, 233, 298);
    driver.tap(Point::new(233, 258));
    for step in 0..160 {
        let partial = buffers.draw(&driver.stage, driver.stage.changed());
        let whole = render(&driver.stage);
        assert_eq!(differing(partial, &whole), 0, "step {step}");
        driver.step(Input::default());
    }
}

/// Opens the replay chooser from the device page and drags it `steps` choices down the list.
fn open_replay_chooser(steps: i32) -> Driver<'static> {
    let mut driver = open_device_page();
    scroll_device_to_end(&mut driver);
    tap(&mut driver, 233, 298);
    assert!(matches!(driver.stage.page(), Some(Page::Replay(_))));
    if steps > 0 {
        driver.swipe(
            Point::new(233, 330),
            Point::new(233, 330 - 40 * steps - 10),
            300_000,
        );
    }
    driver
}

#[test]
fn cancel_on_the_replay_chooser_goes_back_to_the_device_page() {
    let mut driver = open_replay_chooser(0);
    tap(&mut driver, 100, 120);
    assert!(matches!(driver.stage.page(), Some(Page::Device(_))));
}

#[test]
fn a_demonstrated_failure_runs_the_self_test_and_the_fault_screen_then_the_clock() {
    let mut driver = open_replay_chooser(5);
    tap(&mut driver, 233, 258);
    assert!(driver.stage.starting_up());
    // The self-test to the last report at 1.3 s, its last glyph and its 300 ms hold, then 4.6 s.
    driver.wait(5_900_000);
    assert!(driver.stage.starting_up());
    driver.wait(400_000);
    assert!(!driver.stage.starting_up());
    assert_eq!(driver.stage.screen(), Screen::Clock);
}

#[test]
fn a_demonstration_leaves_the_boot_record_for_a_good_replay() {
    let mut driver = Driver::starting();
    boot_all(&mut driver, None);
    driver.wait(5_000_000);
    let replay = |driver: &mut Driver, steps: i32| {
        driver.swipe(Point::new(233, 80), Point::new(233, 420), 250_000);
        driver.settle();
        driver.wait(600_000);
        tap_cell(driver, panel::Cell::Device);
        scroll_device_to_end(driver);
        tap(driver, 233, 298);
        if steps > 0 {
            driver.swipe(
                Point::new(233, 330),
                Point::new(233, 330 - 40 * steps - 10),
                300_000,
            );
        }
        tap(driver, 233, 258);
    };
    replay(&mut driver, 1);
    driver.wait(6_600_000);
    assert!(!driver.stage.starting_up());
    // A good replay goes straight to the identity: no self-test, and no fault screen.
    replay(&mut driver, 0);
    driver.wait(4_500_000);
    assert!(!driver.stage.starting_up());
}

#[test]
fn demonstration_damage_redraws_what_changed() {
    let mut driver = open_replay_chooser(3);
    let mut buffers = Buffers::new();
    for _ in 0..2 {
        buffers.draw(&driver.stage, &Dirty::new_full());
    }
    driver.tap(Point::new(233, 258));
    for step in 0..360 {
        let partial = buffers.draw(&driver.stage, driver.stage.changed());
        let whole = render(&driver.stage);
        assert_eq!(
            differing_as_shown(&driver.stage, partial, &whole),
            0,
            "step {step}"
        );
        assert_eq!(
            differing_as_shown(&driver.stage, &buffers.panel, &whole),
            0,
            "step {step}, on the panel"
        );
        driver.step(Input::default());
    }
}

use octowhere_ui::ui::rest::{self, AlwaysOn, Rest, Timeout};

/// Settled on `screen` with the clock running, resting after `timeout`.
fn resting_on(screen: Screen, timeout: Timeout, always_on: bool) -> Driver<'static> {
    let mut driver = Driver::on(screen);
    driver.stage = Stage::new(PeripheralState {
        firmware: "0.1.0",
        timeout,
        always_on: if always_on {
            AlwaysOn::Dim
        } else {
            AlwaysOn::Off
        },
        ..PeripheralState::default()
    });
    driver.stage.show(screen);
    driver.step(sensors(
        clock_at(12, 7, 42),
        zone("Europe/Dublin", ZoneMode::Automatic),
    ));
    driver.motion(heading(470));
    driver.run_clock();
    driver.wait(600_000);
    driver
}

/// Steps without input until the screen's rest passes `reached`, and returns every update on
/// the way.
fn wait_until(driver: &mut Driver, within: Micros, reached: impl Fn(Rest) -> bool) -> Vec<Update> {
    let end = driver.now() + within;
    let mut updates = Vec::new();
    while !reached(driver.stage.rest()) {
        assert!(driver.now() < end, "still {:?}", driver.stage.rest());
        updates.push(driver.step(Input::default()));
    }
    updates
}

/// The levels sent over `duration` of steps without input.
fn levels_over(driver: &mut Driver, duration: Micros) -> Vec<u8> {
    (0..frames_in(duration))
        .filter_map(|_| driver.step(Input::default()).brightness)
        .collect()
}

fn is_dimmed(rest: Rest) -> bool {
    matches!(rest, Rest::Dimmed { .. })
}

#[test]
fn the_screen_dims_at_the_timeout_and_a_touch_only_restores_it() {
    let mut driver = resting_on(Screen::Clock, Timeout::default(), false);
    // The timer starts with the first step, and nothing has restarted it since.
    let start = script::FRAME;
    driver.wait(58_000_000);
    assert_eq!(driver.stage.rest(), Rest::Awake);
    wait_until(&mut driver, 3_000_000, is_dimmed);
    let after = driver.now() - start;
    assert!((59_900_000..60_100_000).contains(&after), "{after}");
    let levels = levels_over(&mut driver, rest::DIM_FADE + script::FRAME);
    assert!(
        levels.len() > 5 && levels.is_sorted_by(|a, b| a >= b),
        "{levels:?}"
    );
    assert_eq!(levels.last(), Some(&rest::dim_level(120)));

    let mut updates = driver.swipe(Point::new(400, 233), Point::new(100, 233), 200_000);
    updates.extend((0..30).map(|_| driver.step(Input::default())));
    let levels: Vec<_> = updates
        .iter()
        .filter_map(|update| update.brightness)
        .collect();
    assert!(levels.len() > 5 && levels.is_sorted(), "{levels:?}");
    assert_eq!(levels.last(), Some(&120));
    assert_eq!(driver.stage.rest(), Rest::Awake);
    assert!(
        !driver.stage.is_changing(),
        "the touch that lifted the dim turned the page"
    );
    assert_eq!(driver.stage.screen(), Screen::Clock);
}

#[test]
fn a_cover_while_dimmed_does_nothing() {
    let mut driver = resting_on(Screen::Compass, Timeout::Seconds15, false);
    wait_until(&mut driver, 16_000_000, is_dimmed);
    driver.wait(rest::DIM_FADE);
    let update = driver.cover();
    assert_eq!(update.brightness, None);
    driver.wait(300_000);
    assert!(is_dimmed(driver.stage.rest()));
    assert_eq!(driver.stage.screen(), Screen::Compass);
}

#[test]
fn after_the_dim_the_panel_goes_off_and_nothing_redraws() {
    let mut driver = resting_on(Screen::Clock, Timeout::Seconds15, false);
    wait_until(&mut driver, 16_000_000, is_dimmed);
    let since = driver.now();
    let updates = wait_until(&mut driver, 6_000_000, |rest| rest == Rest::Off);
    let off = updates.last().unwrap();
    let after = driver.now() - since;
    assert!((5_300_000..5_400_000).contains(&after), "{after}");
    assert_eq!((off.brightness, off.display_on), (Some(0), Some(false)));
    let darkening: Vec<_> = updates
        .iter()
        .rev()
        .skip(1)
        .take(10)
        .filter_map(|update| update.brightness)
        .collect();
    assert!(
        darkening.len() > 5 && darkening.is_sorted(),
        "the level fades out before the panel goes off: {darkening:?}"
    );
    for _ in 0..200 {
        let update = driver.step(Input::default());
        assert!(driver.stage.changed().is_empty());
        assert_eq!((update.brightness, update.display_on), (None, None));
    }
    assert_eq!(driver.stage.next_change(), None);
}

#[test]
fn a_wake_from_off_runs_the_entry_and_the_climb_once_the_panel_is_on() {
    let mut driver = resting_on(Screen::Compass, Timeout::Seconds15, false);
    wait_until(&mut driver, 22_000_000, |rest| rest == Rest::Off);
    let woken = driver.now() + 2 * script::FRAME;
    let mut updates = vec![driver.double_tap()];
    assert_eq!(updates[0].display_on, Some(true));
    assert_eq!(driver.stage.rest(), Rest::Awake);
    assert_eq!(driver.stage.screen(), Screen::Compass);
    while driver.now() < woken + rest::PANEL_WAKE - script::FRAME {
        updates.push(driver.step(Input::default()));
        assert_eq!(
            accents(&driver),
            Accents::HIDDEN,
            "the entry started on a dark panel"
        );
    }
    while driver.stage.is_changing() {
        updates.push(driver.step(Input::default()));
    }
    let levels: Vec<_> = updates
        .iter()
        .filter_map(|update| update.brightness)
        .collect();
    assert!(levels.len() > 3, "{levels:?}");
    assert!(
        levels.is_sorted() && levels.last() == Some(&120),
        "{levels:?}"
    );
    assert_eq!(accents(&driver), Accents::FULL);
    assert_eq!(
        driver.stage.screen(),
        Screen::Compass,
        "the waking touch turned the page"
    );
}

#[test]
fn the_always_on_face_shows_after_the_dim_and_redraws_once_a_minute() {
    let mut driver = resting_on(Screen::Compass, Timeout::Seconds15, true);
    let updates = wait_until(&mut driver, 22_000_000, |rest| rest == Rest::AlwaysOn);
    assert_eq!(
        updates.last().unwrap().brightness,
        Some(rest::dim_level(120))
    );
    assert!(driver.stage.changed().is_full());
    assert_eq!(tiled_differs(&driver.stage), 0);
    // The clock started at 12:07:42 and is now about 21 seconds on, so the next two minutes
    // change twice.
    let redraws = (0..frames_in(120_000_000))
        .filter(|_| {
            driver.step(Input::default());
            !driver.stage.changed().is_empty()
        })
        .count();
    assert_eq!(redraws, 2);
}

/// Steps until the step changes something, and returns how long that took.
fn until_redrawn(driver: &mut Driver, within: Micros) -> Micros {
    let start = driver.now();
    while driver.now() < start + within {
        driver.step(Input::default());
        if !driver.stage.changed().is_empty() {
            return driver.now() - start;
        }
    }
    panic!("nothing redrew within {within} µs");
}

#[test]
fn the_always_on_face_takes_a_new_battery_reading_with_its_minute() {
    let mut driver = resting_on(Screen::Clock, Timeout::Seconds15, true);
    wait_until(&mut driver, 22_000_000, |rest| rest == Rest::AlwaysOn);
    let dublin = zone("Europe/Dublin", ZoneMode::Automatic);
    let battery = |percent| {
        Some(Battery {
            present: true,
            percent,
            millivolts: 3800,
            charging: false,
            usb: false,
        })
    };
    // The clock's own minute ends 55 s after 12:08:05.
    driver.sensors(Sensors {
        clock: clock_at(12, 8, 5),
        zone: dublin,
        battery: battery(50),
        ..Sensors::default()
    });
    assert!(
        driver.stage.changed().is_empty(),
        "the battery alone redrew the face"
    );
    assert!(until_redrawn(&mut driver, 60_000_000) > 50_000_000);
    // A stopped clock has no minute of its own, so the battery waits for the stage's.
    driver.sensors(Sensors {
        clock: ClockState {
            stopped: true,
            ..clock_at(12, 9, 0)
        },
        zone: dublin,
        battery: battery(50),
        ..Sensors::default()
    });
    assert!(
        !driver.stage.changed().is_empty(),
        "the change to STOPPED did not redraw"
    );
    driver.sensors(Sensors {
        clock: ClockState {
            stopped: true,
            ..clock_at(12, 9, 0)
        },
        zone: dublin,
        battery: battery(40),
        ..Sensors::default()
    });
    assert!(
        driver.stage.changed().is_empty(),
        "the battery alone redrew the face"
    );
    assert!(until_redrawn(&mut driver, 61_000_000) <= 60_000_000);
}

fn frames_in(duration: Micros) -> Micros {
    duration / script::FRAME
}

#[test]
fn a_wake_from_the_always_on_face_climbs_from_its_level() {
    let mut driver = resting_on(Screen::Clock, Timeout::Seconds15, true);
    wait_until(&mut driver, 22_000_000, |rest| rest == Rest::AlwaysOn);
    let mut updates = vec![driver.double_tap()];
    assert_eq!(updates[0].display_on, None);
    while driver.stage.is_changing() {
        updates.push(driver.step(Input::default()));
    }
    let levels: Vec<_> = updates
        .iter()
        .filter_map(|update| update.brightness)
        .collect();
    assert!(
        levels[0] > rest::dim_level(120) && levels.is_sorted(),
        "{levels:?}"
    );
    assert_eq!(levels.last(), Some(&120));
    assert_eq!(clock_accents(&driver), ClockAccents::FULL);
}

#[test]
fn a_resting_screen_wakes_only_on_a_double_tap() {
    for always_on in [true, false] {
        let mut driver = resting_on(Screen::Clock, Timeout::Seconds15, always_on);
        assert!(!driver.stage.watches_for_wake());
        let resting = if always_on { Rest::AlwaysOn } else { Rest::Off };
        wait_until(&mut driver, 22_000_000, |rest| rest == resting);
        assert!(driver.stage.watches_for_wake());

        driver.tap(Point::new(233, 233));
        driver.read(Touch::Gesture(TouchGesture::SwipeLeft));
        driver.read(Touch::Gesture(TouchGesture::Tap));
        driver.wait(rest::DOUBLE_TAP + script::FRAME);
        driver.read(Touch::Gesture(TouchGesture::Tap));
        assert_eq!(
            driver.stage.rest(),
            resting,
            "a contact, a swipe or two slow taps woke it"
        );

        driver.read(Touch::Gesture(TouchGesture::Tap));
        assert_eq!(driver.stage.rest(), Rest::Awake);
        assert!(!driver.stage.watches_for_wake());
    }
}

#[test]
fn a_wake_from_the_panel_lands_on_the_clock_and_drops_the_edit() {
    let mut driver = open_panel(Screen::Compass);
    tap(&mut driver, 150, 190);
    driver.swipe(Point::new(150, 280), Point::new(420, 280), 300_000);
    let updates = wait_until(&mut driver, 70_000_000, |rest| rest == Rest::Off);
    assert!(
        updates
            .iter()
            .any(|update| update.brightness == Some(rest::dim_level(255))),
        "the dim is taken from the level that shows"
    );
    let mut updates = vec![driver.double_tap()];
    while driver.stage.is_changing() {
        updates.push(driver.step(Input::default()));
    }
    assert_eq!(stored(&updates), None);
    assert!(driver.stage.page().is_none());
    assert_eq!(driver.stage.panel_offset(), 0);
    assert_eq!(driver.stage.screen(), Screen::Clock);
    assert_eq!(
        updates.iter().rev().find_map(|update| update.brightness),
        Some(120)
    );
    assert_eq!(driver.stage.peripherals().brightness, 120);
}

#[test]
fn turning_the_compass_keeps_the_screen_lit() {
    let mut driver = resting_on(Screen::Compass, Timeout::Seconds15, false);
    for turn in 1..=8 {
        driver.wait(5_000_000);
        driver.motion(heading(470 + turn * 150));
    }
    assert_eq!(driver.stage.rest(), Rest::Awake);
    let turned = driver.now();
    for nudge in 1..=3 {
        driver.wait(5_000_000);
        driver.motion(heading(470 + 8 * 150 + nudge * 30));
    }
    driver.wait(500_000);
    assert!(is_dimmed(driver.stage.rest()));
    let Rest::Dimmed { since } = driver.stage.rest() else {
        unreachable!()
    };
    assert!(
        (15_000_000..15_100_000).contains(&(since - turned)),
        "{}",
        since - turned
    );
}

#[test]
fn the_timer_waits_for_the_start_up() {
    let mut driver = Driver::starting();
    driver.stage = Stage::starting(PeripheralState {
        firmware: "0.1.0",
        timeout: Timeout::Seconds15,
        ..PeripheralState::default()
    });
    driver.wait(20_000_000);
    assert!(driver.stage.starting_up());
    assert_eq!(driver.stage.rest(), Rest::Awake);
    boot_all(&mut driver, None);
    while driver.stage.starting_up() {
        driver.step(Input::default());
    }
    driver.wait(14_000_000);
    assert_eq!(driver.stage.rest(), Rest::Awake);
    driver.wait(1_100_000);
    assert!(is_dimmed(driver.stage.rest()));
}

#[test]
fn never_keeps_the_screen_lit() {
    let mut driver = resting_on(Screen::Clock, Timeout::Never, true);
    driver.wait(600_000_000);
    assert_eq!(driver.stage.rest(), Rest::Awake);
    assert_eq!(driver.stage.next_change(), None);
}

/// Redrawing only what each step marked leaves the buffers as a full redraw would, through the
/// dim, the always-on face, a wake and the entry after it.
#[test]
fn rest_damage_redraws_what_changed() {
    let mut driver = resting_on(Screen::Clock, Timeout::Seconds15, true);
    let mut buffers = Buffers::new();
    let mut check = |driver: &Driver, when: &str| {
        let partial = buffers.draw(&driver.stage, driver.stage.changed());
        let whole = render(&driver.stage);
        assert_eq!(differing(partial, &whole), 0, "{when}");
        assert_eq!(differing(&buffers.panel, &whole), 0, "{when}, on the panel");
    };
    check(&driver, "awake");
    while driver.stage.rest() != Rest::AlwaysOn {
        driver.step(Input::default());
        check(&driver, &format!("{:?}", driver.stage.rest()));
    }
    for step in 0..frames_in(61_000_000) {
        driver.step(Input::default());
        if !driver.stage.changed().is_empty() || step < 3 {
            check(&driver, "always on");
        }
    }
    driver.double_tap();
    check(&driver, "woken");
    for step in 0..40 {
        driver.touch(None);
        check(&driver, &format!("entry {step}"));
    }
}

#[test]
fn the_timeout_screen_steps_through_the_five_and_keeps_one_on_a_tap() {
    let mut driver = open_panel(Screen::Clock);
    tap_cell(&mut driver, panel::Cell::Timeout);
    assert!(matches!(driver.stage.page(), Some(Page::Timeout(_))));
    // Up two steps from 1 MIN, past 5 MIN, to NEVER, and no further.
    driver.swipe(Point::new(233, 300), Point::new(233, 170), 300_000);
    let updates = tap(&mut driver, 233, 258);
    assert_eq!(stored(&updates), Some(Store::Timeout(Timeout::Never)));
    assert!(driver.stage.page().is_none());
    assert_eq!(driver.stage.peripherals().timeout, Timeout::Never);

    tap_cell(&mut driver, panel::Cell::Timeout);
    driver.swipe(Point::new(233, 200), Point::new(233, 360), 300_000);
    let updates = tap(&mut driver, 120, 120);
    assert_eq!(stored(&updates), None, "cancel kept a timeout");
    assert_eq!(driver.stage.peripherals().timeout, Timeout::Never);
    assert!(driver.stage.page().is_none());
}

/// Opens ALWAYS ON, drags each of `travels` pixels up past the tap slop, and keeps the choice.
fn choose_always_on(driver: &mut Driver, travels: &[i32]) -> Vec<Update> {
    tap_cell(driver, panel::Cell::AlwaysOn);
    assert!(driver.stage.page().is_some());
    for &travel in travels {
        let slop = 20 * travel.signum();
        let start = Point::new(233, 233 + travel / 2);
        driver.stroke(&[
            start,
            start - Point::new(0, slop),
            start - Point::new(0, slop + travel),
        ]);
    }
    tap(driver, 233, 258)
}

#[test]
fn always_on_steps_from_off_through_the_dim_level_to_each_percent() {
    let mut driver = open_panel(Screen::Clock);
    let updates = choose_always_on(&mut driver, &[25]);
    assert_eq!(stored(&updates), Some(Store::AlwaysOn(AlwaysOn::Dim)));
    assert_eq!(driver.stage.peripherals().always_on, AlwaysOn::Dim);
    assert!(driver.stage.page().is_none());
    // Twenty steps on from the dim level.
    let updates = choose_always_on(&mut driver, &[200, 200]);
    assert_eq!(
        stored(&updates),
        Some(Store::AlwaysOn(AlwaysOn::Percent(24)))
    );
    // The far end holds at the last level.
    let updates = choose_always_on(&mut driver, &[200, 200, 200]);
    assert_eq!(
        stored(&updates),
        Some(Store::AlwaysOn(AlwaysOn::Percent(50)))
    );
    let updates = choose_always_on(&mut driver, &[-200, -200, -200, -200, -200]);
    assert_eq!(stored(&updates), Some(Store::AlwaysOn(AlwaysOn::Off)));
}

#[test]
fn the_always_on_face_takes_a_fixed_level_whatever_the_brightness() {
    for (choice, level) in [
        (AlwaysOn::Dim, rest::dim_level(120)),
        (AlwaysOn::Percent(40), rest::level_of(40)),
    ] {
        let mut driver = resting_on(Screen::Clock, Timeout::Seconds15, true);
        driver.stage = Stage::new(PeripheralState {
            always_on: choice,
            ..*driver.stage.peripherals()
        });
        driver.stage.show(Screen::Clock);
        let updates = wait_until(&mut driver, 22_000_000, |rest| rest == Rest::AlwaysOn);
        assert_eq!(
            updates.iter().rev().find_map(|update| update.brightness),
            Some(level),
            "{choice:?}"
        );
    }
}

#[test]
fn a_stored_always_on_byte_reads_back() {
    for index in 0..AlwaysOn::CHOICES {
        let choice = AlwaysOn::choice(index);
        assert_eq!(choice.index(), index);
        assert_eq!(AlwaysOn::from_byte(choice.to_byte()), Some(choice));
    }
    // Saved as on or off, before there was a level.
    assert_eq!(AlwaysOn::from_byte(1), Some(AlwaysOn::Dim));
    assert_eq!(AlwaysOn::from_byte(0), Some(AlwaysOn::Off));
    assert_eq!(AlwaysOn::from_byte(51), None);
}

#[test]
fn clearing_puts_the_timeout_and_always_on_back() {
    let mut driver = open_panel(Screen::Clock);
    choose_always_on(&mut driver, &[25]);
    assert!(driver.stage.peripherals().always_on.is_on());
    tap_cell(&mut driver, panel::Cell::Timeout);
    driver.swipe(Point::new(233, 300), Point::new(233, 250), 300_000);
    tap(&mut driver, 233, 258);
    assert_eq!(driver.stage.peripherals().timeout, Timeout::Minutes5);
    tap_cell(&mut driver, panel::Cell::Device);
    scroll_device_to_end(&mut driver);
    tap(&mut driver, 233, 342);
    driver.swipe(Point::new(120, 258), Point::new(420, 258), 300_000);
    assert_eq!(driver.stage.peripherals().timeout, Timeout::Minute1);
    assert_eq!(driver.stage.peripherals().always_on, AlwaysOn::Off);
}

use octowhere_ui::ui::{power_off, stage::Key};

/// The slide on the power-off confirmation, from its handle to its target.
fn slide_to_power_off(driver: &mut Driver) -> Vec<Update> {
    driver.swipe(Point::new(90, 258), Point::new(420, 258), 300_000)
}

#[test]
fn a_long_press_asks_and_the_slide_powers_off_once_dark() {
    let mut driver = resting_on(Screen::Compass, Timeout::Seconds15, false);
    driver.key(Key::Long);
    assert!(driver.stage.power_off().is_some());
    assert!(driver.stage.changed().is_full());
    let mut updates = slide_to_power_off(&mut driver);
    assert!(updates.iter().all(|update| !update.power_off));
    updates.extend((0..30).map(|_| driver.step(Input::default())));
    let off: Vec<_> = updates.iter().filter(|update| update.power_off).collect();
    assert_eq!(off.len(), 1, "power off goes out once");
    assert_eq!(off[0].display_on, Some(false));
    assert_eq!(driver.stage.shown_level(), 0);
    // Nothing takes it back once it is on its way.
    driver.key(Key::Short);
    driver.cover();
    assert!(
        driver
            .stage
            .power_off()
            .is_some_and(|p| p.confirmed().is_some())
    );
}

#[test]
fn the_power_off_confirmation_cancels_by_its_button_a_cover_or_waiting() {
    let mut driver = resting_on(Screen::Compass, Timeout::Seconds15, false);
    driver.key(Key::Long);
    tap(&mut driver, 133, 118);
    assert!(driver.stage.power_off().is_none());
    assert_eq!(driver.stage.screen(), Screen::Compass);

    driver.key(Key::Long);
    driver.wait(100_000);
    driver.cover();
    assert!(driver.stage.power_off().is_none());
    // The cover only cancels; it does not also send the pages home.
    driver.settle();
    assert_eq!(driver.stage.screen(), Screen::Compass);

    driver.wait(300_000);
    driver.key(Key::Long);
    let opened = driver.now();
    while driver.stage.power_off().is_some() {
        assert!(driver.now() < opened + power_off::TIMEOUT + 100_000);
        driver.step(Input::default());
    }
    assert!(driver.now() >= opened + power_off::TIMEOUT);
    // The timeout of 15 s did not dim the screen under it.
    assert_eq!(driver.stage.rest(), Rest::Awake);
}

#[test]
fn a_short_press_rests_the_screen_at_once_and_another_wakes_it() {
    let mut driver = resting_on(Screen::Compass, Timeout::Seconds15, false);
    let update = driver.key(Key::Short);
    assert_eq!(driver.stage.rest(), Rest::Off);
    assert_eq!(
        (update.brightness, update.display_on),
        (Some(0), Some(false))
    );
    let update = driver.key(Key::Short);
    assert_eq!(driver.stage.rest(), Rest::Awake);
    assert_eq!(update.display_on, Some(true));

    let mut driver = resting_on(Screen::Clock, Timeout::Seconds15, true);
    let update = driver.key(Key::Short);
    assert_eq!(driver.stage.rest(), Rest::AlwaysOn);
    assert_eq!(update.brightness, Some(rest::dim_level(120)));
    assert!(driver.stage.changed().is_full());
}

#[test]
fn a_short_press_cancels_the_confirmation_and_rests() {
    let mut driver = resting_on(Screen::Clock, Timeout::Seconds15, false);
    driver.key(Key::Long);
    driver.key(Key::Short);
    assert!(driver.stage.power_off().is_none());
    assert_eq!(driver.stage.rest(), Rest::Off);
}

#[test]
fn a_long_press_from_off_wakes_onto_the_confirmation() {
    let mut driver = resting_on(Screen::Clock, Timeout::Seconds15, false);
    wait_until(&mut driver, 22_000_000, |rest| rest == Rest::Off);
    let update = driver.key(Key::Long);
    assert_eq!(update.display_on, Some(true));
    assert_eq!(driver.stage.rest(), Rest::Awake);
    assert!(driver.stage.power_off().is_some());
}

#[test]
fn the_start_up_ignores_the_power_key() {
    let mut driver = Driver::starting();
    driver.key(Key::Long);
    driver.key(Key::Short);
    assert!(driver.stage.starting_up());
    assert!(driver.stage.power_off().is_none());
    assert_eq!(driver.stage.rest(), Rest::Awake);
}

#[test]
fn a_release_short_of_the_target_or_a_tap_on_the_slide_keeps_the_confirmation() {
    let mut driver = resting_on(Screen::Clock, Timeout::Seconds15, false);
    driver.key(Key::Long);
    tap(&mut driver, 113, 257);
    // The handle's middle stops short of the target's left edge.
    driver.swipe(Point::new(113, 258), Point::new(320, 258), 300_000);
    driver.wait(100_000);
    let power_off = driver.stage.power_off().expect("the confirmation stays");
    assert_eq!(power_off.confirmed(), None);
    // Back at its start, the handle leaves the rail beside it black.
    assert_eq!(
        render(&driver.stage).pixel(Point::new(150, 240)),
        Some(chrome::BLACK)
    );
}

#[test]
fn a_drag_on_the_power_off_slide_redraws_only_the_slide() {
    let mut driver = resting_on(Screen::Clock, Timeout::Seconds15, false);
    driver.key(Key::Long);
    let mut buffers = Buffers::new();
    for _ in 0..2 {
        buffers.draw(&driver.stage, &Dirty::new_full());
    }
    for x in (113..=330).step_by(31) {
        driver.touch(Some(Point::new(x, 258)));
        assert!(!driver.stage.changed().is_full(), "at {x}");
        let partial = buffers.draw(&driver.stage, driver.stage.changed());
        let wrong = differing(partial, &render(&driver.stage));
        assert_eq!(wrong, 0, "at {x}: {wrong} pixels differ");
    }
}

#[test]
fn drawing_the_power_off_confirmation_in_tiles_matches_drawing_whole() {
    let mut rest = resting_on(Screen::Clock, Timeout::Seconds15, false);
    rest.key(Key::Long);
    let mut sliding = resting_on(Screen::Clock, Timeout::Seconds15, false);
    sliding.key(Key::Long);
    for x in (113..=270).step_by(20) {
        sliding.touch(Some(Point::new(x, 258)));
    }
    let mut confirmed = resting_on(Screen::Clock, Timeout::Seconds15, false);
    confirmed.key(Key::Long);
    slide_to_power_off(&mut confirmed);
    for (name, driver) in [
        ("rest", rest),
        ("sliding", sliding),
        ("confirmed", confirmed),
    ] {
        let differing = tiled_differs(&driver.stage);
        assert_eq!(differing, 0, "{name}: {differing} pixels differ");
    }
}

#[test]
fn a_cancel_puts_back_the_rest_the_key_woke_from() {
    let mut driver = resting_on(Screen::Clock, Timeout::Seconds15, false);
    wait_until(&mut driver, 22_000_000, |rest| rest == Rest::Off);
    driver.key(Key::Long);
    let updates: Vec<_> = (0..frames_in(power_off::TIMEOUT + 500_000))
        .map(|_| driver.step(Input::default()))
        .collect();
    assert!(driver.stage.power_off().is_none());
    assert_eq!(driver.stage.rest(), Rest::Off);
    assert_eq!(updates.last().map(|update| update.display_on), Some(None));
    assert!(
        updates
            .iter()
            .any(|update| update.display_on == Some(false))
    );

    let mut driver = resting_on(Screen::Clock, Timeout::Seconds15, true);
    driver.key(Key::Short);
    assert_eq!(driver.stage.rest(), Rest::AlwaysOn);
    driver.key(Key::Long);
    driver.wait(400_000);
    tap(&mut driver, 133, 118);
    assert_eq!(driver.stage.rest(), Rest::AlwaysOn);

    let mut driver = resting_on(Screen::Clock, Timeout::Seconds15, false);
    wait_until(&mut driver, 16_000_000, is_dimmed);
    driver.key(Key::Long);
    driver.cover();
    assert!(driver.stage.power_off().is_none());
    assert!(is_dimmed(driver.stage.rest()));
}

#[test]
fn the_confirmation_waits_ten_seconds_from_full_view_and_from_the_last_touch() {
    let mut driver = resting_on(Screen::Clock, Timeout::Seconds15, false);
    wait_until(&mut driver, 22_000_000, |rest| rest == Rest::Off);
    driver.key(Key::Long);
    let opened = driver.now();
    driver.wait(power_off::TIMEOUT + rest::PANEL_WAKE + rest::WAKE_FADE - 100_000);
    assert!(
        driver.stage.power_off().is_some(),
        "counted from the panel coming up"
    );
    driver.wait(200_000);
    assert!(driver.stage.power_off().is_none());
    assert!(driver.now() < opened + power_off::TIMEOUT + 1_000_000);

    let mut driver = resting_on(Screen::Clock, Timeout::Seconds15, false);
    driver.key(Key::Long);
    driver.wait(6_000_000);
    tap(&mut driver, 233, 200);
    driver.wait(6_000_000);
    assert!(driver.stage.power_off().is_some(), "restarted by the tap");
    driver.wait(4_500_000);
    assert!(driver.stage.power_off().is_none());
}

#[test]
fn the_power_off_slide_takes_a_drag_started_near_the_handle() {
    for start in [
        Point::new(100, 190),
        Point::new(60, 310),
        Point::new(170, 258),
    ] {
        let mut driver = resting_on(Screen::Clock, Timeout::Seconds15, false);
        driver.key(Key::Long);
        driver.swipe(start, start + Point::new(330, 20), 300_000);
        assert!(
            driver
                .stage
                .power_off()
                .is_some_and(|p| p.confirmed().is_some()),
            "from {start:?}"
        );
    }
}

/// Cancelling onto a dimming or darkening screen puts back the level it had reached and carries
/// on with the time it had left, never brightening it.
#[test]
fn a_cancel_onto_a_dimming_screen_resumes_it_where_it_was() {
    for darkening in [false, true] {
        for (before, open) in [(100_000, 100_000), (1_000_000, 1_000_000)] {
            let mut driver = resting_on(Screen::Clock, Timeout::Seconds15, false);
            if darkening {
                wait_until(&mut driver, 22_000_000, |rest| {
                    matches!(rest, Rest::Darkening { .. })
                });
            } else {
                wait_until(&mut driver, 16_000_000, is_dimmed);
            }
            let case = format!("darkening {darkening}, {before} µs in");
            // Darkening lasts less than a second, so it is caught early either way.
            if !darkening {
                driver.wait(before);
            }
            let (rest, level) = (driver.stage.rest(), driver.stage.shown_level());
            driver.key(Key::Long);
            driver.wait(open);
            let cancel = driver.cover();
            assert!(driver.stage.power_off().is_none());
            assert_eq!(cancel.brightness, Some(level), "{case}: back at its level");
            let resumed = match (rest, driver.stage.rest()) {
                (Rest::Dimmed { since: a }, Rest::Dimmed { since: b })
                | (Rest::Darkening { since: a }, Rest::Darkening { since: b }) => b - a,
                other => panic!("{case}: {other:?}"),
            };
            // The time the confirmation showed, give or take the key's and the cover's own steps.
            assert!(
                resumed.abs_diff(open) <= 2 * script::FRAME,
                "{case}: shifted {resumed}"
            );
            let mut shown = level;
            let later =
                (0..frames_in(rest::DIM_HOLD + 2_000_000)).map(|_| driver.step(Input::default()));
            for level in later.filter_map(|update| update.brightness) {
                assert!(level <= shown, "{case}: {shown} then {level}");
                shown = level;
            }
            assert_eq!(driver.stage.rest(), Rest::Off, "{case}");
        }
    }
}

/// Plugging in closes the solid fill in on its middle and builds the slices out from there;
/// unplugging runs the build back and grows the solid out from the middle again.
#[test]
fn charging_builds_the_slices_out_from_the_middle_and_unplugging_covers_them_from_it() {
    let mut driver = Driver::on(Screen::Clock);
    let reading = |charging| Sensors {
        clock: clock_at(12, 8, 5),
        zone: zone("Europe/Dublin", ZoneMode::Automatic),
        battery: Some(Battery {
            present: true,
            percent: 87,
            millivolts: 3900,
            charging,
            usb: charging,
        }),
        ..Sensors::default()
    };
    driver.sensors(reading(false));
    driver.wait(1_000_000);
    driver.sensors(reading(true));
    driver.wait(150_000);
    // The fill runs from x 265 to 415 at 87 %, on rows 284 to 307.
    let dark = |fb: &FB, xs: core::ops::Range<i32>| {
        xs.filter(|&x| fb.pixel(Point::new(x, 295)) == Some(chrome::BLACK))
            .count()
    };
    let closing = render(&driver.stage);
    assert!(dark(&closing, 265..290) > 0 && dark(&closing, 390..415) > 0);
    assert_eq!(dark(&closing, 320..360), 0, "the middle is still solid");
    driver.wait(350_000);
    assert_eq!(
        dark(&render(&driver.stage), 265..415),
        150,
        "the solid has gone"
    );
    driver.wait(100_000);
    let seeded = render(&driver.stage);
    assert!(dark(&seeded, 265..330) == 65 && dark(&seeded, 360..415) == 55);
    assert!(dark(&seeded, 330..360) < 30, "a seed grows in the middle");
    driver.wait(800_000);
    let built = render(&driver.stage);
    assert_ne!(built.pixel(Point::new(265, 295)), Some(chrome::BLACK));
    assert!(dark(&built, 265..415) > 0, "the slices reach the start");
    driver.sensors(reading(false));
    driver.wait(400_000);
    let unbuilding = render(&driver.stage);
    assert!(
        dark(&unbuilding, 265..415) > dark(&built, 265..415),
        "slices have gone"
    );
    assert_eq!(unbuilding.pixel(Point::new(265, 295)), Some(chrome::BLACK));
    driver.wait(700_000);
    let covering = render(&driver.stage);
    assert_eq!(dark(&covering, 320..360), 0, "the middle is solid again");
    assert_eq!(dark(&covering, 265..280), 15, "with nothing beside it");
}

/// After the start-up the gauge builds in from an empty fill rather than rising: the slices'
/// build while charging, otherwise the solid growing out from the middle.
#[test]
fn after_the_start_up_the_gauge_builds_in_from_the_middle() {
    for charging in [true, false] {
        let mut driver = Driver::starting();
        driver.sensors(Sensors {
            clock: clock_at(12, 7, 42),
            zone: zone("Europe/Dublin", ZoneMode::Automatic),
            battery: Some(Battery {
                present: true,
                percent: 87,
                millivolts: 3900,
                charging,
                usb: charging,
            }),
            ..Sensors::default()
        });
        boot_all(&mut driver, None);
        let mut buffers = Buffers::new();
        let mut step = |driver: &mut Driver| {
            driver.step(Input::default());
            let partial = buffers.draw(&driver.stage, driver.stage.changed());
            let whole = render(&driver.stage);
            assert_eq!(
                differing_as_shown(&driver.stage, partial, &whole),
                0,
                "{charging}"
            );
            whole
        };
        let mut shown = step(&mut driver);
        for _ in 0..400 {
            if !driver.stage.starting_up() {
                break;
            }
            shown = step(&mut driver);
        }
        assert!(!driver.stage.starting_up());
        // The fill runs from x 265 to 415 at 87 %, on rows 284 to 307.
        let dark = |fb: &FB, xs: core::ops::Range<i32>| {
            xs.filter(|&x| fb.pixel(Point::new(x, 295)) == Some(chrome::BLACK))
                .count()
        };
        assert_eq!(
            dark(&shown, 265..415),
            150,
            "{charging}: the fill starts empty"
        );
        let handed_over = driver.now();
        let mut at = |driver: &mut Driver, after: Micros| {
            let mut shown = None;
            while driver.now() < handed_over + after {
                shown = Some(step(driver));
            }
            shown.unwrap()
        };
        if charging {
            let seeded = at(&mut driver, 160_000 + 150_000);
            assert!(dark(&seeded, 330..360) < 30, "a seed grows in the middle");
            assert_eq!(dark(&seeded, 265..300), 35, "with nothing at the ends yet");
            let built = at(&mut driver, 160_000 + 1_000_000);
            assert_ne!(built.pixel(Point::new(265, 295)), Some(chrome::BLACK));
            assert!(dark(&built, 265..415) > 0, "the slices reach the start");
        } else {
            let growing = at(&mut driver, 160_000 + 250_000);
            assert_eq!(dark(&growing, 320..360), 0, "the middle is solid");
            assert_eq!(dark(&growing, 265..280), 15, "with nothing beside it");
            let grown = at(&mut driver, 160_000 + 600_000);
            assert_eq!(dark(&grown, 265..415), 0, "the solid fills it");
        }
    }
}

/// The step that cancels onto the always-on face draws that face, not the page under it.
#[test]
fn a_cancel_onto_the_always_on_face_draws_it_at_once() {
    let mut driver = resting_on(Screen::Clock, Timeout::Seconds15, true);
    wait_until(&mut driver, 22_000_000, |rest| rest == Rest::AlwaysOn);
    let resting = render(&driver.stage);
    driver.key(Key::Long);
    driver.wait(400_000);
    driver.touch(Some(Point::new(133, 118)));
    // The tap lands as the finger lifts; the reports without one are what the stage steps on.
    while driver.stage.power_off().is_some() {
        assert!(driver.now() < 30_000_000, "the tap did not cancel");
        driver.touch(None);
    }
    assert_eq!(driver.stage.rest(), Rest::AlwaysOn);
    let wrong = differing(&render(&driver.stage), &resting);
    assert_eq!(wrong, 0, "{wrong} pixels differ from the always-on face");
}

/// The BOOT key reaches the stage, which has no behaviour designed for it yet.
#[test]
fn the_boot_key_changes_nothing_yet() {
    let mut driver = resting_on(Screen::Compass, Timeout::Seconds15, false);
    for key in [Key::Short, Key::Long] {
        driver.boot_key(key);
        assert!(driver.stage.changed().is_empty(), "{key:?}");
        assert!(driver.stage.power_off().is_none());
        assert_eq!(driver.stage.rest(), Rest::Awake);
        assert_eq!(driver.stage.screen(), Screen::Compass);
    }
}

/// Pixel shift: the picture moves a step when a new page settles, and holds while it slides
/// and after.
#[test]
fn a_page_change_moves_the_picture_once_it_settles() {
    let mut driver = driver_on(Screen::Clock);
    assert_eq!(driver.stage.shift(), Point::zero());
    driver.touch(Some(Point::new(420, 233)));
    driver.touch(Some(Point::new(300, 233)));
    assert_eq!(
        driver.stage.shift(),
        Point::zero(),
        "not while the page slides"
    );
    driver.touch(Some(Point::new(60, 233)));
    driver.lift();
    driver.settle();
    assert_eq!(driver.stage.screen(), Screen::Compass);
    assert_eq!(driver.stage.shift(), Point::new(3, 0));
    driver.wait(5_000_000);
    assert_eq!(driver.stage.shift(), Point::new(3, 0));
}

/// Opening the panel and closing it each move the picture a step.
#[test]
fn the_panel_settling_open_or_shut_moves_the_picture() {
    let mut driver = open_panel(Screen::Clock);
    assert_eq!(driver.stage.shift(), Point::new(3, 0));
    driver.swipe(Point::new(233, 420), Point::new(233, 60), 250_000);
    driver.settle();
    driver.wait(500_000);
    assert_eq!(driver.stage.shift(), Point::new(2, 2));
}

/// A pinned picture stays where it was put through a page change, and moves again once let go.
#[test]
fn a_pinned_picture_holds_through_a_page_change() {
    let mut driver = driver_on(Screen::Clock);
    let now = driver.now();
    driver.stage.pin_shift(Some(4), now);
    assert_eq!(driver.stage.shift(), Point::new(-2, 2));
    driver.swipe(Point::new(420, 233), Point::new(60, 233), 250_000);
    driver.settle();
    assert_eq!(driver.stage.screen(), Screen::Compass);
    assert_eq!(driver.stage.shift(), Point::new(-2, 2));
    let now = driver.now();
    driver.stage.pin_shift(None, now);
    driver.swipe(Point::new(60, 233), Point::new(420, 233), 250_000);
    driver.settle();
    assert_eq!(driver.stage.shift(), Point::new(-3, 0));
}

/// A touch counts where the picture showed under it, not where the panel was touched.
#[test]
fn a_touch_lands_on_the_shifted_picture() {
    let mut driver = open_panel(Screen::Clock);
    let offset = driver.stage.shift();
    assert_ne!(offset, Point::zero());
    driver.touch(Some(Point::new(150, 150)));
    assert_eq!(driver.stage.contact(), Some(Point::new(150, 150) - offset));
}

/// With nothing to hide a move, the picture moves at a minute's change once it has held for
/// ten.
#[test]
fn an_unchanging_screen_moves_after_ten_minutes() {
    let mut driver = resting_on(Screen::Clock, Timeout::Never, false);
    driver.wait(9 * 60_000_000);
    assert_eq!(driver.stage.shift(), Point::zero());
    driver.wait(2 * 60_000_000);
    assert_eq!(driver.stage.shift(), Point::new(3, 0));
    driver.wait(8 * 60_000_000);
    assert_eq!(driver.stage.shift(), Point::new(3, 0));
}

/// The always-on face moves a step with each redraw, and a wake from it moves another.
#[test]
fn the_always_on_face_moves_with_each_redraw_and_a_wake() {
    let mut driver = resting_on(Screen::Clock, Timeout::Seconds15, true);
    wait_until(&mut driver, 22_000_000, |rest| rest == Rest::AlwaysOn);
    let entered = driver.stage.shift();
    assert_ne!(entered, Point::zero(), "the face's first redraw moves it");
    until_redrawn(&mut driver, 61_000_000);
    let minute = driver.stage.shift();
    assert_ne!(minute, entered);
    driver.double_tap();
    driver.wait(100_000);
    assert_eq!(driver.stage.rest(), Rest::Awake);
    assert_ne!(driver.stage.shift(), minute);
}

/// The start-up shows unshifted.
#[test]
fn the_start_up_shows_unshifted() {
    let mut driver = Driver::starting();
    boot_all(&mut driver, None);
    assert!(driver.stage.starting_up());
    assert_eq!(driver.stage.shift(), Point::zero());
}

/// The clock's band reaches past the glass as far as the shift moves it, so a shifted band still
/// meets the edge.
#[test]
fn the_band_reaches_past_the_glass_by_the_shifts_reach() {
    let mut driver = driver_on(Screen::Clock);
    driver.wait(2_000_000);
    let fb = render(&driver.stage);
    let band = fb.pixel(Point::new(233, 250));
    assert_ne!(band, Some(chrome::BLACK));
    let reach = octowhere_ui::ui::shift::REACH;
    for y in [200, 233, 280, 315] {
        // The glass's first column on this row, and the band's within a pixel of the reach
        // before it, the last one past the edge's antialiasing.
        let dy = y as f32 + 0.5 - 233.0;
        let glass = libm::ceilf(233.0 - libm::sqrtf(233.0 * 233.0 - dy * dy)) as i32;
        let from = (glass - reach + 1).max(0);
        assert_eq!(fb.pixel(Point::new(from, y)), band, "row {y}");
        assert_eq!(fb.pixel(Point::new(465 - from, y)), band, "row {y}");
    }
}
