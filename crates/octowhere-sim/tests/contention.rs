//! Contention at size: nodes send when the channel is clear, after a random backoff, rather
//! than in slots. These hold the scheme to what it was built for, a message's latency, and to
//! what it risks, packets lost when two nodes send at once.

use octowhere_node::{Command, view::Text};
use octowhere_sim::{Config, Link, Sim, UTC0_S, grouped};

/// `count` nodes of one group, all in reach of each other or each only of the next in a `line`,
/// recording the air, once each has heard every node in its reach.
fn group(count: u8, seed: u64, line: bool) -> Sim {
    let mut sim = Sim::new(seed);
    sim.record(true);
    for start in grouped(count, UTC0_S as u32 - 3_600) {
        sim.add(start, Config::default());
    }
    let nodes = usize::from(count);
    if line {
        for n in 1..nodes {
            sim.link(n - 1, n, Some(Link::default()));
            sim.link(n, n - 1, Some(Link::default()));
        }
    } else {
        sim.link_all(Link::default());
    }
    let met = sim.run_while_not(60 * 60, |sim| {
        (0..nodes).all(|node| {
            (0..nodes)
                .filter(|&other| sim.link_of(other, node).is_some())
                .all(|other| sim.count(node, &format!("heard id={other} ")) >= 1)
        })
    });
    assert!(met, "the nodes met");
    sim
}

/// Seconds from now until every node but 0 holds a group message 0 sends now, if they all do
/// within ten minutes.
fn flood(sim: &mut Sim, nodes: usize) -> Option<f64> {
    let text = Text::new(b"Meet at the bridge.").unwrap();
    let at = sim.now_us();
    sim.command(0, Command::Send { to: None, text });
    let reached = sim.run_while_not(10 * 60, |sim| {
        (1..nodes).all(|node| sim.count(node, "[MSG] from 0 to all") >= 1)
    });
    reached.then(|| (sim.now_us() - at) as f64 / 1e6)
}

/// Every other node of 32 in reach has a message within two seconds, from the origin's one
/// packet: it covers everyone, so nobody relays it.
#[test]
fn a_message_reaches_32_nodes_in_reach_within_two_seconds() {
    let mut sim = group(32, 1, false);
    let carried = |sim: &Sim| -> usize { (0..32).map(|node| sim.count(node, "messages=1 ")).sum() };
    let before = carried(&sim);
    let took = flood(&mut sim, 32).expect("every node has the message");
    assert!(took < 2.0, "{took} s");
    assert_eq!(carried(&sim), before + 1, "one packet carried it");
}

/// A message crosses a line of twelve a hop at a time, each relay sending as soon as the
/// channel lets it rather than in its next slot.
#[test]
fn a_message_crosses_a_line_of_twelve_in_seconds() {
    let mut sim = group(12, 1, true);
    let took = flood(&mut sim, 12).expect("every node has the message");
    assert!(took < 6.0, "{took} s for 11 hops");
}

/// 32 idle nodes in reach of each other, sending their floors, lose few packets to two of them
/// sending at once.
#[test]
fn idle_nodes_in_reach_lose_few_packets_to_collisions() {
    let mut sim = group(32, 2, false);
    let from = sim.now_us();
    sim.run_for(30 * 60);
    let sent = sim
        .recorded()
        .into_iter()
        .filter(|sent| sent.start >= from)
        .count();
    let heard: usize = (0..32)
        .map(|node| {
            sim.lines(node)
                .into_iter()
                .filter(|line| line.at >= from && line.text.contains("[MESH] heard id="))
                .count()
        })
        .sum();
    let lost = 1.0 - heard as f64 / (sent * 31) as f64;
    assert!(
        lost < 0.05,
        "{:.1}% of {} receptions",
        100.0 * lost,
        sent * 31
    );
}
