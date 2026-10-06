//! The settings changes waiting to be saved, each setting's latest, so that the queue cannot
//! overflow and a clear is never dropped. It has no board dependency, and `host-tests` tests it.

use octowhere_ui::ui::{
    clock::ZoneId,
    rest::{AlwaysOn, Timeout},
};

/// One change to save.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Write {
    /// Where GNSS placed the device.
    AutomaticZone(ZoneId),
    /// A zone chosen by hand, which puts the zone in manual mode.
    ManualZone(ZoneId),
    /// Back to automatic mode.
    Automatic,
    Brightness(u8),
    Timeout(Timeout),
    AlwaysOn(AlwaysOn),
    /// Forget every setting.
    Clear,
}

/// The changes waiting, which [`SettingsQueue::pop`] gives back in the order they are saved.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SettingsQueue {
    /// Every setting is forgotten before the changes below are saved.
    clear: bool,
    automatic_zone: Option<ZoneId>,
    manual_zone: Option<ZoneId>,
    /// Back to automatic mode, after any zone chosen by hand.
    automatic: bool,
    brightness: Option<u8>,
    timeout: Option<Timeout>,
    always_on: Option<AlwaysOn>,
}

impl SettingsQueue {
    pub const fn new() -> Self {
        Self {
            clear: false,
            automatic_zone: None,
            manual_zone: None,
            automatic: false,
            brightness: None,
            timeout: None,
            always_on: None,
        }
    }

    /// Adds `write`, which replaces a change of the same setting waiting. A clear replaces
    /// every change waiting.
    pub fn push(&mut self, write: Write) {
        match write {
            Write::Clear => {
                *self = Self {
                    clear: true,
                    ..Self::new()
                };
            }
            Write::AutomaticZone(zone) => self.automatic_zone = Some(zone),
            Write::ManualZone(zone) => {
                self.manual_zone = Some(zone);
                self.automatic = false;
            }
            Write::Automatic => self.automatic = true,
            Write::Brightness(level) => self.brightness = Some(level),
            Write::Timeout(timeout) => self.timeout = Some(timeout),
            Write::AlwaysOn(choice) => self.always_on = Some(choice),
        }
    }

    /// Takes the next change to save: a clear first, then a zone chosen by hand before the
    /// return to automatic mode that followed it.
    pub fn pop(&mut self) -> Option<Write> {
        if core::mem::take(&mut self.clear) {
            return Some(Write::Clear);
        }
        if let Some(zone) = self.automatic_zone.take() {
            return Some(Write::AutomaticZone(zone));
        }
        if let Some(zone) = self.manual_zone.take() {
            return Some(Write::ManualZone(zone));
        }
        if core::mem::take(&mut self.automatic) {
            return Some(Write::Automatic);
        }
        self.brightness
            .take()
            .map(Write::Brightness)
            .or_else(|| self.timeout.take().map(Write::Timeout))
            .or_else(|| self.always_on.take().map(Write::AlwaysOn))
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        *self == Self::new()
    }
}
