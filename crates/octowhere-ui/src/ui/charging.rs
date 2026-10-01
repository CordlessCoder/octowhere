//! The clock's battery gauge while charging: the level split into upright slices in the identity
//! barcode's proportions, and the solid layer under them. The 27 September implementation update
//! designed the solid's wipe, and the 29 September one the slices. As charging starts the solid
//! closes in on the fill's middle and the slices build out from there; they then rest apart,
//! with pairs docking, holding and splitting back twice a loop; as it stops they settle open, the
//! build runs backwards, and the solid grows back out from the middle. The build and the beats follow the bars of the Marathon logo
//! animation, its frames 34 to 54 and 72 to 97 (owner, 2026-10-01, after the design's barcode
//! motion review).
//!
//! The bands' frame counts the build from 0, the loop from `OPEN` and the build backwards from
//! `UNBUILD`. Every mark's place is
//! worked out from that frame alone, from fixed poses, so a skipped frame changes nothing after
//! it.

use super::{
    ease::{FLIGHT, flight},
    gesture::Micros,
    screens::Battery,
};

const FRAME: Micros = 1_000_000 / 30;
/// How long a whole wipe takes, whatever the level: as long as the end slices' flight, whose
/// curve it follows.
const WIPE: Micros = (FLIGHT.len() as Micros - 1) * FRAME;

/// The build, in frames from the solid's wipe ending: the fill stays empty for `SEED_FROM`
/// frames, then follows the logo's bars from its frame `LOGO_SEED` on. Places are fractions of
/// the fill's length, from the logo's bars, which run from its column 18 to 280.
/// The logo's last inner bars appear on its frame 54.
const BUILD_FRAMES: u8 = SEED_FROM + 55 - LOGO_SEED;
const LOGO_SEED: u8 = 34;
/// A seed in the middle, growing over three frames in sixteenths of the two end slices'
/// widths together, which then splits into those two and sends them out to the ends.
const SEED: [u32; 3] = [9, 11, 14];
const SEED_FROM: u8 = 3;
/// The two end slices travel on `FLIGHT` from here.
const FLY_FROM: u8 = SEED_FROM + SEED.len() as u8;
/// The ends arrive here, and hold until the last inner slices appear.
const LANDED: u8 = FLY_FROM + FLIGHT.len() as u8 - 1;
const LATE_POP: u8 = SEED_FROM + 53 - LOGO_SEED;
/// The logo's frame each of its inner bars appears in, by the bar's middle: the slice nearest
/// each one appears with it.
const POPS: [(f32, u8); 10] = [
    (0.069, 53),
    (0.126, 47),
    (0.225, 50),
    (0.351, 54),
    (0.479, 50),
    (0.592, 49),
    (0.706, 43),
    (0.805, 47),
    (0.859, 47),
    (0.931, 44),
];

/// The loop, after the build: two beats, each an open hold, a dock, a joined hold and a split.
const OPEN_FRAMES: u8 = 45;
const JOINED_FRAMES: u8 = 15;
/// How far a dock has gone at each of its frames, and a split back the same way.
const MOVE: [f32; 5] = [0.14, 0.29, 0.71, 0.86, 1.0];
const MOVE_FRAMES: u8 = MOVE.len() as u8;
const BEAT_FRAMES: u8 = OPEN_FRAMES + 2 * MOVE_FRAMES + JOINED_FRAMES;
const LOOP_FRAMES: u8 = 2 * BEAT_FRAMES;
/// The frame of the open pose: the loop starts there, and the wipes hold it.
pub const OPEN: u8 = BUILD_FRAMES;
/// The build backwards: frame `UNBUILD + k` shows the build's frame `BUILD_FRAMES - 1 - k`.
const UNBUILD: u8 = OPEN + LOOP_FRAMES;

/// The open pose's gaps, the design study's dispersed layout. The slices' otherwise even
/// spacing moves by `BEAT` of their unevenness.
const OPEN_GAPS: [f32; 10] = [5.0, 1.0, 2.0, 5.0, 1.0, 4.0, 1.0, 4.0, 1.0, 1.0];
const BEAT: f32 = 0.35;
/// Each of the logo's eleven gaps joined, over itself open. The first beat repeats it along the
/// fill's gaps and the second the same reversed; a gap at 0 closes, and the others share what is
/// left of the gaps' length in proportion.
const JOINED: [f32; 11] = [
    0.0,
    21.0 / 7.0,
    0.0,
    22.0 / 15.0,
    19.0 / 16.0,
    10.0 / 17.0,
    26.0 / 17.0,
    1.0 / 13.0,
    21.0 / 7.0,
    0.0,
    12.0 / 7.0,
];
/// The identity barcode's bars for version 0.1.0, in units: a one bit is two, a zero one. Its
/// bars and its two-unit gaps between them set the slices' widths and their share of the level.
const BARCODE: [f32; 20] = [
    1.0, 1.0, 1.0, 1.0, 1.0, 2.0, 2.0, 2.0, 2.0, 1.0, 1.0, 1.0, 1.0, 2.0, 2.0, 2.0, 1.0, 1.0, 1.0,
    1.0,
];
/// The slices at `MOTIF_LENGTH` px, the 87% level, and the most there can be.
const MOTIF_SLICES: f32 = 20.0;
const MOTIF_LENGTH: f32 = 150.0;
pub const MAX_SLICES: usize = 23;

