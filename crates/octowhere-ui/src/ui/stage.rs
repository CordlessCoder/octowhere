//! The UI between input and drawing: which screen shows, what touch does to it, and when it has
//! to be redrawn. The frame loop feeds it readings and draws it, and acts on the effects it
//! returns. It never touches a device, so the host drives it the same way.

use embedded_graphics::prelude::Point;

use super::{
    axis_check::{self, AxisCheck},
    compass::CompassView,
    gesture::{GestureEvent, GestureTracker, Micros},
    input::TouchState,
    pager::Pager,
    prototypes::{self, ClockState, PeripheralState, Screen},
};
use crate::{
    board,
    chrome::{self, Color, CoverageTarget, Dirty, FontdueRenderer, FontdueRendererCtx},
};

/// What the motion task publishes each sample.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Motion {
    pub accel_micro_ms2: [i32; 3],
    pub gyro_micro_rad_s: [i32; 3],
    pub imu_valid: bool,
    pub magnetic_microtesla: [i32; 3],
    pub compass: CompassView,
}

/// The parts of the sensor task's snapshot the screens show.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Sensors {
    pub battery_mv: Option<u16>,
    pub vbus_mv: Option<u16>,
    pub vsys_mv: Option<u16>,
    pub gnss_bytes: u16,
    pub gnss_fix: bool,
    pub lora_irq: u8,
    pub clock: ClockState,
}

/// Everything that arrived since the last step.
#[derive(Clone, Copy, Debug, Default)]
pub struct Input {
    pub now: Micros,
    /// A fresh touch reading, up to two contacts in panel coordinates. `None` when the touch
    /// controller was not read this step; the last reading then stands.
    pub touch: Option<[Option<Point>; 2]>,
    pub motion: Option<Motion>,
    pub sensors: Option<Sensors>,
}

/// What the frame loop owes after a step.
#[derive(Debug)]
pub struct Update {
    /// The regions whose pixels changed.
    pub changed: Dirty,
    /// A tap on the compass dial asked for the calibration to restart.
    pub recalibrate: bool,
    /// The axis check finished capturing a pose.
    pub pose: Option<axis_check::Record>,
    /// A screen that needs fast motion samples shows, or is sliding in.
    pub samples_fast: bool,
}

pub struct Stage {
    screen: Screen,
    selected_node: Option<u8>,
    raw_touch: [Option<Point>; 2],
    gesture: GestureTracker,
    pager: Pager,
    axis_check: AxisCheck,
    touch_state: TouchState,
    peripherals: PeripheralState,
    renderer: FontdueRenderer<'static, Color>,
}

impl Stage {
    #[must_use]
    pub fn new(peripherals: PeripheralState) -> Self {
        Self {
            screen: Screen::Map,
            selected_node: None,
            raw_touch: [None; 2],
            gesture: GestureTracker::default(),
            pager: Pager::new(0, Screen::ALL.len(), board::LCD_WIDTH as i32),
            axis_check: AxisCheck::default(),
            touch_state: TouchState::default(),
            peripherals,
            renderer: FontdueRenderer::new(
                FontdueRendererCtx::new_rc(),
                20,
                chrome::WHITE,
                chrome::BLACK,
                chrome::FONTS,
            ),
        }
    }

    /// Jumps to `screen` as though the pager had come to rest on it.
    pub fn show(&mut self, screen: Screen) {
        let page = Screen::ALL
            .iter()
            .position(|&each| each == screen)
            .expect("every screen is in the ring");
        self.pager = Pager::new(page, Screen::ALL.len(), board::LCD_WIDTH as i32);
        self.screen = screen;
        self.selected_node = None;
    }

    #[must_use]
    pub fn screen(&self) -> Screen {
        self.screen
    }

    #[must_use]
    pub fn peripherals(&self) -> &PeripheralState {
        &self.peripherals
    }

    /// A finger is down, or the gesture tracker has not yet seen it lift. Touch should be read
    /// again soon even without an interrupt.
    #[must_use]
    pub fn in_contact(&self) -> bool {
        self.peripherals.touch_position.is_some() || self.gesture.in_contact()
    }

    /// A page slide is under way, so the next step should come without waiting for input.
    #[must_use]
    pub fn is_animating(&self) -> bool {
        self.pager.is_moving()
    }

