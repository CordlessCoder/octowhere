//! Contention at size: nodes send when the channel is clear, after a random backoff, rather
//! than in slots. These hold the scheme to what it was built for, a message's latency, and to
//! what it risks, packets lost when two nodes send at once.

use octowhere_node::{Command, view::Text};
use octowhere_sim::{Config, Link, Sim, UTC0_S, grouped};

/// `count` nodes of one group, all in reach of each other or each only of the next in a `line`,
/// recording the air, once each has heard every node in its reach.
fn group(count: u8, seed: u64, line: bool) -> Sim {
    group_over(count, seed, line, Link::default())
}

/// As [`group`], every pair in reach by `link`.
fn group_over(count: u8, seed: u64, line: bool, link: Link) -> Sim {
    let mut sim = Sim::new(seed);
    sim.record(true);
    for start in grouped(count, UTC0_S as u32 - 3_600) {
        sim.add(start, Config::default());
    }
    let nodes = usize::from(count);
    if line {
        for n in 1..nodes {
            sim.link(n - 1, n, Some(link));
            sim.link(n, n - 1, Some(link));
        }
    } else {
        sim.link_all(link);
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

/// A chain of twelve whose every link loses a packet in five. A relay that does not hear its
/// neighbour pass a message on sends it again, so a loss costs seconds rather than a floor.
#[test]
fn a_message_crosses_a_lossy_chain_within_two_minutes() {
    let lossy = Link {
        loss: 0.2,
        ..Link::default()
    };
    let mut sim = group_over(12, 1, true, lossy);
    let took = flood(&mut sim, 12).expect("every node has the message");
    assert!(took < 120.0, "{took} s for 11 hops");
}

/// A removal along a chain of twelve: every key message reaches the far end before the switch,
/// though a relay's collisions with a node two hops away lose some on the way.
#[test]
fn a_removal_reaches_the_far_end_of_a_chain_before_its_switch() {
    let mut sim = group(12, 1, true);
    sim.command(0, Command::Remove(11));
    let switched = sim.run_while_not(30 * 60, |sim| {
        (0..11).all(|node| sim.count(node, "switched to generation 1") >= 1)
    });
    assert!(switched, "after {} s", sim.now_s());
    let at = |node: usize| {
        sim.lines(node)
            .into_iter()
            .find(|line| line.text.contains("switched to generation 1"))
            .map(|line| line.at)
    };
    let first = (0..11).filter_map(at).min().unwrap();
    for node in 0..11 {
        assert!(
            at(node).unwrap() < first + 5_000_000,
            "node {node} switched late"
        );
    }
}