/// A column run of the gauge: its first column from the fill's left end, and its width.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Mark {
    pub left: u32,
    pub width: u32,
}

/// The indices of `values` by their fractional parts, largest first, ties in index order.
fn by_remainder<const N: usize>(values: &[f32], order: &mut heapless::Vec<usize, N>) {
    order.clear();
    order.extend(0..values.len());
    order.sort_by(|&a, &b| {
        let (a, b) = (
            values[a] - libm::floorf(values[a]),
            values[b] - libm::floorf(values[b]),
        );
        b.total_cmp(&a)
    });
}

/// `weights` interpolated to `count` entries, keeping their unevenness.
fn resample(weights: &[f32], count: usize, out: &mut heapless::Vec<f32, MAX_SLICES>) {
    out.clear();
    if count == 1 {
        _ = out.push(weights[weights.len() / 2]);
        return;
    }
    let last = weights.len() - 1;
    for i in 0..count {
        let position = i as f32 * last as f32 / (count - 1) as f32;
        let j = libm::floorf(position) as usize;
        let t = position - j as f32;
        _ = out.push(weights[j] * (1.0 - t) + weights[(j + 1).min(last)] * t);
    }
}

/// `total` split in proportion to `weights`, the parts rounded down and the pixels left over
/// given to the largest remainders. A zero weight gets nothing.
fn apportion(weights: &[f32], total: u32, out: &mut heapless::Vec<u32, MAX_SLICES>) {
    let sum: f32 = weights.iter().sum();
    let raw: heapless::Vec<f32, MAX_SLICES> =
        weights.iter().map(|&w| total as f32 * w / sum).collect();
    out.clear();
    out.extend(raw.iter().map(|&v| libm::floorf(v) as u32));
    let short = total - out.iter().sum::<u32>();
    let mut order = heapless::Vec::<usize, MAX_SLICES>::new();
    by_remainder(&raw, &mut order);
    for &i in order.iter().take(short as usize) {
        out[i] += 1;
    }
}

/// The slices of a fill `length` px long in their open pose, and in each beat's joined pose:
/// their widths, and each one's first column from the fill's left end.
struct Poses {
    widths: heapless::Vec<u32, MAX_SLICES>,
    open: heapless::Vec<u32, MAX_SLICES>,
    joined: [heapless::Vec<u32, MAX_SLICES>; 2],
}

impl Poses {
    /// The slices grow in number with the length, keep the barcode's narrow and broad widths,
    /// and the gaps take about the barcode's share of it.
    fn new(length: u32) -> Self {
        let mut poses = Self {
            widths: heapless::Vec::new(),
            open: heapless::Vec::new(),
            joined: [heapless::Vec::new(), heapless::Vec::new()],
        };
        if length == 0 {
            return poses;
        }
        let count = libm::rintf(length as f32 * MOTIF_SLICES / MOTIF_LENGTH)
            .clamp(1.0, MAX_SLICES as f32) as usize;
        if count == 1 {
            _ = poses.widths.push(length);
            _ = poses.open.push(0);
            poses.joined = [poses.open.clone(), poses.open.clone()];
            return poses;
        }
        let (mut weights, mut spaces) = (heapless::Vec::new(), heapless::Vec::new());
        resample(&BARCODE, count, &mut weights);
        let between = 2.0 * (count - 1) as f32;
        let share = between / (weights.iter().sum::<f32>() + between);
        let gap_total = (libm::rintf(length as f32 * share) as u32)
            .min(length - count as u32)
            .max(count as u32 - 1);
        apportion(&weights, length - gap_total, &mut poses.widths);
        resample(&OPEN_GAPS, count - 1, &mut weights);
        let mean = weights.iter().sum::<f32>() / weights.len() as f32;
        for weight in &mut weights {
            *weight = 1.0 + BEAT * (*weight / mean - 1.0);
        }
        apportion(&weights, gap_total, &mut spaces);
        Self::place(&poses.widths, &spaces, &mut poses.open);
        for (beat, joined) in poses.joined.iter_mut().enumerate() {
            let ratio = |i: usize| match beat {
                0 => JOINED[i % JOINED.len()],
                _ => JOINED[JOINED.len() - 1 - i % JOINED.len()],
            };
            weights.clear();
            weights.extend(
                spaces
                    .iter()
                    .enumerate()
                    .map(|(i, &space)| space as f32 * ratio(i)),
            );
            let open = weights.iter().filter(|&&weight| weight > 0.0).count() as u32;
            if open == 0 {
                joined.clone_from(&poses.open);
                continue;
            }
            // Every gap that stays open keeps a column at least, so only the chosen ones close.
            let mut gaps = heapless::Vec::new();
            apportion(&weights, gap_total - open, &mut gaps);
            for (gap, &weight) in gaps.iter_mut().zip(&weights) {
                *gap += u32::from(weight > 0.0);
            }
            Self::place(&poses.widths, &gaps, joined);
        }
        poses
    }

