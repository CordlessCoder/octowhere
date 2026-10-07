//! What a flood costs on the air today: how many packets carry one group message until every
//! node holds it, on several shapes of group. `tools/relay-cost.py` sets the counts beside what
//! a flood on the same links could cost at best, and what designated relays would cost.
//!
//! ```text
//! RELAY_COST_OUT=<abs file> RELAY_COST_LINES=<abs dir> RELAY_COST_VARIANTS=32:3,128:1 \
//!   cargo test -p octowhere-sim --release --test relay_cost -- --ignored --nocapture
//! ```
//!
//! A variant is `steps:resends[:a|:n]`: the backoff steps a packet holding records waits up to,
//! how many times a message goes again for a neighbour not heard passing it on, and with `a` a
//! neighbour that holds a message a sender carries a second time answers so, or with `n` a
//! message sent again names the neighbours still waited for and a named one that holds it
//! answers so, either at no cost on the air; `d` has each sender name the neighbours that are to
//! pass its messages on, as few as reach every node two hops away, and `dn` adds `n` to it.
//! `32:3` is the firmware's. Paths are absolute, since the test runs in the crate's directory.
//! `RELAY_COST_SEEDS=a..b` runs those seeds, `RELAY_COST_CHECK=scan` checks for the end of each
//! phase by scanning every line logged, as the bench first did, and `[timing]` lines on stderr
//! give each phase's time on the host.
//! `RELAY_COST_SHAPES` names the shapes to run, all by default, and `RELAY_COST_KIND=removal`
//! has node 0 remove the last node instead, until every other member holds its key message.
//!
//! A variant may add `rN`, a rest of N airtimes after each packet in place of nine, after
//! `steps:resends`; `32:1` is the firmware's since the removal changes. Across a run:
//! `RELAY_COST_FIX=still` gives every node a fix that stays put, refreshed each second;
//! `RELAY_COST_CATCH_UP=rounds` sends a
//! catch-up under an old key again first after `rounds`, 13 in the firmware; and
//! `RELAY_COST_EXTRA_ROUNDS` adds that many rounds to a removal's switch; and
//! `RELAY_COST_FOLLOW_UP=n` follows a packet that carried messages to a neighbour expected to
//! pass nothing on with another a round later, up to n times until it is heard holding what
//! the sender holds, and `RELAY_COST_ANSWER=1` sends the message again on hearing that
//! neighbour's digest unlike the sender's.

use std::{fmt::Write as _, io::Write as _};

use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use octowhere_node::{Command, view::Text};
use octowhere_sim::{Config, Link, Sim, UTC0_S, grouped};

/// Seeds each shape runs at.
const SEEDS: u64 = 4;

/// Node `n`'s fix at UTC second `utc`, on a grid 100 m apart, with `RELAY_COST_FIX=still`.
fn still_fix(n: usize, utc: u32) -> octowhere_node::Fix {
    octowhere_node::Fix {
        latitude: 515_000_000 + (n / 8) as i32 * 9_000,
        longitude: -1_000_000 + (n % 8) as i32 * 14_000,
        stamp: utc,
        quality: octowhere_mesh::packet::Quality::Autonomous,
        hdop_milli: Some(900),
    }
}

struct Shape {
    name: String,
    nodes: usize,
    edges: Vec<(usize, usize)>,
    /// What share of packets each link loses.
    loss: f64,
}

impl Shape {
    fn lossy(mut self, loss: f64) -> Self {
        self.name = format!("{}-loss{:.0}", self.name, loss * 100.0);
        self.loss = loss;
        self
    }
}

/// What the bench changes: the steps a packet holding records waits up to, and how many times a
/// message goes again for a neighbour not heard passing it on.
#[derive(Clone, Copy)]
struct Variant {
    record_steps: u32,
    resends: u8,
    /// Whether a node answers, at no cost, a message carried again that it holds.
    answer: bool,
    /// Whether a message sent again names the neighbours waited for, and only those answer.
    named: bool,
    /// Whether each sender names the neighbours that are to pass its messages on.
    designate: bool,
    /// The airtimes a node rests after each transmission.
    rest: i64,
}

