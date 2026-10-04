//! The simulated air several devices share, `octowhere-sim`'s paced air, and the nodes' log
//! lines it prints to the terminal.

use octowhere_sim::air::Paced;

use crate::device::Readings;

/// The most log lines kept, which a long run would otherwise grow without end.
const LINES_KEPT: usize = 10_000;

pub struct Air {
    pub paced: Paced,
    /// The log lines already printed, by number.
    printed: usize,
    /// Whether every line goes to the terminal, or only warnings.
    every_line: bool,
}

impl Air {
    /// `count` devices in reach of each other, in one group when `in_group`, else each alone,
    /// with UTC starting at `utc_s`.
    pub fn new(count: usize, in_group: bool, utc_s: i64, every_line: bool) -> Self {
        Self {
            paced: Paced::new(count, in_group, utc_s, LINES_KEPT),
            printed: 0,
            every_line,
        }
    }

    /// Runs virtual time on to the host's clock `wall`, in µs, at the host's pace while `input`
    /// holds, and prints what the nodes logged meanwhile.
    pub fn advance(&mut self, wall: u64, input: bool) {
        self.paced.advance(wall, input);
        let (lines, next) = self.paced.sim.lines_since(self.printed);
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

    /// Gives node `n` what its device's readings say of its fix and clocks.
    pub fn sense(&self, n: usize, readings: &Readings) {
        self.paced.sense(
            n,
            readings.fix.then_some(readings.position),
            readings.rtc_readable(),
        );
    }
}
