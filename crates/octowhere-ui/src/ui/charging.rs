//! The clock's battery gauge while charging: the level split into upright slices in the identity
//! barcode's proportions, whose gaps gather and regroup on a loop, and the solid layer under them
//! that retreats from the fill's end to its start as charging starts and covers them again as it
//! stops. The 27 September implementation update designed the loop and the wipe, and the 29
//! September one the slices.

use super::{gesture::Micros, screens::Battery};

/// The loop, in frames at 30 fps, and the rest after each pass, on its dispersed layout, where
/// the pass both starts and ends (owner).
pub const LOOP_FRAMES: u8 = 72;
const REST_FRAMES: u64 = 120;
const FRAME: Micros = 1_000_000 / 30;
/// A phase in the loop's assembled hold: the wipes hold it, and a loop entered by one starts
/// from it.
pub const ASSEMBLED_PHASE: u8 = 27;
/// How long a whole wipe takes, whatever the level.
const WIPE: Micros = 450_000;

/// The motif's gaps at the loop's three rests: dispersed, assembled and paired. Each set sums
/// to `GAP_SUM`.
const SCATTER: [f64; 10] = [5.0, 1.0, 2.0, 5.0, 1.0, 4.0, 1.0, 4.0, 1.0, 1.0];
const ASSEMBLED: [f64; 10] = [1.0, 1.0, 5.0, 2.0, 4.0, 1.0, 5.0, 1.0, 3.0, 2.0];
const PAIRED: [f64; 10] = [1.0, 3.0, 6.0, 1.0, 3.0, 2.0, 1.0, 5.0, 1.0, 2.0];
const GAP_SUM: u32 = 25;
/// The order each gap starts its move in, gathering and then pairing.
const BUILD_ORDER: [u8; 10] = [0, 4, 2, 8, 1, 6, 9, 3, 7, 5];
const PAIR_ORDER: [u8; 10] = [8, 0, 5, 2, 9, 4, 1, 7, 3, 6];
/// The identity barcode's bars for version 0.1.0, in units: a one bit is two, a zero one. Its
/// bars and its two-unit gaps between them set the slices' widths and their share of the level.
const BARCODE: [f64; 20] = [
    1.0, 1.0, 1.0, 1.0, 1.0, 2.0, 2.0, 2.0, 2.0, 1.0, 1.0, 1.0, 1.0, 2.0, 2.0, 2.0, 1.0, 1.0, 1.0,
    1.0,
];
/// The slices at `MOTIF_LENGTH` px, the 87% level, and the most there can be.
const MOTIF_SLICES: f64 = 20.0;
const MOTIF_LENGTH: f64 = 150.0;
pub const MAX_SLICES: usize = 23;
/// How far the loop's gaps move the slices' otherwise even spacing.
const BEAT: f64 = 0.35;