impl Variant {
    fn name(self) -> String {
        format!(
            "steps{}-again{}{}{}",
            self.record_steps,
            self.resends,
            match (self.answer, self.named, self.designate) {
                (true, _, _) => "-answered",
                (_, true, true) => "-designated-named",
                (_, false, true) => "-designated",
                (_, true, _) => "-named",
                _ => "",
            },
            match self.rest {
                9 => String::new(),
                rest => format!("-rest{rest}"),
            },
        )
    }

    fn apply(self) {
        use std::sync::atomic::Ordering;
        octowhere_node::access::BENCH_RECORD_STEPS.store(self.record_steps, Ordering::Relaxed);
        octowhere_mesh::relays::BENCH_RESENDS.store(self.resends, Ordering::Relaxed);
        octowhere_mesh::relays::BENCH_ANSWER.store(self.answer, Ordering::Relaxed);
        octowhere_mesh::relays::BENCH_NAMED.store(self.named, Ordering::Relaxed);
        octowhere_mesh::relays::BENCH_DESIGNATE.store(self.designate, Ordering::Relaxed);
        octowhere_node::access::BENCH_REST_TIMES.store(self.rest, Ordering::Relaxed);
        for designated in &octowhere_mesh::relays::BENCH_DESIGNATED {
            designated.store(0, Ordering::Relaxed);
        }
        octowhere_mesh::relays::BENCH_ANSWERS.clear();
        octowhere_mesh::relays::BENCH_NAMED_SETS.clear();
    }
}

fn line(n: usize) -> Shape {
    Shape {
        name: format!("line{n}"),
        nodes: n,
        edges: (1..n).map(|i| (i - 1, i)).collect(),
        loss: 0.0,
    }
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
        loss: 0.0,
    }
}

/// `rows` by `cols`, each node in reach of the next in its row and column, and with
/// `diagonal` of the four diagonally next to it.
fn grid(rows: usize, cols: usize, diagonal: bool) -> Shape {
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
            if diagonal && r + 1 < rows && c + 1 < cols {
                edges.push((at(r, c), at(r + 1, c + 1)));
                edges.push((at(r, c + 1), at(r + 1, c)));
            }
        }
    }
    Shape {
        name: format!("grid{rows}x{cols}{}", if diagonal { "d" } else { "" }),
        nodes: rows * cols,
        edges,
        loss: 0.0,
    }
}

/// Two groups of `half` all in reach of each other, joined by one node in reach of both.
fn clusters(half: usize) -> Shape {
    let n = 2 * half + 1;
    let bridge = 2 * half;
    let mut edges = Vec::new();
    for side in 0..2 {
        let base = side * half;
        for a in 0..half {
            for b in a + 1..half {
                edges.push((base + a, base + b));
            }
            edges.push((base + a, bridge));
        }
    }
    Shape {
        name: format!("clusters{half}"),
        nodes: n,
        edges,
        loss: 0.0,
    }
}

/// `n` nodes at random in a unit square, in reach within `range`, drawn again until every node
/// reaches every other over some path.
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
                loss: 0.0,
            };
        }
    }
}

struct Run {
    reached_s: Option<f64>,
    /// Packets that carried a message, the origin's first among them.
    carrying: usize,
    /// Of them, sent again because a neighbour was not heard passing it on.
    again: usize,
    /// Packets of any kind from the send until the flood's end and 30 s after.
    packets: usize,
    airtime_ms: f64,
}

/// Shows `phase` of a run on `bar`, up to `limit` seconds of virtual time.
fn phase(bar: &ProgressBar, phase: &str, limit: u64) {
    bar.set_message(phase.to_owned());
    bar.set_length(limit);
    bar.set_position(0);
}