    fn place(widths: &[u32], gaps: &[u32], out: &mut heapless::Vec<u32, MAX_SLICES>) {
        out.clear();
        let mut left = 0;
        for (i, &width) in widths.iter().enumerate() {
            _ = out.push(left);
            left += width + gaps.get(i).copied().unwrap_or(0);
        }
    }

    fn slice(&self, i: usize, left: u32) -> Mark {
        Mark {
            left,
            width: self.widths[i],
        }
    }
}

/// Which beat a loop frame falls in, and how far its slices are from the open pose, 0, to
/// joined, 1.
fn beat(frame: u8) -> (usize, f32) {
    let local = frame % BEAT_FRAMES;
    let joined = OPEN_FRAMES + MOVE_FRAMES;
    let split = joined + JOINED_FRAMES;
    let t = match local {
        _ if local < OPEN_FRAMES => 0.0,
        _ if local < joined => MOVE[usize::from(local - OPEN_FRAMES)],
        _ if local < split => 1.0,
        _ => 1.0 - MOVE[usize::from(local - split)],
    };
    (usize::from(frame / BEAT_FRAMES), t)
}

/// The frame `frames` on from `bands`: through the build, then round the loop.
fn advance(bands: u8, frames: u64) -> u8 {
    let at = u64::from(bands) + frames;
    if at < u64::from(OPEN) {
        at as u8
    } else {
        OPEN + ((at - u64::from(OPEN)) % u64::from(LOOP_FRAMES)) as u8
    }
}

/// How many frames from loop frame `frame` until the slices next rest open.
fn until_open(frame: u8) -> u8 {
    let local = frame % BEAT_FRAMES;
    if local < OPEN_FRAMES {
        0
    } else {
        BEAT_FRAMES - local
    }
}

/// The frame that shows the same as `bands`: the first of its hold, or `bands` itself while
/// something moves. A hold then changes nothing to redraw.
fn settled(bands: u8) -> u8 {
    if bands >= UNBUILD {
        return UNBUILD + BUILD_FRAMES - 1 - settled(BUILD_FRAMES - 1 - (bands - UNBUILD));
    }
    if bands < OPEN {
        return match bands {
            0..SEED_FROM => 0,
            LANDED..LATE_POP => LANDED,
            _ => bands,
        };
    }
    let frame = bands - OPEN;
    let (index, t) = beat(frame);
    match t {
        0.0 => OPEN,
        1.0 => OPEN + index as u8 * BEAT_FRAMES + OPEN_FRAMES + MOVE_FRAMES - 1,
        _ => bands,
    }
}

/// A mark `width` wide whose middle is `at` of the way along a fill `length` px long.
fn centred(at: f32, width: u32, length: u32) -> Mark {
    let width = width.min(length);
    let left = libm::rintf(at * length as f32 - width as f32 / 2.0)
        .clamp(0.0, (length - width) as f32) as u32;
    Mark { left, width }
}

