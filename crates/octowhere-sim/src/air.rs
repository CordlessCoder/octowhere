//! The air as a simulator's window runs it beside its devices: virtual time paced by the host's
//! clock at a speed, devices spread around Dublin, and each link in reach, lossy or out of
//! reach. `tools/ui-sim` and `tools/ui-web` share it.

use octowhere_mesh::packet::Quality;
use octowhere_node::Fix;

use crate::{Config, Link, Sim, alone, grouped};

/// The speeds a window offers: virtual seconds a second of the host's clock.
pub const SPEEDS: [u32; 7] = [1, 2, 5, 10, 30, 60, 120];

/// While a finger or a key is down, and this long after, the air keeps the host's pace. A
/// device's stage runs on its node's clock, so a press or a double tap would otherwise last the
/// speed's times longer there, and a short press or a double tap could not be made.
pub const INPUT_GRACE_US: u64 = 600_000;

/// How a lossy link loses packets.
const LOSSY: f64 = 0.5;

/// Dublin, where the first device stands, in degrees × 10⁷.
pub const DUBLIN: (i32, i32) = (533_498_000, -62_603_000);

/// How a packet from one device reaches another.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Reach {
    None,
    InReach,
    Lossy,
}

impl Reach {
    /// The next a click steps to: in reach, lossy, out of reach, and round again.
    #[must_use]
    pub fn next(self) -> Self {
        match self {
            Self::InReach => Self::Lossy,
            Self::Lossy => Self::None,
            Self::None => Self::InReach,
        }
    }
}

/// Where device `n` stands: the first in Dublin and the others spread around it, each further
/// out, so that their bearings and distances differ.
#[must_use]
pub fn position(n: usize) -> (i32, i32) {
    if n == 0 {
        return DUBLIN;
    }
    let metres = 300.0 * n as f64;
    let bearing = (137.5 * n as f64).to_radians();
    let latitude = f64::from(DUBLIN.0) / 1e7;
    let north = metres * bearing.cos() / 111_320.0;
    let east = metres * bearing.sin() / (111_320.0 * latitude.to_radians().cos());
    (
        DUBLIN.0 + (north * 1e7) as i32,
        DUBLIN.1 + (east * 1e7) as i32,
    )
}

pub struct Paced {
    pub sim: Sim,
    /// Which of [`SPEEDS`] virtual time runs at.
    pub speed: usize,
    /// The host's clock when the air last ran, in µs.
    last: Option<u64>,
    /// Until when the air keeps the host's pace for input.
    input_until: u64,
}

impl Paced {
    /// `count` devices in reach of each other, in one group when `in_group`, else each alone,
    /// with UTC starting at `utc_s`, keeping the latest `lines` log lines.
    #[must_use]
    pub fn new(count: usize, in_group: bool, utc_s: i64, lines: usize) -> Self {
        let mut sim = Sim::starting_at(1, utc_s);
        sim.keep_lines(lines);
        let starts = if in_group {
            grouped(count as u8, utc_s as u32 - 3_600)
        } else {
            (0..count as u8).map(alone).collect()
        };
        for start in starts {
            sim.add(start, Config::default());
        }
        sim.link_all(Link::default());
        Self {
            sim,
            speed: 0,
            last: None,
            input_until: 0,
        }
    }

    /// Runs virtual time on to the host's clock `host_us`, at the speed set, or at the host's
    /// pace while `input` holds and for [`INPUT_GRACE_US`] after.
    pub fn advance(&mut self, host_us: u64, input: bool) {
        if input {
            self.input_until = host_us + INPUT_GRACE_US;
        }
        let last = self.last.replace(host_us).unwrap_or(host_us);
        let speed = if host_us < self.input_until {
            1
        } else {
            SPEEDS[self.speed]
        };
        let elapsed = host_us.saturating_sub(last) * u64::from(speed);
        self.sim.run_to(self.sim.now_us() + elapsed);
    }

    pub fn faster(&mut self, faster: bool) {
        self.speed = if faster {
            (self.speed + 1).min(SPEEDS.len() - 1)
        } else {
            self.speed.saturating_sub(1)
        };
    }

    #[must_use]
    pub fn reach(&self, from: usize, to: usize) -> Reach {
        match self.sim.link_of(from, to) {
            None => Reach::None,
            Some(link) if link.loss == 0.0 => Reach::InReach,
            Some(_) => Reach::Lossy,
        }
    }

    pub fn set_reach(&self, from: usize, to: usize, reach: Reach) {
        let link = match reach {
            Reach::None => None,
            Reach::InReach => Some(Link::default()),
            Reach::Lossy => Some(Link {
                loss: LOSSY,
                ..Link::default()
            }),
        };
        self.sim.link(from, to, link);
    }

    /// UTC now, in whole seconds.
    #[must_use]
    pub fn utc_s(&self) -> i64 {
        self.sim.utc_us().div_euclid(1_000_000)
    }

    /// Gives node `n` a fix at `position` from GNSS, or none, and RTC time if its RTC reads.
    pub fn sense(&self, n: usize, position: Option<(i32, i32)>, rtc: bool) {
        let fix = position.map(|(latitude, longitude)| Fix {
            latitude,
            longitude,
            stamp: self.utc_s() as u32,
            quality: Quality::Autonomous,
            hdop_milli: Some(900),
        });
        self.sim.set_fix(n, fix, fix.is_some());
        self.sim.set_rtc(n, rtc.then_some(0));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_holds_the_air_to_the_host_pace() {
        let mut air = Paced::new(2, true, crate::UTC0_S, 100);
        air.speed = SPEEDS.len() - 1;
        air.advance(0, false);
        air.advance(1_000_000, false);
        assert_eq!(air.sim.now_us(), 120_000_000);
        air.advance(1_100_000, true);
        air.advance(1_600_000, false);
        assert_eq!(air.sim.now_us(), 120_600_000);
        air.advance(2_200_000, false);
        assert_eq!(air.sim.now_us(), 120_600_000 + 120 * 600_000);
    }

    #[test]
    fn a_click_steps_a_link_round() {
        let air = Paced::new(2, true, crate::UTC0_S, 100);
        let mut reach = air.reach(0, 1);
        assert_eq!(reach, Reach::InReach);
        for expected in [Reach::Lossy, Reach::None, Reach::InReach] {
            reach = reach.next();
            air.set_reach(0, 1, reach);
            assert_eq!(air.reach(0, 1), expected);
        }
    }
}
