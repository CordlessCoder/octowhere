use embedded_graphics::prelude::{Point, Size};
use embedded_graphics::primitives::Rectangle;
use octowhere_ui::{
    chrome::{Window, FB},
    ui::{
        compass::CompassView,
        gesture::{LIFT_SAMPLES, Micros},
        prototypes::{COMPASS_CENTER, HEADER, PeripheralState, Screen},
        stage::{Input, Motion, Stage, Update},
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
        self.step(Input {
            touch: Some([contact, None]),
            ..Input::default()
        })
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
fn a_tap_inside_the_compass_dial_asks_for_recalibration() {
    let mut driver = Driver::on(Screen::Compass);
    let updates = driver.stroke(&[COMPASS_CENTER + Point::new(30, -40)]);
    assert!(updates.iter().any(|update| update.recalibrate));

    let updates = driver.stroke(&[COMPASS_CENTER + Point::new(150, 0)]);
    assert!(updates.iter().all(|update| !update.recalibrate));
    assert_eq!(driver.stage.screen(), Screen::Compass);
}

#[test]
fn motion_redraws_only_the_screens_that_show_it() {
    let mut driver = Driver::on(Screen::Compass);
    assert!(driver.motion(heading(900)).changed.is_full());

    let mut driver = Driver::on(Screen::Clock);
    assert!(!driver.motion(heading(900)).changed.is_full());
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