/// The gauge's marks across a fill `length` px long at frame `bands`. Marks may overlap, and
/// the gauge is their union. In the loop each slice moves straight between its open place and
/// its joined one, all of a beat's slices together, rounded once, so none turns back mid-move
/// and none crosses another.
#[must_use]
pub fn marks(length: u32, bands: u8) -> heapless::Vec<Mark, MAX_SLICES> {
    if bands >= UNBUILD {
        return marks(length, BUILD_FRAMES - 1 - (bands - UNBUILD));
    }
    let poses = Poses::new(length);
    let mut out = heapless::Vec::new();
    let count = poses.widths.len();
    if count == 0 {
        return out;
    }
    if bands >= OPEN {
        let (index, t) = beat((bands - OPEN) % LOOP_FRAMES);
        for (i, (&open, &joined)) in poses.open.iter().zip(&poses.joined[index]).enumerate() {
            let left = open as f32 + (joined as f32 - open as f32) * t;
            _ = out.push(poses.slice(i, libm::rintf(left) as u32));
        }
        return out;
    }
    if (SEED_FROM..FLY_FROM).contains(&bands) {
        let ends = poses.widths[0] + poses.widths[count - 1];
        let width = (SEED[usize::from(bands - SEED_FROM)] * ends).div_ceil(16);
        _ = out.push(centred(0.5, width.max(1), length));
    } else if bands >= FLY_FROM {
        let middle = length as f32 / 2.0;
        let t = FLIGHT[usize::from(bands - FLY_FROM).min(FLIGHT.len() - 1)];
        let last = count - 1;
        // The two end slices leave the middle a column apart.
        let starts = [middle - 0.5 - poses.widths[0] as f32, middle + 0.5];
        for (start, i) in starts.into_iter().zip([0, last]) {
            let start = start.clamp(0.0, (length - poses.widths[i]) as f32);
            let end = poses.open[i] as f32;
            _ = out.push(poses.slice(i, libm::rintf(start + (end - start) * t) as u32));
            if last == 0 {
                break;
            }
        }
        for i in 1..last {
            let middle = (poses.open[i] as f32 + poses.widths[i] as f32 / 2.0) / length as f32;
            let nearest = POPS
                .iter()
                .min_by(|a, b| (a.0 - middle).abs().total_cmp(&(b.0 - middle).abs()))
                .map_or(0, |&(_, frame)| frame);
            if bands >= nearest + SEED_FROM - LOGO_SEED {
                _ = out.push(poses.slice(i, poses.open[i]));
            }
        }
    }
    out
}

/// The charging state the gauge shows: how far its bands show through the solid layer, and
/// which frame of their build or loop they show.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Charge {
    /// The last reading's charging flag, if there has been one.
    charging: Option<bool>,
    wipe: Wipe,
    /// The frame the bands hold, and from when they run on from it while charging.
    held: u8,
    runs_from: Micros,
}

impl Default for Charge {
    fn default() -> Self {
        Self {
            charging: None,
            wipe: Wipe::default(),
            held: OPEN,
            runs_from: 0,
        }
    }
}

/// The solid layer's edge on its way between covering the bands, 0, and showing them, 1. It
/// moves along `FLIGHT`'s curve, and `from` and `to` are places on the curve rather than on the
/// fill, so a wipe turned back partway retraces the way it came.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Wipe {
    from: f32,
    to: f32,
    start: Micros,
    length: Micros,
}

impl Wipe {
    /// Where the edge is along the curve.
    fn along(&self, now: Micros) -> f32 {
        let t = if self.length == 0 {
            1.0
        } else {
            (now.saturating_sub(self.start) as f32 / self.length as f32).min(1.0)
        };
        self.from + (self.to - self.from) * t
    }

    fn at(&self, now: Micros) -> f32 {
        flight(self.along(now))
    }

    fn end(&self) -> Micros {
        self.start + self.length
    }
}

impl Charge {
    /// Takes a reading. A change of the charging flag between two readings of a known, nonzero
    /// level wipes the solid in to the fill's middle and builds the bands in, or runs the build
    /// back and wipes the solid out over them; any other change shows its end at once.
    pub fn read(&mut self, battery: Option<Battery>, now: Micros) {
        let level = battery
            .filter(|battery| battery.present)
            .map(|battery| battery.percent);
        let charging = battery.is_some_and(|battery| battery.present && battery.charging);
        let shown = self.frame(now);
        let before = self.charging.replace(charging);
        if before == Some(charging) {
            return;
        }
        let to = if charging { 1.0 } else { 0.0 };
        let exposed = self.exposed(now);
        let along = self.wipe.along(now);
        let (mut start, mut length) = (now, 0);
        match level {
            Some(level) if level > 0 && before.is_some() => {
                length = libm::roundf(WIPE as f32 * (to - along).abs()) as Micros;
                if charging {
                    // Bands still partly showing go on from where they are, a build running
                    // back turning forwards again; covered ones build in.
                    self.held = match shown {
                        _ if exposed <= 0.0 => 0,
                        UNBUILD.. => BUILD_FRAMES - 1 - (shown - UNBUILD),
                        _ => shown,
                    };
                    self.runs_from = now + length;
                } else {
                    // The bands settle open, or a build turns back where it is; the solid
                    // follows once the build has run back to the empty fill.
                    self.held = shown;
                    self.runs_from = now;
                    let back = match shown {
                        ..OPEN => u64::from(shown) + 1,
                        UNBUILD.. => u64::from(UNBUILD + BUILD_FRAMES - shown),
                        _ => u64::from(until_open(shown - OPEN) + BUILD_FRAMES),
                    };
                    start = now + back * FRAME;
                }
            }
            _ => {
                self.held = OPEN;
                self.runs_from = now;
            }
        }
        self.wipe = Wipe {
            from: if length == 0 { to } else { along },
            to,
            start,
            length,
        };
    }

