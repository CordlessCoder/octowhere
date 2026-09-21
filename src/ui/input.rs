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
        if contact_active && !self.contact_active {
            self.captured = hit;
        } else if !contact_active {
            self.captured = false;
        }
        self.contact_active = contact_active;
        self.update(contact_active && self.captured)
    }
}

#[cfg(test)]
mod tests {
    use super::{Button, ButtonEvent};

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
}