fn smoothstep(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The indices of `values` by their fractional parts, largest first, ties in index order.
fn by_remainder<const N: usize>(values: &[f64], order: &mut heapless::Vec<usize, N>) {
    order.clear();
    order.extend(0..values.len());
    order.sort_by(|&a, &b| {
        let (a, b) = (
            values[a] - libm::floor(values[a]),
            values[b] - libm::floor(values[b]),
        );
        b.total_cmp(&a)
    });
}

/// The gaps partway from `from` to `to`, each starting its move a quarter of the way in at
/// most, in `order`, and kept summing to `GAP_SUM`.
fn gaps_between(from: &[f64; 10], to: &[f64; 10], t: f64, order: &[u8; 10]) -> [f64; 10] {
    let mut values: [f64; 10] = core::array::from_fn(|i| {
        from[i] + (to[i] - from[i]) * smoothstep((t - 0.25 * f64::from(order[i]) / 9.0) / 0.75)
    });
    let sum: f64 = values.iter().sum();
    for value in &mut values {
        *value = *value * GAP_SUM as f64 / sum;
    }
    let mut ints = values.map(libm::floor);
    let short = GAP_SUM as usize - ints.iter().sum::<f64>() as usize;
    let mut order = heapless::Vec::<usize, 10>::new();
    by_remainder(&values, &mut order);
    for &i in order.iter().take(short) {
        ints[i] += 1.0;
    }
    ints
}

/// The motif's gaps at `phase` in the loop.
fn gaps(phase: u8) -> [f64; 10] {
    let n = f64::from(phase % LOOP_FRAMES);
    match phase % LOOP_FRAMES {
        0..18 => gaps_between(&SCATTER, &ASSEMBLED, n / 17.0, &BUILD_ORDER),
        18..40 => ASSEMBLED,
        40..54 => gaps_between(&ASSEMBLED, &PAIRED, (n - 40.0) / 13.0, &PAIR_ORDER),
        54..62 => PAIRED,
        _ => gaps_between(&PAIRED, &SCATTER, (n - 62.0) / 9.0, &PAIR_ORDER),
    }
}

/// `weights` interpolated to `count` entries, keeping their unevenness.
fn resample(weights: &[f64], count: usize, out: &mut heapless::Vec<f64, MAX_SLICES>) {
    out.clear();
    if count == 1 {
        _ = out.push(weights[weights.len() / 2]);
        return;
    }
    let last = weights.len() - 1;
    for i in 0..count {
        let position = i as f64 * last as f64 / (count - 1) as f64;
        let j = libm::floor(position) as usize;
        let t = position - j as f64;
        _ = out.push(weights[j] * (1.0 - t) + weights[(j + 1).min(last)] * t);
    }
}

/// `total` split in proportion to `weights`, the parts rounded down and the pixels left over
/// given to the largest remainders.
fn apportion(weights: &[f64], total: u32, out: &mut heapless::Vec<u32, MAX_SLICES>) {
    let sum: f64 = weights.iter().sum();
    let raw: heapless::Vec<f64, MAX_SLICES> = weights
        .iter()
        .map(|&w| f64::from(total) * w / sum)
        .collect();
    out.clear();
    out.extend(raw.iter().map(|&v| libm::floor(v) as u32));
    let short = total - out.iter().sum::<u32>();
    let mut order = heapless::Vec::<usize, MAX_SLICES>::new();
    by_remainder(&raw, &mut order);
    for &i in order.iter().take(short as usize) {
        out[i] += 1;
    }
}

/// A fill `length` px long split into slices at `phase` in the loop: each slice's first column
/// and width, from the fill's left end. The slices grow in number with the length, keep the
/// barcode's narrow and broad widths, and the gaps take about the barcode's share of it.
#[must_use]
pub fn slices(length: u32, phase: u8) -> heapless::Vec<(u32, u32), MAX_SLICES> {
    let mut out = heapless::Vec::new();
    if length == 0 {
        return out;
    }
    let count = libm::rint(f64::from(length) * MOTIF_SLICES / MOTIF_LENGTH)
        .clamp(1.0, MAX_SLICES as f64) as usize;
    if count == 1 {
        _ = out.push((0, length));
        return out;
    }
    let (mut weights, mut widths, mut spaces) = (
        heapless::Vec::new(),
        heapless::Vec::new(),
        heapless::Vec::new(),
    );
    resample(&BARCODE, count, &mut weights);
    let between = 2.0 * (count - 1) as f64;
    let share = between / (weights.iter().sum::<f64>() + between);
    let gap_total = (libm::rint(f64::from(length) * share) as u32)
        .min(length - count as u32)
        .max(count as u32 - 1);
    apportion(&weights, length - gap_total, &mut widths);
    resample(&gaps(phase), count - 1, &mut weights);
    let mean = weights.iter().sum::<f64>() / weights.len() as f64;
    for weight in &mut weights {
        *weight = 1.0 + BEAT * (*weight / mean - 1.0);
    }
    apportion(&weights, gap_total, &mut spaces);
    let mut left = 0;
    for (i, &width) in widths.iter().enumerate() {
        _ = out.push((left, width));
        left += width + spaces.get(i).copied().unwrap_or(0);
    }
    out
}

/// The charging state the gauge shows: how far its bands show through the solid layer, and
/// where their loop is.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Charge {
    /// The last reading's charging flag, if there has been one.
    charging: Option<bool>,
    wipe: Wipe,
    /// The phase the loop holds, and from when it runs on from it while charging.
    held: u8,
    runs_from: Micros,
}