    /// Builds the gauge in from an empty fill at `at`, as the clock face comes in after the
    /// start-up: the bands' build while charging, otherwise the solid growing out from the
    /// fill's middle over bands that stay blank.
    pub fn enter(&mut self, at: Micros) {
        self.held = 0;
        self.runs_from = at;
        let to = if self.charging == Some(true) {
            1.0
        } else {
            0.0
        };
        self.wipe = Wipe {
            from: 1.0,
            to,
            start: at,
            length: if to == 0.0 { WIPE } else { 0 },
        };
    }

    /// How far the bands show through the solid layer, from 0, covered, to 1. The solid is
    /// centred on the fill.
    #[must_use]
    pub fn exposed(&self, now: Micros) -> f32 {
        self.wipe.at(now)
    }

    /// The bands' frame, in their build, their loop or the build run back. Within a hold it is
    /// the hold's first, so nothing changes to redraw.
    #[must_use]
    pub fn phase(&self, now: Micros) -> u8 {
        settled(self.frame(now))
    }

    fn frame(&self, now: Micros) -> u8 {
        if now < self.runs_from {
            return self.held;
        }
        let frames = (now - self.runs_from) / FRAME;
        let held = self.held;
        let back =
            |from: u8| UNBUILD + (u64::from(from) + frames).min(u64::from(BUILD_FRAMES - 1)) as u8;
        match self.charging {
            Some(true) => advance(held, frames),
            _ if held < OPEN => back(BUILD_FRAMES - 1 - held),
            _ if held >= UNBUILD => back(held - UNBUILD),
            _ => {
                let wait = until_open(held - OPEN);
                if frames < u64::from(wait) {
                    advance(held, frames)
                } else {
                    UNBUILD + (frames - u64::from(wait)).min(u64::from(BUILD_FRAMES - 1)) as u8
                }
            }
        }
    }

    /// Whether the gauge changes without another reading: the bands are looping, or a wipe is
    /// under way or still to come.
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

    /// The loop frame `frame` frames in.
    fn looped(frame: u8) -> u8 {
        OPEN + frame
    }

    /// The slices' first columns and the gaps between them at loop frame `frame`.
    fn pose(
        length: u32,
        frame: u8,
    ) -> (
        heapless::Vec<u32, MAX_SLICES>,
        heapless::Vec<u32, MAX_SLICES>,
    ) {
        let marks = marks(length, looped(frame));
        let lefts = marks.iter().map(|mark| mark.left).collect();
        let gaps = marks
            .windows(2)
            .map(|pair| pair[1].left - pair[0].left - pair[0].width)
            .collect();
        (lefts, gaps)
    }

    fn widths(length: u32) -> heapless::Vec<u32, MAX_SLICES> {
        marks(length, OPEN).iter().map(|mark| mark.width).collect()
    }

    const JOINED_FRAMES_AT: [u8; 2] = [
        OPEN_FRAMES + MOVE_FRAMES,
        BEAT_FRAMES + OPEN_FRAMES + MOVE_FRAMES,
    ];

    #[test]
    fn the_open_pose_is_the_design_studys_dispersed_layout() {
        // 172 px from the 29 September study's `title_pattern` at its first frame; 150 px is the
        // 87% level the review measured.
        let cases: [(u32, &[u32], &[u32]); 3] = [
            (
                172,
                &[
                    2, 2, 2, 2, 2, 3, 5, 5, 5, 5, 3, 2, 2, 2, 3, 5, 5, 5, 3, 2, 2, 2, 2,
                ],
                &[
                    6, 5, 4, 4, 4, 5, 5, 6, 5, 4, 4, 5, 5, 5, 4, 4, 5, 5, 4, 4, 4, 4,
                ],
            ),
            (
                150,
                &[3, 2, 2, 2, 2, 5, 5, 5, 5, 2, 2, 2, 2, 5, 5, 5, 2, 2, 2, 2],
                &[6, 5, 4, 4, 4, 5, 6, 5, 4, 5, 5, 5, 4, 5, 5, 4, 4, 4, 4],
            ),
            (5, &[5], &[]),
        ];
        for (length, bars, gaps) in cases {
            for frame in 0..OPEN_FRAMES {
                assert_eq!(widths(length).as_slice(), bars, "{length} {frame}");
                assert_eq!(pose(length, frame).1.as_slice(), gaps, "{length} {frame}");
            }
        }
    }

