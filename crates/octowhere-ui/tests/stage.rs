use embedded_graphics::prelude::{Point, Size};
use embedded_graphics::primitives::Rectangle;
use octowhere_ui::{
    chrome::{Clip, Color, Dirty, Window, FB},
    tz::DATABASE,
    ui::{
        clock::{ClockState, DateTime, ZoneMode, ZoneState},
        clock_screen::Accents as ClockAccents,
        compass::CompassView,
        gesture::{LIFT_SAMPLES, Micros},
        compass_screen::{Accents, CENTER as COMPASS_CENTER},
        screens::Screen,
        script::{self, Driver},
        stage::{Input, Motion, Sensors, Stage},
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
    let left: Vec<_> = (0..8).map(|step| Point::new(400 - step * 40, 233)).collect();
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
    assert_eq!(driver.stage.peripherals().compass.heading_decidegrees, Some(470));
}

#[test]
fn a_cover_on_the_clock_changes_nothing() {
    let mut driver = Driver::on(Screen::Clock);
    driver.wait(500_000);
    driver.cover();
    assert!(!driver.stage.is_animating());
    assert_eq!(driver.stage.screen(), Screen::Clock);
}

#[test]
fn a_tap_on_the_compass_does_nothing() {
    let mut driver = settled_on_compass();
    let updates = driver.stroke(&[COMPASS_CENTER + Point::new(30, -40)]);
    assert!(updates.iter().all(|update| !update.recalibrate));
    driver.stroke(&[Point::new(233, 73)]);
    assert!(!driver.stage.is_animating());
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
    assert_eq!(accents(&driver).ring, 0);
    assert!(driver.stage.is_animating());
    driver.wait(150_000);
    let early = accents(&driver);
    assert_eq!(early.ring, 255);
    assert!(early.icon_rows > 0 && early.icon_rows < 5, "{early:?}");
    assert_eq!(early.dial, 0);
    driver.wait(300_000);
    assert_eq!(accents(&driver), Accents::FULL);
    assert!(!driver.stage.is_animating());
}

#[test]
fn a_swipe_off_the_compass_takes_its_accents_reversibly() {
    let mut driver = settled_on_compass();
    driver.touch(Some(Point::new(400, 233)));
    driver.touch(Some(Point::new(360, 233)));
    driver.touch(Some(Point::new(340, 233)));
    let part = accents(&driver);
    assert!(part.caption < 255 && part.ring == 255, "{part:?}");
    driver.touch(Some(Point::new(200, 233)));
    assert_eq!(accents(&driver), Accents::HIDDEN);
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
    for _ in 0..LIFT_SAMPLES {
        driver.touch(None);
    }
    for _ in 0..120 {
        if driver.stage.screen() == Screen::Compass {
            break;
        }
        assert_eq!(driver.stage.accents().ring, 255, "an unsettled compass changed its accents");
        driver.step(Input::default());
    }
    assert_eq!(driver.stage.screen(), Screen::Compass);
    assert_eq!(accents(&driver).ring, 0, "the entry began before the page settled");
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
    for _ in 0..LIFT_SAMPLES {
        driver.touch(None);
    }
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

fn top_edge_up() -> Motion {
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
fn a_heading_back_quickly_from_top_edge_up_skips_the_reveal() {
    let back = heading_back_after(top_edge_up(), 300_000);
    assert_eq!((back.dial, back.icon_rows, back.caption), (255, 5, 255), "{back:?}");
}

#[test]
fn a_heading_back_slowly_from_top_edge_up_reveals_again() {
    let accents = heading_back_after(top_edge_up(), 1_000_000);
    assert!(accents.dial < 255 && accents.icon_rows < 5, "{accents:?}");
}

#[test]
fn a_heading_back_from_no_data_reveals_again() {
    let accents = heading_back_after(Motion::default(), 300_000);
    assert!(accents.dial < 255 && accents.icon_rows < 5, "{accents:?}");
}

/// Two framebuffers drawn in turn as the frame loop draws them: each repaints the damage of the
/// step before as well as its own, since it last saw the frame before that.
struct Buffers {
    fbs: [Box<FB>; 2],
    next: usize,
    previous: Dirty,
    drawn: [bool; 2],
    /// Pixels repainted, over every step.
    pixels: u64,
    /// What the panel shows: each buffer's flush rectangles copied in after it is drawn.
    panel: Box<FB>,
    flushed: bool,
}

impl Buffers {
    fn new() -> Self {
        Self {
            fbs: [FB::boxed(), FB::boxed()],
            next: 0,
            previous: Dirty::new_full(),
            drawn: [false; 2],
            pixels: 0,
            panel: FB::boxed(),
            flushed: false,
        }
    }

    /// Draws `stage` after a step that changed `changed`, and returns the buffer drawn into.
    fn draw(&mut self, stage: &Stage, changed: &Dirty) -> &FB {
        let mut repaint = self.previous.clone();
        repaint.extend(changed);
        self.previous = changed.clone();
        let index = self.next;
        self.next ^= 1;
        let fb = &mut *self.fbs[index];
        if repaint.is_full() || !self.drawn[index] {
            self.drawn[index] = true;
            self.pixels += 466 * 466;
            stage.draw(fb);
        } else if !repaint.is_empty() {
            self.pixels += u64::from(repaint.pixels());
            stage.draw(&mut Clip::new(fb, &repaint));
        }
        // The panel shows the step before, so the flush sends only this step's damage.
        let flush = if self.flushed { changed.clone() } else { Dirty::new_full() };
        self.flushed = true;
        let rows = |rect: Rectangle| {
            (rect.top_left.y..rect.top_left.y + rect.size.height as i32).map(move |y| {
                let start = (y * 466 + rect.top_left.x) as usize * 2;
                start..start + rect.size.width as usize * 2
            })
        };
        let whole = Rectangle::new(Point::zero(), Size::new(466, 466));
        let rects: Vec<_> = if flush.is_full() {
            vec![whole]
        } else {
            flush.rectangles(octowhere_ui::chrome::FLUSH_OVERHEAD).collect()
        };
        for rect in rects {
            for range in rows(rect) {
                self.panel.buffer_mut()[range.clone()].copy_from_slice(&fb.buffer()[range]);
            }
        }
        &self.fbs[index]
    }
}

/// How many pixels on the round panel differ. The square's corners are never seen or cleared,
/// and a page moving across them leaves what it drew there.
fn differing(a: &FB, b: &FB) -> usize {
    (0..466 * 466)
        .map(|index| Point::new(index % 466, index / 466))
        .filter(|&point| {
            let (x, y) = (point.x as f32 + 0.5 - 233.0, point.y as f32 + 0.5 - 233.0);
            x * x + y * y <= 233.0 * 233.0
        })
        .filter(|&point| a.pixel(point) != b.pixel(point))
        .count()
}

/// Readings that walk the compass through every state and turn the dial both ways.
fn compass_walk() -> Vec<(String, Motion)> {
    let mut walk = Vec::new();
    for step in 0..40 {
        // Uneven steps, across north, both ways.
        let decidegrees = (3400 + step * 37) % 3600;
        walk.push((format!("heading {decidegrees}"), heading(decidegrees as u16)));
    }
    for step in 0..12 {
        let mut motion = heading(1234 - step * 13);
        motion.compass.pitch_deg = (step as i8 - 6) * 17;
        motion.compass.roll_deg = (step as i8).wrapping_mul(23).wrapping_sub(90);
        motion.compass.disturbed = step % 3 == 0;
        walk.push((format!("tilted {step}"), motion));
    }
    // A degree at a time, through zero and across a change in the line's length.
    for (pitch, roll) in [(5, -12), (6, -12), (6, -11), (1, -1), (0, 0), (-1, 1), (9, 99), (10, 100), (-10, -100)] {
        let mut motion = heading(1800);
        motion.compass.pitch_deg = pitch;
        motion.compass.roll_deg = roll;
        walk.push((format!("tilt {pitch} {roll}"), motion));
    }
    walk.push(("top edge up".into(), top_edge_up()));
    walk.push(("back from top edge".into(), heading(900)));
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
            assert_eq!(wrong, 0, "{name}, frame {frame}: {wrong} pixels differ on the panel");
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
    println!("a degree a step repaints {:.1}% of the panel", share * 100.0);
    // What the flush would send for the last step, at a few region costs, and what that takes
    // at the cost per pixel and per region measured on the target.
    let mut repaint = buffers.previous.clone();
    driver.motion(heading(470 + STEPS as u16 * 10 + 10));
    repaint.extend(driver.stage.changed());
    for overhead in [0, 64, 128, 256, 512, 1024] {
        let (regions, pixels) = repaint
            .rectangles(overhead)
            .fold((0, 0), |(regions, pixels), rect| (regions + 1, pixels + rect.size.width * rect.size.height));
        println!(
            "overhead {overhead}: {regions} regions, {pixels} px, modelled {:.2} ms",
            f64::from(pixels) * 59e-6 + f64::from(regions) * 0.056
        );
    }
    assert!(share < 0.5, "{:.1}%", share * 100.0);
}

fn clock_at(hour: u8, minute: u8, second: u8) -> ClockState {
    ClockState {
        utc: Some(DateTime { year: 2026, month: 10, day: 24, hour, minute, second }.to_unix()),
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
        sensors: Some(Sensors { clock, zone, ..Sensors::default() }),
        ..Input::default()
    }
}

fn clock_accents(driver: &Driver) -> ClockAccents {
    let mut fb = FB::boxed();
    driver.stage.draw(&mut *fb);
    driver.stage.clock_accents()
}

/// Readings that walk the clock through ticks, rollovers and every state.
fn clock_walk() -> Vec<(String, Input)> {
    let dublin = zone("Europe/Dublin", ZoneMode::Automatic);
    let mut walk = Vec::new();
    for second in 55..60 {
        walk.push((format!("second {second}"), sensors(clock_at(22, 59, second), dublin)));
    }
    // Into the next hour, then across local midnight.
    walk.push(("hour".into(), sensors(clock_at(23, 0, 0), dublin)));
    walk.push(("midnight".into(), sensors(clock_at(23, 0, 1), zone("Europe/Berlin", ZoneMode::Automatic))));
    walk.push(("manual".into(), sensors(clock_at(23, 0, 2), zone("Europe/Berlin", ZoneMode::Manual))));
    walk.push((
        "rtc".into(),
        sensors(ClockState { set_from_gnss: false, ..clock_at(23, 0, 3) }, dublin),
    ));
    walk.push(("gnss".into(), sensors(clock_at(23, 0, 4), dublin)));
    walk.push((
        "stopped".into(),
        sensors(ClockState { stopped: true, ..clock_at(0, 0, 0) }, dublin),
    ));
    walk.push(("set again".into(), sensors(clock_at(23, 0, 5), dublin)));
    walk.push(("no zone".into(), sensors(clock_at(23, 0, 6), ZoneState::default())));
    walk.push(("no zone tick".into(), sensors(clock_at(23, 1, 6), ZoneState::default())));
    walk.push(("found".into(), sensors(clock_at(23, 1, 7), zone("America/Argentina/Buenos_Aires", ZoneMode::Automatic))));
    walk.push(("no data".into(), sensors(ClockState { utc: None, ..clock_at(0, 0, 0) }, dublin)));
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
            assert_eq!(wrong, 0, "{name}, frame {frame}: {wrong} pixels differ on the panel");
            driver.step(Input::default());
        }
    }
}

#[test]
fn a_second_repaints_one_small_region() {
    let dublin = zone("Europe/Dublin", ZoneMode::Automatic);
    let mut driver = Driver::on(Screen::Clock);
    driver.step(sensors(clock_at(12, 7, 41), dublin));
    driver.wait(500_000);
    driver.step(sensors(clock_at(12, 7, 42), dublin));
    let pixels = driver.stage.changed().pixels();
    assert!(pixels > 0 && pixels < 1_500, "{pixels} px");
    driver.step(sensors(clock_at(12, 7, 42), dublin));
    assert!(driver.stage.changed().is_empty(), "an unchanged reading repainted");
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
    driver.wait(400_000);
    assert_eq!(clock_accents(&driver), ClockAccents::FULL);
    assert!(!driver.stage.is_animating());

    driver.touch(Some(Point::new(400, 233)));
    driver.touch(Some(Point::new(380, 233)));
    let part = clock_accents(&driver);
    assert!(part.zone < 255 && part.ring == 255, "{part:?}");
    driver.touch(Some(Point::new(200, 233)));
    assert_eq!(clock_accents(&driver).icon_rows, 0);
    driver.touch(Some(Point::new(399, 233)));
    for _ in 0..LIFT_SAMPLES {
        driver.touch(None);
    }
    driver.settle();
    assert_eq!(driver.stage.screen(), Screen::Clock);
    assert_eq!(clock_accents(&driver), ClockAccents::FULL, "the entry replayed");
}

#[test]
fn the_clock_rebuilds_on_a_change_and_shows_a_fault_at_once() {
    let dublin = zone("Europe/Dublin", ZoneMode::Automatic);
    let mut driver = Driver::on(Screen::Clock);
    driver.step(sensors(ClockState { set_from_gnss: false, ..clock_at(12, 7, 42) }, dublin));
    driver.wait(500_000);
    driver.step(sensors(clock_at(12, 7, 43), dublin));
    let resync = clock_accents(&driver);
    assert_eq!((resync.icon_rows, resync.label, resync.plate), (1, 255, 255), "{resync:?}");
    driver.wait(500_000);
    driver.step(sensors(ClockState { utc: None, ..clock_at(12, 7, 43) }, dublin));
    assert_eq!(clock_accents(&driver), ClockAccents::FULL);
    driver.step(sensors(clock_at(12, 7, 44), dublin));
    // The plate kept the zone's offset through the fault, so only the icon and label rebuild.
    let back = clock_accents(&driver);
    assert!(back.icon_rows < 5 && back.label < 255 && back.plate == 255, "{back:?}");
}

#[test]
fn a_tap_on_the_clock_does_nothing() {
    let mut driver = Driver::on(Screen::Clock);
    driver.wait(500_000);
    driver.stroke(&[Point::new(233, 73)]);
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
        assert_eq!((changing.ring, changing.dial, changing.caption), (255, 255, 255), "{changing:?}");
        assert!(changing.icon_rows < 5 && !changing.change.done(), "{changing:?}");
        assert!(driver.stage.is_animating());
        driver.wait(200_000);
        assert_eq!(accents(&driver), Accents::FULL);
        assert!(!driver.stage.is_animating());
    }
}

