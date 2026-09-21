#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ButtonEvent {
    None,
    Pressed,
    Released,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Button {
    stable_pressed: bool,
    active_samples: u8,
    inactive_samples: u8,
    contact_active: bool,
    contact_inactive_samples: u8,
    captured: bool,
}

impl Button {
    const DEBOUNCE_SAMPLES: u8 = 3;

    pub fn update(&mut self, active: bool) -> ButtonEvent {
        if active {
            self.inactive_samples = 0;
            if self.stable_pressed {
                return ButtonEvent::None;
            }
            self.active_samples = self.active_samples.saturating_add(1);
            if self.active_samples >= Self::DEBOUNCE_SAMPLES {
                self.active_samples = 0;
                self.stable_pressed = true;
                return ButtonEvent::Pressed;
            }
        } else {
            self.active_samples = 0;
            if !self.stable_pressed {
                return ButtonEvent::None;
            }
            self.inactive_samples = self.inactive_samples.saturating_add(1);
            if self.inactive_samples >= Self::DEBOUNCE_SAMPLES {
                self.inactive_samples = 0;
                self.stable_pressed = false;
                return ButtonEvent::Released;
            }
        }
        ButtonEvent::None
    }

    pub fn update_touch(&mut self, contact_active: bool, hit: bool) -> ButtonEvent {
        if contact_active {
            if !self.contact_active {
                self.captured = hit;
                self.contact_active = true;
            }
            self.contact_inactive_samples = 0;
        } else if self.contact_active {
            self.contact_inactive_samples = self.contact_inactive_samples.saturating_add(1);
            if self.contact_inactive_samples >= Self::DEBOUNCE_SAMPLES {
                self.contact_active = false;
                self.contact_inactive_samples = 0;
                self.captured = false;
            }
        }
        self.update(contact_active && self.captured)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TouchState {
    active: bool,
    candidate_active: bool,
    active_samples: u8,
    points: u8,
    candidate_points: u8,
    point_samples: u8,
    position: Option<Point>,
    positions: [Option<Point>; 2],
}

impl TouchState {
    const DEBOUNCE_SAMPLES: u8 = 3;

    pub fn update(&mut self, points: u8, position: Option<Point>) -> (u8, Option<Point>) {
        let mut positions = [None; 2];
        positions[0] = position;
        let (points, positions) = self.update_positions_with_count(points, positions);
        (points, positions[0])
    }

    pub fn update_positions(&mut self, positions: [Option<Point>; 2]) -> (u8, [Option<Point>; 2]) {
        let points = positions.iter().flatten().count() as u8;
        self.update_positions_with_count(points, positions)
    }

    fn update_positions_with_count(
        &mut self,
        points: u8,
        positions: [Option<Point>; 2],
    ) -> (u8, [Option<Point>; 2]) {
        let raw_active = points != 0;
        if raw_active == self.active {
            self.active_samples = 0;
        } else {
            if self.candidate_active != raw_active {
                self.candidate_active = raw_active;
                self.active_samples = 0;
            }
            self.active_samples = self.active_samples.saturating_add(1);
            if self.active_samples >= Self::DEBOUNCE_SAMPLES {
                self.active = raw_active;
                self.active_samples = 0;
                if !self.active {
                    self.points = 0;
                    self.position = None;
                    self.positions = [None; 2];
                } else {
                    self.points = points;
                    self.candidate_points = points;
                    self.position = positions[0];
                    self.positions = positions;
                }
            }
        }

        if self.active {
            self.position = positions[0].or(self.position);
            if points == self.points {
                self.point_samples = 0;
                if points != 0 {
                    self.positions = positions;
                    self.position = positions[0].or(self.position);
                }
            } else {
                if self.candidate_points != points {
                    self.candidate_points = points;
                    self.point_samples = 0;
                }
                self.point_samples = self.point_samples.saturating_add(1);
                if self.point_samples >= Self::DEBOUNCE_SAMPLES {
                    self.points = points;
                    self.point_samples = 0;
                    self.positions = positions;
                    self.position = positions[0].or(self.position);
                }
            }
        }

        (self.points, self.positions)
    }
}

#[cfg(test)]
mod tests {
    use embedded_graphics_core::geometry::Point;

    use super::{Button, ButtonEvent, TouchState};

    #[test]
    fn press_is_reported_once() {
        let mut button = Button::default();
        assert_eq!(button.update(true), ButtonEvent::None);
        assert_eq!(button.update(true), ButtonEvent::None);
        assert_eq!(button.update(true), ButtonEvent::Pressed);
        assert_eq!(button.update(true), ButtonEvent::None);
        assert_eq!(button.update(true), ButtonEvent::None);
    }

    #[test]
    fn short_gaps_do_not_rearm_a_held_button() {
        let mut button = Button::default();
        assert_eq!(button.update(true), ButtonEvent::None);
        assert_eq!(button.update(true), ButtonEvent::None);
        assert_eq!(button.update(true), ButtonEvent::Pressed);
        assert_eq!(button.update(false), ButtonEvent::None);
        assert_eq!(button.update(true), ButtonEvent::None);
        assert_eq!(button.update(false), ButtonEvent::None);
        assert_eq!(button.update(false), ButtonEvent::None);
        assert_eq!(button.update(false), ButtonEvent::Released);
    }

    #[test]
    fn dragging_into_a_button_does_not_press_it() {
        let mut button = Button::default();
        assert_eq!(button.update_touch(true, false), ButtonEvent::None);
        assert_eq!(button.update_touch(true, false), ButtonEvent::None);
        assert_eq!(button.update_touch(true, true), ButtonEvent::None);
        assert_eq!(button.update_touch(true, true), ButtonEvent::None);
        assert_eq!(button.update_touch(false, false), ButtonEvent::None);
        assert_eq!(button.update_touch(false, false), ButtonEvent::None);
        assert_eq!(button.update_touch(false, false), ButtonEvent::None);
    }

    #[test]
    fn contact_gaps_do_not_rearm_a_drag() {
        let mut button = Button::default();
        assert_eq!(button.update_touch(true, false), ButtonEvent::None);
        assert_eq!(button.update_touch(false, false), ButtonEvent::None);
        assert_eq!(button.update_touch(true, true), ButtonEvent::None);
        assert_eq!(button.update_touch(false, false), ButtonEvent::None);
        assert_eq!(button.update_touch(true, true), ButtonEvent::None);
    }

    #[test]
    fn touch_state_ignores_short_contact_gaps() {
        let mut state = TouchState::default();
        let point = Some(Point::new(10, 20));
        assert_eq!(state.update(1, point), (0, None));
        assert_eq!(state.update(1, point), (0, None));
        assert_eq!(state.update(1, point), (1, point));
        assert_eq!(state.update(1, point), (1, point));
        assert_eq!(state.update(0, None), (1, point));
        assert_eq!(state.update(0, None), (1, point));
        assert_eq!(state.update(1, point), (1, point));
    }

    #[test]
    fn touch_state_debounces_point_count() {
        let mut state = TouchState::default();
        let point = Some(Point::new(10, 20));
        for _ in 0..3 {
            state.update(1, point);
        }
        assert_eq!(state.update(1, point), (1, point));
        assert_eq!(state.update(2, point), (1, point));
        assert_eq!(state.update(2, point), (1, point));
        assert_eq!(state.update(2, point), (2, point));
    }

    #[test]
    fn touch_state_tracks_two_positions() {
        let mut state = TouchState::default();
        let positions = [Some(Point::new(10, 20)), Some(Point::new(30, 40))];
        assert_eq!(state.update_positions(positions), (0, [None, None]));
        assert_eq!(state.update_positions(positions), (0, [None, None]));
        assert_eq!(state.update_positions(positions), (2, positions));
        assert_eq!(state.update_positions(positions), (2, positions));
    }
}
use embedded_graphics_core::geometry::Point;