    #[test]
    fn every_length_fills_to_its_end_with_the_same_slices_in_order() {
        for length in 1..=172 {
            let open = widths(length);
            for frame in 0..LOOP_FRAMES {
                let marks = marks(length, looped(frame));
                assert_eq!(marks[0].left, 0);
                let last = marks.last().unwrap();
                assert_eq!(last.left + last.width, length, "{length} {frame}");
                assert!(marks.iter().map(|mark| mark.width).eq(open.iter().copied()));
                assert!(
                    marks
                        .windows(2)
                        .all(|pair| pair[1].left >= pair[0].left + pair[0].width),
                    "{length} {frame}"
                );
            }
        }
    }

    #[test]
    fn pairs_join_as_the_logos_bars_do() {
        // With the logo's twelve bars, the gaps close where its do.
        let length = (1..=172)
            .find(|&length| widths(length).len() == 12)
            .unwrap();
        let closed = |frame| {
            let (_, gaps) = pose(length, frame);
            (0..gaps.len())
                .filter(|&i| gaps[i] == 0)
                .collect::<heapless::Vec<usize, 22>>()
        };
        assert_eq!(closed(JOINED_FRAMES_AT[0]), [0, 2, 9]);
        assert_eq!(closed(JOINED_FRAMES_AT[1]), [1, 8, 10]);
        for length in 1..=172 {
            let (open, open_gaps) = pose(length, 0);
            assert!(open_gaps.iter().all(|&gap| gap > 0), "{length}");
            for frame in JOINED_FRAMES_AT {
                let (joined, gaps) = pose(length, frame);
                // Pairs, never three in a row, and the fill's ends stay put.
                let shut: heapless::Vec<usize, 22> =
                    (0..gaps.len()).filter(|&i| gaps[i] == 0).collect();
                assert!(
                    shut.windows(2).all(|pair| pair[1] > pair[0] + 1),
                    "{length}"
                );
                assert_eq!((joined[0], joined.last()), (open[0], open.last()));
                if open.len() >= 12 {
                    let moved = open.iter().zip(&joined).filter(|(a, b)| a != b).count();
                    assert!(
                        moved * 2 > open.len(),
                        "{length}: {moved} of {}",
                        open.len()
                    );
                }
            }
            assert_eq!(pose(length, BEAT_FRAMES - 1).0, open);
            assert_eq!(pose(length, LOOP_FRAMES - 1).0, open);
        }
    }

    #[test]
    fn a_slice_moves_one_way_at_a_time() {
        for length in 1..=172 {
            for beat in 0..2 {
                let first = beat * BEAT_FRAMES;
                let lefts: heapless::Vec<heapless::Vec<u32, MAX_SLICES>, 80> = (first
                    ..first + BEAT_FRAMES)
                    .map(|frame| pose(length, frame).0)
                    .collect();
                let turn = usize::from(OPEN_FRAMES + MOVE_FRAMES);
                for i in 0..lefts[0].len() {
                    let along: heapless::Vec<u32, 80> = lefts.iter().map(|pose| pose[i]).collect();
                    let (from, to) = (along[0], along[turn]);
                    let toward = |run: &[u32], up: bool| {
                        run.windows(2)
                            .all(|pair| pair[1] == pair[0] || (pair[1] > pair[0]) == up)
                    };
                    assert!(toward(&along[..=turn], to > from), "{length} {beat} {i}");
                    assert!(toward(&along[turn..], to < from), "{length} {beat} {i}");
                }
            }
        }
    }

