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
//! answers so, either at no cost on the air. `32:3` is the firmware's. Paths are absolute, since the test runs in the crate's directory.
//! `RELAY_COST_SHAPES` names the shapes to run, all by default, and `RELAY_COST_KIND=removal`
//! has node 0 remove the last node instead, until every other member holds its key message.

use std::{fmt::Write as _, io::Write as _};

use octowhere_node::{Command, view::Text};
use octowhere_sim::{Config, Link, Sim, UTC0_S, grouped};

/// Seeds each shape runs at.
const SEEDS: u64 = 4;

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
}

impl Variant {
    fn name(self) -> String {
        format!(
            "steps{}-again{}{}",
            self.record_steps,
            self.resends,
            match (self.answer, self.named) {
                (true, _) => "-answered",
                (_, true) => "-named",
                _ => "",
            }
        )
    }

    fn apply(self) {
        use std::sync::atomic::Ordering;
        octowhere_node::access::BENCH_RECORD_STEPS.store(self.record_steps, Ordering::Relaxed);
        octowhere_mesh::relays::BENCH_RESENDS.store(self.resends, Ordering::Relaxed);
        octowhere_mesh::relays::BENCH_ANSWER.store(self.answer, Ordering::Relaxed);
        octowhere_mesh::relays::BENCH_NAMED.store(self.named, Ordering::Relaxed);
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

fn run(shape: &Shape, seed: u64, variant: Variant) -> Run {
    variant.apply();
    let mut sim = Sim::new(seed);
    sim.record(true);
    for start in grouped(shape.nodes as u8, UTC0_S as u32 - 3_600) {
        sim.add(start, Config::default());
    }
    let link = Link {
        loss: shape.loss,
        ..Link::default()
    };
    for &(a, b) in &shape.edges {
        sim.link(a, b, Some(link));
        sim.link(b, a, Some(link));
    }
    let nodes = shape.nodes;
    let met = sim.run_while_not(60 * 60, |sim| {
        (0..nodes).all(|node| {
            (0..nodes)
                .filter(|&other| other != node && sim.link_of(other, node).is_some())
                .all(|other| sim.count(node, &format!("heard id={other} ")) >= 1)
        })
    });
    assert!(met, "{}: the nodes met", shape.name);
    // A floor and more, so that every neighbour's report holds its whole reach.
    sim.run_for(180);
    let from = sim.now_us();
    let removal = std::env::var("RELAY_COST_KIND").is_ok_and(|kind| kind == "removal");
    let reached = if removal {
        // Every member but the one removed holds its key message.
        sim.command(0, Command::Remove((nodes - 1) as u8));
        sim.run_while_not(30 * 60, |sim| {
            (1..nodes - 1).all(|node| sim.count(node, " asks to remove ") >= 1)
        })
    } else {
        let text = Text::new(b"Meet at the bridge.").unwrap();
        sim.command(0, Command::Send { to: None, text });
        sim.run_while_not(10 * 60, |sim| {
            (1..nodes).all(|node| sim.count(node, "[MSG] from 0 to all") >= 1)
        })
    };
    let reached_s = reached.then(|| (sim.now_us() - from) as f64 / 1e6);
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
            let mode = parts.next();
            Variant {
                record_steps,
                resends,
                answer: mode == Some("a"),
                named: mode == Some("n"),
            }
        })
        .collect();
    let mut out = String::new();
    for &variant in &variants {
        for shape in &shapes {
            for seed in 1..=SEEDS {
                let run = run(shape, seed, variant);
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
                println!(
                    "{} {} seed {seed}: reached {:?} s, {} packets carried it, {} again",
                    variant.name(),
                    shape.name,
                    run.reached_s,
                    run.carrying,
                    run.again
                );
                let _ = writeln!(out, "{line}");
            }
        }
    }
    if let Ok(path) = std::env::var("RELAY_COST_OUT") {
        std::fs::File::create(path)
            .unwrap()
            .write_all(out.as_bytes())
            .unwrap();
    }
}
