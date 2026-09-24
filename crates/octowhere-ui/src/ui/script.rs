//! Drives a [`Stage`] on a simulated clock, one frame per step, the way the firmware's frame
//! loop feeds it. Tests assert against it, and `tools/ui-sim` plays and records scenes written
//! on it, so a scene comes out the same every run.

use alloc::{boxed::Box, vec::Vec};

use embedded_graphics::prelude::Point;

use super::{
    gesture::{LIFT_SAMPLES, Micros},
    screens::{PeripheralState, Screen},
    stage::{Input, Motion, Sensors, Stage, Touch, Update},
};

/// The time between steps.
pub const FRAME: Micros = 16_667;
/// How often the sensor task publishes, and so how often a running clock ticks.
const SENSOR_PERIOD: Micros = 1_000_000;

type Observer<'a> = Box<dyn FnMut(&Stage, Micros) + 'a>;

pub struct Driver<'a> {
    pub stage: Stage,
    now: Micros,
    /// The last sensor readings stepped in, and when.
    sensors: Option<(Sensors, Micros)>,
    clock_runs: bool,
    observer: Option<Observer<'a>>,
}

impl<'a> Driver<'a> {
    pub fn new() -> Self {
        Self {
            stage: Stage::new(PeripheralState::default()),
            now: 0,
            sensors: None,
            clock_runs: false,
            observer: None,
        }
    }

    pub fn on(screen: Screen) -> Self {
        let mut driver = Self::new();
        driver.stage.show(screen);
        driver
    }

    /// Calls `observer` after every step, with the stage and the step's time.
    pub fn observe(&mut self, observer: impl FnMut(&Stage, Micros) + 'a) {
        self.observer = Some(Box::new(observer));
    }

    /// From now on, steps the last sensor readings in again each second with the clock
    /// advanced, as the sensor task publishes them. A stopped clock stays where it is.
    pub fn run_clock(&mut self) {
        self.clock_runs = true;
    }

    pub fn now(&self) -> Micros {
        self.now
    }

    /// Advances the clock a frame and steps `input` in at the new time.
    pub fn step(&mut self, input: Input) -> Update {
        self.now += FRAME;
        let mut input = Input { now: self.now, ..input };
        if let Some(sensors) = input.sensors {
            self.sensors = Some((sensors, self.now));
        } else if let Some((sensors, since)) = self.sensors {
            let elapsed = self.now - since;
            let due = self.clock_runs && elapsed / SENSOR_PERIOD != (elapsed - FRAME) / SENSOR_PERIOD;
            if due && !sensors.clock.stopped {
                let mut advanced = sensors;
                advanced.clock.utc = sensors.clock.utc.map(|utc| utc + (elapsed / SENSOR_PERIOD) as i64);
                input.sensors = Some(advanced);
            }
        }
        let update = self.stage.step(input);
        if let Some(observer) = &mut self.observer {
            observer(&self.stage, self.now);
        }
        update
    }

    pub fn touch(&mut self, contact: Option<Point>) -> Update {
        self.read(Touch::Contacts([contact, None]))
    }

    pub fn read(&mut self, touch: Touch) -> Update {
        self.step(Input { touch: Some(touch), ..Input::default() })
    }

    pub fn cover(&mut self) -> Update {
        self.read(Touch::Cover)
    }

    pub fn motion(&mut self, motion: Motion) -> Update {
        self.step(Input { motion: Some(motion), ..Input::default() })
    }

    pub fn sensors(&mut self, sensors: Sensors) -> Update {
        self.step(Input { sensors: Some(sensors), ..Input::default() })
    }

    /// Steps without input for at least `duration`.
    pub fn wait(&mut self, duration: Micros) -> Update {
        let end = self.now + duration;
        let mut update = self.step(Input::default());
        while self.now < end {
            update = self.step(Input::default());
        }
        update
    }

    /// A contact along `path`, a point a step, then the empty reads that count as a lift.
    /// Returns every update.
    pub fn stroke(&mut self, path: &[Point]) -> Vec<Update> {
        let mut updates: Vec<_> = path.iter().map(|&point| self.touch(Some(point))).collect();
        updates.extend((0..LIFT_SAMPLES).map(|_| self.touch(None)));
        updates
    }

    /// A stroke in a straight line from `from` to `to`, taking `duration` before the lift.
    pub fn swipe(&mut self, from: Point, to: Point, duration: Micros) -> Vec<Update> {
        let steps = frames(duration) as i32;
        let path: Vec<_> = (0..=steps).map(|step| from + (to - from) * step / steps).collect();
        self.stroke(&path)
    }

    /// Steps a reading in every step for `duration`, from `motion` given the fraction of the
    /// way through, which reaches 1 on the last step.
    pub fn motion_over(&mut self, duration: Micros, mut motion: impl FnMut(f32) -> Motion) {
        let steps = frames(duration);
        for step in 1..=steps {
            self.motion(motion(step as f32 / steps as f32));
        }
    }

    /// Steps without input until the pager comes to rest.
    ///
    /// # Panics
    ///
    /// If it has not come to rest after two seconds.
    pub fn settle(&mut self) {
        for _ in 0..120 {
            if !self.stage.is_animating() {
                return;
            }
            self.step(Input::default());
        }
        panic!("the pager never came to rest");
    }
}

/// The whole steps nearest `duration`, at least one.
fn frames(duration: Micros) -> Micros {
    ((duration + FRAME / 2) / FRAME).max(1)
}

impl Default for Driver<'_> {
    fn default() -> Self {
        Self::new()
    }
}