/// The slab's colour at its top row and its bottom row.
fn slab_ends(stage: &Stage) -> (Option<Color>, Option<Color>) {
    let fb = render(stage);
    (fb.pixel(Point::new(140, 205)), fb.pixel(Point::new(140, 293)))
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
    assert_eq!((entering.ring, entering.icon_rows, entering.caption), (255, 5, 255));
    let mut driver = settled_on_compass();
    driver.motion(Motion::default());
    let fault = accents(&driver);
    assert_eq!((fault.ring, fault.icon_rows, fault.caption), (255, 5, 255));
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
    assert_eq!(driver.stage.peripherals().clock.clock().utc, clock_at(12, 7, 42).utc);

    driver.step(sensors(ClockState { stopped: true, ..clock_at(12, 7, 50) }, dublin));
    driver.wait(1_500_000);
    assert_eq!(driver.stage.peripherals().clock.clock().utc, clock_at(12, 7, 50).utc);
}

#[test]
fn a_swipe_takes_its_duration_and_turns_the_page() {
    let mut driver = Driver::new();
    let updates = driver.swipe(Point::new(400, 233), Point::new(60, 233), 300_000);
    assert_eq!(updates.len(), 18 + 1 + usize::from(LIFT_SAMPLES));
    driver.settle();
    assert_eq!(driver.stage.screen(), Screen::Compass);
}

