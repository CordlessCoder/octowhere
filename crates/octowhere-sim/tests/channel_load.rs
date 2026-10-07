//! What the channel carries once every node has a position: packets, airtime, receptions lost
//! to overlaps, and how old each node's view of the others' positions gets, with nodes still or
//! moving, and with the rest of the table rotated into every packet, as the firmware does, into
//! none, or only into a packet that holds nothing else.
//!
//! ```text
//! CHANNEL_LOAD_OUT=<abs file> cargo test -p octowhere-sim --release --test channel_load \
//!   -- --ignored --nocapture
//! ```
//!
//! `CHANNEL_LOAD_MOTION` lists `none`, `still` and `moving` (all by default): no fix, a fix that
//! never moves, or one that moves about 44 m every 30 s. `CHANNEL_LOAD_ROTATION` lists `0`, `1`
//! and `2` (`0` by default), as `table::BENCH_ROTATION` reads them. `CHANNEL_LOAD_SHAPES`,
//! `CHANNEL_LOAD_SEEDS=a..b` and `CHANNEL_LOAD_MINUTES` (20 by default) choose the rest.

use std::io::Write as _;

use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use octowhere_mesh::packet::Quality;
use octowhere_node::{Fix, view::Position};
use octowhere_sim::{Config, Link, Sim, UTC0_S, grouped};

struct Shape {
    name: String,
    nodes: usize,
    edges: Vec<(usize, usize)>,
}

fn full(n: usize) -> Shape {
    let mut edges = Vec::new();
    for a in 0..n {
        for b in a + 1..n {
            edges.push((a, b));
        }
    }
    Shape {
        name: format!("full{n}"),
        nodes: n,
        edges,
    }
}

fn grid(rows: usize, cols: usize) -> Shape {
    let at = |r: usize, c: usize| r * cols + c;
    let mut edges = Vec::new();
    for r in 0..rows {
        for c in 0..cols {
            if c + 1 < cols {
                edges.push((at(r, c), at(r, c + 1)));
            }
            if r + 1 < rows {
                edges.push((at(r, c), at(r + 1, c)));
            }
        }
    }
    Shape {
        name: format!("grid{rows}x{cols}"),
        nodes: rows * cols,
        edges,
    }
}