/// The solid layer's edge on its way between covering the bands, 0, and showing them, 1.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Wipe {
    from: f32,
    to: f32,
    start: Micros,
    length: Micros,
}

impl Wipe {
    fn at(&self, now: Micros) -> f32 {
        let t = if self.length == 0 {
            1.0
        } else {
            now.saturating_sub(self.start) as f32 / self.length as f32
        };
        self.from + (self.to - self.from) * smoothstep(f64::from(t)) as f32
    }

    fn end(&self) -> Micros {
        self.start + self.length
    }
}

impl Charge {
    /// Takes a reading. A change of the charging flag between two readings of a known, nonzero
    /// level wipes the solid layer; any other change shows its end at once.
    pub fn read(&mut self, battery: Option<Battery>, now: Micros) {
        let level = battery
            .filter(|battery| battery.present)
            .map(|battery| battery.percent);
        let charging = battery.is_some_and(|battery| battery.present && battery.charging);
        let before = self.charging.replace(charging);
        if before == Some(charging) {
            return;
        }
        let to = if charging { 1.0 } else { 0.0 };
        let exposed = self.exposed(now);
        match level {
            Some(level) if level > 0 && before.is_some() => {
                let length = libm::roundf(WIPE as f32 * (to - exposed).abs()) as Micros;
                if charging {
                    // Bands that are still partly showing keep their layout; covered ones
                    // come back assembled.
                    if exposed <= 0.0 {
                        self.held = ASSEMBLED_PHASE;
                    }
                    self.runs_from = now + length;
                } else {
                    self.held = self.phase(now);
                }
                self.wipe = Wipe {
                    from: exposed,
                    to,
                    start: now,
                    length,
                };
            }
            _ => {
                self.wipe = Wipe {
                    from: to,
                    to,
                    start: now,
                    length: 0,
                };
                self.held = ASSEMBLED_PHASE;
                self.runs_from = now;
            }
        }
    }

    /// How far the bands show through the solid layer, from 0, covered, to 1.
    #[must_use]
    pub fn exposed(&self, now: Micros) -> f32 {
        self.wipe.at(now)
    }

    /// The bands' phase in their loop.
    #[must_use]
    pub fn phase(&self, now: Micros) -> u8 {
        if self.charging == Some(true) && now >= self.runs_from {
            let frames = (now - self.runs_from) / FRAME;
            let at = (u64::from(self.held) + frames) % (u64::from(LOOP_FRAMES) + REST_FRAMES);
            if at < u64::from(LOOP_FRAMES) {
                at as u8
            } else {
                0
            }
        } else {
            self.held
        }
    }