    #[test]
    fn the_build_waits_sends_the_ends_out_and_fills_in() {
        let length = 150;
        let open = marks(length, OPEN);
        assert!((0..SEED_FROM).all(|frame| marks(length, frame).is_empty()));
        let seed: heapless::Vec<u32, 3> = (SEED_FROM..FLY_FROM)
            .map(|frame| marks(length, frame)[0].width)
            .collect();
        assert!(seed.windows(2).all(|pair| pair[1] > pair[0]), "{seed:?}");
        // The ends travel out without turning back, and land on their open places.
        let (mut left, mut right) = (u32::MAX, 0);
        for frame in FLY_FROM..OPEN {
            let marks = marks(length, frame);
            assert!(marks[0].left <= left && marks[1].left >= right, "{frame}");
            (left, right) = (marks[0].left, marks[1].left);
        }
        assert_eq!((left, right), (open[0].left, open[open.len() - 1].left));
        // Every inner slice appears at its own place and stays.
        let mut shown = 2;
        for frame in FLY_FROM..OPEN {
            let marks = marks(length, frame);
            assert!(marks.len() >= shown, "{frame}");
            shown = marks.len();
            assert!(marks[2..].iter().all(|mark| open.contains(mark)));
        }
        assert_eq!(shown, open.len());
        for length in 1..=172 {
            for frame in 0..OPEN {
                let marks = marks(length, frame);
                assert!(
                    marks
                        .iter()
                        .all(|mark| mark.width > 0 && mark.left + mark.width <= length)
                );
            }
        }
    }

    #[test]
    fn a_hold_keeps_its_frame_so_it_redraws_nothing() {
        let mut charge = Charge::default();
        charge.read(battery(87, false), 0);
        charge.read(battery(87, true), 0);
        let frame = |frame: u8| charge.phase(WIPE + u64::from(frame) * FRAME);
        let loop_frame = |frame: u8| charge.phase(WIPE + u64::from(OPEN + frame) * FRAME);
        for frame in 0..OPEN {
            assert_eq!(marks(150, frame), marks(150, settled(frame)));
        }
        assert!((0..SEED_FROM).all(|f| frame(f) == 0));
        let open = (0..OPEN_FRAMES).chain(BEAT_FRAMES - 1..BEAT_FRAMES + OPEN_FRAMES);
        assert!(open.into_iter().all(|f| loop_frame(f) == OPEN));
        for joined in JOINED_FRAMES_AT {
            let held = loop_frame(joined - 1);
            assert!((joined..joined + JOINED_FRAMES).all(|f| loop_frame(f) == held));
            assert_eq!(marks(150, held), marks(150, looped(joined)));
        }
        assert_ne!(loop_frame(OPEN_FRAMES), OPEN);
    }

    #[test]
    fn a_charge_builds_in_and_then_loops_from_the_open_pose() {
        let mut charge = Charge::default();
        charge.read(battery(87, false), 0);
        assert_eq!(charge.exposed(0), 0.0);
        let start = 1_000_000;
        charge.read(battery(87, true), start);
        assert_eq!(charge.exposed(start), 0.0);
        assert_eq!(charge.exposed(start + WIPE), 1.0);
        assert_eq!(charge.phase(start + WIPE), 0);
        let looping = start + WIPE + u64::from(OPEN) * FRAME;
        assert_eq!(charge.phase(looping), OPEN);
        let dock = looping + u64::from(OPEN_FRAMES) * FRAME;
        assert_eq!(charge.phase(dock), OPEN + OPEN_FRAMES);
        let again = looping + u64::from(LOOP_FRAMES) * FRAME;
        assert_eq!(charge.phase(again), OPEN);
    }

    #[test]
    fn entering_while_charging_builds_the_bands_from_an_empty_fill() {
        let mut charge = Charge::default();
        charge.read(battery(87, true), 0);
        let at = 1_000_000;
        charge.enter(at);
        for k in 0..BUILD_FRAMES {
            let now = at + u64::from(k) * FRAME;
            assert_eq!(marks(150, charge.phase(now)), marks(150, k), "{k}");
            assert_eq!(charge.exposed(now), 1.0, "{k}");
        }
        assert!(marks(150, charge.phase(at - 1)).is_empty());
        assert_eq!(charge.phase(at + u64::from(OPEN) * FRAME), OPEN);
        assert!(charge.is_moving(at));
    }

    #[test]
    fn entering_unplugged_grows_the_solid_from_the_middle_over_blank_bands() {
        let mut charge = Charge::default();
        charge.read(battery(87, false), 0);
        let at = 1_000_000;
        charge.enter(at);
        assert_eq!(charge.exposed(at), 1.0);
        let mut was = 1.0;
        for now in (at..=at + WIPE).step_by(FRAME as usize) {
            assert!(marks(150, charge.phase(now)).is_empty());
            assert!(charge.exposed(now) <= was);
            was = charge.exposed(now);
        }
        assert_eq!(charge.exposed(at + WIPE), 0.0);
        assert!(charge.is_moving(at + WIPE / 2));
        assert!(!charge.is_moving(at + WIPE));
        // Plugging in afterwards still wipes in and builds.
        charge.read(battery(87, true), at + 2 * WIPE);
        assert_eq!(charge.exposed(at + 3 * WIPE), 1.0);
        assert_eq!(charge.phase(at + 3 * WIPE), 0);
    }