/// As `relay_cost.rs` places them, so that its layouts and these agree.
fn scattered(n: usize, range: f64, seed: u64) -> Shape {
    let mut state = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state >> 11) as f64 / (1u64 << 53) as f64
    };
    loop {
        let points: Vec<(f64, f64)> = (0..n).map(|_| (next(), next())).collect();
        let mut edges = Vec::new();
        for a in 0..n {
            for b in a + 1..n {
                let (dx, dy) = (points[a].0 - points[b].0, points[a].1 - points[b].1);
                if dx * dx + dy * dy <= range * range {
                    edges.push((a, b));
                }
            }
        }
        let mut reached = vec![false; n];
        let mut stack = vec![0];
        reached[0] = true;
        while let Some(at) = stack.pop() {
            for &(a, b) in &edges {
                for (from, to) in [(a, b), (b, a)] {
                    if from == at && !reached[to] {
                        reached[to] = true;
                        stack.push(to);
                    }
                }
            }
        }
        if reached.iter().all(|&r| r) {
            return Shape {
                name: format!("scatter{n}r{:.0}s{seed}", range * 100.0),
                nodes: n,
                edges,
            };
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Motion {
    None,
    Still,
    Moving,
}

impl Motion {
    fn name(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Still => "still",
            Self::Moving => "moving",
        }
    }
}

/// Node `n`'s fix at UTC second `utc`: on a grid 100 m apart, and when moving, 44 m further north
/// every 30 s, each node at its own phase of the 30.
fn fix(n: usize, utc: u32, motion: Motion) -> Fix {
    let steps = match motion {
        Motion::Moving => i32::try_from((utc + 7 * n as u32) / 30).unwrap() % 10_000,
        _ => 0,
    };
    Fix {
        latitude: 515_000_000 + (n / 8) as i32 * 9_000 + steps * 4_000,
        longitude: -1_000_000 + (n % 8) as i32 * 14_000,
        stamp: utc,
        quality: Quality::Autonomous,
        hdop_milli: Some(900),
    }
}

fn set_fixes(sim: &Sim, nodes: usize, motion: Motion) {
    if motion == Motion::None {
        return;
    }
    let utc = u32::try_from(sim.utc_us() / 1_000_000).unwrap();
    for n in 0..nodes {
        sim.set_fix(n, Some(fix(n, utc, motion)), true);
    }
}

#[derive(Default)]
struct Run {
    packets: usize,
    airtime_s: f64,
    mean_len: f64,
    /// The largest share of the time any one node spent sending.
    max_duty: f64,
    /// The share of the time a node's neighbours or itself were sending, mean and largest over
    /// the nodes.
    busy_mean: f64,
    busy_max: f64,
    delivered: u64,
    drowned: u64,
    deaf: u64,
    /// Ages of every position a node held of another, sampled every 10 s, in seconds.
    ages: Vec<f64>,
    /// Samples where a node held no position of another it should.
    missing: usize,
    /// Each pair's samples without a position, to count the pairs missing in every sample.
    missing_pairs: std::collections::HashMap<(usize, usize), usize>,
    samples: usize,
}

fn run(
    shape: &Shape,
    seed: u64,
    motion: Motion,
    rotation: u8,
    minutes: u64,
    bar: &ProgressBar,
) -> Run {
    octowhere_mesh::table::BENCH_ROTATION.store(rotation, std::sync::atomic::Ordering::Relaxed);
    let mut sim = Sim::new(seed);
    let nodes = shape.nodes;
    for (n, start) in grouped(nodes as u8, UTC0_S as u32 - 3_600)
        .into_iter()
        .enumerate()
    {
        let config = match motion {
            Motion::None => Config::default(),
            _ => Config {
                gps: true,
                fix: Some(fix(n, UTC0_S as u32, motion)),
                ..Config::default()
            },
        };
        sim.add(start, config);
    }
    for &(a, b) in &shape.edges {
        sim.link(a, b, Some(Link::default()));
        sim.link(b, a, Some(Link::default()));
    }
    let wanted: Vec<u64> = (0..nodes)
        .map(|node| {
            (0..nodes)
                .filter(|&other| other != node && sim.link_of(other, node).is_some())
                .fold(0, |set, other| set | 1 << other)
        })
        .collect();
    let mut heard = vec![0u64; nodes];
    let mut met = 0;
    let mut cursor = 0;
    bar.set_message("meeting");
    bar.set_length(60 * 60);
    bar.set_position(0);
    let start = sim.now_us();
    let mut next_fix = 0;
    let ok = sim.run_while_not(60 * 60, |sim| {
        bar.set_position((sim.now_us() - start) / 1_000_000);
        if sim.now_us() >= next_fix {
            set_fixes(sim, nodes, motion);
            next_fix = sim.now_us() + 1_000_000;
        }
        let (lines, total) = sim.lines_since(cursor);
        cursor = total;
        for line in lines {
            let Some(rest) = line.text.strip_prefix("[MESH] heard id=") else {
                continue;
            };
            let Some(id) = rest.split(' ').next().and_then(|id| id.parse::<u32>().ok()) else {
                continue;
            };
            let before = heard[line.node] & wanted[line.node] == wanted[line.node];
            heard[line.node] |= 1 << id;
            if !before && heard[line.node] & wanted[line.node] == wanted[line.node] {
                met += 1;
            }
        }
        met == nodes
    });
    assert!(ok, "{}: the nodes met", shape.name);
    sim.keep_lines(10_000);
    bar.set_message("settling");
    for _ in 0..180 {
        set_fixes(&sim, nodes, motion);
        sim.run_for(1);
    }
    sim.record(true);
    let from = sim.now_us();
    let before = sim.receptions();
    let mut out = Run::default();
    bar.set_message("measuring");
    bar.set_length(minutes * 60);
    for tick in 0..minutes * 60 {
        set_fixes(&sim, nodes, motion);
        sim.run_for(1);
        bar.set_position(tick + 1);
        if motion == Motion::None || tick % 10 != 9 {
            continue;
        }
        out.samples += 1;
        for node in 0..nodes {
            let Some(group) = sim.view(node).and_then(|view| view.group) else {
                continue;
            };
            let now = sim.clock(node);
            for other in (0..nodes).filter(|&other| other != node) {
                match group.member(other as u8).map(|member| member.position) {
                    Some(Position::At(at)) => out.ages.push((now - at) as f64 / 1e6),
                    _ => {
                        out.missing += 1;
                        *out.missing_pairs.entry((node, other)).or_default() += 1;
                    }
                }
            }
        }
    }
    let to = sim.now_us();
    let after = sim.receptions();
    let sent: Vec<_> = sim
        .recorded()
        .into_iter()
        .filter(|sent| sent.start >= from)
        .collect();
    let span = (to - from) as f64;
    out.packets = sent.len();
    out.airtime_s = sent.iter().map(|s| (s.end - s.start) as f64).sum::<f64>() / 1e6;
    out.mean_len = sent.iter().map(|s| s.bytes.len() as f64).sum::<f64>() / sent.len() as f64;
    let mut busy = Vec::new();
    for node in 0..nodes {
        let own: f64 = sent
            .iter()
            .filter(|s| s.sender == node)
            .map(|s| (s.end.min(to) - s.start) as f64)
            .sum();
        out.max_duty = out.max_duty.max(own / span);
        let mut spans: Vec<(u64, u64)> = sent
            .iter()
            .filter(|s| s.sender == node || sim.link_of(s.sender, node).is_some())
            .map(|s| (s.start, s.end.min(to)))
            .collect();
        spans.sort_unstable();
        let (mut covered, mut reach) = (0u64, 0u64);
        for (start, end) in spans {
            let start = start.max(reach);
            if end > start {
                covered += end - start;
                reach = end;
            }
        }
        busy.push(covered as f64 / span);
    }
    out.busy_mean = busy.iter().sum::<f64>() / busy.len() as f64;
    out.busy_max = busy.iter().copied().fold(0.0, f64::max);
    out.delivered = after.delivered - before.delivered;
    out.drowned = after.drowned - before.drowned;
    out.deaf = after.deaf - before.deaf;
    out
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return f64::NAN;
    }
    sorted[((sorted.len() - 1) as f64 * p).round() as usize]
}