    /// Whether the gauge changes without another reading: a wipe is under way, or the bands
    /// are looping.
    #[must_use]
    pub fn is_moving(&self, now: Micros) -> bool {
        self.charging == Some(true) || now < self.wipe.end()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn battery(percent: u8, charging: bool) -> Option<Battery> {
        Some(Battery {
            present: true,
            percent,
            millivolts: 3_900,
            charging,
            usb: charging,
        })
    }

    fn widths(
        length: u32,
        phase: u8,
    ) -> (
        heapless::Vec<u32, MAX_SLICES>,
        heapless::Vec<u32, MAX_SLICES>,
    ) {
        let slices = slices(length, phase);
        let widths = slices.iter().map(|&(_, width)| width).collect();
        let gaps = slices
            .windows(2)
            .map(|pair| pair[1].0 - pair[0].0 - pair[0].1)
            .collect();
        (widths, gaps)
    }

    #[test]
    fn the_slices_match_the_design_study() {
        // From the 29 September study's `title_pattern`.
        let cases: [(u32, u8, &[u32], &[u32]); 6] = [
            (
                150,
                27,
                &[3, 2, 2, 2, 2, 5, 5, 5, 5, 2, 2, 2, 2, 5, 5, 5, 2, 2, 2, 2],
                &[4, 4, 4, 5, 6, 5, 4, 5, 5, 5, 4, 5, 6, 5, 4, 4, 5, 4, 4],
            ),
            (
                150,
                46,
                &[3, 2, 2, 2, 2, 5, 5, 5, 5, 2, 2, 2, 2, 5, 5, 5, 2, 2, 2, 2],
                &[4, 4, 4, 6, 7, 5, 4, 5, 5, 5, 4, 5, 5, 5, 4, 4, 4, 4, 4],
            ),
            (21, 27, &[3, 3, 3], &[5, 7]),
            (
                172,
                0,
                &[
                    2, 2, 2, 2, 2, 3, 5, 5, 5, 5, 3, 2, 2, 2, 3, 5, 5, 5, 3, 2, 2, 2, 2,
                ],
                &[
                    6, 5, 4, 4, 4, 5, 5, 6, 5, 4, 4, 5, 5, 5, 4, 4, 5, 5, 4, 4, 4, 4,
                ],
            ),
            (40, 60, &[3, 5, 3, 5, 2], &[5, 5, 5, 7]),
            (5, 10, &[5], &[]),
        ];
        for (length, phase, bars, gaps) in cases {
            let (widths, spaces) = widths(length, phase);
            assert_eq!(
                (widths.as_slice(), spaces.as_slice()),
                (bars, gaps),
                "{length} {phase}"
            );
        }
    }

    #[test]
    fn every_length_fills_to_its_end_with_no_closed_gap() {
        for length in 1..=172 {
            for phase in 0..LOOP_FRAMES {
                let slices = slices(length, phase);
                assert_eq!(slices[0].0, 0);
                let &(left, width) = slices.last().unwrap();
                assert_eq!(left + width, length, "length {length} phase {phase}");
                assert!(slices.iter().all(|&(_, width)| width > 0));
                assert!(
                    slices
                        .windows(2)
                        .all(|pair| pair[1].0 > pair[0].0 + pair[0].1)
                );
            }
        }
    }

    #[test]
    fn a_charge_starts_with_a_wipe_and_loops_from_the_assembled_layout() {
        let mut charge = Charge::default();
        charge.read(battery(87, false), 0);
        assert_eq!(charge.exposed(0), 0.0);
        charge.read(battery(87, true), 1_000_000);
        assert_eq!(charge.exposed(1_000_000), 0.0);
        assert_eq!(charge.phase(1_200_000), ASSEMBLED_PHASE);
        assert_eq!(charge.exposed(1_000_000 + WIPE), 1.0);
        assert_eq!(charge.phase(1_000_000 + WIPE + FRAME), ASSEMBLED_PHASE + 1);
        let rest = 1_000_000 + WIPE + u64::from(LOOP_FRAMES - ASSEMBLED_PHASE) * FRAME;
        assert_eq!(charge.phase(rest), 0);
        assert_eq!(charge.phase(rest + (REST_FRAMES - 1) * FRAME), 0);
        assert_eq!(charge.phase(rest + REST_FRAMES * FRAME), 0);
        assert_eq!(charge.phase(rest + (REST_FRAMES + 1) * FRAME), 1);
    }

    #[test]
    fn a_reversal_goes_back_from_where_the_edge_is() {
        let mut charge = Charge::default();
        charge.read(battery(87, false), 0);
        charge.read(battery(87, true), 0);
        let halfway = charge.exposed(WIPE / 2);
        charge.read(battery(87, false), WIPE / 2);
        assert_eq!(charge.exposed(WIPE / 2), halfway);
        assert!(charge.is_moving(WIPE / 2 + WIPE / 4));
        assert_eq!(charge.exposed(WIPE / 2 + WIPE / 2), 0.0);
        assert!(!charge.is_moving(WIPE));
    }

    #[test]
    fn a_first_reading_or_an_unknown_level_changes_at_once() {
        let mut charge = Charge::default();
        charge.read(battery(87, true), 0);
        assert_eq!(charge.exposed(0), 1.0);
        let mut empty = Charge::default();
        empty.read(battery(0, false), 0);
        empty.read(battery(0, true), 10);
        assert_eq!(empty.exposed(10), 1.0);
        let mut low = Charge::default();
        low.read(battery(12, false), 0);
        low.read(battery(12, true), 0);
        assert_eq!(low.exposed(WIPE), 1.0);
    }
}