    /// A charge that has looped since `0`, and when its loop started.
    fn looping() -> (Charge, Micros) {
        let mut charge = Charge::default();
        charge.read(battery(87, false), 0);
        charge.read(battery(87, true), 0);
        (charge, WIPE + u64::from(OPEN) * FRAME)
    }

    #[test]
    fn unplugging_runs_the_build_back_before_the_solid_returns() {
        let (mut charge, looped) = looping();
        let stop = looped + 10 * FRAME;
        assert_eq!(charge.phase(stop), OPEN);
        charge.read(battery(87, false), stop);
        for k in 0..BUILD_FRAMES {
            let at = stop + u64::from(k) * FRAME;
            let built = marks(150, BUILD_FRAMES - 1 - k);
            assert_eq!(marks(150, charge.phase(at)), built, "{k}");
            assert_eq!(charge.exposed(at), 1.0, "{k}");
        }
        let covered = stop + u64::from(BUILD_FRAMES) * FRAME;
        assert!(marks(150, charge.phase(covered)).is_empty());
        assert!(charge.exposed(covered + WIPE / 2) < 1.0);
        assert_eq!(charge.exposed(covered + WIPE), 0.0);
        assert!(charge.is_moving(covered + WIPE / 2));
        assert!(!charge.is_moving(covered + WIPE));
    }

    #[test]
    fn unplugged_mid_beat_the_slices_settle_open_first() {
        let (mut charge, looped) = looping();
        let stop = looped + u64::from(OPEN_FRAMES + MOVE_FRAMES + 3) * FRAME;
        let joined = charge.phase(stop);
        assert_ne!(marks(150, joined), marks(150, OPEN));
        charge.read(battery(87, false), stop);
        assert_eq!(charge.phase(stop), joined);
        let wait = u64::from(BEAT_FRAMES - OPEN_FRAMES - MOVE_FRAMES - 3);
        assert_eq!(
            marks(150, charge.phase(stop + (wait - 1) * FRAME)),
            marks(150, OPEN)
        );
        assert!(charge.phase(stop + wait * FRAME) >= UNBUILD);
        assert_eq!(
            charge.exposed(stop + (wait + u64::from(BUILD_FRAMES)) * FRAME),
            1.0
        );
    }

    #[test]
    fn plugged_in_again_the_build_turns_forwards_where_it_is() {
        let (mut charge, looped) = looping();
        let stop = looped + 10 * FRAME;
        charge.read(battery(87, false), stop);
        let again = stop + 8 * FRAME;
        let shown = marks(150, charge.phase(again));
        charge.read(battery(87, true), again);
        assert_eq!(marks(150, charge.phase(again)), shown);
        assert_eq!(charge.phase(again + FRAME), settled(BUILD_FRAMES - 8));
        assert_eq!(charge.phase(again + 9 * FRAME), OPEN);
        // Under the returning solid the bands are gone, so they build in from the start.
        let (mut charge, looped) = looping();
        charge.read(battery(87, false), looped);
        let covering = looped + u64::from(BUILD_FRAMES) * FRAME + WIPE / 2;
        let edge = charge.exposed(covering);
        assert!(edge > 0.0 && edge < 1.0);
        charge.read(battery(87, true), covering);
        assert_eq!(charge.exposed(covering), edge);
        let open = charge.wipe.end();
        assert!(open < covering + WIPE);
        assert_eq!(charge.exposed(open), 1.0);
        assert_eq!(charge.phase(open), 0);
        assert_eq!(charge.phase(open + u64::from(OPEN) * FRAME), OPEN);
    }

    #[test]
    fn a_first_reading_or_an_unknown_level_changes_at_once() {
        let mut charge = Charge::default();
        charge.read(battery(87, true), 0);
        assert_eq!((charge.exposed(0), charge.phase(0)), (1.0, OPEN));
        let mut empty = Charge::default();
        empty.read(battery(0, false), 0);
        empty.read(battery(0, true), 10);
        assert_eq!((empty.exposed(10), empty.phase(10)), (1.0, OPEN));
        let mut low = Charge::default();
        low.read(battery(12, false), 0);
        low.read(battery(12, true), 0);
        assert_eq!((low.exposed(WIPE), low.phase(WIPE)), (1.0, 0));
    }
}