fn run(
    shape: &Shape,
    seed: u64,
    variant: Variant,
    bar: &ProgressBar,
    multi: &MultiProgress,
) -> Run {
    variant.apply();
    // `RELAY_COST_FOLLOW_UP=n`: a packet that carried messages to a neighbour expected to pass
    // nothing on is followed by another a round later, up to n times while one is not heard
    // holding what this node holds.
    octowhere_node::BENCH_FOLLOW_UP.store(
        std::env::var("RELAY_COST_FOLLOW_UP").map_or(0, |count| count.parse().unwrap()),
        std::sync::atomic::Ordering::Relaxed,
    );
    // `RELAY_COST_ANSWER=1`: hearing such a neighbour's digest unlike this node's sends the
    // message again.
    octowhere_node::BENCH_ANSWER_MISMATCH.store(
        std::env::var("RELAY_COST_ANSWER").is_ok_and(|on| on == "1"),
        std::sync::atomic::Ordering::Relaxed,
    );
    // `RELAY_COST_EXTRA_ROUNDS`: rounds a remover adds to its switch, 0 by default.
    octowhere_mesh::rekey::BENCH_EXTRA_ROUNDS.store(
        std::env::var("RELAY_COST_EXTRA_ROUNDS").map_or(0, |extra| extra.parse().unwrap()),
        std::sync::atomic::Ordering::Relaxed,
    );
    // `RELAY_COST_CATCH_UP=rounds`: a catch-up goes again first after this many rounds.
    let gap = std::env::var("RELAY_COST_CATCH_UP").map_or(13, |gap| gap.parse().unwrap());
    octowhere_node::removals::BENCH_CATCH_UP_GAP.store(gap, std::sync::atomic::Ordering::Relaxed);
    let mut sim = Sim::new(seed);
    sim.record(true);
    let fixes = std::env::var("RELAY_COST_FIX").is_ok_and(|fix| fix == "still");
    for (n, start) in grouped(shape.nodes as u8, UTC0_S as u32 - 3_600)
        .into_iter()
        .enumerate()
    {
        let config = match fixes {
            true => Config {
                gps: true,
                fix: Some(still_fix(n, UTC0_S as u32)),
                ..Config::default()
            },
            false => Config::default(),
        };
        sim.add(start, config);
    }
    let mut next_fix = 0;
    let mut refresh = move |sim: &Sim| {
        if fixes && sim.now_us() >= next_fix {
            let utc = u32::try_from(sim.utc_us() / 1_000_000).unwrap();
            for n in 0..shape.nodes {
                sim.set_fix(n, Some(still_fix(n, utc)), true);
            }
            next_fix = sim.now_us() + 1_000_000;
        }
    };
    let link = Link {
        loss: shape.loss,
        ..Link::default()
    };
    for &(a, b) in &shape.edges {
        sim.link(a, b, Some(link));
        sim.link(b, a, Some(link));
    }
    let nodes = shape.nodes;
    // The old checks scan every line logged at each event; the default reads only new lines.
    let scan = std::env::var("RELAY_COST_CHECK").is_ok_and(|check| check == "scan");
    let clock = std::time::Instant::now();
    phase(bar, "meeting", 60 * 60);
    let start = sim.now_us();
    let met = if scan {
        sim.run_while_not(60 * 60, |sim| {
            bar.set_position((sim.now_us() - start) / 1_000_000);
            (0..nodes).all(|node| {
                (0..nodes)
                    .filter(|&other| other != node && sim.link_of(other, node).is_some())
                    .all(|other| sim.count(node, &format!("heard id={other} ")) >= 1)
            })
        })
    } else {
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
        sim.run_while_not(60 * 60, |sim| {
            bar.set_position((sim.now_us() - start) / 1_000_000);
            refresh(sim);
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
        })
    };
    assert!(met, "{}: the nodes met", shape.name);
    let met_at = clock.elapsed();
    // A floor and more, so that every neighbour's report holds its whole reach.
    phase(bar, "settling", 180);
    for _ in 0..180 {
        refresh(&sim);
        sim.run_for(1);
    }
    let settled_at = clock.elapsed();
    let from = sim.now_us();
    let removal = std::env::var("RELAY_COST_KIND").is_ok_and(|kind| kind == "removal");
    let (needle, wanted) = match removal {
        // Every member but the one removed holds its key message.
        true => (" asks to remove ", 1..nodes - 1),
        false => ("[MSG] from 0 to all", 1..nodes),
    };
    if removal {
        sim.command(0, Command::Remove((nodes - 1) as u8));
    } else {
        let text = Text::new(b"Meet at the bridge.").unwrap();
        sim.command(0, Command::Send { to: None, text });
    }
    let limit = if removal { 30 * 60 } else { 10 * 60 };
    phase(bar, if removal { "removal" } else { "flood" }, limit);
    let reached = if scan {
        sim.run_while_not(limit, |sim| {
            bar.set_position((sim.now_us() - from) / 1_000_000);
            wanted.clone().all(|node| sim.count(node, needle) >= 1)
        })
    } else {
        let mut done = vec![false; nodes];
        let mut count = 0;
        let mut cursor = sim.lines_since(0).1;
        sim.run_while_not(limit, |sim| {
            bar.set_position((sim.now_us() - from) / 1_000_000);
            refresh(sim);
            let (lines, total) = sim.lines_since(cursor);
            cursor = total;
            for line in lines {
                if wanted.contains(&line.node) && !done[line.node] && line.text.contains(needle) {
                    done[line.node] = true;
                    count += 1;
                }
            }
            count == wanted.len()
        })
    };
    let reached_s = reached.then(|| (sim.now_us() - from) as f64 / 1e6);
    let tail_from = clock.elapsed();
    phase(bar, "tail", 30);
    sim.run_for(30);
    if let Ok(dir) = std::env::var("RELAY_COST_LINES") {
        let mut dump = String::new();
        for node in 0..nodes {
            for line in sim.lines(node).into_iter().filter(|line| line.at >= from) {
                let _ = writeln!(dump, "{} {} {}", line.at - from, node, line.text);
            }
        }
        std::fs::write(
            format!("{dir}/{}-{}-{seed}.log", variant.name(), shape.name),
            dump,
        )
        .unwrap();
    }
    let mut carrying = 0;
    let mut again = 0;
    for node in 0..nodes {
        for line in sim.lines(node).into_iter().filter(|line| line.at >= from) {
            if line.text.contains("[MESH] sent") && !line.text.contains(" messages=0 ") {
                carrying += 1;
            }
            if line.text.contains("goes again") {
                again += 1;
            }
        }
    }
    let ran_at = clock.elapsed();
    multi.suspend(|| eprintln!(
        "[timing] {} seed {seed}: meet {:.2} s, settle {:.2} s, {} {:.2} s, tail and counts {:.2} s",
        shape.name,
        met_at.as_secs_f64(),
        (settled_at - met_at).as_secs_f64(),
        if removal { "removal" } else { "flood" },
        (tail_from - settled_at).as_secs_f64(),
        (ran_at - tail_from).as_secs_f64(),
    ));
    let sent: Vec<_> = sim
        .recorded()
        .into_iter()
        .filter(|sent| sent.start >= from)
        .collect();
    Run {
        reached_s,
        carrying,
        again,
        packets: sent.len(),
        airtime_ms: sent
            .iter()
            .map(|sent| (sent.end - sent.start) as f64 / 1e3)
            .sum(),
    }
}

