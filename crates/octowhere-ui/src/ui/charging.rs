//! The clock's battery gauge while charging: the level split into uneven bands that gather and
//! regroup on a loop, and the solid layer under them that retreats as charging starts and covers
//! them again as it stops. The 27 September implementation update is its design.

use super::{gesture::Micros, screens::Battery};

/// The loop, in frames at 30 fps.
pub const LOOP_FRAMES: u8 = 72;
const FRAME: Micros = 1_000_000 / 30;
/// A phase in the loop's assembled hold: the wipes hold it, and a loop entered by one starts
/// from it.
pub const ASSEMBLED_PHASE: u8 = 27;
/// How long a whole wipe takes, for a level at or above `SHORT_BELOW` and for one below it.
const WIPE: Micros = 450_000;
const SHORT_WIPE: Micros = 180_000;
const SHORT_BELOW: u8 = 35;

/// The 87% motif's band heights, and its gaps at the loop's three rests: dispersed, assembled
/// and paired. Each set of gaps sums to `GAP_SUM`.
const HEIGHTS: [f64; 11] = [2.0, 3.0, 2.0, 10.0, 7.0, 3.0, 11.0, 2.0, 9.0, 4.0, 8.0];
const SCATTER: [f64; 10] = [5.0, 1.0, 2.0, 5.0, 1.0, 4.0, 1.0, 4.0, 1.0, 1.0];
const ASSEMBLED: [f64; 10] = [1.0, 1.0, 5.0, 2.0, 4.0, 1.0, 5.0, 1.0, 3.0, 2.0];
const PAIRED: [f64; 10] = [1.0, 3.0, 6.0, 1.0, 3.0, 2.0, 1.0, 5.0, 1.0, 2.0];
const GAP_SUM: u32 = 25;
/// The order each gap starts its move in, gathering and then pairing.
const BUILD_ORDER: [u8; 10] = [0, 4, 2, 8, 1, 6, 9, 3, 7, 5];
const PAIR_ORDER: [u8; 10] = [8, 0, 5, 2, 9, 4, 1, 7, 3, 6];
/// The motif's filled height: its level is where the counts and gap share are set.
const MOTIF: u32 = 86;
const MAX_BANDS: usize = 13;