#[test]
#[ignore = "a measurement; prints and writes CHANNEL_LOAD_OUT"]
fn channel_load() {
    let mut shapes = vec![full(32), grid(4, 8)];
    for seed in 1..=2 {
        shapes.push(scattered(32, 0.25, seed));
        shapes.push(scattered(32, 0.35, seed));
    }
    if let Ok(names) = std::env::var("CHANNEL_LOAD_SHAPES") {
        let names: Vec<&str> = names.split(',').collect();
        shapes.retain(|shape| names.contains(&shape.name.as_str()));
    }
    let motions: Vec<Motion> = std::env::var("CHANNEL_LOAD_MOTION")
        .unwrap_or_else(|_| "none,still,moving".to_owned())
        .split(',')
        .map(|motion| match motion {
            "none" => Motion::None,
            "still" => Motion::Still,
            "moving" => Motion::Moving,
            other => panic!("motion {other}"),
        })
        .collect();
    let rotations: Vec<u8> = std::env::var("CHANNEL_LOAD_ROTATION")
        .unwrap_or_else(|_| "0".to_owned())
        .split(',')
        .map(|rotation| rotation.parse().unwrap())
        .collect();
    let seeds = std::env::var("CHANNEL_LOAD_SEEDS").map_or(1..=2, |seeds| {
        let (from, to) = seeds.split_once("..").expect("a..b");
        from.parse().unwrap()..=to.parse().unwrap()
    });
    let minutes = std::env::var("CHANNEL_LOAD_MINUTES").map_or(20, |m| m.parse().unwrap());
    let mut out = std::env::var("CHANNEL_LOAD_OUT")
        .ok()
        .map(|path| std::fs::File::create(path).unwrap());
    let multi = MultiProgress::new();
    let runs = multi.add(ProgressBar::new(
        (shapes.len() * motions.len() * rotations.len() * seeds.clone().count()) as u64,
    ));
    runs.set_style(
        ProgressStyle::with_template(
            "{elapsed_precise} [{bar:30}] {pos}/{len} runs, eta {eta} {msg}",
        )
        .unwrap()
        .progress_chars("=> "),
    );
    let bar = multi.add(ProgressBar::new(0));
    bar.set_style(
        ProgressStyle::with_template("  {msg:>9} [{bar:30}] {pos}/{len} s of virtual time")
            .unwrap()
            .progress_chars("=> "),
    );
    for shape in &shapes {
        for &motion in &motions {
            for &rotation in &rotations {
                for seed in seeds.clone() {
                    runs.set_message(format!(
                        "{} {} rotation {rotation} seed {seed}",
                        shape.name,
                        motion.name()
                    ));
                    let mut run = run(shape, seed, motion, rotation, minutes, &bar);
                    run.ages.sort_by(f64::total_cmp);
                    let received = run.delivered + run.drowned;
                    let line = format!(
                        "{{\"shape\":\"{}\",\"motion\":\"{}\",\"rotation\":{rotation},\"seed\":{seed},\"minutes\":{minutes},\"packets\":{},\"airtime_s\":{:.1},\"mean_len\":{:.1},\"max_duty\":{:.4},\"busy_mean\":{:.4},\"busy_max\":{:.4},\"delivered\":{},\"drowned\":{},\"deaf\":{},\"drowned_share\":{:.4},\"age_median\":{:.1},\"age_p95\":{:.1},\"age_max\":{:.1},\"missing\":{},\"never\":{}}}",
                        shape.name,
                        motion.name(),
                        run.packets,
                        run.airtime_s,
                        run.mean_len,
                        run.max_duty,
                        run.busy_mean,
                        run.busy_max,
                        run.delivered,
                        run.drowned,
                        run.deaf,
                        run.drowned as f64 / received.max(1) as f64,
                        percentile(&run.ages, 0.5),
                        percentile(&run.ages, 0.95),
                        run.ages.last().copied().unwrap_or(f64::NAN),
                        run.missing,
                        run.missing_pairs
                            .values()
                            .filter(|&&count| count == run.samples)
                            .count(),
                    );
                    multi.suspend(|| println!("{line}"));
                    if let Some(out) = &mut out {
                        writeln!(out, "{line}").unwrap();
                    }
                    runs.inc(1);
                }
            }
        }
    }
    runs.finish();
    bar.finish_and_clear();
}