#[test]
#[ignore = "a measurement; prints and writes RELAY_COST_OUT"]
fn flood_cost() {
    let mut shapes = vec![
        full(32),
        line(12),
        grid(2, 8, false),
        grid(4, 8, false),
        grid(4, 8, true),
        clusters(10),
        line(12).lossy(0.2),
        grid(4, 8, false).lossy(0.1),
        scattered(32, 0.25, 1).lossy(0.1),
    ];
    for seed in 1..=4 {
        shapes.push(scattered(32, 0.25, seed));
        shapes.push(scattered(32, 0.35, seed));
    }
    // Only the shapes named, if any are.
    if let Ok(names) = std::env::var("RELAY_COST_SHAPES") {
        let names: Vec<&str> = names.split(',').collect();
        shapes.retain(|shape| names.contains(&shape.name.as_str()));
    }
    // `steps:resends`, each a variant, the firmware's first.
    let variants: Vec<Variant> = std::env::var("RELAY_COST_VARIANTS")
        .unwrap_or_else(|_| "32:3".to_owned())
        .split(',')
        .map(|variant| {
            let mut parts = variant.split(':');
            let record_steps = parts.next().unwrap().parse().unwrap();
            let resends = parts.next().expect("steps:resends").parse().unwrap();
            let flags: Vec<&str> = parts.collect();
            let mode = flags
                .iter()
                .copied()
                .find(|flag| ["a", "n", "d", "dn"].contains(flag));
            Variant {
                record_steps,
                resends,
                answer: mode == Some("a"),
                named: matches!(mode, Some("n" | "dn")),
                designate: matches!(mode, Some("d" | "dn")),
                rest: flags
                    .iter()
                    .find_map(|flag| flag.strip_prefix('r'))
                    .map_or(9, |rest| rest.parse().unwrap()),
            }
        })
        .collect();
    // `RELAY_COST_SEEDS=a..b` runs those seeds, inclusive.
    let seeds = std::env::var("RELAY_COST_SEEDS").map_or(1..=SEEDS, |seeds| {
        let (from, to) = seeds.split_once("..").expect("a..b");
        from.parse().unwrap()..=to.parse().unwrap()
    });
    let multi = MultiProgress::new();
    let runs = multi.add(ProgressBar::new(
        (variants.len() * shapes.len() * seeds.clone().count()) as u64,
    ));
    runs.set_style(
        ProgressStyle::with_template(
            "{elapsed_precise} [{bar:30}] {pos}/{len} runs, eta {eta} {msg}",
        )
        .unwrap()
        .progress_chars("=> "),
    );
    let virtual_time = multi.add(ProgressBar::new(0));
    virtual_time.set_style(
        ProgressStyle::with_template("  {msg:>8} [{bar:30}] {pos}/{len} s of virtual time")
            .unwrap()
            .progress_chars("=> "),
    );
    let mut out = String::new();
    for &variant in &variants {
        for shape in &shapes {
            for seed in seeds.clone() {
                runs.set_message(format!("{} {} seed {seed}", variant.name(), shape.name));
                let run = run(shape, seed, variant, &virtual_time, &multi);
                runs.inc(1);
                let edges = shape
                    .edges
                    .iter()
                    .map(|(a, b)| format!("[{a},{b}]"))
                    .collect::<Vec<_>>()
                    .join(",");
                let line = format!(
                    "{{\"variant\":\"{}\",\"shape\":\"{}\",\"seed\":{seed},\"nodes\":{},\"reached_s\":{},\"carrying\":{},\"again\":{},\"packets\":{},\"airtime_ms\":{:.1},\"edges\":[{edges}]}}",
                    variant.name(),
                    shape.name,
                    shape.nodes,
                    run.reached_s
                        .map_or("null".to_owned(), |s| format!("{s:.2}")),
                    run.carrying,
                    run.again,
                    run.packets,
                    run.airtime_ms,
                );
                multi.suspend(|| {
                    println!(
                        "{} {} seed {seed}: reached {:?} s, {} packets carried it, {} again",
                        variant.name(),
                        shape.name,
                        run.reached_s,
                        run.carrying,
                        run.again
                    )
                });
                let _ = writeln!(out, "{line}");
            }
        }
    }
    virtual_time.finish_and_clear();
    runs.finish();
    if let Ok(path) = std::env::var("RELAY_COST_OUT") {
        std::fs::File::create(path)
            .unwrap()
            .write_all(out.as_bytes())
            .unwrap();
    }
}