fn smoothstep(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The indices of `values` by their fractional parts, largest first, ties in index order.
fn by_remainder<const N: usize>(values: &[f64], order: &mut heapless::Vec<usize, N>) {
    order.clear();
    order.extend(0..values.len());
    order.sort_by(|&a, &b| {
        let (a, b) = (values[a] - libm::floor(values[a]), values[b] - libm::floor(values[b]));
        b.total_cmp(&a)
    });
}

/// The gaps partway from `from` to `to`, each starting its move a quarter of the way in at
/// most, in `order`, and kept summing to `GAP_SUM`.
fn gaps_between(from: &[f64; 10], to: &[f64; 10], t: f64, order: &[u8; 10]) -> [f64; 10] {
    let mut values: [f64; 10] =
        core::array::from_fn(|i| from[i] + (to[i] - from[i]) * smoothstep((t - 0.25 * f64::from(order[i]) / 9.0) / 0.75));
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
fn resample(weights: &[f64], count: usize, out: &mut heapless::Vec<f64, MAX_BANDS>) {
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

/// `total` split in proportion to `weights`, each part at least 1.
fn apportion(weights: &[f64], total: u32, out: &mut heapless::Vec<u32, MAX_BANDS>) {
    let free = (total - weights.len() as u32) as f64;
    let excess_sum: f64 = weights.iter().map(|w| (w - 1.0).max(0.0)).sum();
    let basis = |w: f64| if excess_sum > 0.0 { (w - 1.0).max(0.0) } else { w };
    let basis_sum: f64 = weights.iter().map(|&w| basis(w)).sum();
    let fractions: heapless::Vec<f64, MAX_BANDS> = weights.iter().map(|&w| free * basis(w) / basis_sum).collect();
    out.clear();
    out.extend(fractions.iter().map(|&f| 1 + libm::floor(f) as u32));
    let short = total - out.iter().sum::<u32>();
    let mut order = heapless::Vec::<usize, MAX_BANDS>::new();
    by_remainder(&fractions, &mut order);
    for &i in order.iter().take(short as usize) {
        out[i] += 1;
    }
}

/// `n / d` rounded, a half to even.
fn round_half_even(n: u32, d: u32) -> u32 {
    let (quotient, twice) = (n / d, 2 * (n % d));
    quotient + u32::from(twice > d || (twice == d && quotient % 2 == 1))
}

/// A filled height of `height` rows split into bands at `phase` in the loop: each band's first
/// row and row count, from the top of the fill. The bands grow in number with the height, and
/// the gaps between them take about the share of it they take at the motif's.
#[must_use]
pub fn bands(height: u32, phase: u8) -> heapless::Vec<(u32, u32), MAX_BANDS> {
    let mut out = heapless::Vec::new();
    if height == 0 {
        return out;
    }
    let count = ((22 * height + MOTIF) / (2 * MOTIF)).clamp(1, MAX_BANDS as u32) as usize;
    if count == 1 {
        _ = out.push((0, height));
        return out;
    }
    let gap_total = round_half_even(height * GAP_SUM, MOTIF).min(height - count as u32).max(count as u32 - 1);
    let (mut weights, mut heights, mut spaces) = (heapless::Vec::new(), heapless::Vec::new(), heapless::Vec::new());
    resample(&HEIGHTS, count, &mut weights);
    apportion(&weights, height - gap_total, &mut heights);
    resample(&gaps(phase), count - 1, &mut weights);
    apportion(&weights, gap_total, &mut spaces);
    let mut top = 0;
    for (i, &rows) in heights.iter().enumerate() {
        _ = out.push((top, rows));
        top += rows + spaces.get(i).copied().unwrap_or(0);
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
        let t = if self.length == 0 { 1.0 } else { now.saturating_sub(self.start) as f32 / self.length as f32 };
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
        let level = battery.filter(|battery| battery.present).map(|battery| battery.percent);
        let charging = battery.is_some_and(|battery| battery.present && battery.charging);
        let before = self.charging.replace(charging);
        if before == Some(charging) {
            return;
        }
        let to = if charging { 1.0 } else { 0.0 };
        let exposed = self.exposed(now);
        match level {
            Some(level) if level > 0 && before.is_some() => {
                let whole = if level >= SHORT_BELOW { WIPE } else { SHORT_WIPE };
                let length = libm::roundf(whole as f32 * (to - exposed).abs()) as Micros;
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
                self.wipe = Wipe { from: exposed, to, start: now, length };
            }
            _ => {
                self.wipe = Wipe { from: to, to, start: now, length: 0 };
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
            ((u64::from(self.held) + frames) % u64::from(LOOP_FRAMES)) as u8
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
        Some(Battery { present: true, percent, millivolts: 3_900, charging, usb: charging })
    }

    fn rows(height: u32, phase: u8) -> (u32, u32, usize) {
        let bands = bands(height, phase);
        let lit = bands.iter().map(|&(_, rows)| rows).sum();
        let &(top, rows) = bands.last().unwrap();
        (lit, top + rows, bands.len())
    }

    #[test]
    fn the_motif_level_has_eleven_bands_and_twenty_five_rows_of_gap() {
        for phase in 0..LOOP_FRAMES {
            assert_eq!(rows(86, phase), (61, 86, 11), "phase {phase}");
        }
        assert_eq!(rows(12, 0).2, 2);
        assert_eq!(bands(1, 0).as_slice(), &[(0, 1)]);
    }

    #[test]
    fn every_level_fills_its_height_with_no_closed_gap() {
        for height in 1..=99 {
            for phase in 0..LOOP_FRAMES {
                let bands = bands(height, phase);
                assert_eq!(bands[0].0, 0);
                let &(top, rows) = bands.last().unwrap();
                assert_eq!(top + rows, height, "height {height} phase {phase}");
                assert!(bands.windows(2).all(|pair| pair[1].0 > pair[0].0 + pair[0].1));
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
        assert_eq!(low.exposed(SHORT_WIPE), 1.0);
    }
}
