//! The simulated air several devices share: `octowhere-sim` runs each one's mesh node, on
//! virtual time that follows the host's clock at a speed the keyboard sets.

use octowhere_mesh::packet::Quality;
use octowhere_node::Fix;
use octowhere_sim::{Config, Link, Sim, alone, grouped};

use crate::device::{DUBLIN, Readings};

/// The speeds `[` and `]` step through: virtual seconds a host second.
pub const SPEEDS: [u32; 7] = [1, 2, 5, 10, 30, 60, 120];

/// The most log lines kept, which a long run would otherwise grow without end.
const LINES_KEPT: usize = 10_000;

/// How a link a click turns lossy loses packets.
const LOSSY: f64 = 0.5;

pub struct Air {
    pub sim: Sim,
    /// Which of [`SPEEDS`] virtual time runs at.
    pub speed: usize,
    /// The host's clock when the air last ran, in µs.
    last_wall: Option<u64>,
    /// The log lines already printed, by number.
    printed: usize,
    /// Whether every line goes to the terminal, or only warnings.
    every_line: bool,
}

/// Where device `n` stands: the first in Dublin and the others spread around it, each further
/// out, so that their bearings and distances differ.
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

impl Air {
    /// `count` devices in reach of each other, in one group when `grouped`, else each alone,
    /// with UTC starting at `utc_s`.
    pub fn new(count: usize, in_group: bool, utc_s: i64, every_line: bool) -> Self {
        let mut sim = Sim::starting_at(1, utc_s);
        sim.keep_lines(LINES_KEPT);
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
            last_wall: None,
            printed: 0,
            every_line,
        }
    }

    /// Runs virtual time on to the host's clock `wall`, in µs, at the speed set, and prints
    /// what the nodes logged meanwhile.
    pub fn advance(&mut self, wall: u64) {
        let last = self.last_wall.replace(wall).unwrap_or(wall);
        let at = self.sim.now_us() + (wall - last) * u64::from(SPEEDS[self.speed]);
        self.sim.run_to(at);
        let (lines, next) = self.sim.lines_since(self.printed);
        self.printed = next;
        for line in lines
            .iter()
            .filter(|line| self.every_line || line.level <= log::Level::Warn)
        {
            println!(
                "[{}] {:>10.3} {:<5} {}",
                line.node + 1,
                line.at as f64 / 1e6,
                line.level,
                line.text
            );
        }
    }

    pub fn faster(&mut self, faster: bool) {
        self.speed = if faster {
            (self.speed + 1).min(SPEEDS.len() - 1)
        } else {
            self.speed.saturating_sub(1)
        };
    }

    /// Steps the link from `from` to `to` through in reach, lossy and out of reach.
    pub fn cycle_link(&self, from: usize, to: usize) {
        let next = match self.sim.link_of(from, to) {
            None => Some(Link::default()),
            Some(link) if link.loss == 0.0 => Some(Link {
                loss: LOSSY,
                ..link
            }),
            Some(_) => None,
        };
        self.sim.link(from, to, next);
    }

    /// Gives node `n` what its device's readings say of its fix and clocks.
    pub fn sense(&self, n: usize, readings: &Readings) {
        let utc_s = self.sim.utc_us().div_euclid(1_000_000);
        let fix = readings.fix.then_some(Fix {
            latitude: readings.position.0,
            longitude: readings.position.1,
            stamp: utc_s as u32,
            quality: Quality::Autonomous,
            hdop_milli: Some(900),
        });
        self.sim.set_fix(n, fix, readings.fix);
        self.sim.set_rtc(n, readings.rtc_readable().then_some(0));
    }
}
