use embedded_graphics::prelude::{Point, Size};
use embedded_graphics::primitives::Rectangle;
use octowhere_ui::{
    chrome::{Clip, Dirty, Window, FB},
    ui::{
        compass::CompassView,
        gesture::{LIFT_SAMPLES, Micros},
        prototypes::{COMPASS_CENTER, HEADER, PeripheralState, Screen},
        compass_screen::Accents,
        stage::{Input, Motion, Stage, Touch, Update},
    },
};

const FRAME: Micros = 16_667;

/// Drives a stage on a clock that advances one frame per step.
struct Driver {
    stage: Stage,
    now: Micros,
}

impl Driver {
    fn new() -> Self {
        Self {
            stage: Stage::new(PeripheralState::default()),
            now: 0,
        }
    }

    fn on(screen: Screen) -> Self {
        let mut driver = Self::new();
        driver.stage.show(screen);
        driver
    }

    fn step(&mut self, input: Input) -> Update {
        self.now += FRAME;
        self.stage.step(Input {
            now: self.now,
            ..input
        })
    }

    fn touch(&mut self, contact: Option<Point>) -> Update {
        self.read(Touch::Contacts([contact, None]))
    }

    fn read(&mut self, touch: Touch) -> Update {
        self.step(Input {
            touch: Some(touch),
            ..Input::default()
        })
    }

    fn cover(&mut self) -> Update {
        self.read(Touch::Cover)
    }

    /// Steps without input for at least `duration`.
    fn wait(&mut self, duration: Micros) -> Update {
        let end = self.now + duration;
        let mut update = self.step(Input::default());
        while self.now < end {
            update = self.step(Input::default());
        }
        update
    }

    /// Steps until the compass page's entry has faded in.
    fn settled_on_compass() -> Self {
        let mut driver = Self::on(Screen::Compass);
        driver.motion(heading(470));
        driver.wait(500_000);
        driver
    }

    fn motion(&mut self, motion: Motion) -> Update {
        self.step(Input {
            motion: Some(motion),
            ..Input::default()
        })
    }

    /// A contact along `path`, then the empty reads that count as a lift. Returns every update.
    fn stroke(&mut self, path: &[Point]) -> Vec<Update> {
        let mut updates: Vec<_> = path.iter().map(|&point| self.touch(Some(point))).collect();
        updates.extend((0..LIFT_SAMPLES).map(|_| self.touch(None)));
        updates
    }

    /// Steps without input until the pager comes to rest.
    fn settle(&mut self) {
        for _ in 0..120 {
            if !self.stage.is_animating() {
                return;
            }
            self.step(Input::default());
        }
        panic!("the pager never came to rest");
    }
}

fn heading(decidegrees: u16) -> Motion {
    Motion {
        imu_valid: true,
        compass: CompassView {
            live: true,
            calibration_percent: 100,
            heading_decidegrees: Some(decidegrees),
            ..CompassView::default()
        },
        ..Motion::default()
    }
}

#[test]
fn a_tap_on_the_header_moves_to_the_next_screen() {
    let mut driver = Driver::new();
    driver.stroke(&[HEADER.center()]);
    assert!(driver.stage.is_animating());
    driver.settle();
    assert_eq!(driver.stage.screen(), Screen::Map.next());
}

#[test]
fn a_swipe_left_moves_to_the_next_screen_and_right_moves_back() {
    let mut driver = Driver::new();
    let left: Vec<_> = (0..8).map(|step| Point::new(400 - step * 40, 233)).collect();
    driver.stroke(&left);
    driver.settle();
    assert_eq!(driver.stage.screen(), Screen::Motion);

    let right: Vec<_> = left.iter().rev().copied().collect();
    driver.stroke(&right);
    driver.settle();
    assert_eq!(driver.stage.screen(), Screen::Map);
}

#[test]
fn a_cover_on_the_settled_compass_asks_for_recalibration_once_per_hand() {
    let mut driver = Driver::settled_on_compass();
    assert!(driver.cover().recalibrate);
    // The dial goes at once, before the motion task confirms the reset.
    assert_eq!(driver.stage.peripherals().compass.heading_decidegrees, None);
    // A held hand keeps reporting, with unreadable reports among them that arrive as no contacts.
    for _ in 0..20 {
        assert!(!driver.cover().recalibrate, "a held cover fired twice");
        assert!(!driver.touch(None).recalibrate);
        driver.wait(150_000);
    }
    // Lifted: no cover report for longer than a held hand leaves between them.
    driver.wait(300_000);
    assert!(driver.cover().recalibrate, "a second hand was ignored");
}

#[test]
fn a_finger_rearms_the_cover_at_once() {
    let mut driver = Driver::settled_on_compass();
    assert!(driver.cover().recalibrate);
    driver.touch(Some(COMPASS_CENTER));
    assert!(driver.cover().recalibrate, "a cover after a touch was ignored");
}

#[test]
fn a_tap_on_the_compass_does_nothing() {
    let mut driver = Driver::settled_on_compass();
    let updates = driver.stroke(&[COMPASS_CENTER + Point::new(30, -40)]);
    assert!(updates.iter().all(|update| !update.recalibrate));
    driver.stroke(&[HEADER.center()]);
    assert!(!driver.stage.is_animating());
    assert_eq!(driver.stage.screen(), Screen::Compass);
}