    pub fn step(&mut self, input: Input) -> Update {
        let Input {
            now,
            touch,
            motion,
            sensors,
        } = input;
        let mut update = Update {
            changed: Dirty::new(),
            recalibrate: false,
            pose: None,
            samples_fast: false,
        };
        let changed = &mut update.changed;
        let previous_touch = (
            self.peripherals.touch_points,
            self.peripherals.touch_position,
            self.peripherals.touch_positions,
        );

        if let Some(motion) = motion {
            let peripherals = &mut self.peripherals;
            peripherals.accel_micro_ms2 = motion.accel_micro_ms2;
            peripherals.gyro_micro_rad_s = motion.gyro_micro_rad_s;
            peripherals.imu_valid = motion.imu_valid;
            peripherals.magnetic_microtesla = motion.magnetic_microtesla;
            peripherals.compass = motion.compass;
            if matches!(
                self.screen,
                Screen::Motion | Screen::Compass | Screen::AxisCheck
            ) {
                changed.make_full();
            }
        }
        if let Some(sensors) = sensors {
            let peripherals = &mut self.peripherals;
            peripherals.battery_mv = sensors.battery_mv;
            peripherals.vbus_mv = sensors.vbus_mv;
            peripherals.vsys_mv = sensors.vsys_mv;
            peripherals.gnss_bytes = sensors.gnss_bytes;
            peripherals.gnss_valid = sensors.gnss_fix;
            peripherals.lora_irq = sensors.lora_irq;
            peripherals.clock = sensors.clock;
            changed.make_full();
        }

        if let Some(raw) = touch {
            self.raw_touch = raw;
        }
        let (touch_points, touch_positions) = self.touch_state.update_positions(self.raw_touch);
        self.peripherals.touch_points = touch_points;
        self.peripherals.touch_position = touch_positions[0];
        self.peripherals.touch_positions = touch_positions;

        let previous_view = self.pager.view();
        let previous_selected_node = self.selected_node;
        let event = if touch.is_some() {
            self.gesture.update(self.raw_touch[0], now)
        } else {
            GestureEvent::None
        };
        self.pager.handle(&event, now);
        if self.screen == Screen::AxisCheck {
            if let GestureEvent::DragEnd(drag) = event {
                let offset = drag.offset();
                if offset.y.abs() > 60 && offset.y.abs() > offset.x.abs() {
                    self.axis_check.step(offset.y < 0);
                }
            }
            if let Some(motion) = motion {
                update.pose = self.axis_check.sample(
                    motion.magnetic_microtesla.map(|value| value as f32 / 1e3),
                    motion.accel_micro_ms2.map(|value| value as f32 / 1e6),
                    now,
                );
            }
        }
        if let GestureEvent::Tap(point) = event {
            if self.screen == Screen::AxisCheck {
                self.axis_check.start(now);
            } else if self.screen == Screen::Compass {
                let offset = point - prototypes::COMPASS_CENTER;
                if offset.x * offset.x + offset.y * offset.y <= 100 * 100 {
                    update.recalibrate = true;
                }
            } else if prototypes::HEADER.contains(point) {
                self.pager.advance(true, now);
            } else if self.screen == Screen::Map
                && let Some(node) = selected_node(point)
            {
                self.selected_node = Some(node);
            }
        }
        self.pager.step(now);
        let view = self.pager.view();
        let screen = Screen::ALL[view.page];
        if screen != self.screen {
            self.screen = screen;
            self.selected_node = None;
        }
        let samples_fast = |screen: Screen| matches!(screen, Screen::Compass | Screen::AxisCheck);
        update.samples_fast = samples_fast(screen)
            || view
                .neighbour
                .is_some_and(|(page, _)| samples_fast(Screen::ALL[page]));

        let axis_check = self.axis_check.view();
        if axis_check != self.peripherals.axis_check {
            self.peripherals.axis_check = axis_check;
            changed.make_full();
        }
        if view != previous_view || self.selected_node != previous_selected_node {
            changed.make_full();
        }
        if self.screen == Screen::Touch
            && (touch_points, touch_positions[0], touch_positions) != previous_touch
        {
            changed.make_full();
        }
        update
    }

    /// Draws the current state into `target` and returns the regions it drew.
    pub fn draw<D>(&self, target: &mut D) -> Dirty
    where
        D: CoverageTarget<Color = Color>,
        D::Error: core::fmt::Debug,
    {
        let view = self.pager.view();
        prototypes::render(
            prototypes::ACTIVE_ARCHITECTURE,
            prototypes::State {
                screen: self.screen,
                selected_node: self.selected_node,
                peripherals: self.peripherals,
                offset: view.offset,
                neighbour: view
                    .neighbour
                    .map(|(page, offset)| (Screen::ALL[page], offset)),
            },
            &self.renderer,
            target,
        )
        .expect("prototype renderer failed")
    }
}

fn selected_node(point: Point) -> Option<u8> {
    let (x, y) = (point.x, point.y);
    if (100..=164).contains(&x) && (146..=210).contains(&y) {
        Some(1)
    } else if (262..=326).contains(&x) && (206..=270).contains(&y) {
        Some(2)
    } else if (322..=386).contains(&x) && (284..=348).contains(&y) {
        Some(3)
    } else {
        None
    }
}