/// Whether the time is typing in again after a step with `input`, then whether it finished.
fn retypes(driver: &mut Driver, input: Input) -> bool {
    driver.step(input);
    let typing = clock_accents(driver).time < 255;
    if typing {
        driver.wait(400_000);
        assert_eq!(clock_accents(driver), ClockAccents::FULL, "the reveal never finished");
    }
    typing
}

#[test]
fn the_time_types_in_again_when_a_fix_or_zone_replaces_it_and_not_on_a_tick() {
    let dublin = zone("Europe/Dublin", ZoneMode::Automatic);
    let unset = |clock: ClockState| ClockState { set_from_gnss: false, ..clock };
    let mut driver = Driver::on(Screen::Clock);
    driver.step(sensors(unset(clock_at(12, 7, 40)), ZoneState::default()));
    driver.wait(500_000);

    assert!(retypes(&mut driver, sensors(clock_at(12, 7, 41), dublin)), "a first fix");
    // The reveal took 400 ms and the clock ticks a second later, which is no jump.
    driver.wait(100_000);
    assert!(!retypes(&mut driver, sensors(clock_at(12, 7, 42), dublin)), "a tick");
    driver.wait(500_000);
    assert!(!retypes(&mut driver, sensors(unset(clock_at(12, 7, 43)), dublin)), "no time change");
    assert!(retypes(&mut driver, sensors(clock_at(12, 12, 0), dublin)), "a correction");
    // Both are an hour ahead of UTC in October.
    let london = zone("Europe/London", ZoneMode::Automatic);
    assert!(!retypes(&mut driver, sensors(clock_at(12, 12, 0), london)), "the same offset");
    let berlin = zone("Europe/Berlin", ZoneMode::Automatic);
    assert!(retypes(&mut driver, sensors(clock_at(12, 12, 0), berlin)), "a new offset");
    let stopped = ClockState { stopped: true, ..clock_at(12, 12, 0) };
    assert!(!retypes(&mut driver, sensors(stopped, berlin)), "dashes");
    assert!(retypes(&mut driver, sensors(clock_at(12, 12, 1), berlin)), "leaving STOPPED");
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
    driver.stage = Stage::new(PeripheralState { firmware: "0.1.0", ..PeripheralState::default() });
    driver.stage.show(screen);
    driver.step(Input {
        sensors: Some(Sensors {
            clock: clock_at(12, 7, 42),
            zone: zone("Europe/Dublin", ZoneMode::Automatic),
            battery: Some(Battery { present: true, percent: 87, millivolts: 4020, charging: true, usb: true }),
            gnss: Gnss { fix: true, in_use: 9, in_view: 14, position: None },
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
    let updates = driver.stroke(&[Point::new(x, y)]);
    driver.wait(300_000);
    updates
}

fn stored(updates: &[Update]) -> Option<Store> {
    updates.iter().find_map(|update| update.store)
}

#[test]
fn a_downward_drag_on_either_face_opens_the_panel() {
    for screen in Screen::ALL {
        let driver = open_panel(screen);
        assert_eq!(driver.stage.panel_offset(), HEIGHT, "from {screen:?}");
        assert_eq!(driver.stage.panel_accents(), PanelAccents::FULL);
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
    assert_eq!(driver.stage.panel_accents(), PanelAccents::HIDDEN);
    for _ in 0..3 {
        driver.touch(None);
    }
    while driver.stage.panel_offset() < HEIGHT {
        driver.wait(0);
    }
    driver.wait(100_000);
    let early = driver.stage.panel_accents();
    assert!(early.ring > 0 && early.hint == 0 && early.rows[5] == 0, "{early:?}");
    driver.wait(400_000);
    assert_eq!(driver.stage.panel_accents(), PanelAccents::FULL);
}

#[test]
fn an_upward_drag_closes_the_panel_to_the_face_it_came_from() {
    let mut driver = open_panel(Screen::Compass);
    driver.swipe(Point::new(233, 420), Point::new(233, 100), 250_000);
    driver.settle();
    assert_eq!(driver.stage.panel_offset(), 0);
    assert_eq!(driver.stage.screen(), Screen::Compass);
    assert_eq!(accents(&driver), Accents::FULL, "the compass entry did not run again");
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
fn a_sideways_drag_scrolls_the_grid_and_it_snaps_to_either_end() {
    let mut driver = open_panel(Screen::Clock);
    driver.swipe(Point::new(380, 250), Point::new(260, 250), 400_000);
    driver.settle();
    assert_eq!(driver.stage.panel_scroll(), panel::MAX_SCROLL);
    assert_eq!(driver.stage.panel_offset(), HEIGHT);
    driver.swipe(Point::new(200, 250), Point::new(260, 250), 400_000);
    driver.settle();
    assert_eq!(driver.stage.panel_scroll(), panel::MAX_SCROLL, "a short drag moved it");
    driver.swipe(Point::new(200, 250), Point::new(260, 250), 50_000);
    driver.settle();
    assert_eq!(driver.stage.panel_scroll(), 0, "a flick did not carry it back");
}

#[test]
fn a_tap_on_the_cropped_column_scrolls_it_into_view_without_opening_it() {
    let mut driver = open_panel(Screen::Clock);
    tap(&mut driver, 420, 150);
    assert_eq!(driver.stage.panel_scroll(), panel::MAX_SCROLL);
    assert!(driver.stage.page().is_none());
    tap(&mut driver, 330, 300);
    assert!(matches!(driver.stage.page(), Some(Page::Device(_))));
}

#[test]
fn the_compass_cell_restarts_calibration_and_closes_to_the_compass() {
    let mut driver = open_panel(Screen::Clock);
    let updates = tap(&mut driver, 300, 150);
    assert!(updates.iter().any(|update| update.recalibrate));
    driver.settle();
    assert_eq!(driver.stage.panel_offset(), 0);
    assert_eq!(driver.stage.screen(), Screen::Compass);
    assert_eq!(driver.stage.peripherals().compass.heading_decidegrees, None);
}

#[test]
fn the_brightness_editor_shows_each_step_live_and_stores_on_a_tap_on_its_hint() {
    let mut driver = open_panel(Screen::Clock);
    tap(&mut driver, 150, 300);
    assert!(matches!(driver.stage.page(), Some(Page::Brightness(_))));
    let updates = driver.swipe(Point::new(150, 280), Point::new(420, 280), 300_000);
    let levels: Vec<_> = updates.iter().filter_map(|update| update.brightness).collect();
    assert!(levels.len() > 3 && levels.last() == Some(&255), "{levels:?}");
    assert_eq!(driver.stage.peripherals().brightness, 120, "the level was stored before a tap");
    let updates = tap(&mut driver, 233, 378);
    assert_eq!(stored(&updates), Some(Store::Brightness(255)));
    assert!(driver.stage.page().is_none());
    assert_eq!(driver.stage.peripherals().brightness, 255);
}

#[test]
fn cancel_or_a_cover_puts_the_brightness_back() {
    let mut driver = open_panel(Screen::Clock);
    tap(&mut driver, 150, 300);
    driver.swipe(Point::new(150, 280), Point::new(80, 280), 200_000);
    let updates = tap(&mut driver, 120, 120);
    assert_eq!(updates.iter().rev().find_map(|update| update.brightness), Some(120));
    assert!(driver.stage.page().is_none());
    assert_eq!(driver.stage.panel_offset(), HEIGHT);

    tap(&mut driver, 150, 300);
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
    tap(&mut driver, 150, 300);
    driver.swipe(Point::new(150, 280), Point::new(420, 280), 300_000);
    tap(&mut driver, 233, 250);
    tap(&mut driver, 300, 300);
    scroll_device_to_end(&mut driver);
    tap(&mut driver, 233, 342);
    assert!(matches!(driver.stage.page(), Some(Page::Clear(_))));
    // Short of the target, or not starting on the handle, erases nothing.
    let updates = driver.swipe(Point::new(90, 258), Point::new(250, 258), 300_000);
    assert_eq!(stored(&updates), None);
    let updates = driver.swipe(Point::new(200, 258), Point::new(420, 258), 300_000);
    assert_eq!(stored(&updates), None);
    let updates = driver.swipe(Point::new(90, 258), Point::new(420, 258), 300_000);
    assert_eq!(stored(&updates), Some(Store::Clear));
    assert!(driver.stage.page().is_none());
    assert_eq!(driver.stage.peripherals().brightness, 120);
}

#[test]
fn the_picker_stores_a_zone_by_hand_and_automatic_from_its_first_step() {
    let mut driver = open_panel(Screen::Clock);
    tap(&mut driver, 150, 150);
    assert!(matches!(driver.stage.page(), Some(Page::Picker(_))));
    // One row up is the next offset, +02:00, and its zones start at the top of the list.
    driver.swipe(Point::new(233, 300), Point::new(233, 255), 300_000);
    tap(&mut driver, 233, 250);
    let updates = tap(&mut driver, 233, 250);
    let Some(Store::ManualZone(zone)) = stored(&updates) else {
        panic!("no zone was stored: {:?}", stored(&updates));
    };
    assert_eq!(DATABASE.zone(zone).at(clock_at(12, 7, 42).utc.unwrap()).utc_offset, 7200);
    assert_eq!(driver.stage.peripherals().clock.zone(), octowhere_ui::ui::clock::ZoneState {
        mode: ZoneMode::Manual,
        zone: Some(zone),
    });

    tap(&mut driver, 150, 150);
    let updates = tap(&mut driver, 233, 390);
    assert_eq!(stored(&updates), Some(Store::AutomaticZone));
    assert_eq!(driver.stage.peripherals().clock.zone().mode, ZoneMode::Automatic);
}

#[test]
fn a_fling_in_the_picker_keeps_stepping_and_stops() {
    let mut driver = open_panel(Screen::Clock);
    tap(&mut driver, 150, 150);
    let Some(Page::Picker(before)) = driver.stage.page().cloned() else { panic!() };
    driver.swipe(Point::new(233, 300), Point::new(233, 200), 60_000);
    driver.settle();
    let Some(Page::Picker(after)) = driver.stage.page().cloned() else { panic!() };
    assert_ne!(before, after);
    assert!(!driver.stage.is_animating());
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
    for (name, (x, y)) in [("brightness", (150, 300)), ("device", (300, 300)), ("picker", (150, 150))] {
        let mut driver = open_panel(Screen::Clock);
        tap(&mut driver, x, y);
        stages.push((name, driver.stage));
    }
    let mut driver = open_panel(Screen::Clock);
    tap(&mut driver, 150, 150);
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
        ("swiping left", [Point::new(400, 233), Point::new(380, 233), Point::new(250, 236)]),
        ("swiping right", [Point::new(60, 233), Point::new(80, 233), Point::new(300, 230)]),
        ("pulling the panel", [Point::new(233, 40), Point::new(233, 60), Point::new(236, 200)]),
    ] {
        for screen in Screen::ALL {
            let mut driver = Driver::on(screen);
            driver.step(sensors(clock_at(12, 7, 42), zone("Europe/Dublin", ZoneMode::Automatic)));
            driver.wait(500_000);
            for point in path {
                driver.touch(Some(point));
            }
            cases.push((format!("{screen:?} {name}"), driver.stage));
        }
    }
    for (name, stage) in cases {
        let mut over = FB::boxed();
        over.fill_solid(&Rectangle::new(Point::zero(), Size::new(466, 466)), octowhere_ui::chrome::PURPLE)
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
        let outcome = if failing == Some(part) { Outcome::NoReply } else { Outcome::Answered };
        driver.boot(part, outcome);
    }
}

/// Redrawing only what each step marked leaves both buffers and the panel as a full redraw
/// would, from the first self-test frame through the identity, the card and the clock's entry,
/// and through a failure's fault screen.
#[test]
fn start_up_damage_redraws_what_changed() {
    for failing in [None, Some(Part::Magnet)] {
        let mut driver = Driver::starting();
        driver.sensors(sensors(clock_at(12, 7, 42), zone("Europe/Dublin", ZoneMode::Automatic)).sensors.unwrap());
        let mut buffers = Buffers::new();
        let mut check = |driver: &Driver, when: &str| {
            let partial = buffers.draw(&driver.stage, driver.stage.changed());
            let whole = render(&driver.stage);
            assert_eq!(differing(partial, &whole), 0, "{failing:?} {when}");
            assert_eq!(differing(&buffers.panel, &whole), 0, "{failing:?} {when}, on the panel");
        };
        for part in Part::ALL {
            for _ in 0..6 {
                driver.step(Input::default());
                check(&driver, "waiting");
            }
            let outcome = if failing == Some(part) { Outcome::NoReply } else { Outcome::Answered };
            driver.boot(part, outcome);
            check(&driver, part.name());
        }
        for step in 0..300 {
            driver.step(Input::default());
            check(&driver, &format!("step {step}"));
        }
        assert!(!driver.stage.starting_up());
    }
}

#[test]
fn the_brightness_climbs_from_dark_on_the_first_frame() {
    let mut driver = Driver::starting();
    assert_eq!(driver.step(Input::default()).brightness, Some(0));
    let levels: Vec<_> = (0..14).filter_map(|_| driver.step(Input::default()).brightness).collect();
    assert!(levels.windows(2).all(|pair| pair[0] < pair[1]), "{levels:?}");
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
    assert_eq!(driver.stage.clock_accents(), ClockAccents::FULL);
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
    driver.sensors(sensors(clock_at(12, 7, 42), zone("Europe/Dublin", ZoneMode::Automatic)).sensors.unwrap());
    boot_all(&mut driver, None);
    while driver.stage.starting_up() {
        driver.step(Input::default());
    }
    let accents = driver.stage.clock_accents();
    assert!(accents.time < u8::MAX && accents.ring < u8::MAX, "{accents:?}");
}

#[test]
fn a_failure_shows_the_fault_screen_then_the_clock() {
    let mut driver = Driver::starting();
    boot_all(&mut driver, Some(Part::Gnss));
    // The hold, then 120 frames at 30 fps.
    driver.wait(4_200_000);
    assert!(driver.stage.starting_up());
    driver.wait(200_000);
    assert!(!driver.stage.starting_up());
    assert_eq!(driver.stage.screen(), Screen::Clock);
}

/// Scrolls the open device page past its end, which leaves the clear box on rows 320 to 363
/// and the replay box on rows 376 to 419.
fn scroll_device_to_end(driver: &mut Driver) {
    driver.swipe(Point::new(233, 400), Point::new(233, 200), 300_000);
}

fn open_device_page() -> Driver<'static> {
    let mut driver = open_panel(Screen::Clock);
    tap(&mut driver, 300, 300);
    assert!(matches!(driver.stage.page(), Some(Page::Device(_))));
    driver
}

#[test]
fn replay_on_the_device_page_plays_the_identity_and_the_card_then_the_clock() {
    let mut driver = open_device_page();
    scroll_device_to_end(&mut driver);
    tap(&mut driver, 233, 398);
    tap(&mut driver, 233, 258);
    assert!(driver.stage.starting_up());
    assert!(driver.stage.page().is_none());
    assert_eq!(driver.stage.panel_offset(), 0);
    // Seventy-one frames at 30 fps, 300 ms of them waited out after the tap.
    driver.wait(2_000_000);
    assert!(driver.stage.starting_up());
    driver.wait(100_000);
    assert!(!driver.stage.starting_up());
    assert_eq!(driver.stage.screen(), Screen::Clock);
}

#[test]
fn a_replay_after_a_failed_boot_still_shows_the_identity() {
    let mut driver = Driver::starting();
    boot_all(&mut driver, Some(Part::Magnet));
    driver.wait(4_800_000);
    assert!(!driver.stage.starting_up());
    driver.swipe(Point::new(233, 80), Point::new(233, 420), 250_000);
    driver.settle();
    driver.wait(600_000);
    tap(&mut driver, 300, 300);
    scroll_device_to_end(&mut driver);
    tap(&mut driver, 233, 398);
    tap(&mut driver, 233, 258);
    assert!(driver.stage.starting_up());
    driver.wait(1_000_000);
    assert!(driver.stage.starting_up(), "the fault screen would have ended by now");
}

#[test]
fn replay_damage_redraws_what_changed() {
    let mut driver = open_device_page();
    scroll_device_to_end(&mut driver);
    let mut buffers = Buffers::new();
    for _ in 0..2 {
        buffers.draw(&driver.stage, &Dirty::new_full());
    }
    tap(&mut driver, 233, 398);
    driver.stroke(&[Point::new(233, 258)]);
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
    tap(&mut driver, 233, 398);
    assert!(matches!(driver.stage.page(), Some(Page::Replay(_))));
    if steps > 0 {
        driver.swipe(Point::new(233, 330), Point::new(233, 330 - 40 * steps - 10), 300_000);
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
    // The self-test to the last report at 1.3 s, its last glyph and its 300 ms hold, then 4 s.
    driver.wait(5_300_000);
    assert!(driver.stage.starting_up());
    driver.wait(400_000);
    assert!(!driver.stage.starting_up());
    assert_eq!(driver.stage.screen(), Screen::Clock);
}

#[test]
fn a_demonstration_leaves_the_boot_record_for_a_good_replay() {
    let mut driver = Driver::starting();
    boot_all(&mut driver, None);
    driver.wait(2_700_000);
    let replay = |driver: &mut Driver, steps: i32| {
        driver.swipe(Point::new(233, 80), Point::new(233, 420), 250_000);
        driver.settle();
        driver.wait(600_000);
        tap(driver, 300, 300);
        scroll_device_to_end(driver);
        tap(driver, 233, 398);
        if steps > 0 {
            driver.swipe(Point::new(233, 330), Point::new(233, 330 - 40 * steps - 10), 300_000);
        }
        tap(driver, 233, 258);
    };
    replay(&mut driver, 1);
    driver.wait(6_000_000);
    assert!(!driver.stage.starting_up());
    // A good replay goes straight to the identity: no self-test, and no fault screen.
    replay(&mut driver, 0);
    driver.wait(2_200_000);
    assert!(!driver.stage.starting_up());
}

#[test]
fn demonstration_damage_redraws_what_changed() {
    let mut driver = open_replay_chooser(3);
    let mut buffers = Buffers::new();
    for _ in 0..2 {
        buffers.draw(&driver.stage, &Dirty::new_full());
    }
    driver.stroke(&[Point::new(233, 258)]);
    for step in 0..360 {
        let partial = buffers.draw(&driver.stage, driver.stage.changed());
        let whole = render(&driver.stage);
        assert_eq!(differing(partial, &whole), 0, "step {step}");
        assert_eq!(differing(&buffers.panel, &whole), 0, "step {step}, on the panel");
        driver.step(Input::default());
    }
}

use octowhere_ui::ui::rest::{self, Rest, Timeout};

/// Settled on `screen` with the clock running, resting after `timeout`.
fn resting_on(screen: Screen, timeout: Timeout, always_on: bool) -> Driver<'static> {
    let mut driver = Driver::on(screen);
    driver.stage = Stage::new(PeripheralState { firmware: "0.1.0", timeout, always_on, ..PeripheralState::default() });
    driver.stage.show(screen);
    driver.step(sensors(clock_at(12, 7, 42), zone("Europe/Dublin", ZoneMode::Automatic)));
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
    (0..frames_in(duration)).filter_map(|_| driver.step(Input::default()).brightness).collect()
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
    assert!(levels.len() > 5 && levels.is_sorted_by(|a, b| a >= b), "{levels:?}");
    assert_eq!(levels.last(), Some(&rest::dim_level(120)));

    let mut updates = driver.swipe(Point::new(400, 233), Point::new(100, 233), 200_000);
    updates.extend((0..30).map(|_| driver.step(Input::default())));
    let levels: Vec<_> = updates.iter().filter_map(|update| update.brightness).collect();
    assert!(levels.len() > 5 && levels.is_sorted(), "{levels:?}");
    assert_eq!(levels.last(), Some(&120));
    assert_eq!(driver.stage.rest(), Rest::Awake);
    assert!(!driver.stage.is_animating(), "the touch that lifted the dim turned the page");
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
    let darkening: Vec<_> = updates.iter().rev().skip(1).take(10).filter_map(|update| update.brightness).collect();
    assert!(darkening.len() > 5 && darkening.is_sorted(), "the level fades out before the panel goes off: {darkening:?}");
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
    let woken = driver.now() + script::FRAME;
    let mut updates = driver.stroke(&[Point::new(400, 233), Point::new(300, 233), Point::new(200, 233)]);
    assert_eq!(updates[0].display_on, Some(true));
    assert_eq!(driver.stage.rest(), Rest::Awake);
    assert_eq!(driver.stage.screen(), Screen::Compass);
    while driver.now() < woken + rest::PANEL_WAKE - script::FRAME {
        updates.push(driver.step(Input::default()));
        assert_eq!(accents(&driver), Accents::HIDDEN, "the entry started on a dark panel");
    }
    while driver.stage.is_animating() {
        updates.push(driver.step(Input::default()));
    }
    let levels: Vec<_> = updates.iter().filter_map(|update| update.brightness).collect();
    assert!(levels.len() > 3, "{levels:?}");
    assert!(levels.is_sorted() && levels.last() == Some(&120), "{levels:?}");
    assert_eq!(accents(&driver), Accents::FULL);
    assert_eq!(driver.stage.screen(), Screen::Compass, "the waking touch turned the page");
}

#[test]
fn the_always_on_face_shows_after_the_dim_and_redraws_once_a_minute() {
    let mut driver = resting_on(Screen::Compass, Timeout::Seconds15, true);
    let updates = wait_until(&mut driver, 22_000_000, |rest| rest == Rest::AlwaysOn);
    assert_eq!(updates.last().unwrap().brightness, Some(rest::always_on_level(120)));
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

fn frames_in(duration: Micros) -> Micros {
    duration / script::FRAME
}

#[test]
fn a_wake_from_the_always_on_face_climbs_from_its_level() {
    let mut driver = resting_on(Screen::Clock, Timeout::Seconds15, true);
    wait_until(&mut driver, 22_000_000, |rest| rest == Rest::AlwaysOn);
    let mut updates = driver.stroke(&[Point::new(233, 233)]);
    assert_eq!(updates[0].display_on, None);
    while driver.stage.is_animating() {
        updates.push(driver.step(Input::default()));
    }
    let levels: Vec<_> = updates.iter().filter_map(|update| update.brightness).collect();
    assert!(levels[0] > rest::always_on_level(120) && levels.is_sorted(), "{levels:?}");
    assert_eq!(levels.last(), Some(&120));
    assert_eq!(clock_accents(&driver), ClockAccents::FULL);
}

#[test]
fn a_wake_from_the_panel_lands_on_the_clock_and_drops_the_edit() {
    let mut driver = open_panel(Screen::Compass);
    tap(&mut driver, 150, 300);
    driver.swipe(Point::new(150, 280), Point::new(420, 280), 300_000);
    let updates = wait_until(&mut driver, 70_000_000, |rest| rest == Rest::Off);
    assert!(
        updates.iter().any(|update| update.brightness == Some(rest::dim_level(255))),
        "the dim is taken from the level that shows"
    );
    let mut updates = driver.stroke(&[Point::new(233, 233)]);
    while driver.stage.is_animating() {
        updates.push(driver.step(Input::default()));
    }
    assert_eq!(stored(&updates), None);
    assert!(driver.stage.page().is_none());
    assert_eq!(driver.stage.panel_offset(), 0);
    assert_eq!(driver.stage.screen(), Screen::Clock);
    assert_eq!(updates.iter().rev().find_map(|update| update.brightness), Some(120));
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
    let Rest::Dimmed { since } = driver.stage.rest() else { unreachable!() };
    assert!((15_000_000..15_100_000).contains(&(since - turned)), "{}", since - turned);
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
    driver.touch(Some(Point::new(233, 233)));
    check(&driver, "woken");
    for step in 0..40 {
        driver.touch(None);
        check(&driver, &format!("entry {step}"));
    }
}