#[test]
fn a_cover_is_ignored_without_data_or_off_a_settled_compass() {
    let mut driver = Driver::on(Screen::Compass);
    driver.motion(Motion::default());
    driver.wait(500_000);
    assert!(!driver.cover().recalibrate, "NO DATA accepted a cover");

    let mut driver = Driver::settled_on_compass();
    driver.touch(Some(Point::new(400, 233)));
    driver.touch(Some(Point::new(340, 233)));
    assert!(!driver.cover().recalibrate, "a cover mid-drag was accepted");

    let mut driver = Driver::on(Screen::Clock);
    assert!(!driver.cover().recalibrate);
}

fn accents(driver: &Driver) -> Accents {
    let mut fb = FB::boxed();
    driver.stage.draw(&mut *fb);
    driver.stage.accents()
}

#[test]
fn the_compass_accents_fade_in_after_the_page_settles() {
    let mut driver = Driver::on(Screen::Compass);
    driver.motion(heading(470));
    assert_eq!(accents(&driver).ring, 0);
    assert!(driver.stage.is_animating());
    driver.wait(150_000);
    let early = accents(&driver);
    assert_eq!(early.ring, 255);
    assert!(early.icon > 0 && early.icon < 255, "{early:?}");
    assert_eq!(early.ticks, 0);
    driver.wait(300_000);
    assert_eq!(accents(&driver), Accents::FULL);
    assert!(!driver.stage.is_animating());
}

#[test]
fn a_swipe_off_the_compass_fades_its_accents_reversibly() {
    let mut driver = Driver::settled_on_compass();
    driver.touch(Some(Point::new(400, 233)));
    driver.touch(Some(Point::new(360, 233)));
    driver.touch(Some(Point::new(340, 233)));
    let part = accents(&driver);
    assert!(part.ring > 0 && part.ring < 255, "{part:?}");
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
    let mut driver = Driver::on(Screen::Navigation);
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
fn a_heading_after_calibration_reveals_the_ticks_then_the_letters() {
    let mut driver = Driver::on(Screen::Compass);
    let mut calibrating = heading(470);
    calibrating.compass.heading_decidegrees = None;
    calibrating.compass.calibration_percent = 99;
    driver.motion(calibrating);
    driver.wait(500_000);
    driver.motion(heading(470));
    driver.wait(60_000);
    let early = accents(&driver);
    assert!(early.ticks > 0 && early.ticks < 255, "{early:?}");
    assert_eq!(early.letters, 0);
    driver.wait(200_000);
    assert_eq!(accents(&driver), Accents::FULL);
}

#[test]
fn motion_redraws_only_the_screens_that_show_it() {
    let mut driver = Driver::on(Screen::Compass);
    driver.motion(heading(900));
    assert!(driver.stage.changed().is_full());

    let mut driver = Driver::on(Screen::Clock);
    driver.motion(heading(900));
    assert!(!driver.stage.changed().is_full());
}

#[test]
fn the_compass_asks_for_fast_samples_while_it_shows_or_slides_in() {
    let mut driver = Driver::on(Screen::Compass);
    assert!(driver.step(Input::default()).samples_fast);

    let mut driver = Driver::on(Screen::Navigation);
    assert!(!driver.step(Input::default()).samples_fast);
    // Dragging left uncovers the compass, which follows navigation.
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
        driver.motion(Motion {
            compass,
            ..Motion::default()
        });
        driver.wait(500_000);
        stages.push((format!("compass {name}"), driver.stage));
    }
    stages
}

/// A redraw clipped to regions must leave the same pixels as a full one, or partial flushing
/// shows seams.
#[test]
fn drawing_in_tiles_matches_drawing_whole() {
    const TILE: u32 = 70;
    for (name, stage) in stages() {
        let whole = render(&stage);
        let mut tiled = FB::boxed();
        for y in (0..466).step_by(TILE as usize) {
            for x in (0..466).step_by(TILE as usize) {
                let area = Rectangle::new(Point::new(x, y), Size::new_equal(TILE));
                stage.draw(&mut Window::new(&mut *tiled, Point::zero(), area));
            }
        }
        let differing = (0..466 * 466)
            .map(|index| Point::new(index % 466, index / 466))
            .filter(|&point| whole.pixel(point) != tiled.pixel(point))
            .count();
        assert_eq!(differing, 0, "{name}: {differing} pixels differ");
    }
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
    let mut driver = Driver::settled_on_compass();
    driver.motion(absence);
    driver.wait(gap);
    driver.motion(heading(470));
    accents(&driver)
}

#[test]
fn a_heading_back_quickly_from_top_edge_up_skips_the_reveal() {
    assert_eq!(heading_back_after(top_edge_up(), 300_000), Accents::FULL);
}

#[test]
fn a_heading_back_slowly_from_top_edge_up_reveals_again() {
    let accents = heading_back_after(top_edge_up(), 1_000_000);
    assert!(accents.ticks < 255 && accents.letters == 0, "{accents:?}");
}

#[test]
fn a_heading_back_from_no_data_reveals_again() {
    let accents = heading_back_after(Motion::default(), 300_000);
    assert!(accents.ticks < 255 && accents.letters == 0, "{accents:?}");
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

fn differing(a: &FB, b: &FB) -> usize {
    (0..466 * 466)
        .map(|index| Point::new(index % 466, index / 466))
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
        motion.compass.roll_deg = (step as i8 * 23).wrapping_sub(90);
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
    let mut driver = Driver::settled_on_compass();
    let mut buffers = Buffers::new();
    for _ in 0..2 {
        buffers.draw(&driver.stage, &Dirty::new_full());
    }
    for (name, motion) in compass_walk() {
        // Each reading, then a few quiet steps for any fade it starts.
        driver.motion(motion);
        for frame in 0..6 {
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
    let mut driver = Driver::settled_on_compass();
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
